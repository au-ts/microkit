#include "print.h"

#include <microkit.h>

uintptr_t uart_base;

#define UART_REG(x) ((volatile uint32_t *)(uart_base + (x)))

#if defined(CONFIG_PLAT_ODROIDC4)
#define UART_WFIFO 0x0
#define UART_STATUS 0xC
#define UART_TX_FULL (1 << 21)

void putc(uint8_t ch) {
    while ((*UART_REG(UART_STATUS) & UART_TX_FULL));
    *UART_REG(UART_WFIFO) = ch;
}


#else
#error "unsupported board"
#endif

void puts(const char *s) {
    while (*s) {
        if (*s == '\n') {
            putc('\r');
        }
        putc(*s);
        s++;
    }
}

void print(const char *s) {
    puts(microkit_name);
    puts(": ");
    puts(s);
}

char hexchar(unsigned int v) {
    return v < 10 ? '0' + v : ('a' - 10) + v;
}

void puthex32(uint32_t val) {
    char buffer[8 + 3];
    buffer[0] = '0';
    buffer[1] = 'x';
    buffer[8 + 3 - 1] = 0;
    for (unsigned i = 8 + 1; i > 1; i--) {
        buffer[i] = hexchar(val & 0xf);
        val >>= 4;
    }
    puts(buffer);
}

void puthex64(uint64_t val) {
    char buffer[16 + 3];
    buffer[0] = '0';
    buffer[1] = 'x';
    buffer[16 + 3 - 1] = 0;
    for (unsigned i = 16 + 1; i > 1; i--) {
        buffer[i] = hexchar(val & 0xf);
        val >>= 4;
    }
    puts(buffer);
}
