#include <microkit.h>

#define TIMER_CH 0

void init() {

}

void notified(microkit_channel ch)
{
    switch (ch) {
    case TIMER_CH:
        microkit_dbg_puts("CLIENT|INFO: Got timer notification\n");
        break;
    default:
        microkit_dbg_puts("CLIENT|ERROR: unexpected channel!\n");
    }
}
