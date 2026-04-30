<!--
     Copyright 2026, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->

This example constitutes an early experiment in verification of Microkit-based
systems by combining automated local proofs about protection domains with
planned hand-written machine-checked global correctness proofs about the whole
system.

The first section explains the general verification approach, and the second
section instantiates the approach on the toy `pingpong` system.

# Verification of Microkit-based systems

The long-term goal is to prove system-level properties of a system running on
top of Microkit. A system-level property talks about the stream of Microkit
events produced by the system. Examples of Microkit events include:
notifications, protected procedure calls, receives, IRQ-related events, and
accesses to shared memory regions.

A system-level property might say, for example, that no PD ever emits two
notifications on  the same channel in a row, or that two PDs never both hold
the same resource at the same time, or that a word from one part of shared
memory gets correctly copied to a different part of the shared memory.

Ideally, proofs of system-level properties should reason only about abstract
protection domain states, shared memory, and the stream of Microkit events,
instead of about each protection domain's specific implementation.

System-level properties should connect to the implementation using
*PD-level properties*. The idea is to make each protection domain prove a local
event contract: when it may emit a Microkit event, how that event changes its
abstract state, and what assumptions it needs from the rest of the system.


## PD-level verification

Each verified Pancake protection domain gets a local specification.
The local specification defines:

* the abstract state machine that represents the PD at the system level;
* event guards (guarantees),
* event transition relations,
* local state preconditions and postconditions for Microkit calls,
* reliances, which are assumptions about the PD's environment, and
* loop invariants needed by the Microkit handler loop.

In this project, a *guarantee* means an event guard: the PD promises not to
emit a certain type of Microkit event (e.g. a notify on a certain channel, or a
write to a certain area of memory) unless the guarantee property holds about
its current abstract state. A *reliance* means an environment assumption that
the PD proof may use (e.g. that other PDs won't notify it unless its abstract
machine state satisfies a predicate). **NB** These names somewhat clash with
"rely/guarantee reasoning", but these are not rely/guarantee proof rules in the
classic concurrency-verification sense!

For example, a PD specification can say:

* the PD may notify on channel 5 only when its abstract state says it has a 
resource token,
* after that notify event, the PD no longer has the token, and
* after a receive event, the PD may assume only facts that the global system
proof later proves.

Viper checks that the Pancake implementation honors this local specification.

E.g. when a PD calls the `microkit_notify(5)` API function, the verifier must:
* establish common preconditions, such as "the channel 5 exists, and allows
  notifications according to the SDF",
* establish that the PD's abstract state `state` at the time of the call
  satisfies the the event guarantee `guarantee_microkit_notify(5,state)`, and
* assume that after the call, the abstract ghost state changes according to the
  event transition relation.

For calls that receive information from another PD or from the system, the 
postcondition may also assume a reliance about the possible values that might be
returned in a given state. Those reliances will later on have to be justified by
any system-level proof.


## System-level verification

The system level will get modeled as a stream of Microkit events.

Shared memory regions appear in this model. Private memory inside each 
protection domain does not. Instead, each PD is represented by its abstract
state machine. The abstract state machine changes state only when a PD emits
a modeled Microkit event. The shared memory changes state only when some PD
writes to it. Between Microkit events, the abstract state stays unchanged.

This gives the global proof a compact view: one does not need to reason
reason about a PD's private state, or even its concrete implementation, only
about the event behavior that the PD exposes. As long as this event behavior
remains correct, the programmers can make changes to the PDs without affecting
global correctness. Automated provers ensure that the local changes do not
violate the prescribed event behavior.

Currently, there are no system-level proofs. Eventual such proofs should use
an interactive theorem prover, and should eventually connect to the existing
seL4 proofs.

Eventually, there should be a checked, automated way to export the proved
PD-levle guarantees and transition relations as facts about the PD to the
global proof. These would consists of:

- the abstract state space;
- the guarantees that guard emitted events;
- the transition relations for those events;
- the reliances that the system-level proof must discharge.

## Viper export of Microkit invariants

The Microkit tool exports a per-PD Viper view derived from the SDF and 
generated CapDL state.

This export describes the following information as Viper predicates:

* which `CPtr` slots contain caps (according to the initializer CapDL),
* what kind of cap each relevant slot contains,
* which channels may be used to make notifications or protected calls (according
  to the SDF),
* which channels may deliver notifications or protected calls to this PD,
* which IRQs are available,
* which child PDs are available,
* which shared-memory virtual ranges are readable or writeable.

The Pancake Microkit and seL4 wrappers use these facts as preconditions. For
example, a notify requires a valid notify target, and an IRQ acknowledgement 
kernel call requires an IRQ-handler cap.

These contracts check that the Pancake code calls libsel4 with appropriate
caps, valid message-info values, valid message-register indices, and valid FFI 
memory usage.

This means that Pancake code which passes verification
* will not make any libsel4 calls that would fault because of missing or
  wrong-kind caps,
* will not perform out-of-range modeled shared-memory accesses, and
* will avoid arithmetic errors such as signed overflow or division by zero.

Currently, the CapDL export is limited: it emits slot locations and cap kinds,
but not cap permissions. The guarantees provided by the proof can be improved
by explicitly representing these, and adding them to the Pacake libsel4 spec
as preconditions. Once this is done, Pancake code which passes verification
will be guaranteed not to make any libsel4 calls that will fault because of
incorrect permissions.

## What is proved and assumed

For a verified Pancake PD, it is proved that:

* each emitted Microkit event satisfies its guarantee,
* the Microkit handler-loop entry points satisfy their contracts,
* under these contracts, the Microkit main handler loop works correctly (see [here](https://trustworthy.systems/projects/microkit/gordian-report.pdf)),
* any libsel4 calls satisfy their preconditions,
* any shared-memory accesses stay within bounds and permissions.

Because reliances are assumptions, a PD proof should be read as conditional:
the highest-level correctness result is that the PD implementation obeys its
event contract if its environment obeys the stated reliances.

The current proof story still has a very large trust base. We have to trust:

* the Viper verifier itself,
* the Pancake-to-Viper translator and its Pancake semantics,
* the Microkit tool's export of SDF, CapDL and memory views,
* the assumptions the Pancake libsel4 wrapper makes about the C libsel4 implementation,
* the bitvector, heap, and shared-memory axioms used in Viper, and
* the PD reliances (to be discharged in the eventual system-level proofs).

The current proof story also does not check whether the user makes any direct
libsel4 calls (e.g. in external code) which violate the Microkit boundaries.
However, such a check is easy to introduce using Viper's `acc(-)` mechanism.

## Missing or incomplete specification coverage

**PD control events not fully specified**

The calls `microkit_pd_stop` and `microkit_pd_restart` are implemented and
usable in the Pancake Microkit API. Viper can verify local safety facts about
using these APIs, such as "the target is a child PD" and "the needed TCB cap
exists", but they are not yet modeled as events: they have no user guarantee,
no transition relation, no local pre/post hooks, and no exported event
interface.

**Message register reasoning is not available**

The Pancake Microkit API exposes `microkit_mr_set` and `microkit_mr_get` as
aliases for `panseL4_SetMR` and `panseL4_GetMR`. However, the current
spec does not support functional reasoning about message register payloads
during PPC. The best way to model these and represent them at the Viper level
is still unclear.

**Architecture-specific calls are not implemented**

The Pancake implementation of Microkit only supports RISC-V at the moment.
Architecture-specific calls such as Arm SMC, x86 I/O port operations, and
VCPUs are not implemented, and not specified.

**The cap model still lacks permission information**

Cap slots and their kinds are exported, but the current spec does not check
against rights/badges/grant rights. See the section on PD-level specification.

**No guards against raw kernel / libsel4 calls**

If one has access to direct kernel calls, one can bypass verification by
using a manual call to trigger a Microkit event. For example, instead of
calling `microkit_notify(x)`, one can get the `CPtr y` corresponding to the
notification of channel `x`, and make a direct call to `panseL4_Signal(y)`.
This will emit a Microkit event, but will not create an obligation to show
that the `microkit_notify` guarantee and transition relation hold.

This has an easy solution: one can add a precondition,
`acc(panseL4.unsafe_calls)`, to each `panseL4_` call, and inhale this inside
`microkit_` calls only. This will let the verifier prevent any unintended
or accidental such violations. This is easy to implement, and has not been
done yet only to cut down the number of access rights one needs to handle
during this experimental phase.

However, the verifier cannot prevent kernel calls made outside of Pancake, in
the FFI code. Ensuring that FFI calls are safe remains the proof engineer's
responsibility.

# The ping-pong example

The `pingpong` system consists of two protection domains, which behave like two
players playing table tennis, passing a resource (the "pingpong ball") back and
forth.

The `ping` domain starts with the ball, sends a notification to `pong`, then
waits until `pong` sends a notification back, *ad infinitum*.

The system-level property is meant to ensure the coherence of the game:
the two protection domains never both "have the ball" at the same time, and
neither side gets more than one turn ahead of the other (passing the ball,
to the other without the other having passed the ball back first).

## PD-level contract

Working backwards from this intended system-level property, we can decide to
model each PD using a small PD-level ghost state consisting of the following
fields:

```
field has_the_ball: Bool
field ping_counter: Int
```

The PD-level state keeps track of whether the PD currently has the ball, and
how many times it has already sent it.

The PD-level contract governs what kinds of Microkit events these PDs can emit,
and how their state may change when they do so. These are as follows:

**The `microkit_notify` contract**

The `ping` domain may cause a `microkit_notify` event only when it has the
ball. After notifying, it transitions into a state that gives up the ball and
increments its pass counter.

More formally, the guarantee (guard) that the PD-level state has to satisfy
before a notify event is that `has_the_ball == true`.

The corresponding transition relation is that
```
state.has_the_ball := false;
state.ping_counter := state.ping_counter + 1
```
or, since transitions are non-deterministic, the following relation has to
hold between the old and new states:
```
new_state.has_the_ball == false &&
new_state.ping_counter == old_state.ping_counter + 1
```

**The `receive` contract**

The `ping` domain may wait on its endpoint to `receive` only without the ball,
meaning that after handling the notification it must pass the ball on.

The transition relation says that after the receive call returns, the ball is
received, but the counter remains unchanged:
```
new_state.has_the_ball == true &&
new_state.ping_counter == old_state.ping_counter
```

The global proof can reason only about this abstract state, it should not have
to inspect the implemenatiion of `ping`. The automated Viper proofs establish
that the implementation itself satisfies the PD-level contracts expected by the
system-level proof.

## Verifying the PD-level contract

TODO: WRITE

## Connecting to the system-level proof

TODO:WRITE
