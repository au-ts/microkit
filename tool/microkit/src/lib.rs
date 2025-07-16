//
// Copyright 2024, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

pub mod capdl;
pub mod elf;
pub mod loader;
pub mod sdf;
pub mod sel4;
pub mod util;

// Note that these values are used in the monitor so should also be changed there
// if any of these were to change.
pub const MAX_PDS: usize = 63;
pub const MAX_VMS: usize = 63;
// It should be noted that if you were to change the value of
// the maximum PD/VM name length, you would also have to change
// the monitor and libmicrokit.
pub const PD_MAX_NAME_LENGTH: usize = 64;
pub const VM_MAX_NAME_LENGTH: usize = 64;
