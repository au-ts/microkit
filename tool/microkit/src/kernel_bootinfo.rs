
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
    pub _padding: [u8; 4usize],
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
