//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

pub struct Config {
    pub arch: Arch,
    pub word_size: u64,
    pub minimum_page_size: u64,
    pub paddr_user_device_top: u64,
    pub kernel_frame_size: u64,
    pub init_cnode_bits: u64,
    pub cap_address_bits: u64,
    pub fan_out_limit: u64,
    pub hypervisor: bool,
    pub benchmark: bool,
    pub fpu: bool,
    /// ARM-specific, number of physical address bits
    pub arm_pa_size_bits: Option<usize>,
    /// ARM-specific, where or not SMC forwarding is allowed
    /// False if the kernel config option has not been enabled.
    /// None on any non-ARM architecture.
    pub arm_smc: Option<bool>,
    /// RISC-V specific, what kind of virtual memory system (e.g Sv39)
    pub riscv_pt_levels: Option<RiscvVirtualMemory>,
    /// x86 specific, user context size
    pub x86_xsave_size: Option<usize>,
    pub invocations_labels: serde_json::Value,
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
            }
            Arch::Riscv64 => match self.riscv_pt_levels.unwrap() {
                RiscvVirtualMemory::Sv39 => u64::pow(2, 64) - u64::pow(2,38),
            }
            Arch::X86_64 => u64::pow(2, 64) - u64::pow(2,39),
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
}

#[derive(PartialEq, Eq)]
pub enum Arch {
    Aarch64,
    Riscv64,
    X86_64,
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

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
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
    AsidPool
}

impl ObjectType {
    /// Gets the number of bits to represent the size of a object. The
    /// size depends on architecture as well as kernel configuration.
    pub fn fixed_size_bits(self, config: &Config) -> Option<u64> {
        match self {
            ObjectType::Tcb => match config.arch {
                Arch::Aarch64 => Some(11),
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
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum PageSize {
    Small = 0x1000,
    Large = 0x200_000,
}

impl From<u64> for PageSize {
    fn from(item: u64) -> PageSize {
        match item {
            0x1000 => PageSize::Small,
            0x200_000 => PageSize::Large,
            _ => panic!("Unknown page size {:x}", item),
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

#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
/// The same values apply to all kernel architectures
pub enum ArmRiscvIrqTrigger {
    Level = 0,
    Edge = 1,
}
