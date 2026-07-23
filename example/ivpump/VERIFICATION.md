<!--
     Copyright 2026, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->

This example constitutes an early experiment in verification of Microkit-based
systems by combining automated local proofs about protection domains with
hand-written machine-checked global correctness proofs about the whole system.

This is not the verification of a real system. It is a small example to help
proof engineers and system integrators understand how to write specifications
and verify them using the tools provided by Pancake, Viper, and the Pancake
Microkit libraries.

**Outline**

Each protection domain in this toy system has a corresponding non-deterministic
local state machine model. Viper proves that the Pancake implementation refines
the local state machine.

Globally, we model the system using one local state machine for each PD, and a
linear *event trace*. When a Microkit event occurs, the local state machine of
the PD which emitted the event takes one step, while the local states of all
other PD state machines remain unchanged. The Microkit/shared-memory semantics
constrains, along with the user's local state machine specification, constrain
which events and return values can occur. We prove a global safety property for
our system using these event traces.

The first section explains the general verification approach, and the second
section instantiates the approach on the toy `ivpump` system.

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
*PD-level properties*. The idea is to make each protection domain provide
a specification of its local event contract: when it may emit a Microkit
event, how that event may change its abstract state, and what assumptions
it needs from the rest of the system.


## PD-level verification

Each verified Pancake protection domain gets a local specification.
The local specification defines:

* the abstract state machine that represents the PD at the system level;
* event guards;
* event transition relations;
* local state preconditions and postconditions for Microkit calls;
* reliances, which are assumptions about the PD's environment; and
* loop invariants needed by the Microkit handler loop.

An *event* guard is a predicate representing promise by the PD that it will not
emit a certain type of Microkit event (e.g. a notify on a certain channel, or a
write to a certain area of memory) except when the guard predicate holds about
its current abstract state. A *reliance* means an environment assumption that
the PD proof may use (e.g. that other PDs won't notify it unless its abstract
machine state satisfies a predicate). **NB** These names somewhat resemble
"rely/guarantee reasoning", but these are not rely/guarantee rules in the
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
  satisfies the the event guard `guard_microkit_notify(5,state)`, and
* assume that after the call, the abstract ghost state changes according to the
  event transition relation.

For calls that receive information from another PD or from the system, the 
postcondition may also assume a reliance about the possible values that might be
returned in a given state. Those reliances are assumptions made by the PD-level
proofs, and have to be justified by the system-level argument about the composed
state machines.

The PD-level proof is thus a refinement proof. It shows that each verified
Pancake program preserves a simulation relation between the concrete Pancake
implementation's state and the PD's abstract state machine.

In other words, if one projects a concrete execution to the modeled Microkit
events, the PD's abstract state must be able to step along with that
projection.

## System-level verification

The system level gets modeled as a stream of Microkit events.

Shared memory regions appear in this model, but private memory inside each 
protection domain does not. Instead, each PD is represented by its abstract
state machine. The abstract state machine changes state only when a PD emits
a modeled Microkit event. The shared memory changes state only when some PD
writes to it. Between Microkit events, the abstract state stays unchanged.

This gives the global proof a compact view: one does not need to reason
reason about a PD's private state, or even its concrete implementation, only
about the event behavior that the PD exposes. As long as this event behavior
remains correct, the programmers can make changes to the PDs without affecting
global correctness. Automated provers can ensure that the local changes do not
violate the prescribed event contract.

Eventually, there should be a checked, automated way to export the proved
PD-level guards and transition relations as facts about the PD to the
system-level proof. These would consists of:

- the abstract state space;
- the guards that guard emitted events;
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

Currently, the CapDL export emits only the slot locations and cap kinds,
but not the cap permissions. The guards provided by the proof can be improved
by explicitly representing these, and adding them to the Pancake libsel4 spec
as preconditions. Once this is done, Pancake code which passes verification
will be guaranteed not to make any libsel4 calls that fault because of
incorrect permissions.

## What is proved and assumed

For each verified Pancake PD, it is proved using Viper that:

* each emitted Microkit event satisfies its guard,
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
exists", but they are not yet modeled as events: they have no user guard,
no transition relation and no exportable event interface.

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
that the `microkit_notify` guard and transition relation hold.

This has an easy solution: one can add a precondition,
`acc(panseL4.unsafe_calls)`, to each `panseL4_` call, and inhale this inside
`microkit_` calls only. This will let the verifier prevent any unintended
or accidental such violations. This is easy to implement, and has not been
done yet only to cut down the number of access rights one needs to handle
during this experimental phase.

However, the verifier cannot prevent kernel calls made outside of Pancake, in
the FFI code. Ensuring that FFI calls are safe remains the proof engineer's
responsibility.


# The ivpump example

The `ivpump` system is a toy example motivated by an automatic IV pump.

The actual code in this repository is not a real device driver or
pump controller, only barebones code to support demoing the verification
process!

The imagined device has two buttons:

* pressing button 1 degasses the line, and
* pressing button 2 starts the infusion.

The system consists of:

* A PD representing the keypad driver (`keypad`),
* a PD representing the IV pump controller (`pump`),
* two shared memory regions between the PDs: `control` and `command` words, and
* a `keybuffer` device memory region representing device input.

If, due to a bug, the controller treats a button-1-then-button-2 input as if
it had seen only button-2, the pump may start the infusion while there is air
still in the line.

The system-level safety property is meant to ensure that this does not happen:
the sequence of commands already handled by the pump must always be a prefix
of the sequence of commands issued by the keypad. This means that the pump may
lag behind the keypad, but it cannot get ahead of it, cannot invent a command,
cannot skip a command, and cannot handle commands in a different order than the
order they were issued on the keypad.

The property does not say that every issued command is eventually handled. That
would be a liveness property, which require a stronger refinement story than
the one used here.

## PD-level contracts

Working backwards from this intended system-level property, we can decide how
to best model each PD using a small PD-level ghost state.

**Keypad driver**

The `Keypad` ghost state, declared in `spec-for-keypad.vpr`, has the following
fields:

```viper
field input: Seq[Int]
field has_control: Bool
field issued_cmds: Seq[Int]
```

The field `input` keeps track of keypresses not yet read from the keybuffer,
`has_control` is true when the keypad owns the `command` memory region, and
`issued_cmds` is the trace of commands published by the keypad driver so far.

When the keypad receives a notification (simulating an interrupt), the
abstract state nondeterministically transitions `input` to 1 or 2, leaving
everything else unchanged. Reading the contents of the `control` memory region
transitions `has_control` to true if the read value is zero, and leaves
everything else unchanged.

The guards say that:
* when `Keypad` receives, it has no unhandled input, and does not own the `command` buffer;
* when `Keypad` reads from shared memory, it only reads from `control` and `keybuffer`, and only while there are no unhandled key presses;
* when `Keypad` writes to shared memory, it only writes to `control` and `command`, to the former only the value `1`, etc.;
* no othe Microkit events are emitted by the `Keypad` PD.

These are specified by the system integrator or proof engineeri in the file
`spec-for-keypad.vpr`, in the Viper language. For example, the condition on
receive reads

```viper
state.input == Seq() &&
!state.has_control
```

The corresponding transition relation, when expressed as a relation between
the current pre-transition state `pre` and the post-transition state `post`,
would read:

```viper
(post.input == Seq(1) || post.input == Seq(2)) &&
post.has_control == pre.has_control &&
post.issued_cmds == pre.issued_cmds
```

The system-level proof can reason only about this abstract state, it should not
have to inspect the implemenatiion of `keypad`. The automated Viper proofs
establish that the implementation itself satisfies the PD-level contracts
expected by the system-level proof. After building the example, the complete
Viper proof can be found under `./build/keypad_verification.vpr`.

**Pump controller**

Similarly, one models the `pump` PD using the fields

```viper
field has_control: Bool
field unhandled_cmds: Seq[Int]
field handled_cmds: Seq[Int]
```

where
`has_control` is true precisely if `pump` owns the `command` memory region,
`unhandled_cmds` is the list of command the pump has read but is yet to handle
in the order they were read, and `handled_cmds` is the history of commands
already handled by the pump.

The spec can be found under `spec-for-pump.vpr`, and after the build, the
Viper proof script appears in `./build/pump_verification.vpr`.

## Verifying the PD-level contract

To verify the PD-level contract, go through the build as in `README.md`,
then open the resulting verification files in VS Code with the Viper
extension installed.

If your Viper installation is working correctly, the bottom info area will
briefly show the message `Hello from Viper`, then transition to a progress bar
```
Verification of pd_verification.vpr
```
signifying that verification has started. Verification should take between 7
and 20 seconds depending on your computer's performance. Eventually, the
message will change to
```
Verified pd_verification.vpr with 148 warnings
```
indicating successful proof of refinment.

## System-level proof

This example comes with a small, simplified system-level proof. It is a
manual Agda proof over event traces. Agda is not essential to the approach,
and was chosen for the proof engineer's convenience. More conventional
interactive theorem provers such as Isabelle/HOL, HOL4 or Lean could do the
job equally well.

The proof is located under the `global/` directory. The main result is in
`global/Theorem.agda`, which contains both the system-level safety
specification and the correctness proof.

The safety specification is the predicate `Correct run`, which says that
```agda
Prefix pump.handled_cmds keypad.issued_cmds
```
holds in the running system. In words, correctness requires that the
sequence of commands already handled by the pump must be a prefix of
the sequence of commands ever issued by the keypad.

The global correctness theorem is `global_correctness` at the end of
`Theorem.agda`. It states
```
forall run. Permitted run && Compliant run -> Correct run
```
in other words that every run which is permitted by the Microkit semantics and
complies with the local state machine specifications in `spec-for-keypad.vpr`
and `spec-for-pump.vpr` actually satisfies the correctness condition throughout
the run.

The Viper PD-level specifications are hand-exported to Agda, and are
located in `global/KeypadLSM.agda` and `global/PumpLSM.agda`.

**Event model**

The event model, in `global/Types.agda` and `global/Trace.agda`, contains only
the specifications required for this example. This means that the Agda theory
omits protected procedure calls, reply-receive, deferred calls, PD control
operations and architecture-specific events. Adding them would not change the
proof idea at all, but it would require a larger event type, larger SDF model,
and more proof cases (all of them vacuous, since none of these features are
used in the Pancake implementation).

**Memory model**

A semantics and memory model for Microkit is given in `global/Semantics.agda`.
The example was originally motivated by weak memory, but the current proof uses
sequential trace consistency. The Agda code comments explain how a relaxed
trace semantics could be introduced to model weak memory. Since neither the
Pancake implementation nor the current Microkit model has memory-barrier
events, the global correctness would not hold under weak memory semantics.

## Running the system-level verification

One can run the system level verification by opening `global/Theorem.agda`
in Emacs running the Agda plug-in. Then selecting `Agda > Load` from the menu
or pressing `C-c C-l` in the buffer performs type-checking / verification.

The status bar will display `Theorem.agda  Bot  L2218  (Agda:Checked)` after
successful verification.

Alternatively, one can run `agda --safe Theorem.agda` from the `global/`
directory. If the standard library has already been checked, this will take
only 10 second or so. The first full check (which has to typecheck the proof
as well as the standard library) will take significantly longer, possibly over
two minutes.
