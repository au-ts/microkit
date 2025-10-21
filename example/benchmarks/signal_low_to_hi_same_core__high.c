/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

#include "benchmark.h"

uintptr_t shared;

void init(void)
{
    print("hello world\n");
}

DECLARE_SUBVERTED_MICROKIT()
