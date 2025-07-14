//
// Copyright 2024, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use std::cmp::min;

// use crate::sel4::Object;
use serde_json;

use crate::{elf::ElfFile, sdf::{Channel, ProtectionDomain}, PD_MAX_NAME_LENGTH};

pub fn msb(x: u64) -> u64 {
    64 - x.leading_zeros() as u64 - 1
}

pub fn lsb(x: u64) -> u64 {
    x.trailing_zeros() as u64
}

pub fn str_to_bool(s: &str) -> Option<bool> {
    match s {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub const fn kb(n: u64) -> u64 {
    n * 1024
}

pub const fn mb(n: u64) -> u64 {
    n * 1024 * 1024
}

pub const fn divmod(x: u64, y: u64) -> (u64, u64) {
    (x / y, x % y)
}

pub const fn round_up(n: u64, x: u64) -> u64 {
    let (_, m) = divmod(n, x);
    if m == 0 {
        n
    } else {
        n + x - m
    }
}

pub const fn round_down(n: u64, x: u64) -> u64 {
    let (_, m) = divmod(n, x);
    if m == 0 {
        n
    } else {
        n - m
    }
}

pub fn is_power_of_two(n: u64) -> bool {
    assert!(n > 0);
    n & (n - 1) == 0
}

/// Mask out (set to zero) the lower bits from n
pub fn mask_bits(n: u64, bits: u64) -> u64 {
    (n >> bits) << bits
}

pub fn mask(n: u64) -> u64 {
    (1 << n) - 1
}

/// Check that all objects in the list are adjacent
// pub fn objects_adjacent(objects: &[Object]) -> bool {
//     let mut prev_cap_addr = objects[0].cap_addr;
//     for obj in &objects[1..] {
//         if obj.cap_addr != prev_cap_addr + 1 {
//             return false;
//         }
//         prev_cap_addr = obj.cap_addr;
//     }

//     true
// }

/// Product a 'human readable' string for the size.
///
/// 'strict' means that it must be simply represented.
///  Specifically, it must be a multiple of standard power-of-two.
///  (e.g. KiB, MiB, GiB, TiB, PiB, EiB)
pub fn human_size_strict(size: u64) -> (String, &'static str) {
    for (bits, label) in [
        (60, "EiB"),
        (50, "PiB"),
        (40, "TiB"),
        (30, "GiB"),
        (20, "MiB"),
        (10, "KiB"),
        (0, "bytes"),
    ] {
        let base = 1 << bits;
        if size > base {
            let count;
            if base > 0 {
                let (d_count, extra) = divmod(size, base);
                count = d_count;
                if extra != 0 {
                    return (format!("{:.2}", size as f64 / base as f64), label);
                }
            } else {
                count = size;
            }
            return (comma_sep_u64(count), label);
        }
    }

    panic!("should never reach here in human_size_strict");
}

/// Take an integer, such as 1000000 and add commas such as:
/// 1,000,000.
pub fn comma_sep_u64(n: u64) -> String {
    let mut s = String::new();
    for (i, val) in n.to_string().chars().rev().enumerate() {
        if i != 0 && i % 3 == 0 {
            s.insert(0, ',');
        }
        s.insert(0, val);
    }

    s
}

pub fn comma_sep_usize(n: usize) -> String {
    comma_sep_u64(n as u64)
}

pub fn json_str<'a>(json: &'a serde_json::Value, field: &'static str) -> Result<&'a str, String> {
    match json.get(field) {
        Some(value) => Ok(value
            .as_str()
            .unwrap_or_else(|| panic!("JSON field '{}' is not a string", field))),
        None => Err(format!("JSON field '{}' does not exist", field)),
    }
}

pub fn json_str_as_u64(json: &serde_json::Value, field: &'static str) -> Result<u64, String> {
    match json.get(field) {
        Some(value) => Ok(value
            .as_str()
            .unwrap_or_else(|| panic!("JSON field '{}' is not a string", field))
            .parse::<u64>()
            .unwrap_or_else(|_| panic!("JSON field '{}' could not be converted to u64", field))),
        None => Err(format!("JSON field '{}' does not exist", field)),
    }
}

pub fn json_str_as_bool(json: &serde_json::Value, field: &'static str) -> Result<bool, String> {
    match json.get(field) {
        Some(value) => Ok(value
            .as_bool()
            .unwrap_or_else(|| panic!("JSON field '{}' could not be converted to bool", field))),
        None => Err(format!("JSON field '{}' does not exist", field)),
    }
}

/// Convert a struct into raw bytes in order to be written to
/// disk or some other format.
#[allow(clippy::missing_safety_doc)]
pub unsafe fn struct_to_bytes<T: Sized>(p: &T) -> &[u8] {
    ::core::slice::from_raw_parts((p as *const T) as *const u8, ::core::mem::size_of::<T>())
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn bytes_to_struct<T>(bytes: &[u8]) -> &T {
    let (prefix, body, suffix) = unsafe { bytes.align_to::<T>() };
    assert!(prefix.is_empty());
    assert!(body.len() == 1);
    assert!(suffix.is_empty());

    &body[0]
}

/// Serialise an array of u64 to a Vector of bytes. Pads the Vector of bytes
/// such that the first entry is empty.
pub fn monitor_serialise_u64_vec(vec: &[u64]) -> Vec<u8> {
    let mut bytes = vec![0; (1 + vec.len()) * 8];
    for (i, value) in vec.iter().enumerate() {
        let start = (i + 1) * 8;
        let end = start + 8;
        bytes[start..end].copy_from_slice(&value.to_le_bytes());
    }

    bytes
}

/// For serialising an array of PD or VM names. Pads the Vector of bytes such that
/// the first entry is empty.
pub fn monitor_serialise_names(
    names: Vec<&String>,
    max_len: usize,
    max_name_len: usize,
) -> Vec<u8> {
    let mut names_bytes = vec![0; (max_len + 1) * max_name_len];
    for (i, name) in names.iter().enumerate() {
        // The monitor will index into the array of names based on the badge, which
        // starts at 1 and hence we cannot use the 0th entry in the array.
        let name_bytes = name.as_bytes();
        let start = (i + 1) * max_name_len;
        // Here instead of giving an error we simply take the minimum of the name
        // and how large of a name we can encode. The name length is one less than
        // the maximum since we still have to add the null terminator.
        let name_length = std::cmp::min(name_bytes.len(), max_name_len - 1);
        let end = start + name_length;
        names_bytes[start..end].copy_from_slice(&name_bytes[..name_length]);
        // These bytes will be interpreted as a C string, so we must include
        // a null-terminator.
        names_bytes[end] = 0;
    }

    names_bytes
}

pub fn pd_write_symbols(
    pds: &[ProtectionDomain],
    channels: &[Channel],
    pd_elf_files: &mut [ElfFile],
    pd_setvar_values: &[Vec<u64>],
) -> Result<(), String> {
    for (i, pd) in pds.iter().enumerate() {
        let elf = &mut pd_elf_files[i];
        let name = pd.name.as_bytes();
        let name_length = min(name.len(), PD_MAX_NAME_LENGTH);
        elf.write_symbol("microkit_name", &name[..name_length])?;
        elf.write_symbol("microkit_passive", &[pd.passive as u8])?;

        let mut notification_bits: u64 = 0;
        let mut pp_bits: u64 = 0;
        for channel in channels {
            if channel.end_a.pd == i {
                if channel.end_a.notify {
                    notification_bits |= 1 << channel.end_a.id;
                }
                if channel.end_a.pp {
                    pp_bits |= 1 << channel.end_a.id;
                }
            }
            if channel.end_b.pd == i {
                if channel.end_b.notify {
                    notification_bits |= 1 << channel.end_b.id;
                }
                if channel.end_b.pp {
                    pp_bits |= 1 << channel.end_b.id;
                }
            }
        }

        elf.write_symbol("microkit_irqs", &pd.irq_bits().to_le_bytes())?;
        elf.write_symbol("microkit_notifications", &notification_bits.to_le_bytes())?;
        elf.write_symbol("microkit_pps", &pp_bits.to_le_bytes())?;

        for (setvar_idx, setvar) in pd.setvars.iter().enumerate() {
            let value = pd_setvar_values[i][setvar_idx];
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

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_msb() {
        assert_eq!(msb(37), 5);
    }

    #[test]
    fn test_lsb() {
        assert_eq!(lsb(36), 2);
        assert_eq!(lsb(37), 0);
    }
}
