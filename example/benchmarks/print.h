#pragma once

#include <stdint.h>

void putc(uint8_t ch);
void puts(const char *s);
void print(const char *s);
char hexchar(unsigned int v);
void puthex32(uint32_t val);
void puthex64(uint64_t val);
