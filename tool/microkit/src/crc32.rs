//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

pub fn crc32(bytes: &[u8]) -> u32 {
    // Could be more optimised using the table approach.
    // Source: https://web.archive.org/web/20190108202303/http://www.hackersdelight.org/hdcodetxt/crc.c.txt
    let mut crc: u32 = 0xffff_ffff;
    let mut mask: u32 = 0;

    for byte in bytes.iter() {
        crc = crc ^ byte;
        for i in 0..8 {
            mask = -(crc & 1);
            crc = (crc >> 1) ^ (0xEDB88320 & mask);
        }
    }

    !crc
}