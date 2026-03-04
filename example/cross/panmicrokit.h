#ifndef panmicrokit_h_INCLUDED
#define panmicrokit_h_INCLUDED

#define __thread
#include <sel4/sel4.h>

#define microkit_channel unsigned int
#define microkit_child unsigned int
#define microkit_ioport unsigned int

#define microkit_msginfo seL4_MessageInfo_t
#define microkit_msginfo_get_label seL4_MessageInfo_get_label
#define microkit_msginfo_get_count seL4_MessageInfo_get_length

// messsage registers (TODO synonyms won't do if we need different pre/postconds)
#define microkit_mr_set seL4_SetMR
#define microkit_mr_get seL4_GetMR

#define MONITOR_EP 5
#define TCB_CAP 6
#define BASE_OUTPUT_NOTIFICATION_CAP 10
#define BASE_ENDPOINT_CAP 74
#define BASE_IRQ_CAP 138
#define BASE_TCB_CAP 202

extern void microkit_notify(microkit_channel ch);
extern void microkit_irq_ack(microkit_channel ch);
extern seL4_Error microkit_pd_stop(microkit_child pd);
extern seL4_Error microkit_pd_restart(microkit_child pd, seL4_Word entry_point);
extern microkit_msginfo microkit_ppcall(microkit_channel ch, microkit_msginfo msginfo);
extern microkit_msginfo microkit_msginfo_new(seL4_Word label, seL4_Uint16 count);
extern void microkit_deferred_notify(microkit_channel ch);
extern void microkit_deferred_irq_ack(microkit_channel ch);

extern void microkit_dbg_puts(const char *s);
extern void microkit_dbg_put32(seL4_Uint32 x);

#endif // panmicrokit_h_INCLUDED
