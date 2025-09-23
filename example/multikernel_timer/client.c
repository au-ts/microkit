#include <microkit.h>
#include <stdint.h>

#define TIMER_CH 0

static char hexchar(unsigned int v)
{
    return v < 10 ? '0' + v : ('a' - 10) + v;
}

static void puthex64(uint64_t val)
{
    char buffer[16 + 3];
    buffer[0] = '0';
    buffer[1] = 'x';
    buffer[16 + 3 - 1] = 0;
    for (unsigned i = 16 + 1; i > 1; i--) {
        buffer[i] = hexchar(val & 0xf);
        val >>= 4;
    }
    microkit_dbg_puts(buffer);
}

uintptr_t symbol_shared_buffer;
volatile uint64_t *shared;


void init() {
    shared = (void *)symbol_shared_buffer;
}

void notified(microkit_channel ch)
{
    switch (ch) {
    case TIMER_CH:
        microkit_dbg_puts("CLIENT: Got timer notification\n");
        microkit_dbg_puts("CLIENT: Current time is: ");
        puthex64(*shared);
        microkit_dbg_puts("\n");
        break;
    default:
        microkit_dbg_puts("CLIENT|ERROR: unexpected channel!\n");
    }
}
