//
// Copyright 2026, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

#![no_std]

use core::cmp::min;
use core::ffi::c_char;
use core::fmt;
use core::fmt::Write;
use core::mem;
use core::panic::PanicInfo;

unsafe extern "C" {
    safe fn fail() -> !;
    // safe fn putc(c: c_char);
    unsafe fn puts(s: *const c_char);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe { puts(c"panicked\n".as_ptr()) };

    struct DebugWriter;
    impl fmt::Write for DebugWriter {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            for c in s.bytes() {
                unsafe {
                    puts(core::ffi::CStr::from_bytes_with_nul_unchecked(&[c.into(), 0]).as_ptr())
                };
            }

            Ok(())
        }
    }

    if let Err(_) = writeln!(DebugWriter, "{}", info) {
        // If writeln!() fails (which it should never as our fmt::Write) never
        // fails, then just don't print the extra information.
        unsafe { puts(c"panicked (information unknown)\n".as_ptr()) };
    }

    fail();
}

const PAGE_TABLE_SIZE: usize = 4096;

const fn divmod(x: u64, y: u64) -> (u64, u64) {
    (x / y, x % y)
}

const fn mask(n: u64) -> u64 {
    (1 << n) - 1
}

const fn round_up(n: u64, x: u64) -> u64 {
    let (_, m) = divmod(n, x);
    if m == 0 {
        n
    } else {
        n + x - m
    }
}

const fn round_down(n: u64, x: u64) -> u64 {
    let (_, m) = divmod(n, x);
    if m == 0 {
        n
    } else {
        n - m
    }
}

const fn align_up(n: u64, bits: u64) -> u64 {
    round_up(n, 1 << bits)
}

const fn align_down(n: u64, bits: u64) -> u64 {
    round_down(n, 1 << bits)
}

unsafe extern "C" {
    static mut _text: u8;
}

pub mod aarch64 {
    //! For AArch64, our page tables use the Stage 1 descriptor formats
    //! for both EL2 (TTBR0_EL2) and EL1 (TTBR0_EL1/TTBR1_EL1).
    //! Stage 2 descriptors are only used when in the EL1&0 regime; which is not
    //! the case when in EL2.

    use crate::mask;

    pub const LVL0_BITS: u64 = 9;
    pub const LVL1_BITS: u64 = 9;
    pub const LVL2_BITS: u64 = 9;
    pub const LVL3_BITS: u64 = 9;

    pub fn lvl0_index(addr: u64) -> usize {
        let idx = (addr >> (BLOCK_BITS_2MB + LVL2_BITS + LVL1_BITS)) & mask(LVL0_BITS);
        idx as usize
    }

    pub fn lvl1_index(addr: u64) -> usize {
        let idx = (addr >> (BLOCK_BITS_2MB + LVL2_BITS)) & mask(LVL1_BITS);
        idx as usize
    }

    pub fn lvl2_index(addr: u64) -> usize {
        let idx = (addr >> (BLOCK_BITS_2MB)) & mask(LVL2_BITS);
        idx as usize
    }

    pub fn lvl3_index(addr: u64) -> usize {
        let idx = (addr >> PAGE_BITS_4KB) & mask(LVL3_BITS);
        idx as usize
    }

    /// Stage 1 translation table page/block descriptors have bits[4:2] containing
    /// AttrIndex[2:0]. The AttrIndex values depends on our configuration of
    /// the `MAIR_EL1` or `MAIR_EL2` registers done in util64.S;
    /// This also needs to match the values that seL4 uses.
    #[allow(non_upper_case_globals, reason = "matching ARM naming convention")]
    pub mod s1_mair_attr_index {
        pub const MT_DEVICE_nGnRnE: u64 = 0b000;
        pub const MT_DEVICE_nGnRE: u64 = 0b001;
        pub const MT_DEVICE_GRE: u64 = 0b010;
        pub const MT_NORMAL_NC: u64 = 0b011;
        pub const MT_NORMAL: u64 = 0b100;
    }

    pub mod descriptor_type {
        //! The translation table descriptor formats, as per §D8.3 "Translation
        //! table descriptor formats" of ARM DDI 0487 L.b. Specifically,
        //! as per "Table D8-48 Determination of descriptor type"

        /// Descriptor type: Table. Condition is lookup level != 3.
        pub const TABLE: u64 = 0b11;
        /// Descriptor type: Page. Condition is lookup level == 3.
        pub const PAGE: u64 = 0b11;
        /// Descriptor type: Block. Condition is lookup level != 3.
        pub const BLOCK: u64 = 0b01;
        /// Descriptor type: Invalid. Strictly speaking bit[1] does not matter.
        pub const INVALID: u64 = 0b00;
    }

    pub mod shareability_attributes {
        //! Per §D8.6.2 "Stage 1 Shareability attributes", these contain the
        //! shareability attributes of the descriptor OA for normal-cacheable
        //! memory.

        /// Non-shareable
        pub const NON_SHAREABLE: u64 = 0b00;
        /// Outer-shareable
        pub const OUTER_SHAREABLE: u64 = 0b10;
        /// Inner-shareable
        pub const INNER_SHAREABLE: u64 = 0b11;
    }

    /// Per "Figure D8-14 VMSAv8-64 Block descriptor formats" of ARM DDI0487L.b,
    /// subfigure "4KB, 16KB, and 64KB granules, 48-bit OA", the Output address
    /// is bits [47:n], and:
    ///
    /// > For the 4KB granule size, the level 1 descriptor n is 30,
    /// > and the level 2 descriptor n is 21.
    pub const BLOCK_BITS_1GB: u64 = 30;

    /// Per "Figure D8-14 VMSAv8-64 Block descriptor formats" of ARM DDI0487L.b,
    /// subfigure "4KB, 16KB, and 64KB granules, 48-bit OA", the Output address
    /// is bits [47:n], and:
    ///
    /// > For the 4KB granule size, the level 1 descriptor n is 30,
    /// > and the level 2 descriptor n is 21.
    pub const BLOCK_BITS_2MB: u64 = 21;

    // TODO:

    pub const BLOCK_BITS_512GB: u64 = 39;
    pub const PAGE_BITS_4KB: u64 = 12;

    /// Per "Table D8-52 Stage 1 VMSAv8-64 Block and Page descriptor fields" and
    /// "Figure D8-14 VMSAv8-64 Block descriptor formats" of ARM DDI0487L.b;
    /// specifically subfigure "4KB, 16KB, and 64KB granules, 48-bit OA"
    pub fn block_descriptor(level: usize, addr: u64, attr_index: u64) -> u64 {
        // Per Table D8-48, Condition for descriptor_type::BLOCK is level != 3.
        assert!(level != 3);

        let upper_attributes: u64 = 0;

        let shareability = if attr_index == s1_mair_attr_index::MT_NORMAL {
            // Match what the seL4 kernel uses for its page tables, which
            // is especially necessary for SMP booting which relies on it
            // for coherency. See the comment in seL4 `release_secondary_cpus()`.
            shareability_attributes::INNER_SHAREABLE
        } else {
            // Per $R_{PYFVQ}$:
            // > If a region is mapped as Device memory or Normal Non-cacheable
            // > memory after all enabled translation stages, then the region
            // > has an effective Shareability attribute of Outer Shareable.
            //
            // We override the value we place in here to OUTER_SHAREABLE to match
            // how the hardware behaves. This is not necessary but for clarity.
            shareability_attributes::OUTER_SHAREABLE
        };

        // AP[2:1], which we set as 0b00 for read/write access:
        //   stage 1: 0b00 is {PrivRead, PrivWrite} and we are EL1
        //   stage 2: 0b00 is RW for EL2 and no perms for EL1.
        const AP_KERNEL_RW: u64 = 0b00;

        // bit[11] is the not global (nG) field, we leave as 0 (global).
        // bit[10] is the access flag; depending on FEAT_HAFDBS, when software
        //         manages the AF memory accesses to the page/block when AF=0
        //         raise an Access Fault; when hardware manages the AF it will
        //         become 1.
        // bit[9:8] is SH[1:0] containing stage 1 shareability attributes
        // bit[7:6] contains AP[2:1]
        // bit[5] is RES0
        // bit[4:2] contains AttrIndex
        let lower_attributes: u64 =
            (1 << 10) | (AP_KERNEL_RW << 6) | (shareability << 8) | (attr_index << 2);

        // bits[47:n]
        let output_address: u64 = addr
            & !mask(match level {
                1 => BLOCK_BITS_1GB,
                2 => BLOCK_BITS_2MB,
                _ => panic!("unsupported level {level} for block descriptor"),
            });

        // address must not have bits above 47 set.
        assert!(addr & mask(48) == addr);

        // bits[63:50] describing the "Upper attributes" are left at 0.
        // bits[49:48] are RES0
        // bits[47:n] contain the Output address
        // bits[n-1:12] are RES0
        // bits[11:2] contain the "Lower attributes"
        // bits[1:0] contains the descriptor type
        upper_attributes | output_address | lower_attributes | descriptor_type::BLOCK
    }

    /// Per "Table D8-52 Stage 1 VMSAv8-64 Block and Page descriptor fields" and
    /// "Figure D8-15 VMSAv8-64 Page descriptor formats" of ARM DDI0487L.b;
    /// specifically subfigure "4KB granule 48-bit OA".
    pub fn page_descriptor(addr: u64, attr_index: u64) -> u64 {
        // The main difference between a page descriptor and block descriptor
        // is in the size of the output address (OA) and in the descriptor type.

        let upper_attributes: u64 = 0;

        let shareability = if attr_index == s1_mair_attr_index::MT_NORMAL {
            // Match what the seL4 kernel uses for its page tables, which
            // is especially necessary for SMP booting which relies on it
            // for coherency.
            shareability_attributes::INNER_SHAREABLE
        } else {
            // Per $R_{PYFVQ}$:
            // > If a region is mapped as Device memory or Normal Non-cacheable
            // > memory after all enabled translation stages, then the region
            // > has an effective Shareability attribute of Outer Shareable.
            // We override the value we place in here to OUTER_SHAREABLE to match
            // how the hardware behaves.
            shareability_attributes::OUTER_SHAREABLE
        };

        // AP[2:1], which we set as 0b00 for read/write access:
        //   stage 1: 0b00 is {PrivRead, PrivWrite} and we are EL1/El2 (priv)
        const AP_KERNEL_RW: u64 = 0b00;

        // bit[11] is the not global (nG) field, we leave as 0 (global).
        // bit[10] is the access flag; depending on FEAT_HAFDBS, when software
        //         manages the AF memory accesses to the page/block when AF=0
        //         raise an Access Fault; when hardware manages the AF it will
        //         become 1.
        // bit[9:8] is SH[1:0] containing stage 1 shareability attributes
        // bit[7:6] contains AP[2:1]
        // bit[5] is RES0
        // bit[4:2] contains AttrIndex
        let lower_attributes: u64 =
            (1 << 10) | (AP_KERNEL_RW << 6) | (shareability << 8) | (attr_index << 2);

        // bits[47:12]
        let output_address: u64 = addr & !mask(12);

        // address must not have bits above 47 set.
        assert!(addr & mask(48) == addr);

        // bits[63:50] describing the "Upper attributes" are left at 0.
        // bits[49:48] are RES0
        // bits[47:12] contain the Output address
        // bits[11:2] contain the "Lower attributes"
        // bits[1:0] contains the descriptor type
        upper_attributes | output_address | lower_attributes | descriptor_type::PAGE
    }

    /// Per "Table D8-50 Stage 1 VMSAv8-64 Table descriptor fields" and
    /// "Figure D8-12 VMSAv8-64 Table descriptor formats" of ARM DDI0487L.b;
    /// specifically subfigure "4KB, 16KB, and 64KB granules, 48-bit OA"
    pub fn table_descriptor(addr: u64) -> u64 {
        // Per Table D8-48, Condition for descriptor_type::TABLE is level != 3.

        // We don't set any of these attributes, most are hardware-feature conditional
        let attributes: u64 = 0;

        // address must not have bits above 47 or below 12 set
        assert!(addr & mask(12) == 0x0);
        assert!(addr & mask(48) == addr);

        let next_level_table_address = addr;

        // bits[63:59] are "Attributes"
        // bits[58:51] are ignored
        // bits[50:48] are RES0
        // bits[47:m] is the next-level table address
        //  note: here m=12 for 4KB granule
        // bits[m-1:12] are RES0
        //  so this doesn't exist for 4KB granule
        // bits[11:2] are ignored
        // bits[1:0] contain the descriptor type
        attributes | next_level_table_address | descriptor_type::TABLE
    }
}

mod riscv64 {
    pub(crate) const BLOCK_BITS_1GB: u64 = 30;
    pub(crate) const BLOCK_BITS_2MB: u64 = 21;
    pub(crate) const PAGE_BITS_4K: u64 = 12;

    pub(crate) const PAGE_TABLE_INDEX_BITS: u64 = 9;
    pub(crate) const PAGE_SHIFT: u64 = 12;
    /// This sets the page table entry bits: D,A,X,W,R.
    pub(crate) const PTE_TYPE_BITS: u64 = 0b11001110;
    // TODO: where does this come from?
    pub(crate) const PTE_TYPE_TABLE: u64 = 0;
    pub(crate) const PTE_TYPE_VALID: u64 = 1;

    pub(crate) const PTE_PPN0_SHIFT: u64 = 10;

    /// Due to RISC-V having various virtual memory setups, we have this generic function to
    /// figure out the page-table index given the total number of page table levels for the
    /// platform and which level we are currently looking at.
    pub fn pt_index(pt_levels: usize, addr: u64, level: usize) -> usize {
        let pt_index_bits = PAGE_TABLE_INDEX_BITS * (pt_levels - level) as u64;
        let idx = (addr >> (pt_index_bits + PAGE_SHIFT)) % 512;

        idx as usize
    }

    /// Generate physical page number given an address
    pub fn pte_ppn(addr: u64) -> u64 {
        (addr >> PAGE_SHIFT) << PTE_PPN0_SHIFT
    }

    pub fn pte_next(addr: u64) -> u64 {
        pte_ppn(addr) | PTE_TYPE_TABLE | PTE_TYPE_VALID
    }

    pub fn pte_leaf(addr: u64) -> u64 {
        pte_ppn(addr) | PTE_TYPE_BITS | PTE_TYPE_VALID
    }
}

/// RISC-V 64 page tables for our purposes uses the Sv39 translation scheme
/// (3-level page tables).
///
/// It is split into two halves: the Upper/Kernel part of the page tables,
/// which matches the format seL4 expects. The lower half contains an
/// identity mapped region for the loader.
///
/// ```txt
///            (512 GiB)
///   512 +---- Level 1 ---+ 2^39
///       |                |
///       |     (empty)    |
///       |                |
///   k+1 +----------------+                   (1 GiB)
///       | Level 2 Kernel | ----------> +---- Level 2 ---+             +-------------+
///     k +----------------+             |                | ----------> | 2 MiB block |
///       |                |         511 |----------------|             +-------------+
///       |                |             |                | ----------> | 2 MiB block |
///       |                |         510 |----------------|             +-------------+
///       |                |             |                | ----------> | 2 MiB block |
///       |                |             |----------------|             +-------------
///       |                |                   (...)           (...)         (...)          Kernel Regions
///       |                |             |----------------|             +-------------+
///       |                |             |                | ----------> | 2 MiB block |
///       |                |         l+1 |----------------|             +-------------+
///       |                |             | Level 3 Kernel | ----+
///       |                |           l |----------------|     |
///       |                |             |                |     |           (2 MiB)
///       |                |             |                |     +-----> +-- Level 3 --+             +------------+
///       |                |             |                |             |             | ----------> | 4 KiB page |
///       |                |             |                |         511 |-------------|             +------------+
///       |                |             |     (empty)    |             |             | ----------> | 4 KiB page |
///       |     (empty)    |             |                |             |-------------|             +------------+
///       |                |             |                |             |             | ----------> | 4 KiB page |
///       |                |             |                |           m |-------------|             +------------+ p
///       |                |             |                |             |   (empty)   |
///       |                |             |                |             +-------------+
///       |                |             |                |
///       |                |           0 +----------------+
///       |                |
///       |                |
///       |                |
///       |                |
///       |                |
///   s+1 +----------------+                  (1 GiB)
///       | Level 2 Loader | ---------->  +-- Level 2 --+             +-------------+
///     s +----------------+              |             | ----------> | 2 MiB block |
///       |                |          511 +-------------+             +-------------+
///       |                |              |             | ----------> | 2 MiB block |
///       |    (empty)     |          510 +-------------+             +-------------+
///       |                |              |             | ----------> | 2 MiB block |
///       |                |              |-------------|             +-------------+
///     0 +----------------+              |             | ----------> | 2 MiB block |
///                                       |-------------|             +-------------+
///                                            (...)         (...)         (...)          Loader Regions
///                                       |-------------|             +-------------+
///                                       |             | ----------> | 2 MiB block |
///                                       |-------------|             +-------------+
///                                       |             | ----------> | 2 MiB block |
///                                     t +-------------+             +-------------+
///                                       |             |
///                                       |   (empty)   |
///                                       |             |
///                                       +-------------+
///
///
/// Where:
///      k = align_down(kernel_first_vaddr, 1GiB),
///      l = align_down(kernel_first_vaddr, 2MiB),
///      m = align_down(kernel_first_vaddr, 4KiB),
///      p = align_down(kernel_first_paddr, 4KiB),
///
///      s = align_down(text_addr, 1GiB),
///      t = align_down(text_addr, 2MiB),
/// ```
///
#[unsafe(no_mangle)]
pub extern "C" fn riscv64_setup_pagetables(
    kernel_first_vaddr: u64,
    kernel_first_paddr: u64,
    page_tables_paddr_start: u64,
) -> u64 {
    use riscv64::{pt_index, pte_leaf, pte_next, BLOCK_BITS_1GB, BLOCK_BITS_2MB, PAGE_BITS_4K};

    let text_addr = &raw const _text as u64;

    // We map the loader using 2MB pages, so make sure the base is actually aligned.
    assert!(text_addr.is_multiple_of(1 << BLOCK_BITS_2MB));

    const PAGE_TABLE_ENTRIES: usize = PAGE_TABLE_SIZE / mem::size_of::<u64>();

    let mut serialise_page_table_to_paddr = {
        assert!(
            page_tables_paddr_start
                == page_tables_paddr_start.next_multiple_of(PAGE_TABLE_SIZE as u64)
        );

        // This maintains the current end of the PT array.
        let mut next_pt_paddr = page_tables_paddr_start;

        move |page_table: &mut [u64; PAGE_TABLE_ENTRIES]| -> u64 {
            let pt_paddr = next_pt_paddr;
            // page_table_bytes.extend(page_table.iter().flat_map(|pte| pte.to_le_bytes()));
            next_pt_paddr += PAGE_TABLE_SIZE as u64;
            page_table.fill(0);
            pt_paddr
        }
    };

    struct Config {
        riscv_pt_levels: usize,
    }
    let config = Config { riscv_pt_levels: 3 };

    let num_pt_levels = config.riscv_pt_levels;
    assert!(num_pt_levels == 3);

    // Manufacture the constants as per the diagram.
    let k = align_down(kernel_first_vaddr, BLOCK_BITS_1GB);
    let l = align_down(kernel_first_vaddr, BLOCK_BITS_2MB);
    let m = align_down(kernel_first_vaddr, PAGE_BITS_4K);
    let p = align_down(kernel_first_paddr, PAGE_BITS_4K);

    let s = align_down(text_addr, BLOCK_BITS_1GB);
    let t = align_down(text_addr, BLOCK_BITS_2MB);

    // Manufacture the kernel page tables
    let kernel_lvl2_pt_paddr = {
        let mut lvl2_pt_kernel = [0u64; PAGE_TABLE_ENTRIES];

        let mut paddr = p;
        let index_l = pt_index(num_pt_levels, l, 2);

        lvl2_pt_kernel[index_l] = if kernel_first_vaddr.is_multiple_of(1 << BLOCK_BITS_2MB) {
            assert!(paddr.is_multiple_of(1 << BLOCK_BITS_2MB));
            let pte = pte_leaf(paddr);
            paddr += 1 << BLOCK_BITS_2MB;
            pte
        } else {
            let mut lvl3_pt_kernel = [0u64; PAGE_TABLE_ENTRIES];

            let index_m = pt_index(num_pt_levels, m, 3);

            for index in index_m..512 {
                lvl3_pt_kernel[index] = pte_leaf(paddr);
                paddr += 1 << PAGE_BITS_4K;
            }

            let kernel_lvl3_pt_paddr = serialise_page_table_to_paddr(&mut lvl3_pt_kernel);
            pte_next(kernel_lvl3_pt_paddr)
        };

        for index in (index_l + 1)..512 {
            lvl2_pt_kernel[index] = pte_leaf(paddr);
            paddr += 1 << BLOCK_BITS_2MB;
        }

        serialise_page_table_to_paddr(&mut lvl2_pt_kernel)
    };

    // Manufacture the loader page tables, which is relatively straightforward
    let loader_lvl2_pt_paddr = {
        let mut lvl2_pt_loader = [0u64; PAGE_TABLE_ENTRIES];

        // Identity mapped, so vaddr == paddr.
        let mut paddr = t;

        for index in pt_index(num_pt_levels, t, 2)..512 {
            lvl2_pt_loader[index] = pte_leaf(paddr);
            paddr += 1 << BLOCK_BITS_2MB;
        }

        serialise_page_table_to_paddr(&mut lvl2_pt_loader)
    };

    // Manufacture the Level 1 table
    let mut boot_lvl1_pt = [0u64; PAGE_TABLE_ENTRIES];

    let index_s = pt_index(num_pt_levels, s, 1);
    let index_k = pt_index(num_pt_levels, k, 1);
    boot_lvl1_pt[index_k] = pte_next(kernel_lvl2_pt_paddr);
    boot_lvl1_pt[index_s] = pte_next(loader_lvl2_pt_paddr);

    serialise_page_table_to_paddr(&mut boot_lvl1_pt)
}

/// AArch64 loader page tables have two variations:
///  - Loader in EL2, then Stage 1 translations in use, so we have the
///    singular TTBR0_EL2 register containing the Level 0 table;
///    this allows virtual address in the range [0,2^48).
///  - Loader in EL1, then Stage 1 translations are in use, so we have both
///    the TTBR0_EL1 (covering vaddr in range [0,2^48)) and TTBR1_EL2 (
///    (covering vaddr in the range [2^64-2^48,2^64)), and containing
///    the "Level 0 Lower" page table, and "Level 0 Upper" page table
///    physical addresses respectively.
///
/// Thus, for EL2 loader, the singular Level 0 page table contains the table
/// descriptors for the "Level 1 Upper" and "Level 1 Lower" page tables.
/// For the EL1 loader, we instead have two Level 0 page tables, and
/// "Level 0 Lower" contains the "Level 1 Lower" descriptor, and "Level 0
/// Upper" contains the "Level 1 Upper" descriptor.
/// Otherwise, the page tables layout from Level 1 downwards are identical
/// (but not necessarily the layout within the page/table/block descriptors).
///
/// ```txt
///          (256 TiB)
///   512 +-- Level 0 --+ 2^48
///       |             |
///       |   (empty)   |
///       |             |
///   k+1 +-------------+                 (512 GiB)
///       | Level 1 Upr | ---------->  +-- Level 1 --+
///     k +-------------+              |             |
///       |             |              |   (empty)   |
///       |             |              |             |
///       |             |         l+1  +-------------+                 (1 GiB)
///       |             |              | Level 2 Upr | ----------> +-- Level 2 --+             +-------------+
///       |             |           l  +-------------+             |             | ----------> | 2 MiB block |
///       |             |              |             |         511 |-------------|             +-------------+
///       |             |              |   (empty)   |             |             | ----------> | 2 MiB block |
///       |             |              |             |         510 |-------------|             +-------------+
///       |             |              +-------------+             |             | ----------> | 2 MiB block |
///       |             |                                          |-------------|             +-------------+
///       |   (empty)   |                          Kernel Regions       (...)         (...)         (...)
///       |             |                                          |-------------|             +-------------+
///       |             |                                          |             | ----------> | 2 MiB block |
///       |             |                                        m |-------------|             +-------------+ p
///       |             |                                          |             |
///       |             |                                          |   (empty)   |
///       |             |                                          |             |
///       |             |                                        0 +-------------+
///       |             |
///       |             |
///       |             |
///     1 +-------------+                 (512 GiB)
///       | Level 1 Lwr | ---------->  +-- Level 1 --+
///     0 +-------------+              TODO: RAM.
///
///
/// Where:
///      k = align_down(kernel_first_vaddr, 512GiB),
///      l = align_down(kernel_first_vaddr, 1GiB),
///      m = align_down(kernel_first_vaddr, 2MiB),
///      p = align_down(kernel_first_paddr, 2MiB),
///      u = align_down(uart_base, 1GiB),
/// ```
///
#[unsafe(no_mangle)]
pub extern "C" fn aarch64_setup_pagetables(
    kernel_first_vaddr: u64,
    kernel_first_paddr: u64,
    page_tables_paddr_start: u64,
) -> (u64, u64, u64) {
    use aarch64::{
        block_descriptor, lvl0_index, lvl1_index, lvl2_index, lvl3_index, page_descriptor,
        s1_mair_attr_index::{MT_DEVICE_nGnRnE, MT_NORMAL},
        table_descriptor, BLOCK_BITS_1GB, BLOCK_BITS_2MB, BLOCK_BITS_512GB, PAGE_BITS_4KB,
    };

    let kernel_first_vaddr = 551366426624;
    let kernel_first_paddr = 1610612736;

    const PAGE_TABLE_ENTRIES: usize = PAGE_TABLE_SIZE / mem::size_of::<u64>();

    let mut serialise_page_table_to_paddr = {
        #[repr(align(4096))]
        struct PtBytes([[u8; 4096]; 100]);
        static mut PAGE_TABLE_BYTES: PtBytes = PtBytes([[0; _]; _]);
        // SAFETY: Trust me (lol)
        #[allow(static_mut_refs)]
        let mut page_table_bytes = unsafe { &mut PAGE_TABLE_BYTES.0 };

        let page_tables_paddr_start = &raw mut PAGE_TABLE_BYTES as u64;

        assert!(
            page_tables_paddr_start
                == page_tables_paddr_start.next_multiple_of(PAGE_TABLE_SIZE as u64)
        );

        // This maintains the current end of the PT array.
        let mut next_pt_paddr = page_tables_paddr_start;
        let mut i = 0;

        move |page_table: &mut [u64; PAGE_TABLE_ENTRIES]| -> u64 {
            let pt_paddr = next_pt_paddr;
            page_table
                .iter()
                .flat_map(|pte| pte.to_le_bytes())
                .zip(page_table_bytes[i].iter_mut())
                .for_each(|(byte, dest)| *dest = byte);

            next_pt_paddr += PAGE_TABLE_SIZE as u64;
            i += 0;
            page_table.fill(0);
            pt_paddr
        }
    };

    struct Region {
        start: u64,
        end: u64,
    }
    let ram_regions = [
        Region { start: 0x60000000, end: 0xc0000000 },
    ];

    const MAX_NUM_REGIONS: usize = 16;

    let mut regions = [const { core::mem::MaybeUninit::uninit() }; MAX_NUM_REGIONS];
    let identity_mapped_regions: &mut [(Region, _)] = {
        // Conceptually want we want is an 'arrayvec', but to not pull in more
        // code we implement this less-efficiently MaybeUninit.
        // We implement something very similar to the currently-unstable
        // write_iter implementation:
        // https://github.com/rust-lang/rust/blob/1.97.1/library/core/src/mem/maybe_uninit.rs#L1384-L1406
        let mut regions_len = 0;

        assert!(ram_regions.len() <= regions.len());

        let ram_regions_it = ram_regions.into_iter().map(|r| (r, MT_DEVICE_nGnRnE));

        let all_regions_it = ram_regions_it.chain([(Region { start: 0x9000000, end: 0x9001000 }, MT_DEVICE_nGnRnE)]);

        //     // FIXME: Derive from the kernel build system.
        //     if let Some(uart_base) = read_symbol_maybe(elf, "uart_addr") {
        //         let uart_base = align_down(uart_base, PAGE_BITS_4KB);
        //         regions.push((
        //             PlatformConfigRegion {
        //                 start: uart_base,
        //                 end: uart_base + (1 << PAGE_BITS_4KB),
        //             },
        //             MT_DEVICE_nGnRnE,
        //         ));
        //     }
        //     // FIXME: This is currently assuming implementation details of the BCM2711/
        //     //        Raspberry Pi 4B spin table implementation, as it is the only
        //     //        platform we have that uses spin tables. Specifically, that
        //     //        it is always located at the 0 page.
        //     if elf.find_symbol("cpus_release_addr").is_ok() {
        //         regions.push((
        //             PlatformConfigRegion {
        //                 start: 0x0,
        //                 end: 1 << PAGE_BITS_4KB,
        //             },
        //             MT_DEVICE_nGnRnE,
        //         ));
        //     }

        for (entry, region) in regions.iter_mut().zip(all_regions_it) {
            entry.write(region);
            regions_len += 1;
        }

        let regions = unsafe { (&mut regions[0..regions_len]).assume_init_mut() };

        // Need to use 'sort_unstable_by_key' as sort_by_key is not in-place.
        regions.sort_unstable_by_key(|(region, _)| region.start);

        regions
    };

    // Manufacture the constants as per the diagram.
    let k = align_down(kernel_first_vaddr, BLOCK_BITS_512GB);
    let l = align_down(kernel_first_vaddr, BLOCK_BITS_1GB);
    let m = align_down(kernel_first_vaddr, BLOCK_BITS_2MB);
    let p = align_down(kernel_first_paddr, BLOCK_BITS_2MB);

    // Manufacture the kernel page tables, which is relatively straightforward.
    let kernel_lvl1_pt_paddr = {
        // First, the Level 2 Upr table.
        let lvl2_pt_paddr = {
            let mut lvl2_pt_kernel = [0u64; PAGE_TABLE_ENTRIES];

            let mut vaddr = m;
            let mut paddr = p;
            while lvl1_index(m) == lvl1_index(vaddr) {
                lvl2_pt_kernel[lvl2_index(vaddr)] = block_descriptor(2, paddr, MT_NORMAL);

                vaddr += 1 << BLOCK_BITS_2MB;
                paddr += 1 << BLOCK_BITS_2MB;
            }

            serialise_page_table_to_paddr(&mut lvl2_pt_kernel)
        };

        // Then, the Level 1 Upr table.
        let mut lvl1_pt_kernel = [0u64; PAGE_TABLE_ENTRIES];
        lvl1_pt_kernel[lvl1_index(l)] = table_descriptor(lvl2_pt_paddr);

        serialise_page_table_to_paddr(&mut lvl1_pt_kernel)
    };

    // Manufacture the RAM page tables, which is a little bit more complicated.
    // We assume that normal RAM lies between 0 <= paddr < 512GiB, i.e.
    // that lvl0_index(any ram region addr) = 0.
    let ram_lvl1_pt_paddr = {
        // Validation of assumptions about the identity mapped regions.
        let mut previous_end = None;
        for (region, _) in identity_mapped_regions.iter() {
            assert!(lvl0_index(region.start) == 0);
            assert!(lvl0_index(region.end - 1) == 0);
            // This is probably an unnecessary assumption.
            assert!(region.start.is_multiple_of(4096));
            assert!(region.end.is_multiple_of(4096));
            // This is definitely necessary.
            assert!(region.start >= previous_end.unwrap_or(0));
            previous_end = Some(region.end);
        }

        // We maintain three active page tables, which contain our previous
        // known page table data. As we process regions in ascending order,
        // once we have exceeded the bounds of the current reservation we
        // can simply push to the page_table_bytes storage and insert into
        // the parent PT the descriptor.
        // When the current vaddr (/paddr, as identity mapped) exceeds the
        // top value we rotate to a new PT.

        struct PageTableConstructor<const LEVELS: usize, const ENTRIES: usize, PTE, Addr> {
            empty: PTE,
            levels: [[PTE; ENTRIES]; LEVELS],
            level_top: [Addr; LEVELS],
        }

        impl<const LEVELS: usize, const ENTRIES: usize, PTE: Copy + PartialEq, Addr>
            PageTableConstructor<LEVELS, ENTRIES, PTE, Addr>
        {
            const fn new(empty: PTE, level_top: [Addr; LEVELS]) -> Self {
                Self {
                    empty,
                    levels: [[empty; ENTRIES]; LEVELS],
                    level_top: level_top,
                }
            }

            fn lvl(&mut self, lvl: usize) -> &mut [PTE; ENTRIES] {
                assert!(lvl < LEVELS);
                &mut self.levels[lvl]
            }

            fn lvl_top(&mut self, lvl: usize) -> &mut Addr {
                assert!(lvl < LEVELS);
                &mut self.level_top[lvl]
            }

            fn lvl_is_empty(&self, lvl: usize) -> bool {
                assert!(lvl < LEVELS);
                self.levels[lvl] == [self.empty; ENTRIES]
            }
        }

        static mut PTS: PageTableConstructor<4, PAGE_TABLE_ENTRIES, u64, u64> =
            PageTableConstructor::new(
                0,
                [
                    u64::MAX,
                    1 << BLOCK_BITS_512GB,
                    1 << BLOCK_BITS_1GB,
                    1 << BLOCK_BITS_2MB,
                ],
            );

        // SAFETY: Trust me. This function is not, and can not, be reentrant,
        // and more than that, can only be called once.
        #[allow(static_mut_refs)]
        let pts = unsafe { &mut PTS };

        // TODO: Tests...
        // This is similar to aligned_power_of_two_regions() for the kernel UT,
        // but we restrict it such that the output always is either 1GB, 2MB, or 4KB
        // pages.

        // Allowed externally for the final iteration
        let mut base = 0u64;
        for &(ref region, attr_index) in identity_mapped_regions.iter() {
            // println!("RAM Region: {:#x}..{:#x}", base, region.end);
            // println!(
            //     "  - Current Lvl1: {:#x}..{:#x}, entries: {}",
            //     (*pts.lvl_top(1) - (1 << BLOCK_BITS_512GB)),
            //     *pts.lvl_top(1),
            //     lvl1_pt.iter().filter(|&&v| v != 0).count()
            // );
            // println!(
            //     "  - Current Lvl2: {:#x}..{:#x}, entries: {}",
            //     (*pts.lvl_top(2) - (1 << BLOCK_BITS_1GB)),
            //     *pts.lvl_top(2),
            //     lvl2_pt.iter().filter(|&&v| v != 0).count()
            // );
            // println!(
            //     "  - Current Lvl3: {:#x}..{:#x}, entries: {}",
            //     (lvl3_vaddr_top - (1 << BLOCK_BITS_2MB)),
            //     lvl3_vaddr_top,
            //     lvl3_pt.iter().filter(|&&v| v != 0).count()
            // );

            // Handle the fact that the regions are not contiguous and that
            // we might need to skip PT.

            {
                if region.start >= *pts.lvl_top(3) {
                    if !pts.lvl_is_empty(3) {
                        let lvl3_pt_paddr = serialise_page_table_to_paddr(&mut pts.lvl(3));
                        // println!("[iter] Serialise lvl3 table: {lvl3_pt_paddr:#x} for to {:#x}..{pts.lvl_top(3):#x}", (pts.lvl_top(3) - (1 << BLOCK_BITS_2MB)));
                        assert!(pts.lvl(2)[lvl2_index(base)] == 0);
                        pts.lvl(2)[lvl2_index(base)] = table_descriptor(lvl3_pt_paddr);
                    }

                    // TODO: just compute it.
                    while region.start >= *pts.lvl_top(3) {
                        *pts.lvl_top(3) += 1 << BLOCK_BITS_2MB;
                    }
                }

                if region.start >= *pts.lvl_top(2) {
                    if !pts.lvl_is_empty(2) {
                        let lvl2_pt_paddr = serialise_page_table_to_paddr(&mut pts.lvl(2));
                        // println!("[iter] Serialise lvl2 table: {lvl2_pt_paddr:#x} for to {:#x}..{*pts.lvl_top(2):#x}, base: {:#x} lvl1_index(base): {:#x}", (*pts.lvl_top(2) - (1 << BLOCK_BITS_1GB)), base, lvl1_index(base));
                        assert!(pts.lvl(1)[lvl1_index(base)] == 0);
                        pts.lvl(1)[lvl1_index(base)] = table_descriptor(lvl2_pt_paddr);
                    }

                    // TODO: just compute it.
                    while region.start >= *pts.lvl_top(2) {
                        *pts.lvl_top(2) += 1 << BLOCK_BITS_1GB;
                    }
                }

                if region.start >= *pts.lvl_top(1) {
                    unreachable!(
                        "impossible as everything should fit here: {:#x}",
                        *pts.lvl_top(1)
                    );
                }
            }

            // After serialising the old base, update the new one.
            base = region.start;

            // Inner Loop:
            // Invariant: the page tables in lvl1_pt, lvl2_pt, lvl3_pt
            //            are either (1) for the current address range,
            //            or (2) are empty and for a lower level than the current level.
            //            Also, the values in lvlXXX_vaddr_top are always correct (even if empty)
            //            Also contiguous within the loop.
            // Loop entry: (1) holds by work at the start of each region
            while base != region.end {
                // Condition is !=, but assert that we never skip it.
                assert!(base < region.end);

                let size_bits = region.end.wrapping_sub(base).ilog2();
                let align_bits = min(
                    size_bits,
                    // FIXME: Once MSRV is > 1.97, use .lowest_one() method.
                    if base == 0 {
                        size_bits
                    } else {
                        base.trailing_zeros()
                    },
                );

                // Match the size and alignment of the current region to
                // the valid PT region sizes.
                let (level, bits) = match u64::from(align_bits) {
                    BLOCK_BITS_1GB.. => (1, BLOCK_BITS_1GB),
                    BLOCK_BITS_2MB.. => (2, BLOCK_BITS_2MB),
                    PAGE_BITS_4KB.. => (3, PAGE_BITS_4KB),
                    0.. => panic!("impossible; regions should be aligned to 4K at least"),
                };

                let pt_region_size = 1u64 << bits;
                let top = base + pt_region_size;

                // println!("- Aligned PT region: {:#x}..{:#x} (size_bits: {}, align_bits: {}, bits: {})", base, top, size_bits, align_bits, bits);
                // println!(
                //     "  - Current Lvl1: {:#x}..{:#x}, entries: {}",
                //     (*pts.lvl_top(1) - (1 << BLOCK_BITS_512GB)),
                //     *pts.lvl_top(1),
                //     lvl1_pt.iter().filter(|&&v| v != 0).count()
                // );
                // println!(
                //     "  - Current Lvl2: {:#x}..{:#x}, entries: {}",
                //     (*pts.lvl_top(2) - (1 << BLOCK_BITS_1GB)),
                //     *pts.lvl_top(2),
                //     lvl2_pt.iter().filter(|&&v| v != 0).count()
                // );
                // println!(
                //     "  - Current Lvl3: {:#x}..{:#x}, entries: {}",
                //     (pts.lvl_top(3) - (1 << BLOCK_BITS_2MB)),
                //     pts.lvl_top(3),
                //     lvl3_pt.iter().filter(|&&v| v != 0).count()
                // );

                match level {
                    1 => {
                        // If it belongs in Level 1 PT, then it must go in
                        // lvl1 pt. By the inavariant, base < *pts.lvl_top(1).
                        assert!(base < *pts.lvl_top(1));
                        // top is <= *pts.lvl_top(1) (the case where it is the topmost entry)
                        assert!(top <= *pts.lvl_top(1));

                        assert!(pts.lvl(1)[lvl1_index(base)] == 0);
                        pts.lvl(1)[lvl1_index(base)] = block_descriptor(1, base, attr_index);

                        if top == *pts.lvl_top(1) {
                            // Invariant maintenance: if the new top would be now equal
                            // the end of the page table's region top, we need a new
                            // page table object and add it to the list.

                            // This should be possible to handle - we just need to break out of this loop
                            todo!("handle the case where top of lvl1 is occupied - this would be near the top of 512GiB");
                        }

                        // Invariant: Lower levels are empty.
                        assert!(pts.lvl_is_empty(2));
                        assert!(pts.lvl_is_empty(3));
                        // Invariant maintenance: vaddr_top is right range for current PT.
                        // it's empty so we need to increment the top to be current top (1G aligned) + 2MIB (512 lvl3 entries)
                        *pts.lvl_top(3) = top + (1 << BLOCK_BITS_2MB);
                        // it's empty so we need to increment the top to be current top (1G aligned) + 1G (512 lvl2 entries)
                        *pts.lvl_top(2) = top + (1 << BLOCK_BITS_1GB);
                    }
                    2 => {
                        // If it is a 2MiB block, it must go in the Level 2 PT;
                        // by our invariants: base < *pts.lvl_top(2) and top <= *pts.lvl_top(2)
                        assert!(base < *pts.lvl_top(2));
                        assert!(top <= *pts.lvl_top(2));

                        assert!(pts.lvl(2)[lvl2_index(base)] == 0);
                        pts.lvl(2)[lvl2_index(base)] = block_descriptor(2, base, attr_index);

                        if top == *pts.lvl_top(2) {
                            // Invariant maintenance: keep for current address range.
                            // As we're the top of the range, we can serialise the table.

                            let lvl2_pt_paddr = serialise_page_table_to_paddr(&mut pts.lvl(2));
                            // println!("Serialise lvl2 table: {lvl2_pt_paddr:#x} up to {*pts.lvl_top(2):#x}");
                            *pts.lvl_top(2) += 1 << BLOCK_BITS_1GB;

                            pts.lvl(1)[lvl1_index(base)] = table_descriptor(lvl2_pt_paddr);

                            if top == *pts.lvl_top(1) {
                                todo!("handle the case where top of lvl1 is occupied - this would be near the top of 512GiB");
                            }
                        }

                        // Invariant: Lower levels are empty.
                        assert!(pts.lvl_is_empty(3));
                        // Invariant maintenance: vaddr_top is right range for current PT.
                        // it's empty so we need to increment the top to be current top (2MIB aligned) + 2MIB (512 lvl3 entries)
                        *pts.lvl_top(3) = top + (1 << BLOCK_BITS_2MB);
                    }
                    3 => {
                        // If it is a 4K page, it must go in the Level 3 PT;
                        // by our invariants: base < pts.lvl_top(3) and top <= pts.lvl_top(3)
                        assert!(base < *pts.lvl_top(3));
                        assert!(top <= *pts.lvl_top(3));

                        assert!(pts.lvl(3)[lvl3_index(base)] == 0);
                        pts.lvl(3)[lvl3_index(base)] = page_descriptor(base, attr_index);

                        if top == *pts.lvl_top(3) {
                            // Invariant maintenance: keep for current address range.
                            // As we're the top of the range, we can serialise the table.

                            let lvl3_pt_paddr = serialise_page_table_to_paddr(&mut pts.lvl(3));
                            // println!("Serialise lvl3 table: {lvl3_pt_paddr:#x} for to {:#x}..{pts.lvl_top(3):#x}", (pts.lvl_top(3) - (1 << BLOCK_BITS_2MB)));
                            *pts.lvl_top(3) += 1 << BLOCK_BITS_2MB;

                            assert!(pts.lvl(2)[lvl2_index(base)] == 0);
                            pts.lvl(2)[lvl2_index(base)] = table_descriptor(lvl3_pt_paddr);

                            if top == *pts.lvl_top(2) {
                                let lvl2_pt_paddr = serialise_page_table_to_paddr(&mut pts.lvl(2));
                                // println!("Serialise lvl2 table: {lvl2_pt_paddr:#x} for to {:#x}..{*pts.lvl_top(2):#x}", (*pts.lvl_top(2) - (1 << BLOCK_BITS_1GB)));
                                *pts.lvl_top(2) += 1 << BLOCK_BITS_1GB;

                                assert!(pts.lvl(1)[lvl1_index(base)] == 0);
                                pts.lvl(1)[lvl1_index(base)] = table_descriptor(lvl2_pt_paddr);

                                if top == *pts.lvl_top(1) {
                                    todo!("handle the case where top of lvl1 is occupied - this would be near the top of 512GiB");
                                }
                            }
                        }

                        // Invariant: lower levels empty is vacuuously true
                    }
                    _ => unreachable!("level is 1..=3"),
                }

                base = base + pt_region_size;
            }
        }

        // By the loop invariant, we know that anything before has been serialised.
        // However, as we are at the end of the loop now, we might have
        // page tables that have been partially filled out, and we need to
        // serialise these.

        if !pts.lvl_is_empty(3) {
            let lvl3_pt_paddr = serialise_page_table_to_paddr(&mut pts.lvl(3));
            // println!("[end] Serialise lvl3 table: {lvl3_pt_paddr:#x}");
            assert!(pts.lvl(2)[lvl2_index(base)] == 0);
            pts.lvl(2)[lvl2_index(base)] = table_descriptor(lvl3_pt_paddr);
        }

        if !pts.lvl_is_empty(2) {
            let lvl2_pt_paddr = serialise_page_table_to_paddr(&mut pts.lvl(2));
            // println!("[end] Serialise lvl2 table: {lvl2_pt_paddr:#x} for to {:#x}..{*pts.lvl_top(2):#x}, base: {:#x} lvl1_index(base): {:#x}", (*pts.lvl_top(2) - (1 << BLOCK_BITS_1GB)), base, lvl1_index(base));
            assert!(pts.lvl(1)[lvl1_index(base)] == 0);
            pts.lvl(1)[lvl1_index(base)] = table_descriptor(lvl2_pt_paddr);
        }

        // the level1 pt should not be empty. lol.
        assert!(!pts.lvl_is_empty(1));

        // println!("New lvl1 table");
        serialise_page_table_to_paddr(pts.lvl(1))
    };

    struct Config {
        hypervisor: bool,
    }
    let config = Config { hypervisor: true };

    // Depending on whether we are in hypervisor mode, we either need to
    // return the TTBR0_EL2 or TTBR[0,1]_EL1 values. We return u64::MAX
    // so as to return garbage - an unaligned address outside of physical
    // memory.
    if config.hypervisor {
        // Manufacture the Level 0 table, containing the kernel table
        // and the RAM tables.

        let mut ttbr0_el2_pt = [0u64; PAGE_TABLE_ENTRIES];

        assert!(lvl0_index(k) != lvl0_index(0));
        ttbr0_el2_pt[lvl0_index(k)] = table_descriptor(kernel_lvl1_pt_paddr);
        ttbr0_el2_pt[lvl0_index(0)] = table_descriptor(ram_lvl1_pt_paddr);

        let ttbr0_el2 = serialise_page_table_to_paddr(&mut ttbr0_el2_pt);

        (ttbr0_el2, u64::MAX, u64::MAX)
    } else {
        let mut ttbr0_el1_pt = [0u64; PAGE_TABLE_ENTRIES];
        let mut ttbr1_el1_pt = [0u64; PAGE_TABLE_ENTRIES];

        // Kernel in TTBR1 (Upper)
        ttbr1_el1_pt[lvl0_index(k)] = table_descriptor(kernel_lvl1_pt_paddr);
        // Identity-mapped RAM in TTBR0 (Lower)
        ttbr0_el1_pt[lvl0_index(0)] = table_descriptor(ram_lvl1_pt_paddr);

        let ttbr0_el1 = serialise_page_table_to_paddr(&mut ttbr0_el1_pt);
        let ttbr1_el1 = serialise_page_table_to_paddr(&mut ttbr1_el1_pt);

        (u64::MAX, ttbr0_el1, ttbr1_el1)
    }
}
