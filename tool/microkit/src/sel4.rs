//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use std::{cmp::max, fmt::Display};

use serde::Deserialize;

use crate::{elf::ElfFile, util, DisjointMemoryRegion, MemoryRegion, UntypedObject};

use crate::sdf::{ChannelEnd, SysMemoryRegion, CpuCore, SystemDescription, SysMemoryRegionPaddr};

use std::collections::BTreeMap;

use crate::min;

pub struct KernelPartialBootInfo {
    device_memory: DisjointMemoryRegion,
    normal_memory: DisjointMemoryRegion,
    kernel_p_v_offset: u64,
    boot_region: MemoryRegion,
}

#[derive(Clone, Debug)]
pub struct BootInfo {
    pub p_v_offset: u64,
    pub fixed_cap_count: u64,
    pub sched_control_cap: u64,
    pub paging_cap_count: u64,
    pub page_cap_count: u64,
    pub untyped_objects: Vec<UntypedObject>,
    pub first_available_cap: u64,
}

#[derive(Clone, Debug)]
pub struct FullSystemState {
    pub sgi_irq_numbers: BTreeMap<ChannelEnd, u64>,
    pub sys_memory_regions: Vec<SysMemoryRegion>,
    pub per_core_ram_regions: BTreeMap<CpuCore, DisjointMemoryRegion>,
    pub shared_memory_phys_regions: DisjointMemoryRegion,
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct seL4_KernelBootInfo {
    pub magic: u32,
    pub version: u8,
    pub _padding0: [u8; 3usize],
    pub root_task_entry: u64,
    pub num_kernel_regions: u8,
    pub num_ram_regions: u8,
    pub num_root_task_regions: u8,
    pub num_reserved_regions: u8,
    pub num_mpidrs: u8,
    pub _padding: [u8; 3usize],
}
#[repr(C, packed(8))]
#[derive(Debug, Copy, Clone)]
pub struct seL4_KernelBoot_KernelRegion {
    pub base: u64,
    pub end: u64,
}
#[repr(C, packed(8))]
#[derive(Debug, Copy, Clone)]
pub struct seL4_KernelBoot_RamRegion {
    pub base: u64,
    pub end: u64,
}
#[repr(C, packed(8))]
#[derive(Debug, Copy, Clone)]
pub struct seL4_KernelBoot_RootTaskRegion {
    pub paddr_base: u64,
    pub paddr_end: u64,
    pub vaddr_base: u64,
    pub _padding: [u8; 8usize],
}
#[repr(C, packed(8))]
#[derive(Debug, Copy, Clone)]
pub struct seL4_KernelBoot_ReservedRegion {
    pub base: u64,
    pub end: u64,
}

pub const SEL4_KERNEL_BOOT_INFO_MAGIC: u32 = 0x73654c34;  /* "seL4" */

pub const SEL4_KERNEL_BOOT_INFO_VERSION_0: u8 = 0;        /* Version 0 */

fn kernel_self_mem(kernel_elf: &ElfFile) -> MemoryRegion {
    let segments = kernel_elf.loadable_segments();
    let base = segments[0].phys_addr;
    let (ki_end_v, _) = kernel_elf
        .find_symbol("ki_end")
        .expect("Could not find 'ki_end' symbol");
    let ki_end_p = ki_end_v - segments[0].virt_addr + base;

    MemoryRegion::new(base, ki_end_p)
}

fn kernel_boot_mem(kernel_elf: &ElfFile) -> MemoryRegion {
    let segments = kernel_elf.loadable_segments();
    let base = segments[0].phys_addr;
    let (ki_boot_end_v, _) = kernel_elf
        .find_symbol("ki_boot_end")
        .expect("Could not find 'ki_boot_end' symbol");
    let ki_boot_end_p = ki_boot_end_v - segments[0].virt_addr + base;

    MemoryRegion::new(base, ki_boot_end_p)
}

pub fn kernel_calculate_virt_image(kernel_elf: &ElfFile) -> MemoryRegion {
    let kernel_first_vaddr = kernel_elf
        .loadable_segments()
        .first()
        .expect("kernel has at least one loadable segment")
        .virt_addr;

    let (kernel_last_vaddr, _) = kernel_elf
        .find_symbol("ki_end")
        .expect("Could not find 'ki_end' symbol");


    MemoryRegion::new(kernel_first_vaddr, kernel_last_vaddr)
}

fn kernel_calculate_phys_image(
    kernel_elf: &ElfFile,
    ram_regions: &DisjointMemoryRegion,
) -> (MemoryRegion, MemoryRegion, u64) {
    // Calculate where the kernel image region is
    let kernel_virt_image = kernel_calculate_virt_image(kernel_elf);
    // println!("Kernel Virt Image: {:#x?}", kernel_virt_image);

    // nb: Picked arbitrarily
    let kernel_first_paddr = ram_regions.regions[0].base;
    let kernel_p_v_offset = kernel_virt_image.base - kernel_first_paddr;

    // Remove the kernel image.
    let kernel_last_paddr = kernel_virt_image.end - kernel_p_v_offset;
    let kernel_phys_image = MemoryRegion::new(kernel_first_paddr, kernel_last_paddr);
    // println!("Kernel Phys Image: {:#x?}", kernel_phys_image);

    // but get the boot region, we'll add that back later
    // FIXME: Why calculate it now if we add it back later?
    let (ki_boot_end_v, _) = kernel_elf
        .find_symbol("ki_boot_end")
        .expect("Could not find 'ki_boot_end' symbol");
    assert!(ki_boot_end_v < kernel_virt_image.end);
    let ki_boot_end_p = ki_boot_end_v - kernel_p_v_offset;
    let boot_region = MemoryRegion::new(kernel_first_paddr, ki_boot_end_p);

    (kernel_phys_image, boot_region, kernel_p_v_offset)
}

///
/// Emulate what happens during a kernel boot, up to the point
/// where the reserved region is allocated to determine the memory ranges
/// available. Only valid for ARM and RISC-V platforms.
///
fn kernel_partial_boot(
    kernel_config: &Config,
    kernel_elf: &ElfFile,
    full_system_state: &FullSystemState,
    cpu: CpuCore,
) -> KernelPartialBootInfo {
    // Determine the untyped caps of the system
    // This lets allocations happen correctly.
    // This function follows the kernel boot sequence.

    // println!("kernel_partial_boot cpu: {cpu:?}");

    // Reserved regions will cover device memory, and the memory of other
    // cores that we do not wish to modify.
    let mut reserved_regions = DisjointMemoryRegion::default();

    // This mimics the arguments we pass to the kernel boot in loader.rs
    for (_, other_core_ram) in full_system_state
        .per_core_ram_regions
        .iter()
        .filter(|(&other_cpu, _)| other_cpu != cpu)
    {
        // Add all the regions of other core's allocated ram regions
        // to reserved regions
        // This removes things like other core's kernels from device UT,
        // as well as other cores' normal UT from our device UT.
        for region in other_core_ram.regions.iter() {
            reserved_regions.insert_region(region.base, region.end);
        }
    }

    // println!("other-core: {reserved_regions:#x?}");

    // =====
    //       Here we emulate init_freemem() and arch_init_freemem(), excluding
    //       the addition of the root task memory to the reserved regions,
    //       as we don't know this information yet.
    //       Multikernel: Also follows arch_init_coremem() for subset physical memory.
    // =====

    // Passed to the kernel
    let ram_regions = full_system_state
        .per_core_ram_regions
        .get(&cpu)
        .expect("INTERNAL: should have chosen RAM for a core we are booting");

    // Done during map_kernel_window(): Remove any kernel-reserved device regions
    for region in kernel_config.kernel_devices.as_ref().unwrap().iter() {
        if !region.user_available {
            reserved_regions.insert_region(region.start, region.end);
        }
    }

    // println!("kernel-devices: {reserved_regions:#x?}");

    // ============ arch_init_freemem():
    // XXX: Theoreticallly, the initial task size would be added to reserved regions, as well
    //    as the DTB and the extra reserved region. But it's not since this is partial()

    let (kernel_region, boot_region, kernel_p_v_offset) =
        kernel_calculate_phys_image(kernel_elf, ram_regions);

    reserved_regions.insert_region(kernel_region.base, kernel_region.end);

    // println!("kernel: {reserved_regions:#x?}");

    let mut available_regions = ram_regions.clone();

    // println!("avail: {available_regions:#x?}");

    // ============ init_freemem()

    let mut free_memory = DisjointMemoryRegion::default();

    // "Now iterate through the available regions, removing any reserved regions."
    let reserved_regions = {
        let mut reserved2 = DisjointMemoryRegion::default();
        let mut a = 0;
        let mut r = 0;
        let reserved = &mut reserved_regions.regions;
        let avail_reg = &mut available_regions.regions;
        while a < avail_reg.len() && r < reserved.len() {
            if reserved[r].base == reserved[r].end {
                /* reserved region is empty - skip it */
                r += 1;
            } else if avail_reg[a].base >= avail_reg[a].end {
                /* skip the entire region - it's empty now after trimming */
                a += 1;
            } else if reserved[r].end <= avail_reg[a].base {
                /* the reserved region is below the available region - skip it */
                reserved2.insert_region(reserved[r].base, reserved[r].end);
                r += 1;
            } else if reserved[r].base >= avail_reg[a].end {
                /* the reserved region is above the available region - take the whole thing */
                reserved2.insert_region(avail_reg[a].base, avail_reg[a].end);
                free_memory.insert_region(avail_reg[a].base, avail_reg[a].end);
                a += 1;
            } else {
                /* the reserved region overlaps with the available region */
                if reserved[r].base <= avail_reg[a].base {
                    /* the region overlaps with the start of the available region.
                     * trim start of the available region */
                    avail_reg[a].base = min(avail_reg[a].end, reserved[r].end);
                    /* do not increment reserved index here - there could be more overlapping regions */
                } else {
                    assert!(reserved[r].base < avail_reg[a].end);
                    /* take the first chunk of the available region and move
                     * the start to the end of the reserved region */
                    let mut m = avail_reg[a];
                    m.end = reserved[r].base;
                    reserved2.insert_region(m.base, m.end);
                    free_memory.insert_region(m.base, m.end);
                    if avail_reg[a].end > reserved[r].end {
                        avail_reg[a].base = reserved[r].end;
                        /* we could increment reserved index here, but it's more consistent with the
                         * other overlapping case if we don't */
                    } else {
                        a += 1;
                    }
                }
            }
        }

        // add the rest of the reserved
        while r < reserved.len() {
            if reserved[r].base < reserved[r].end {
                reserved2.insert_region(reserved[r].base, reserved[r].end);
            }

            r += 1;
        }

        // add the rest of the available
        while a < avail_reg.len() {
            if avail_reg[a].base < avail_reg[a].end {
                reserved2.insert_region(avail_reg[a].base, avail_reg[a].end);
                free_memory.insert_region(avail_reg[a].base, avail_reg[a].end);
            }

            a += 1;
        }

        // println!("{:x?}\n{:x?}", reserved_regions.regions, reserved2.regions);

        reserved2
    };

    // println!("init freemem reserved2: {reserved_regions:#x?}");
    // println!("init freemem free: {free_memory:#x?}");


    // println!("Finished the construction of reserved regions from the available ram!");

    // ====
    //     Here we emulate create_untypeds(), where normal_memory represents
    //     the normal memory untypeds, and device_memory is those untypeds
    //     marked as "device". All of this is available to userspace.
    // ====

    let mut device_memory = DisjointMemoryRegion::default();
    let mut normal_memory = DisjointMemoryRegion::default();

    let mut start = 0;
    for reserved_reg in reserved_regions.regions.iter() {
        // If we have a reserved region that starts at [0x0, ...) like is the
        // case on boards where DRAM starts at 0, then we can't insert this
        // region in device memory.
        // Upstream seL4 doesn't run into this issue because it allows inserting
        // regions of zero-length into the array of regions. It's
        // create_untypeds_for_region and (our) aligned_power_of_two_regions
        // will do nothing if the region is empty.
        if start == 0 && reserved_reg.base == 0 {
            start = reserved_reg.end;
            continue;
        }

        device_memory.insert_region(start, reserved_reg.base);
        start = reserved_reg.end;
    }

    if start < kernel_config.paddr_user_device_top {
        device_memory.insert_region(start, kernel_config.paddr_user_device_top);
    }

    // println!("device UT: {device_memory:#x?}");

    // XXX: Don't add the boot_region to normal_memory, for some reason (???)

    // =========== Add the free memory as normal
    for free_memory in free_memory.regions.iter() {
        normal_memory.insert_region(free_memory.base, free_memory.end);
    }

    // println!("normal UT: {normal_memory:#x?}");

    KernelPartialBootInfo {
        device_memory,
        normal_memory,
        kernel_p_v_offset,
        boot_region,
    }
}

pub fn emulate_kernel_boot_partial(
    kernel_config: &Config,
    kernel_elf: &ElfFile,
    full_system_state: &FullSystemState,
    cpu: CpuCore,
) -> (DisjointMemoryRegion, MemoryRegion, u64) {
    // println!("Attempting to emulate kernel boot partial!");
    let partial_info = kernel_partial_boot(kernel_config, kernel_elf, full_system_state, cpu);
    (
        partial_info.normal_memory,
        partial_info.boot_region,
        partial_info.kernel_p_v_offset,
    )
}

fn get_n_paging(region: MemoryRegion, bits: u64) -> u64 {
    let start = util::round_down(region.base, 1 << bits);
    let end = util::round_up(region.end, 1 << bits);

    (end - start) / (1 << bits)
}

fn get_arch_n_paging(config: &Config, region: MemoryRegion) -> u64 {
    match config.arch {
        Arch::Aarch64 => {
            const PT_INDEX_OFFSET: u64 = 12;
            const PD_INDEX_OFFSET: u64 = PT_INDEX_OFFSET + 9;
            const PUD_INDEX_OFFSET: u64 = PD_INDEX_OFFSET + 9;

            if config.aarch64_vspace_s2_start_l1() {
                get_n_paging(region, PUD_INDEX_OFFSET) + get_n_paging(region, PD_INDEX_OFFSET)
            } else {
                const PGD_INDEX_OFFSET: u64 = PUD_INDEX_OFFSET + 9;
                get_n_paging(region, PGD_INDEX_OFFSET)
                    + get_n_paging(region, PUD_INDEX_OFFSET)
                    + get_n_paging(region, PD_INDEX_OFFSET)
            }
        }
        Arch::Riscv64 => match config.riscv_pt_levels.unwrap() {
            RiscvVirtualMemory::Sv39 => {
                const PT_INDEX_OFFSET: u64 = 12;
                const LVL1_INDEX_OFFSET: u64 = PT_INDEX_OFFSET + 9;
                const LVL2_INDEX_OFFSET: u64 = LVL1_INDEX_OFFSET + 9;

                get_n_paging(region, LVL2_INDEX_OFFSET) + get_n_paging(region, LVL1_INDEX_OFFSET)
            }
        },
        Arch::X86_64 => unreachable!("the kernel boot process should not be emulated for x86!"),
    }
}

/// Refer to `calculate_rootserver_size()` in src/kernel/boot.c of seL4
fn calculate_rootserver_size(config: &Config, initial_task_region: MemoryRegion) -> u64 {
    // FIXME: These constants should ideally come from the config / kernel
    // binary not be hard coded here.
    // But they are constant so it isn't too bad.
    let slot_bits = 5; // seL4_SlotBits
    let root_cnode_bits = config.init_cnode_bits; // CONFIG_ROOT_CNODE_SIZE_BITS
    let tcb_bits = ObjectType::Tcb.fixed_size_bits(config).unwrap(); // seL4_TCBBits
    let page_bits = ObjectType::SmallPage.fixed_size_bits(config).unwrap(); // seL4_PageBits
    let asid_pool_bits = 12; // seL4_ASIDPoolBits
    let vspace_bits = ObjectType::VSpace.fixed_size_bits(config).unwrap(); // seL4_VSpaceBits
    let page_table_bits = ObjectType::PageTable.fixed_size_bits(config).unwrap(); // seL4_PageTableBits
    let min_sched_context_bits = 7; // seL4_MinSchedContextBits

    let mut size = 0;
    size += 1 << (root_cnode_bits + slot_bits);
    size += 1 << (tcb_bits);
    size += 2 * (1 << page_bits);
    size += 1 << asid_pool_bits;
    size += 1 << vspace_bits;
    size += get_arch_n_paging(config, initial_task_region) * (1 << page_table_bits);
    size += 1 << min_sched_context_bits;
    size
}

fn rootserver_max_size_bits(config: &Config) -> u64 {
    let slot_bits = 5; // seL4_SlotBits
    let root_cnode_bits = config.init_cnode_bits; // CONFIG_ROOT_CNODE_SIZE_BITS
    let vspace_bits = ObjectType::VSpace.fixed_size_bits(config).unwrap();

    let cnode_size_bits = root_cnode_bits + slot_bits;
    max(cnode_size_bits, vspace_bits)
}

/// Emulate what happens during a kernel boot, generating a
/// representation of the BootInfo struct.
pub fn emulate_kernel_boot(
    config: &Config,
    kernel_elf: &ElfFile,
    full_system_state: &FullSystemState,
    cpu: CpuCore,
    initial_task_phys_region: MemoryRegion,
    user_image_virt_region: MemoryRegion,
) -> BootInfo {
    assert!(initial_task_phys_region.size() == user_image_virt_region.size());
    let partial_info = kernel_partial_boot(config, kernel_elf, full_system_state, cpu);
    let mut normal_memory = partial_info.normal_memory;
    let device_memory = partial_info.device_memory;
    let boot_region = partial_info.boot_region;

    normal_memory.remove_region(initial_task_phys_region.base, initial_task_phys_region.end);

    let mut initial_task_virt_region = user_image_virt_region;
    // Refer to `try_init_kernel()` of src/arch/[arm,riscv]/kernel/boot.c
    let ipc_size = PageSize::Small as u64; // seL4_PageBits
    let bootinfo_size = PageSize::Small as u64; // seL4_BootInfoFrameBits
    initial_task_virt_region.end += ipc_size;
    initial_task_virt_region.end += bootinfo_size;

    // Now, the tricky part! determine which memory is used for the initial task objects
    let initial_objects_size = calculate_rootserver_size(config, initial_task_virt_region);
    let initial_objects_align = rootserver_max_size_bits(config);

    // Find an appropriate region of normal memory to allocate the objects
    // from; this follows the same algorithm used within the kernel boot code
    // (or at least we hope it does!)
    // TODO: this loop could be done better in a functional way?
    let mut region_to_remove: Option<u64> = None;
    for region in normal_memory.regions.iter().rev() {
        let start = util::round_down(
            region.end - initial_objects_size,
            1 << initial_objects_align,
        );
        if start >= region.base {
            region_to_remove = Some(start);
            break;
        }
    }
    if let Some(start) = region_to_remove {
        normal_memory.remove_region(start, start + initial_objects_size);
    } else {
        panic!("Couldn't find appropriate region for initial task kernel objects");
    }

    let fixed_cap_count = 0x10;
    let sched_control_cap_count = 1;
    let paging_cap_count = get_arch_n_paging(config, initial_task_virt_region);
    let page_cap_count = initial_task_virt_region.size() / config.minimum_page_size;
    let first_untyped_cap =
        fixed_cap_count + paging_cap_count + sched_control_cap_count + page_cap_count;
    let sched_control_cap = fixed_cap_count + paging_cap_count;

    let max_bits = match config.arch {
        Arch::Aarch64 => 47,
        Arch::Riscv64 => 38,
        Arch::X86_64 => unreachable!("the kernel boot process should not be emulated for x86!"),
    };
    let device_regions: Vec<MemoryRegion> =
        device_memory.aligned_power_of_two_regions(config, max_bits);
    let normal_regions: Vec<MemoryRegion> = [
        boot_region.aligned_power_of_two_regions(config, max_bits),
        normal_memory.aligned_power_of_two_regions(config, max_bits),
    ]
    .concat();
    let mut untyped_objects = Vec::new();
    for (i, r) in device_regions.iter().enumerate() {
        let cap = i as u64 + first_untyped_cap;
        untyped_objects.push(UntypedObject::new(cap, *r, true));
    }
    let normal_regions_start_cap = first_untyped_cap + device_regions.len() as u64;
    for (i, r) in normal_regions.iter().enumerate() {
        let cap = i as u64 + normal_regions_start_cap;
        untyped_objects.push(UntypedObject::new(cap, *r, false));
    }

    let first_available_cap =
        first_untyped_cap + device_regions.len() as u64 + normal_regions.len() as u64;
    BootInfo {
        p_v_offset: partial_info.kernel_p_v_offset,
        fixed_cap_count,
        paging_cap_count,
        page_cap_count,
        sched_control_cap,
        first_available_cap,
        untyped_objects,
    }
}

pub fn pick_sgi_channels(
    system: &SystemDescription,
    kernel_config: &Config,
) -> BTreeMap<ChannelEnd, u64> {
    // TODO: Add support for different architectures, the sgi mechanisms
    // will be different
    // TODO: other platforms.
    assert!(kernel_config.arch == Arch::Aarch64);
    let num_sgi_irqs_per_core = match kernel_config
        .arm_gic_version
        .expect("INTERNAL: arm_gic_version specified on arm")
    {
        // TODO: Source document?
        // Maybe 16 not always OK????
        ArmGicVersion::GICv2 => 8,
        ArmGicVersion::GICv3 => 16,
    };

    // Because when we issue an SGI sender cap, we can specify a singular target,
    // seL4 gives us the ability for each core to have {NUM_SGI} receivers and
    // be able to distinguish between them.

    // Storing only the receiver is necessary, but to be able to print a good error message we store (send, recv).
    let mut sgi_receivers_by_core = BTreeMap::<CpuCore, Vec<(&ChannelEnd, &ChannelEnd)>>::new();

    for (send, recv, _, recv_pd) in system
        .channels
        .iter()
        // Make both directions of the channels
        .flat_map(|cc| [(&cc.end_a, &cc.end_b), (&cc.end_b, &cc.end_a)])
        .map(|(send, recv)| {
            (
                send,
                recv,
                &system.protection_domains[&send.pd],
                &system.protection_domains[&recv.pd],
            )
        })
        // On different cores.
        .filter(|(_, _, send_pd, recv_pd)| send_pd.cpu != recv_pd.cpu)
        // And only look at the ones where the sender can notify
        //     and where the channel in the right direction
        .filter(|(send, _, _, _)| send.notify)
    {
        sgi_receivers_by_core
            .entry(recv_pd.cpu)
            .or_default()
            .push((send, recv));
    }

    let mut sgi_irq_numbers = BTreeMap::<ChannelEnd, u64>::new();
    let mut failure = false;

    for (_, channels) in sgi_receivers_by_core.iter() {
        if channels.len() > num_sgi_irqs_per_core {
            failure = true;
            continue;
        }

        for (sgi_irq, &(_, recv)) in channels.iter().enumerate() {
            sgi_irq_numbers.insert(recv.clone(), sgi_irq.try_into().expect("IRQ fits in u64"));
        }
    }

    if failure {
        eprintln!("at least one core needed more than {num_sgi_irqs_per_core} SGI IRQs");
        eprintln!("channels needing SGIs:");
        for (cpu, channels) in sgi_receivers_by_core.iter() {
            eprintln!("    receiver {cpu}; count: {}:", channels.len());
            for &(send, recv) in channels.iter() {
                eprintln!(
                    "       {:<30} (id: {:>2}) |-> {:<30} (id: {:>2})",
                    send.pd, send.id, recv.pd, recv.id,
                );
            }
        }

        std::process::exit(1);
    }

    // TODO: add the used SGIs to the report.
    for (cpu, channels) in sgi_receivers_by_core.iter() {
        eprintln!("    receiver {cpu}; count: {}:", channels.len());
        for &(send, recv) in channels.iter() {
            eprintln!(
                "       {:<30} (id: {:>2}) |-> {:<30} (id: {:>2}) ==> SGI {:>2}",
                send.pd, send.id, recv.pd, recv.id, sgi_irq_numbers[recv],
            );
        }
    }

    sgi_irq_numbers
}

pub fn build_full_system_state(
    system: &SystemDescription,
    kernel_config: &Config,
    kernel_virt_image: MemoryRegion
) -> FullSystemState {
    let sgi_irq_numbers = pick_sgi_channels(system, kernel_config);

    // Take all the memory regions used on multiple cores and make them shared.
    // Note: there are a few annoying things we have to deal with:
    // - users might specify phys_addr that point into RAM
    // - users might also specify phys_addr that points to device registers (is this ever sensible?)
    // - the phys_addr the user specify in RAM may overlap with our auto-allocated physical addresses
    //   for when that don't specify RAM phys_addr in shared memory
    // - a lot of user code expects these regions to be zeroed out on boot
    //   (like how seL4 would otherwise make them) so we need to zero them out
    //    in the loader
    //
    // We do the following, which is somewhat hacky:
    //  for each user memory region, if it's used across multiple cores:
    //      1. check if it has a phys_addr; if it does:
    //          a. if it corresponds to device RAM, then we remove it
    //             from "normal memory" and mark it as a shared region
    //             to be zeroed in the loader
    //          b. if it doesn't correspond to device RAM, then it's device registers
    //             this is already device memory which can be "shared" and
    //             shouldn't be zeroed in the loader.
    //      2. else, we need to autoassign it. save it temporarily and once
    //         we have processed all the user-specified memory regions
    //         (which might allocate more holes) we allocate a shared region
    //         started from the top of RAM and moving down.
    //         FIXME: This might need to be split into multiple to handle overlaps
    //                for now we check if it does happen, but we don't resolve it.

    let mut sys_memory_regions = vec![];
    // This checks overlaps of the shared memory regions if specified by paddr
    let mut shared_memory_phys_regions = DisjointMemoryRegion::default();
    let mut to_allocate_phys_addr_shared_indices = vec![];

    for mr in system.memory_regions.iter().cloned() {
        if mr.used_cores.len() > 1 {
            // println!("allocation shared: {mr:#x?}");

            match mr.phys_addr {
                SysMemoryRegionPaddr::Specified(phys_addr) => {
                    if kernel_config.normal_regions.is_some() {
                        let in_ram = kernel_config.normal_regions.as_ref().unwrap().iter().fold(false, |acc, reg| {
                            let in_region = reg.start <= phys_addr && phys_addr < reg.end;

                            // We could early exit instead of reducing, but this extra
                            // check is nice to make sure we haven't messed up any of the
                            // logic.
                            if acc && in_region {
                                panic!("INTERNAL: phys_addr is somehow in two memory regions");
                            }

                            in_region
                        });

                        if in_ram {
                            shared_memory_phys_regions.insert_region(phys_addr, phys_addr + mr.size);
                        } else {
                            // Do nothing to it.
                        }
                    }
                }
                _ => {
                    // index of the memory region in sys_memory_regions.
                    to_allocate_phys_addr_shared_indices.push(sys_memory_regions.len());
                }
            }
        }

        sys_memory_regions.push(mr);
    }

    // FIXME: this would need to be changed if we want to handle overlaps
    //        of specified phys regions.
    // @kwinter: We shouldn't be using is_some
    let last_ram_region = if kernel_config.normal_regions.is_some() {
            kernel_config
            .normal_regions
            .as_ref()
            .unwrap()
            .last()
            .expect("kernel should have one memory region")
        } else {
            panic!("We shouldn't be calling build state for x86 builds yet!\n");
        };

    let mut shared_phys_addr_prev = last_ram_region.end;

    for &shared_index in to_allocate_phys_addr_shared_indices.iter() {
        let mr = sys_memory_regions
            .get_mut(shared_index)
            .expect("should be valid by construction");

        let phys_addr = shared_phys_addr_prev
            .checked_sub(mr.size)
            .expect("no underflow :(");
        mr.phys_addr = SysMemoryRegionPaddr::ToolAllocated(Some(phys_addr));
        // FIXME: This would crash if overlap happens with shared memory paddrs.
        shared_memory_phys_regions.insert_region(phys_addr, phys_addr + mr.size);
        shared_phys_addr_prev = phys_addr;
    }

    let per_core_ram_regions = {
        let mut per_core_regions = BTreeMap::new();

        let mut available_normal_memory = DisjointMemoryRegion::default();
        if kernel_config.normal_regions.is_some() {
            for region in kernel_config.normal_regions.as_ref().unwrap().iter() {
                available_normal_memory.insert_region(region.start, region.end);
            }
        } else {
            panic!("We shouldn't be calling build full system state for x86 builds yet!");
        }

        // Remove shared memory.
        for s_mr in shared_memory_phys_regions.regions.iter() {
            available_normal_memory.remove_region(s_mr.base, s_mr.end);
        }

        println!("available memory:");
        for r in available_normal_memory.regions.iter() {
            println!("    [{:x}..{:x})", r.base, r.end);
        }

        // TODO: I'm not convinced of this algorithm's correctness of always working.
        //       It might not be the most efficient, but I don't know if it will always work.

        let kernel_size = kernel_virt_image.size();

        for cpu in 0..kernel_config.num_multikernels {
            let mut normal_memory = DisjointMemoryRegion::default();

            // FIXME: ARM64 requires LargePage alignment, what about others?
            let kernel_mem = available_normal_memory.allocate_aligned(
                kernel_size,
                ObjectType::LargePage.fixed_size(&kernel_config).unwrap(),
            );

            println!(
                "cpu({cpu}) kernel ram: [{:x}..{:x})",
                kernel_mem.base, kernel_mem.end,
            );

            normal_memory.insert_region(kernel_mem.base, kernel_mem.end);
            per_core_regions.insert(CpuCore(cpu), normal_memory);
        }

        println!("available memory after allocating kernels:");
        for r in available_normal_memory.regions.iter() {
            println!("    [{:x}..{:x})", r.base, r.end);
        }

        let total_ram_size: u64 = available_normal_memory
            .regions
            .iter()
            .map(|mr| mr.size())
            .sum();

        let per_core_ram_size = util::round_down(
            total_ram_size / u64::from(kernel_config.num_multikernels),
            ObjectType::SmallPage.fixed_size(&kernel_config).unwrap(),
        );

        for (&cpu, core_normal_memory) in per_core_regions.iter_mut() {
            let ram_mem = available_normal_memory
                .allocate_non_contiguous(per_core_ram_size)
                .expect("should have been able to allocate part of the RAM we chose");

            println!("cpu({}) normal ram: ", cpu.0);
            for r in ram_mem.regions.iter() {
                println!("    [{:x}..{:x})", r.base, r.end);
            }

            core_normal_memory.extend(&ram_mem);
        }

        per_core_regions
    };

    FullSystemState {
        sgi_irq_numbers,
        sys_memory_regions,
        per_core_ram_regions,
        shared_memory_phys_regions,
    }
}

#[derive(Deserialize)]
pub struct PlatformConfigRegion {
    pub start: u64,
    pub end: u64,
}

#[derive(Deserialize)]
pub struct PlatformKernelDeviceRegion {
    pub start: u64,
    pub end: u64,
    #[serde(rename = "userAvailable")]
    pub user_available: bool,
}

#[derive(Deserialize)]
pub struct PlatformConfig {
    pub devices: Vec<PlatformConfigRegion>,
    pub kernel_devs: Vec<PlatformKernelDeviceRegion>,
    pub memory: Vec<PlatformConfigRegion>,
}

pub struct Config {
    pub arch: Arch,
    pub word_size: u64,
    pub minimum_page_size: u64,
    pub paddr_user_device_top: u64,
    pub kernel_frame_size: u64,
    pub init_cnode_bits: u64,
    pub cap_address_bits: u64,
    pub fan_out_limit: u64,
    pub max_num_bootinfo_untypeds: u64,
    pub hypervisor: bool,
    pub benchmark: bool,
    pub num_cores: u8,
    pub fpu: bool,
    pub num_multikernels: u8,
    /// ARM-specific, number of physical address bits
    pub arm_pa_size_bits: Option<usize>,
    /// ARM-specific, where or not SMC forwarding is allowed
    /// False if the kernel config option has not been enabled.
    /// None on any non-ARM architecture.
    pub arm_smc: Option<bool>,
    /// ARM-specific, the GIC version
    pub arm_gic_version: Option<ArmGicVersion>,
    /// RISC-V specific, what kind of virtual memory system (e.g Sv39)
    pub riscv_pt_levels: Option<RiscvVirtualMemory>,
    /// x86 specific, user context size
    pub x86_xsave_size: Option<usize>,
    pub invocations_labels: serde_json::Value,
    /// The two remaining fields are only valid on ARM and RISC-V
    pub kernel_devices: Option<Vec<PlatformKernelDeviceRegion>>,
    pub normal_regions: Option<Vec<PlatformConfigRegion>>,
    pub domain_scheduler: bool,
}

impl Config {
    pub fn user_top(&self) -> u64 {
        match self.arch {
            Arch::Aarch64 => match self.hypervisor {
                true => match self.arm_pa_size_bits.unwrap() {
                    40 => 0x10000000000,
                    44 => 0x100000000000,
                    _ => panic!("Unknown ARM physical address size bits"),
                },
                false => 0x800000000000,
            },
            Arch::Riscv64 => 0x0000003ffffff000,
            Arch::X86_64 => 0x7ffffffff000,
        }
    }

    pub fn virtual_base(&self) -> u64 {
        match self.arch {
            Arch::Aarch64 => match self.hypervisor {
                true => 0x0000008000000000,
                false => u64::pow(2, 64) - u64::pow(2, 39),
            },
            Arch::Riscv64 => match self.riscv_pt_levels.unwrap() {
                RiscvVirtualMemory::Sv39 => u64::pow(2, 64) - u64::pow(2, 38),
            },
            Arch::X86_64 => u64::pow(2, 64) - u64::pow(2, 39),
        }
    }

    pub fn page_sizes(&self) -> [u64; 2] {
        match self.arch {
            Arch::Aarch64 | Arch::Riscv64 | Arch::X86_64 => [0x1000, 0x200_000],
        }
    }

    pub fn pd_stack_top(&self) -> u64 {
        self.user_top()
    }

    pub fn pd_stack_bottom(&self, stack_size: u64) -> u64 {
        self.pd_stack_top() - stack_size
    }

    /// For simplicity and consistency, the stack of each PD occupies the highest
    /// possible virtual memory region. That means that the highest possible address
    /// for a user to be able to create a mapping at is below the stack region.
    pub fn pd_map_max_vaddr(&self, stack_size: u64) -> u64 {
        // This function depends on the invariant that the stack of a PD
        // consumes the highest possible address of the virtual address space.
        assert!(self.pd_stack_top() == self.user_top());

        self.pd_stack_bottom(stack_size)
    }

    /// Unlike PDs, virtual machines do not have a stack and so the max virtual
    /// address of a mapping is whatever seL4 chooses as the maximum virtual address
    /// in a VSpace.
    pub fn vm_map_max_vaddr(&self) -> u64 {
        self.user_top()
    }

    pub fn paddr_to_kernel_vaddr(&self, paddr: u64) -> u64 {
        paddr.wrapping_add(self.virtual_base())
    }

    pub fn kernel_vaddr_to_paddr(&self, vaddr: u64) -> u64 {
        vaddr.wrapping_sub(self.virtual_base())
    }

    pub fn aarch64_vspace_s2_start_l1(&self) -> bool {
        match self.arch {
            Arch::Aarch64 => self.hypervisor && self.arm_pa_size_bits.unwrap() == 40,
            _ => panic!("internal error"),
        }
    }

    pub fn num_page_table_levels(&self) -> usize {
        match self.arch {
            Arch::Aarch64 => 4,
            Arch::Riscv64 => self.riscv_pt_levels.unwrap().levels(),
            // seL4 only supports 4-level page table on x86-64.
            Arch::X86_64 => 4,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Eq)]
pub enum Arch {
    Aarch64,
    Riscv64,
    X86_64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmGicVersion {
    GICv2,
    GICv3,
}

impl Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arch::Aarch64 => write!(f, "AArch64"),
            Arch::Riscv64 => write!(f, "RISC-V (64-bit)"),
            Arch::X86_64 => write!(f, "x86-64"),
        }
    }
}

/// RISC-V supports multiple virtual memory systems and so we use this enum
/// to make it easier to support more virtual memory systems in the future.
#[derive(Debug, Copy, Clone)]
pub enum RiscvVirtualMemory {
    Sv39,
}

impl RiscvVirtualMemory {
    /// Returns number of page-table levels for a particular virtual memory system.
    pub fn levels(self) -> usize {
        match self {
            RiscvVirtualMemory::Sv39 => 3,
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum ObjectType {
    Untyped,
    Tcb,
    Endpoint,
    Notification,
    CNode,
    SchedContext,
    Reply,
    HugePage,
    VSpace,
    SmallPage,
    LargePage,
    PageTable,
    Vcpu,
    AsidPool,
}

impl ObjectType {
    /// Gets the number of bits to represent the size of a object. The
    /// size depends on architecture as well as kernel configuration.
    pub fn fixed_size_bits(self, config: &Config) -> Option<u64> {
        match self {
            ObjectType::Tcb => match config.arch {
                Arch::Aarch64 => {
                    if config.hypervisor && config.benchmark && config.num_cores > 0 {
                        Some(12)
                    } else {
                        Some(11)
                    }
                }
                Arch::Riscv64 => match config.fpu {
                    true => Some(11),
                    false => Some(10),
                },
                Arch::X86_64 => {
                    // matches seL4/libsel4/sel4_arch_include/x86_64/sel4/sel4_arch/constants.h
                    if config.x86_xsave_size.unwrap() >= 832 {
                        Some(12)
                    } else {
                        Some(11)
                    }
                }
            },
            ObjectType::Endpoint => Some(4),
            ObjectType::Notification => Some(6),
            ObjectType::Reply => Some(5),
            ObjectType::VSpace => match config.arch {
                Arch::Aarch64 => match config.hypervisor {
                    true => match config.arm_pa_size_bits.unwrap() {
                        40 => Some(13),
                        44 => Some(12),
                        _ => {
                            panic!("Unexpected ARM PA size bits when determining VSpace size bits")
                        }
                    },
                    false => Some(12),
                },
                _ => Some(12),
            },
            ObjectType::PageTable => Some(12),
            ObjectType::HugePage => Some(30),
            ObjectType::LargePage => Some(21),
            ObjectType::SmallPage => Some(12),
            ObjectType::Vcpu => match config.arch {
                Arch::Aarch64 => Some(12),
                Arch::X86_64 => Some(14),
                _ => panic!("Unexpected architecture asking for vCPU size bits"),
            },
            ObjectType::AsidPool => Some(12),
            _ => None,
        }
    }

    pub fn fixed_size(self, config: &Config) -> Option<u64> {
        self.fixed_size_bits(config).map(|bits| 1 << bits)
    }
}

#[repr(u64)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum PageSize {
    Small = 0x1000,
    Large = 0x200_000,
}

impl From<u64> for PageSize {
    fn from(item: u64) -> PageSize {
        match item {
            0x1000 => PageSize::Small,
            0x200_000 => PageSize::Large,
            _ => panic!("Unknown page size {item:x}"),
        }
    }
}

impl PageSize {
    pub fn fixed_size_bits(&self, sel4_config: &Config) -> u64 {
        match self {
            PageSize::Small => ObjectType::SmallPage.fixed_size_bits(sel4_config).unwrap(),
            PageSize::Large => ObjectType::LargePage.fixed_size_bits(sel4_config).unwrap(),
        }
    }
}

// @merge: I would rather have the duplication of ARM and RISC-V
// rather than a type that tries to unify both.
#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// The same values apply to ARM and RISC-V
pub enum ArmRiscvIrqTrigger {
    Level = 0,
    Edge = 1,
}

impl From<u8> for ArmRiscvIrqTrigger {
    fn from(item: u8) -> ArmRiscvIrqTrigger {
        match item {
            0 => ArmRiscvIrqTrigger::Level,
            1 => ArmRiscvIrqTrigger::Edge,
            _ => panic!("Unknown ARM/RISC-V IRQ trigger {item:x}"),
        }
    }
}

impl ArmRiscvIrqTrigger {
    pub fn human_name(&self) -> &str {
        match self {
            ArmRiscvIrqTrigger::Level => "level",
            ArmRiscvIrqTrigger::Edge => "edge",
        }
    }
}

#[repr(u64)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum X86IoapicIrqTrigger {
    Level = 1,
    Edge = 0,
}

impl From<u64> for X86IoapicIrqTrigger {
    fn from(item: u64) -> X86IoapicIrqTrigger {
        match item {
            0 => X86IoapicIrqTrigger::Edge,
            1 => X86IoapicIrqTrigger::Level,
            _ => panic!("Unknown x86 IOAPIC IRQ trigger {item:x}"),
        }
    }
}

impl X86IoapicIrqTrigger {
    pub fn human_name(&self) -> &str {
        match self {
            X86IoapicIrqTrigger::Level => "level",
            X86IoapicIrqTrigger::Edge => "edge",
        }
    }
}

#[repr(u64)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum X86IoapicIrqPolarity {
    LowTriggered = 0,
    HighTriggered = 1,
}

impl From<u64> for X86IoapicIrqPolarity {
    fn from(item: u64) -> X86IoapicIrqPolarity {
        match item {
            0 => X86IoapicIrqPolarity::LowTriggered,
            1 => X86IoapicIrqPolarity::HighTriggered,
            _ => panic!("Unknown x86 IOAPIC IRQ polarity {item:x}"),
        }
    }
}

impl X86IoapicIrqPolarity {
    pub fn human_name(&self) -> &str {
        match self {
            X86IoapicIrqPolarity::LowTriggered => "low-triggered",
            X86IoapicIrqPolarity::HighTriggered => "high-triggered",
        }
    }
}
