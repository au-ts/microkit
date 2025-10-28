/*
 * Copyright 2022, UNSW
 * SPDX-License-Identifier: BSD-2-Clause
 */


#include <stdint.h>
#include <microkit.h>

#define TIMER_IRQ_CH 0
#define SEND_CH 1

#define BIT(n) (1ULL << n)

#define GPTx_CR_SWR BIT(15)
#define GPTx_CR_FRR BIT(9)
#define GPTx_CR_CLKSRC_PERIPHERAL (0b001 << 6)
#define GPTx_CR_ENMOD BIT(1)
#define GPTx_CR_EN BIT(0)

#define GPTx_SR_ROV BIT(5)
#define GPTx_SR_IF2 BIT(4)
#define GPTx_SR_IF1 BIT(3)
#define GPTx_SR_OF3 BIT(2)
#define GPTx_SR_OF2 BIT(1)
#define GPTx_SR_OF1 BIT(0)

#define GPTx_IR_OF1IE BIT(0)

uintptr_t timer_regs_1;
uintptr_t timer_regs_2;

typedef struct {
    /* Control Register */
    uint32_t CR;
    /* Prescaler Register */
    uint32_t PR;
    /* Status Register */
    uint32_t SR;
    /* Interrupt Register */
    uint32_t IR;
    /* Output Compare Register 1 */
    uint32_t OCR1;
    /* Output Compare Register 2 */
    uint32_t OCR2;
    /* Output Compare Register 3 */
    uint32_t OCR3;
    /* Input Compare Register 1 */
    uint32_t ICR1;
    /* Input Compare Register 2 */
    uint32_t ICR2;
    /* Counter Register */
    uint32_t CNT;
} imx_timer_reg_t;

typedef struct {
    volatile imx_timer_reg_t *GPT1;
    volatile imx_timer_reg_t *GPT2;
} imx_timer_t;

imx_timer_t timer;

uintptr_t symbol_shared_buffer;
volatile uint64_t *shared;

uint32_t imx_get_time()
{
    return timer.GPT2->CNT;
}

void init()
{
    shared = (void *)symbol_shared_buffer;

    timer.GPT1 = (void *)(timer_regs_1);
    timer.GPT2 = (void *)(timer_regs_2);

    /* ref 12.1 of the imx8 spec for initialisation/register info */

    {
        /* restart mode means we can't easily use GPT1 for time, use a GPT2 for time */

        /* disable */
        timer.GPT2->CR = 0;
        /* clear status registers */
        timer.GPT2->SR = GPTx_SR_ROV | GPTx_SR_IF2 | GPTx_SR_IF1 | GPTx_SR_OF3 | GPTx_SR_OF2 | GPTx_SR_OF1;
        /* all interrupts disable */
        timer.GPT2->IR = 0b00000;

        /* prescalar divides by PR + 1. we divide by 24 (the peripheral clock freq in MHz) so use 23 */
        timer.GPT2->PR = 23;

        /* software reset */
        timer.GPT2->CR = GPTx_CR_SWR;
        /* wait for reset to finish, self-clearing to 0 */
        while (timer.GPT2->CR & GPTx_CR_SWR);

        /* restart mode means we can't easily use this timer, use a second one for time */
        timer.GPT2->CR = GPTx_CR_EN                /* enable */
                            | GPTx_CR_ENMOD             /* reset counter to 0 */
                            | GPTx_CR_CLKSRC_PERIPHERAL /* use peripheral clock */
                            | GPTx_CR_FRR               /* use free run mode  */
                            ;
    }

    {
        /* disable */
        timer.GPT1->CR = 0;
        /* clear status registers */
        timer.GPT1->SR = GPTx_SR_ROV | GPTx_SR_IF2 | GPTx_SR_IF1 | GPTx_SR_OF3 | GPTx_SR_OF2 | GPTx_SR_OF1;
        /* all interrupts disable */
        timer.GPT1->IR = 0b00000;

        /* software reset */
        timer.GPT1->CR = GPTx_CR_SWR;
        /* wait for reset to finish, self-clearing to 0 */
        while (timer.GPT1->CR & GPTx_CR_SWR);

        /* enable output compare channel 1 interrupt only */
        timer.GPT1->IR = GPTx_IR_OF1IE;

        /* prescalar divides by PR + 1. we divide by 24 (the peripheral clock freq in MHz) so use 23 */
        timer.GPT1->PR = 23;

        // Have a timeout of 1 second, and have it be periodic so that it will keep recurring.
        microkit_dbg_puts("Setting a timeout of 1 second.\n");
        /* always periodic - in us */
        timer.GPT1->OCR1 = 1 * 1000 * 1000;

        /* restart mode means we can't easily use this timer, use a second one for time */
        timer.GPT1->CR = GPTx_CR_EN                /* enable */
                            | GPTx_CR_ENMOD             /* reset counter to 0 */
                            | GPTx_CR_CLKSRC_PERIPHERAL /* use peripheral clock */
                            | (0 & GPTx_CR_FRR)         /* use restart mode (FRR = 0) */
                            ;
    }
}


void notified(microkit_channel ch)
{
    switch (ch) {
    case TIMER_IRQ_CH:
        microkit_dbg_puts("TIMER: Got timer interrupt!\n");

        uint32_t sr = timer.GPT1->SR;
        if (sr & ~GPTx_SR_OF1) {
            microkit_dbg_puts("TIMER: got unknown status bits, disabling: ");
            microkit_dbg_put32(sr);
            microkit_dbg_puts("\n");
            timer.GPT1->CR = 0;
            return;
        }

        /* clear status register, w1c */
        timer.GPT1->SR = GPTx_SR_OF1;

        microkit_irq_ack(ch);

        *shared = imx_get_time();
        microkit_notify(SEND_CH);

        break;
    default:
        microkit_dbg_puts("TIMER|ERROR: unexpected channel!\n");
    }
}
