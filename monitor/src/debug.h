/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#pragma once

#include <sel4/sel4.h>

void dump_bootinfo(seL4_BootInfo *bi);
void dump_untyped_info(const seL4_Word untypeds_len, const seL4_UntypedDesc untypedsList[untypeds_len]);
