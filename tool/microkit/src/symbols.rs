//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use std::{cmp::min, collections::BTreeMap, collections::HashMap};

use crate::{
    elf::ElfFile,
    sdf::{self, Channel, ChannelEnd, ProtectionDomain, SysMemoryRegion},
    sel4::{Arch, Config},
    util::{monitor_serialise_names, monitor_serialise_u64_vec},
    MAX_PDS, MAX_VMS, PD_MAX_NAME_LENGTH, VM_MAX_NAME_LENGTH,
};

/// Patch all the required symbols in the Monitor and children PDs according to
/// the Microkit's requirements
pub fn patch_symbols(
    kernel_config: &Config,
    pds: &BTreeMap<String, ProtectionDomain>,
    // TODO: Channel -> [UndirectedChannel, DirectedChannel]
    channels: &[Channel],
    memory_regions: &[SysMemoryRegion],
    pd_elf_files: &mut BTreeMap<String, ElfFile>,
    cross_core_receiver_channels: &[(ChannelEnd, ChannelEnd)],
) -> Result<(), String> {
    // *********************************
    // Step 1. Write ELF symbols in the monitor.
    // *********************************
    // @kwinter: Fix this hack
    // let monitor_elf = pd_elf_files.last_mut()?;
    let monitor_elf = pd_elf_files
        .get_mut("monitor")
        .expect("we added the monitor");

    let pd_names = pds.keys().collect();

    monitor_elf.write_symbol("pd_names_len", &pds.len().to_le_bytes())?;
    monitor_elf.write_symbol(
        "pd_names",
        &monitor_serialise_names(pd_names, MAX_PDS, PD_MAX_NAME_LENGTH),
    )?;

    let vm_names: Vec<&String> = pds
        .values()
        .filter_map(|pd| pd.virtual_machine.as_ref().map(|vm| &vm.name))
        .collect();

    let vm_names_len = match kernel_config.arch {
        Arch::Aarch64 | Arch::Riscv64 => vm_names.len(),
        // VM on x86 doesn't have a separate TCB.
        Arch::X86_64 => 0,
    };
    monitor_elf.write_symbol("vm_names_len", &vm_names_len.to_le_bytes())?;
    monitor_elf.write_symbol(
        "vm_names",
        &monitor_serialise_names(vm_names, MAX_VMS, VM_MAX_NAME_LENGTH),
    )?;

    let pd_stack_bottoms: Vec<_> = pds
        .values()
        .map(|pd| kernel_config.pd_stack_bottom(pd.stack_size))
        .collect();
    monitor_elf.write_symbol(
        "pd_stack_bottom_addrs",
        &monitor_serialise_u64_vec(&pd_stack_bottoms),
    )?;

    // *********************************
    // Step 2. Write ELF symbols for each PD
    // *********************************

    for pd in pds.values() {
        let elf = pd_elf_files
            .get_mut(&pd.name)
            .expect("1:1 mapping of pds and pd_elf_files");

        let name = pd.name.as_bytes();
        let name_length = min(name.len(), PD_MAX_NAME_LENGTH);
        elf.write_symbol("microkit_name", &name[..name_length])?;
        elf.write_symbol("microkit_passive", &[pd.passive as u8])?;

        let mut notification_bits: u64 = 0;
        let mut pp_bits: u64 = 0;
        for channel in channels {
            if channel.end_a.pd == pd.name {
                if channel.end_a.notify {
                    notification_bits |= 1 << channel.end_a.id;
                }
                if channel.end_a.pp {
                    pp_bits |= 1 << channel.end_a.id;
                }
            }
            if channel.end_b.pd == pd.name {
                if channel.end_b.notify {
                    notification_bits |= 1 << channel.end_b.id;
                }
                if channel.end_b.pp {
                    pp_bits |= 1 << channel.end_b.id;
                }
            }
        }

        let mut sgi_bits = 0;
        for (_, recv) in cross_core_receiver_channels.iter() {
            if recv.pd == pd.name {
                sgi_bits |= 1 << recv.id;
            }
        }

        // println!("writing sgi_bits {sgi_bits:#x}");
        // This includes the SGI notification channels too as they need to be
        // microkit_irq_ack(). See the implementation of libmicrokit/main.c
        assert!(sgi_bits & pd.irq_bits() == 0);
        let pd_irq_bits = pd.irq_bits() | sgi_bits;

        elf.write_symbol("microkit_irqs", &pd_irq_bits.to_le_bytes())?;
        elf.write_symbol("microkit_notifications", &notification_bits.to_le_bytes())?;
        elf.write_symbol("microkit_pps", &pp_bits.to_le_bytes())?;
        elf.write_symbol("microkit_ioports", &pd.ioport_bits().to_le_bytes())?;
        elf.write_symbol("microkit_sgi_notifications", &sgi_bits.to_le_bytes())?;

        for setvar in pd.setvars.iter() {
            let value = match &setvar.kind {
                sdf::SysSetVarKind::Size { mr } => {
                    memory_regions.iter().find(|m| *m.name == *mr).unwrap().size
                }
                sdf::SysSetVarKind::Vaddr { address } => *address,
                sdf::SysSetVarKind::Paddr { region } => memory_regions
                    .iter()
                    .find(|mr| mr.name == *region)
                    .unwrap_or_else(|| panic!("Cannot find region: {region}"))
                    .paddr()
                    .unwrap(),
                sdf::SysSetVarKind::X86IoPortAddr { address } => *address,
                sdf::SysSetVarKind::Id { id } => *id,
            };

            let result = elf.write_symbol(&setvar.symbol, &value.to_le_bytes());
            if result.is_err() {
                return Err(format!(
                    "No symbol named '{}' in ELF '{}' for PD '{}'",
                    setvar.symbol,
                    pd.program_image.display(),
                    pd.name
                ));
            }
        }
    }

    Ok(())
}
