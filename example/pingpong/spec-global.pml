/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
*/

/* We show that global properties of Microkit-based systems can be specified
 * using linear temporal logic. This SPIN/Promela file simulates a Microkit
 * system running the Ping and Pong protection domains, which keep notifying
 * each other.
 *
 * The PDs are modeled as state machines, which perform transitions whenever
 * a Microkit event (notify, receive, read/write from shared memory regions,
 * etc.) happens. The global correctness condition is stated as an LTL
 * formula: ping and pong don't have the ball at the same time, and neither
 * the number of pings nor the number of pongs ever races more than one ahead
 * of the other.
 *
 * The local state machine transition guarantees are proved for the actual PD
 * code using `pancake2viper`.
 */


/*****************************************************************************/
// data types

// represents the currently running pd
#define PD_OTHER 0
#define PD_PING 1
#define PD_PONG 2

// represents the current Microkit event
#define EVENT_NONE 0
#define EVENT_NOTIFY 1
#define EVENT_RECEIVE 2

// represents a notification badge (simplified)
#define BADGE_EMPTY 64
#define COUNTER_MAX 15


/*****************************************************************************/
// state machines

// ping PD internal states
bool pd_ping_has_the_ball = true;
byte pd_ping_ping_counter = 0;

// pong PD internal state:
bool pd_pong_has_the_ball = false;
byte pd_pong_pong_counter = 0;

// Microkit state (badges and current action):
byte system_badge_ping = BADGE_EMPTY;
byte system_badge_pong = BADGE_EMPTY;

byte system_running = PD_OTHER;
byte system_action = EVENT_NONE;
byte system_channel = BADGE_EMPTY;


/*****************************************************************************/
// havoc helpers

// next state variables used for havoc:
bool next_pd_ping_has_the_ball;
byte next_pd_ping_ping_counter;

bool next_pd_pong_has_the_ball;
byte next_pd_pong_pong_counter;

byte next_system_badge_ping;
byte next_system_badge_pong;

inline havoc_bool(x)
{
    if
    :: x = false
    :: x = true
    fi
}

inline havoc_pd(x)
{
    if
    :: x = PD_PING
    :: x = PD_PONG
    :: x = PD_OTHER
    fi
}

inline havoc_action(x)
{
    if
    :: x = EVENT_NOTIFY
    :: x = EVENT_RECEIVE
    :: x = EVENT_NONE
    fi
}

inline havoc_channel(x)
{
    // reduce state space, channel can be 2,5,empty or just one random value
    if
    :: x = 2
    :: x = 5
    :: x = 10
    :: x = BADGE_EMPTY
    fi
}

inline havoc_counter(x)
{
    select(x : 0 .. COUNTER_MAX)
}


/*****************************************************************************/
// event predicates

#define system_event_ping_notify \
    (system_running == PD_PING && system_action == EVENT_NOTIFY)

#define system_event_ping_receive \
    (system_running == PD_PING && system_action == EVENT_RECEIVE)

#define system_event_pong_notify \
    (system_running == PD_PONG && system_action == EVENT_NOTIFY)

#define system_event_pong_receive \
    (system_running == PD_PONG && system_action == EVENT_RECEIVE)


/*****************************************************************************/
// system level predicates (Microkit model)

#define system_rely_init \
    (system_badge_ping == BADGE_EMPTY && \
     system_badge_pong == BADGE_EMPTY && \
     system_running == PD_OTHER && \
     system_action == EVENT_NONE && \
     system_channel == BADGE_EMPTY)

#define system_condition_notify \
    (true)

#define system_condition_receive \
    ((system_running == PD_PING && system_badge_ping != BADGE_EMPTY) || \
     (system_running == PD_PONG && system_badge_pong != BADGE_EMPTY))

#define system_action_is_well_formed \
    ((system_running == PD_PING || system_running == PD_PONG) && \
     (system_action == EVENT_NOTIFY || \
      system_action == EVENT_RECEIVE) && \
     ((system_action == EVENT_NOTIFY && system_channel < BADGE_EMPTY) || \
      (system_action != EVENT_NOTIFY && system_channel == BADGE_EMPTY)))

#define system_action_conditions \
    (system_action_is_well_formed && \
     (system_action != EVENT_NOTIFY || system_condition_notify) && \
     (system_action != EVENT_RECEIVE || system_condition_receive))

/*****************************************************************************/
// ping guarantees and transitions

#define ping_rely_init \
    (pd_ping_has_the_ball == true && \
     pd_ping_ping_counter == 0)

#define ping_guarantee_notify \
    (pd_ping_has_the_ball)

#define ping_transition_notify \
    (((system_channel == 5) && \
        pd_ping_ping_counter < COUNTER_MAX && \
        next_pd_ping_has_the_ball == false && \
        next_pd_ping_ping_counter == pd_ping_ping_counter + 1) || \
     ((system_channel != 5) && \
        next_pd_ping_has_the_ball == pd_ping_has_the_ball && \
        next_pd_ping_ping_counter == pd_ping_ping_counter))


#define ping_guarantee_receive \
    (!pd_ping_has_the_ball)

#define ping_transition_receive \
    (next_pd_ping_has_the_ball == true && \
     next_pd_ping_ping_counter == pd_ping_ping_counter)

#define ping_transition_not_running \
    (next_pd_ping_has_the_ball == pd_ping_has_the_ball && \
     next_pd_ping_ping_counter == pd_ping_ping_counter)

#define ping_transition_relation \
    ((system_event_ping_notify && \
        ping_guarantee_notify && \
        ping_transition_notify) || \
     (system_event_ping_receive && \
        ping_guarantee_receive && \
        ping_transition_receive) || \
     (system_running != PD_PING && \
        ping_transition_not_running))


/*****************************************************************************/
// pong guarantees and transitions

#define pong_rely_init \
    (pd_pong_has_the_ball == false && \
     pd_pong_pong_counter == 0)

#define pong_guarantee_notify \
    (pd_pong_has_the_ball)

#define pong_transition_notify \
    (((system_channel == 2) && \
        pd_pong_pong_counter < COUNTER_MAX && \
        next_pd_pong_has_the_ball == false && \
        next_pd_pong_pong_counter == pd_pong_pong_counter + 1) || \
     ((system_channel != 2) && \
        next_pd_pong_has_the_ball == pd_pong_has_the_ball && \
        next_pd_pong_pong_counter == pd_pong_pong_counter))

#define pong_guarantee_receive \
    (!pd_pong_has_the_ball)

#define pong_transition_receive \
    (next_pd_pong_has_the_ball == true && \
     next_pd_pong_pong_counter == pd_pong_pong_counter)

#define pong_transition_not_running \
    (next_pd_pong_has_the_ball == pd_pong_has_the_ball && \
     next_pd_pong_pong_counter == pd_pong_pong_counter)


#define pong_transition_relation \
    ((system_event_pong_notify && \
        pong_guarantee_notify && \
        pong_transition_notify) || \
     (system_event_pong_receive && \
        pong_guarantee_receive && \
        pong_transition_receive) || \
     (system_running != PD_PONG && \
        pong_transition_not_running))

/*****************************************************************************/
// system-level transitions

#define cross_transition_badge_ping \
    ((system_event_pong_notify && \
        system_channel == 2 && \
        next_system_badge_ping == 5) || \
     (system_event_ping_receive && \
        next_system_badge_ping == BADGE_EMPTY) || \
     (!(system_event_pong_notify && system_channel == 2) && \
      !system_event_ping_receive && \
        next_system_badge_ping == system_badge_ping))

#define cross_transition_badge_pong \
    ((system_event_ping_notify && \
        system_channel == 5 && \
        next_system_badge_pong == 2) || \
     (system_event_pong_receive && \
        next_system_badge_pong == BADGE_EMPTY) || \
     (!(system_event_ping_notify && system_channel == 5) && \
      !system_event_pong_receive && \
        next_system_badge_pong == system_badge_pong))

#define cross_transition_relation \
    (cross_transition_badge_ping && \
     cross_transition_badge_pong)

#define init_relation \
    (system_rely_init && ping_rely_init && pong_rely_init)

#define transition_relation \
    (system_action_conditions && \
     ping_transition_relation && \
     pong_transition_relation && \
     cross_transition_relation)


/*****************************************************************************/
// main system loop

init
{
    assert(init_relation);

    do
    :: atomic {
        // havoc according to transition relation
        havoc_pd(system_running);
        havoc_action(system_action);
        havoc_channel(system_channel);

        havoc_bool(next_pd_ping_has_the_ball);
        havoc_counter(next_pd_ping_ping_counter);

        havoc_bool(next_pd_pong_has_the_ball);
        havoc_counter(next_pd_pong_pong_counter);

        havoc_channel(next_system_badge_ping);
        havoc_channel(next_system_badge_pong);

        transition_relation;

        // commit changes for next turn
        pd_ping_has_the_ball = next_pd_ping_has_the_ball;
        pd_ping_ping_counter = next_pd_ping_ping_counter;
        pd_pong_has_the_ball = next_pd_pong_has_the_ball;
        pd_pong_pong_counter = next_pd_pong_pong_counter;
        system_badge_ping = next_system_badge_ping;
        system_badge_pong = next_system_badge_pong

        // reset next states (reduces search space)
        next_pd_ping_has_the_ball = false;
        next_pd_ping_ping_counter = 0;
        next_pd_pong_has_the_ball = false;
        next_pd_pong_pong_counter = 0;
        next_system_badge_ping = BADGE_EMPTY;
        next_system_badge_pong = BADGE_EMPTY;
    }
    od
}


/*****************************************************************************/
// global correctness conditions

#define property_never_both_have_ball \
    (!(pd_ping_has_the_ball && pd_pong_has_the_ball))

ltl never_both_have_ball {
    (
        ([] <> (system_running == PD_PING)) &&
        ([] <> (system_running == PD_PONG))
    )
    ->
    ([] property_never_both_have_ball)
}

#define property_counters_never_more_than_1_apart \
    ((pd_ping_ping_counter <= pd_pong_pong_counter + 1) && \
     (pd_pong_pong_counter <= pd_ping_ping_counter + 1))

ltl counters_never_more_than_1_apart {
    (
        ([] <> (system_running == PD_PING)) &&
        ([] <> (system_running == PD_PONG))
    )
    ->
    ([] property_counters_never_more_than_1_apart)
}
