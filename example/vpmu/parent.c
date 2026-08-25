/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>
#include <sel4/sel4.h>
#define assert(x) ((void)((x) || (__assert_fail(#x, __FILE__, __LINE__, __func__),0)))

#define VPMU0 10
#define VPMU1 35
#define NUM_CHILDREN 2

#define HANDLE_ERROR(...) do { \
        seL4_Error err = __VA_ARGS__; \
        if (err != seL4_NoError) { \
            microkit_dbg_puts("error - " #__VA_ARGS__ " with value: "); \
            microkit_dbg_put32(err); \
            microkit_dbg_puts("\n"); \
            microkit_internal_crash(err); \
        } \
    } while (0)

#define FOR_EACH_CHILD(ID_NAME, ...) do { \
        for (int ID_NAME = 0; ID_NAME < NUM_CHILDREN; ID_NAME++) { \
            __VA_ARGS__ \
        } \
    } while (0)

seL4_Word child0_id = 0;
seL4_Word child1_id = 0;
seL4_Word child0_channel_id = 0;
seL4_Word child1_channel_id = 0;

typedef struct {
    bool ready;
    uint32_t id;
    seL4_Word tcb_cap;
    seL4_Word vpmu_cap;
    seL4_Word channel;
} child_state_t;

child_state_t children[NUM_CHILDREN] = {0};

typedef enum {
    test_0_BINDING = 0,
    test_1_RECORD_ON,
    test_2_WORKLOAD,
    test_3_CHECK_CYCLE_COUNT,
    test_4_RECORD_OFF,
    test_5_WORKLOAD,
    test_6_CHECK_CYCLE_COUNT,
    test_7_RECORD_ON,
    test_8_WORKLOAD,
    test_9_CHECK_CYCLE_COUNT,
    test_10_RESET_CYCLE_COUNT,
    test_11_WORKLOAD,
    test_12_CHECK_CYCLE_COUNT,
    test_13_UNBINDING,
} test_state_e;

test_state_e test_state = test_0_BINDING;
seL4_Error main_loop(void) {
    static seL4_Word cycle_counts[NUM_CHILDREN] = {0};
    // depending on the child that finished.
    // Test that we can bind multiple VPMUs and that they work as expected.
    // Each child has a different workload, so we expect them to have different values.
    // Runs on each of the children:
    #define TEST_CASE(...) { \
            test_state++; \
            __VA_ARGS__ \
        } break;

    while (true) {
        microkit_dbg_puts("Iteration: ");
        microkit_dbg_put32(test_state);
        microkit_dbg_puts("\n");
        switch (test_state) {
            case test_0_BINDING: TEST_CASE(
                FOR_EACH_CHILD(i,
                    HANDLE_ERROR(seL4_TCB_BindVPMU(children[i].tcb_cap, children[i].vpmu_cap));
                );
            );
            case test_1_RECORD_ON: TEST_CASE(
                FOR_EACH_CHILD(i,
                    HANDLE_ERROR(seL4_ARM_VPMU_VPMUCounterControl(children[i].vpmu_cap, 1));
                );
            );
            case test_2_WORKLOAD: {
              test_state++;
              do {
                for (int i = 0; i < 2; i++) {
                  microkit_notify(children[i].channel);
                }
              } while (0);
              return seL4_NoError;
            } break;
              ;
            case test_3_CHECK_CYCLE_COUNT: TEST_CASE(
                FOR_EACH_CHILD(i,
                    seL4_ARM_VPMU_VPMUReadCycleCounter_t res = seL4_ARM_VPMU_VPMUReadCycleCounter(children[i].vpmu_cap);
                    HANDLE_ERROR(res.error);
                    cycle_counts[i] = res.cycle_counter_value;
                    microkit_dbg_puts("Num cycles[");
                    microkit_dbg_put32(i);
                    microkit_dbg_puts("]: ");
                    microkit_dbg_put32(cycle_counts[i]);
                    microkit_dbg_puts("\n");
                );

                // assert that the cycle counts are not the same
                FOR_EACH_CHILD(i,
                    FOR_EACH_CHILD(j,
                        if (i == j) continue;
                        assert(cycle_counts[i] != cycle_counts[j]);
                    );
                    // assert that they are not 0.
                    assert(cycle_counts[i] != 0);
                );
            );
            case test_4_RECORD_OFF: TEST_CASE(
                FOR_EACH_CHILD(i,
                    HANDLE_ERROR(seL4_ARM_VPMU_VPMUCounterControl(children[i].vpmu_cap, 0));
                );
            );
            case test_5_WORKLOAD: TEST_CASE(
                // notify the children to start working 
                FOR_EACH_CHILD(i,
                    microkit_notify(children[i].channel);
                );
                return seL4_NoError;
            );
            case test_6_CHECK_CYCLE_COUNT: TEST_CASE(
                // Check that the cycle counts are the same.
                FOR_EACH_CHILD(i,
                    seL4_ARM_VPMU_VPMUReadCycleCounter_t res = seL4_ARM_VPMU_VPMUReadCycleCounter(children[i].vpmu_cap);
                    HANDLE_ERROR(res.error);
                    assert(cycle_counts[i] == res.cycle_counter_value);
                );
            );
            case test_7_RECORD_ON: TEST_CASE(
                FOR_EACH_CHILD(i,
                    HANDLE_ERROR(seL4_ARM_VPMU_VPMUCounterControl(children[i].vpmu_cap, 1));
                );
            );
            case test_8_WORKLOAD: TEST_CASE(
                // notify the children to start working 
                FOR_EACH_CHILD(i,
                    microkit_notify(children[i].channel);
                );
                return seL4_NoError;
            );
            case test_9_CHECK_CYCLE_COUNT: TEST_CASE(
                // assert that they are different.
                FOR_EACH_CHILD(i,
                    seL4_ARM_VPMU_VPMUReadCycleCounter_t res = seL4_ARM_VPMU_VPMUReadCycleCounter(children[i].vpmu_cap);
                    HANDLE_ERROR(res.error);
                    assert(cycle_counts[i] != res.cycle_counter_value);
                    // don't store them, as we will use cycle_counts for checking for determinism.
                );
            );
            case test_10_RESET_CYCLE_COUNT: TEST_CASE(
                FOR_EACH_CHILD(i,
                    // Doing the reset operation should change all the cycle counter values to 0.
                    HANDLE_ERROR(seL4_ARM_VPMU_VPMUCounterControl(children[i].vpmu_cap, 2));
                    seL4_ARM_VPMU_VPMUReadCycleCounter_t res = seL4_ARM_VPMU_VPMUReadCycleCounter(children[i].vpmu_cap);
                    HANDLE_ERROR(res.error);

                    // After resetting all cycle counts should be 0 semantically.
                    assert(cycle_counts[i] != res.cycle_counter_value);
                    assert(0 == res.cycle_counter_value);
                    cycle_counts[i] = res.cycle_counter_value;
                );
            );
            case test_11_WORKLOAD: TEST_CASE(
                // notify the children to start working 
                FOR_EACH_CHILD(i,
                    microkit_notify(children[i].channel);
                );
                return seL4_NoError;
            );
            case test_12_CHECK_CYCLE_COUNT: TEST_CASE(
                // assert determinism - these new cycle counts should be the same as the original.
                FOR_EACH_CHILD(i,
                    seL4_ARM_VPMU_VPMUReadCycleCounter_t res = seL4_ARM_VPMU_VPMUReadCycleCounter(children[i].vpmu_cap);
                    HANDLE_ERROR(res.error);
                    assert(cycle_counts[i] == res.cycle_counter_value);
                );
            );
            case test_13_UNBINDING: TEST_CASE(
                // unbinding does not change any of the values.
                // also test that we can just bind and unbind.
                FOR_EACH_CHILD(i,
                    HANDLE_ERROR(seL4_TCB_UnbindVPMU(children[i].tcb_cap));
                    HANDLE_ERROR(seL4_TCB_BindVPMU(children[i].tcb_cap, children[i].vpmu_cap));
                );
            );
        }
    }
    return seL4_NoError;
}

void init(void)
{
    microkit_dbg_puts("parent init start\n");
    children[0] = (child_state_t) {
        .ready = false,
        .id = child0_id,
        .tcb_cap = BASE_TCB_CAP + child0_id,
        .vpmu_cap = BASE_VPMU_CAPS + VPMU0,
        .channel = child0_channel_id,
    };
    children[1] = (child_state_t) {
        .ready = false,
        .id = child1_id,
        .tcb_cap = BASE_TCB_CAP + child1_id,
        .vpmu_cap = BASE_VPMU_CAPS + VPMU1,
        .channel = child1_channel_id,
    };
    microkit_dbg_puts("parent init finished\n");
}

void notified(microkit_channel ch)
{
    if (ch > 1)
    {
        microkit_dbg_puts("ch > 1, ignoring\n");
        return;
    }
    children[ch].ready = true;

    for (int i = 0; i < NUM_CHILDREN; i++) {
        if (children[i].ready == false)
        {
            microkit_dbg_puts("not ready\n");
            return;
        }
    }
    microkit_dbg_puts("running mainloop\n");
    main_loop();
    // set them to all be not ready - and then we wait for notifications again.
    FOR_EACH_CHILD(i,
        children[i].ready = false;
    );
}

seL4_Bool fault(microkit_child child, microkit_msginfo msginfo,
                microkit_msginfo *reply_msginfo) {
    microkit_dbg_puts("Faulting child: ");
    microkit_dbg_put32(child);
    microkit_dbg_puts("\n");
    *reply_msginfo = microkit_msginfo_new(0, 0);

    return false;
}

