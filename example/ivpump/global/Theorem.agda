--
-- Copyright 2026, UNSW
--
-- SPDX-License-Identifier: BSD-2-Clause
--

module Theorem where

open import Types
open import Trace

open import Data.Bool
open import Data.Empty
open import Data.List
open import Data.Maybe
open import Data.Nat
open import Data.Product
open import Data.Sum

open import Relation.Binary.PropositionalEquality
open import Data.List.Properties using (++-identityʳ)

import KeypadLSM
import PumpLSM
import Semantics

{-
  In this file we specify the safety property of our system:
  every keypress input is handled without skipping any, in other words
  the temporally ordered list of handled inputs is a prefix of the
  temporally ordered list of issued inputs.

  The safety property is stated as the predicate `Correct`.
  Global correctness (theorem `global-correctness`) follows by induction
  on traces, after strengthening the induction hypothesis. This strengthening
  is accomplished by the `Proper` predicate.

  So the proof of the correctness theorem, in a nutshell, is the following:
  1. If all events in a run are permitted by the Microkit semantics and
     comply with the user-specified guard/transition conditions, then
     the run is proper.
     (`properness-lemma`)
  2. Every proper run is correct.
     (`correctness-lemma`)
  3. Thus if every event in a run is permitted by the Microkit semantics
     and complies with the user's guard/transition conditions, then the run
     is correct.
     (`global-correctness`)
  4. We have established in Viper that every event emitted by our PDs is
     permitted by the Microkit semantics and complies with the user's
     guard/transition conditions.
     (`keypad_verification.vpr` and `pump_verification.vpr`)
  5. Thus, from 3 and 4 we get that every run is correct.
-}

record Prefix (prefix : List ℕ) (total : List ℕ) : Set where
--^ Holds if the list `total` starts with prefix.
--E.g. `Prefix (42 ∷ []) (42 ∷ 66 ∷ [])` holds, while
--something like `Prefix (42 ∷ []) (5 ∷ 6 ∷ 42 ∷ [])` fails.
  field
     datum : List ℕ
     property : prefix ++ datum ≡ total

Prefix-equals : {xs ys : List ℕ} → xs ≡ ys → Prefix xs ys
Prefix-equals {xs} refl =
  record { datum = [] ; property = ++-identityʳ xs }

Prefix-last : {xs ys : List ℕ} → (i : ℕ) → xs ++ (i ∷ []) ≡ ys → Prefix xs ys
Prefix-last {xs} i refl =
  record { datum = i ∷ [] ; property = refl }

Keypad-issued-cmds : Run → List ℕ
Keypad-issued-cmds run =
  KeypadLSM.State.issued-cmds (stateMap run Keypad)

Pump-handled-cmds : Run → List ℕ
Pump-handled-cmds run =
  PumpLSM.State.handled-cmds (stateMap run Pump)

Correct : Run → Set
Correct run = Prefix (Pump-handled-cmds run) (Keypad-issued-cmds run)

data ProperInput : List ℕ → Set where
  ProperInput-[] : ProperInput []
  ProperInput-[1] : ProperInput (1 ∷ [])
  ProperInput-[2] : ProperInput (2 ∷ [])

data ProperInputNE : List ℕ → Set where
  ProperInputNE-[1] : ProperInputNE (1 ∷ [])
  ProperInputNE-[2] : ProperInputNE (2 ∷ [])

data ProperInputE : List ℕ → Set where
  ProperInputE-[] : ProperInputE []


Keypad-input : Run → List ℕ
Keypad-input run =
  KeypadLSM.State.input (stateMap run Keypad)

Keypad-has-control : Run → Bool
Keypad-has-control run =
  KeypadLSM.State.has-control (stateMap run Keypad)

Pump-has-control : Run → Bool
Pump-has-control run =
  PumpLSM.State.has-control (stateMap run Pump)

Pump-unhandled-cmds : Run → List ℕ
Pump-unhandled-cmds run =
  PumpLSM.State.unhandled-cmds (stateMap run Pump)

data InControl (run : Run) : Maybe PD → Set where
  InControl-nothing :
    Keypad-has-control run ≡ false →
    Pump-has-control run ≡ false →
    InControl run nothing
  InControl-Keypad :
    Keypad-has-control run ≡ true →
    Pump-has-control run ≡ false →
    InControl run (just Keypad)
  InControl-Pump :
    Keypad-has-control run ≡ false →
    Pump-has-control run ≡ true →
    InControl run (just Pump)

record Observes (pd : PD) (addr : Address) (val : Word) (run : Run) : Set where
  field
    can-observe : Semantics.CanLoad64 pd addr val run
    must-observe : (x : Word) → Semantics.CanLoad64 pd addr x run → x ≡ val

observes :
  {pd : PD} → {addr : Address} → {val : Word} → {run : Run} →
  Semantics.CanLoad64 pd addr val run →
  Observes pd addr val run
--^ Since STC semantics is sequentially consistent, we get `must-observe`
--for free.
observes [CanLoad64-val] = record
  { can-observe = [CanLoad64-val]
  ; must-observe = λ x [CanLoad64-x] →
    Semantics.sequential-consistency [CanLoad64-x] [CanLoad64-val]
  }

observes-Load64 :
  {observing loading : PD} →
  {observed-address loaded-address : Address} →
  {observed-value loaded-value : Word} →
  {new : StateMap} →
  {run : Run} →
  Observes observing observed-address observed-value run →
  Observes observing observed-address observed-value
    (step (loading ⨾ Load64 loaded-address ⨾ loaded-value) new run)
observes-Load64
  {_} {loading}
  {_} {loaded-address}
  {_} {loaded-value}
  {new}
  {run}
  [observes-old] =
  observes
  (
    Semantics.CanLoad64-Load64
      loading loaded-address loaded-value new run
      (Observes.can-observe [observes-old])
  )

data Proper (run : Run) : Set where
--^ A strengthened induction hypothesis. Instead of proving that every
--permitted, compliant run obeys the global specification, we will show that
--every permitted, compliant run is proper, and then that proper runs obey
--the specification.
--
--A run can be proper for six different reasons: these are the six reasons of
--propriety.
  Proper-idle :
    InControl run nothing →
    Observes Keypad control-addr 0 run →
    Observes Pump control-addr 0 run →
    ProperInput (Keypad-input run) →
    Pump-handled-cmds run ≡ Keypad-issued-cmds run →
    Pump-unhandled-cmds run ≡ [] →
    Proper run

  Proper-Keypad-input :
    InControl run (just Keypad) →
    ProperInputNE (Keypad-input run) →
    Pump-handled-cmds run ≡ Keypad-issued-cmds run →
    Pump-unhandled-cmds run ≡ [] →
    Observes Keypad control-addr 0 run →
    Observes Pump control-addr 0 run →
    Proper run

  Proper-Keypad-issued :
    (i : Word) →
    InControl run (just Keypad) →
    ProperInputE (Keypad-input run) →
    Pump-handled-cmds run ++ (i ∷ []) ≡ Keypad-issued-cmds run →
    Pump-unhandled-cmds run ≡ [] →
    Observes Keypad control-addr 0 run →
    Observes Pump control-addr 0 run →
    Observes Pump command-addr i run →
    Proper run

  Proper-transfer :
    (i : Word) →
    InControl run nothing →
    ProperInput (Keypad-input run) →
    Pump-handled-cmds run ++ (i ∷ []) ≡ Keypad-issued-cmds run →
    Pump-unhandled-cmds run ≡ [] →
    Observes Keypad control-addr 1 run →
    Observes Pump control-addr 1 run →
    Observes Pump command-addr i run →
    Proper run

  Proper-Pump-input :
    (i : Word) →
    InControl run (just Pump) →
    ProperInput (Keypad-input run) →
    Pump-handled-cmds run ++ (i ∷ []) ≡ Keypad-issued-cmds run →
    Pump-unhandled-cmds run ≡ [] →
    Observes Keypad control-addr 1 run →
    Observes Pump control-addr 1 run →
    Observes Pump command-addr i run →
    Proper run

  Proper-Pump-saved :
    (i : Word) →
    InControl run (just Pump) →
    ProperInput (Keypad-input run) →
    Pump-handled-cmds run ++ (i ∷ []) ≡ Keypad-issued-cmds run →
    Pump-unhandled-cmds run ≡ (i ∷ []) →
    Observes Keypad control-addr 1 run →
    Observes Pump control-addr 1 run →
    Observes Pump command-addr i run →
    Proper run

correctness-lemma :
  {run : Run} → Proper run → Correct run
--^ Properness is a strengthening of correctness: every proper run
--satisfies the global specification.
correctness-lemma (Proper-idle _ _ _ _ [Ph=Ki] _) =
  Prefix-equals [Ph=Ki]
correctness-lemma (Proper-Keypad-input _ _ [Ph=Ki] _ _ _) =
  Prefix-equals [Ph=Ki]
correctness-lemma (Proper-Keypad-issued i _ _ [Ph++i=Ki] _ _ _ _) =
  Prefix-last i [Ph++i=Ki]
correctness-lemma (Proper-transfer i _ _ [Ph++i=Ki] _ _ _ _) =
  Prefix-last i [Ph++i=Ki]
correctness-lemma (Proper-Pump-input i _ _ [Ph++i=Ki] _ _ _ _) =
  Prefix-last i [Ph++i=Ki]
correctness-lemma (Proper-Pump-saved i _ _ [Ph++i=Ki] _ _ _ _) =
  Prefix-last i [Ph++i=Ki]

true-and-false :
  ∀ {x : Bool} →
  x ≡ true →
  x ≡ false →
  ∀ {A : Set} → A
--^ This trivial lemma will make quick work of most impossible cases
--in the properness argument.
true-and-false {false} () [x=false]
true-and-false {true} [x=true] ()

properness-lemma :
  (run : Run) →
  Semantics.Permitted run →
  Compliant run →
  Proper run
--^ We must now show by induction on traces that every compliant run permitted
--under the sequential trace consistency semantics is indeed proper.
--This is the "hard" part in that it takes a while, but a lot of these proofs
--end up being quite formulaic.
--We first derive the consequences of the current "new" step being permitted
--and compliant. Then we apply the induction hypothesis, to obtain the reason
--for propriety of the tail. Finally, we must do a case analysis (`progress`)
--to compute a reason for propriety of the extended sequence from the reason
--for propriety of its tail.
properness-lemma
  (init ism)
  Semantics.Permitted-init
  (Compliant-init init-ism) =
  -- The system starts out in an idle state.
  Proper-idle
  (
    InControl-nothing
    (proj₁ (proj₂ (init-ism Keypad)))
    (proj₁ (init-ism Pump))
  )
  (observes (Semantics.CanLoad64-init ism refl))
  (observes (Semantics.CanLoad64-init ism refl))
  (subst ProperInput (sym (proj₁ (init-ism Keypad))) ProperInput-[])
  (
    trans
    (proj₂ (proj₂ (init-ism Pump)))
    (sym (proj₂ (proj₂ (init-ism Keypad))))
  )
  (proj₁ (proj₂ (init-ism Pump)))

properness-lemma
  (step (Keypad ⨾ Notify _ ⨾ ret) _ _)
  (Semantics.Permitted-Notify _ _)
  (Compliant-step [Step-ev-new] _) =
  -- We know from the user's guard that `Keypad` does not emit any
  -- `Notify` events. This was verified by Viper.
  ⊥-elim (Step.guard-holds [Step-ev-new])

properness-lemma
  (step (Pump ⨾ Notify ch ⨾ ret) new run)
  (Semantics.Permitted-Notify _ [Permitted-run])
  (Compliant-step [Step-ev-new] [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  -- We prove that this notification does not change the state at all.
  [Pc-new=Pc-old] :
    PumpLSM.State.has-control (new Pump) ≡
    PumpLSM.State.has-control (old Pump)
  [Pc-new=Pc-old] = proj₁ (Step.pd-transitions [Step-ev-new])
  [Puc-new=Puc-old] :
    PumpLSM.State.unhandled-cmds (new Pump) ≡
    PumpLSM.State.unhandled-cmds (old Pump)
  [Puc-new=Puc-old] = proj₁ (proj₂ (Step.pd-transitions [Step-ev-new]))
  [Phc-new=Phc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump)
  [Phc-new=Phc-old] = proj₂ (proj₂ (Step.pd-transitions [Step-ev-new]))
  [new-Keypad=old-Keypad] : new Keypad ≡ old Keypad
  [new-Keypad=old-Keypad] =
    sym (Step.world-waits [Step-ev-new] Keypad (λ ()))
  [Ki-new=Ki-old] :
    KeypadLSM.State.input (new Keypad) ≡
    KeypadLSM.State.input (old Keypad)
  [Ki-new=Ki-old] =
    cong KeypadLSM.State.input [new-Keypad=old-Keypad]
  [Kc-new=Kc-old] :
    KeypadLSM.State.has-control (new Keypad) ≡
    KeypadLSM.State.has-control (old Keypad)
  [Kc-new=Kc-old] =
    cong KeypadLSM.State.has-control [new-Keypad=old-Keypad]
  [Kic-new=Kic-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad)
  [Kic-new=Kic-old] =
    cong KeypadLSM.State.issued-cmds [new-Keypad=old-Keypad]

  -- Since the state is unchanged, and the old state was proper, so
  -- is the new state.
  progress :
    Proper run →
    Proper (step (Pump ⨾ Notify ch ⨾ _) new run)
  progress
    (Proper-idle
      (InControl-nothing [Kc-old=false] [Pc-old=false])
      [obs-Kcon-0]
      [obs-Pcon-0]
      [ProperInput-old]
      [Phc-old=Kic-old]
      [Puc-old=0]
    ) =
    Proper-idle
      (
        InControl-nothing
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Kcon-0])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcon-0])
        )
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (trans [Phc-new=Phc-old] (trans [Phc-old=Kic-old] (sym [Kic-new=Kic-old])))
      (trans [Puc-new=Puc-old] [Puc-old=0])

  progress
    (Proper-Keypad-input
      (InControl-Keypad [Kc-old=true] [Pc-old=false])
      [ProperInputNE-old]
      [Phc-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-0]
      [obs-Pcon-0]
    ) =
    Proper-Keypad-input
      (
        InControl-Keypad
        (trans [Kc-new=Kc-old] [Kc-old=true])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (subst ProperInputNE (sym [Ki-new=Ki-old]) [ProperInputNE-old])
      (trans [Phc-new=Phc-old] (trans [Phc-old=Kic-old] (sym [Kic-new=Kic-old])))
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Kcon-0])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcon-0])
        )
      )

  progress
    (Proper-Keypad-issued
      i
      (InControl-Keypad [Kc-old=true] [Pc-old=false])
      [ProperInputE-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-0]
      [obs-Pcon-0]
      [obs-Pcom-i]
    ) =
    Proper-Keypad-issued
      i
      (
        InControl-Keypad
        (trans [Kc-new=Kc-old] [Kc-old=true])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (subst ProperInputE (sym [Ki-new=Ki-old]) [ProperInputE-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Kcon-0])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcon-0])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcom-i])
        )
      )

  progress
    (Proper-transfer
      i
      (InControl-nothing [Kc-old=false] [Pc-old=false])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-transfer
      i
      (
        InControl-nothing
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Kcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcom-i])
        )
      )

  progress
    (Proper-Pump-input
      i
      (InControl-Pump [Kc-old=false] [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-input
      i
      (
        InControl-Pump
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Kcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcom-i])
        )
      )

  progress
    (Proper-Pump-saved
      i
      (InControl-Pump [Kc-old=false] [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=i]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-saved
      i
      (
        InControl-Pump
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=i])
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Kcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Notify Pump ch new run
          (Observes.can-observe [obs-Pcom-i])
        )
      )

properness-lemma (step (Keypad ⨾ Recv ⨾ _) new run)
  (Semantics.Permitted-Recv x [Permitted-run])
  (Compliant-step step-ev-sm [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  -- We extract a bunch of equalities implied by the assumptions.
  [Ki-old=false] : KeypadLSM.State.input (old Keypad) ≡ []
  [Ki-old=false] = proj₁ (Step.guard-holds step-ev-sm)
  [Kc-old=false] : KeypadLSM.State.has-control (old Keypad) ≡ false
  [Kc-old=false] = proj₂ (Step.guard-holds step-ev-sm)
  transition-c1 :
    (KeypadLSM.State.input (new Keypad) ≡ 1 ∷ []) ⊎
    (KeypadLSM.State.input (new Keypad) ≡ 2 ∷ [])
  transition-c1 = proj₁ (Step.pd-transitions step-ev-sm)
  [Kc-new=Kc-old] :
    KeypadLSM.State.has-control (new Keypad) ≡
    KeypadLSM.State.has-control (old Keypad)
  [Kc-new=Kc-old] = proj₁ (proj₂ (Step.pd-transitions step-ev-sm))
  [Kic-new=Kic-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad)
  [Kic-new=Kic-old] = proj₂ (proj₂ (Step.pd-transitions step-ev-sm))
  [new-Pump=old-Pump] : new Pump ≡ old Pump
  [new-Pump=old-Pump] = sym (Step.world-waits step-ev-sm Pump λ ())
  [Pc-new=Pc-old] :
    PumpLSM.State.has-control (new Pump) ≡
    PumpLSM.State.has-control (old Pump)
  [Pc-new=Pc-old] =
    cong PumpLSM.State.has-control [new-Pump=old-Pump]
  [ProperInput] :
    (
      (KeypadLSM.State.input (new Keypad) ≡ 1 ∷ []) ⊎
      (KeypadLSM.State.input (new Keypad) ≡ 2 ∷ [])
    ) →
    ProperInput (KeypadLSM.State.input (new Keypad))
  [ProperInput] (inj₁ [Ki-new=1]) =
    subst ProperInput (sym [Ki-new=1]) ProperInput-[1]
  [ProperInput] (inj₂ [Ki-new=2]) =
    subst ProperInput (sym [Ki-new=2]) ProperInput-[2]
  [Phc-new=Phc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump)
  [Phc-new=Phc-old] =
    cong PumpLSM.State.handled-cmds [new-Pump=old-Pump]
  [Puc-new=Puc-old] :
    PumpLSM.State.unhandled-cmds (new Pump) ≡
    PumpLSM.State.unhandled-cmds (old Pump)
  [Puc-new=Puc-old] =
    cong PumpLSM.State.unhandled-cmds [new-Pump=old-Pump]

  -- Finally, we give a transition table, which turns the "reason of propriety"
  -- of the old run into a reason of propriety for the extended run.
  progress :
    Proper run →
    Proper (step (Keypad ⨾ Recv ⨾ _) new run)
  progress
    (Proper-idle
      (InControl-nothing [Kc-old=false] [Pc-old=false])
      [obs-Keypad-control-0]
      [obs-Pump-control-0]
      [ProperInput-old]
      [Phc-old=Kic-old]
      [Puc-old=0]
    ) =
    Proper-idle
      (
        InControl-nothing
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (
        observes
          (
            Semantics.CanLoad64-Recv Keypad new run
            (Observes.can-observe [obs-Keypad-control-0])
          )
      )
      (
        observes
          (
            Semantics.CanLoad64-Recv Keypad new run
            (Observes.can-observe [obs-Pump-control-0])
          )
      )
      ([ProperInput] transition-c1)
      (trans [Phc-new=Phc-old] (trans [Phc-old=Kic-old] (sym [Kic-new=Kic-old])))
      (trans [Puc-new=Puc-old] [Puc-old=0])

  progress
    (Proper-Keypad-input (InControl-Keypad [Kc-old=true] _) _ _ _ _ _) =
    ⊥-elim ([true≠false] [true=false]) where
      [true=false] : true ≡ false
      [true=false] = trans (sym [Kc-old=true]) [Kc-old=false]
      [true≠false] : true ≢ false
      [true≠false] ()

  progress
    (Proper-Keypad-issued _ (InControl-Keypad [Kc-old=true] _) _ _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]
    --^ Our first impossible progress case. The `Keypad` PD could not have
    -- emitted a `Recv` event if the reason for propriety was `Keypad-issued`,
    -- since that event requires `Keypad.has_control == false` in the user's
    -- guard, but we know `Keypad.has_control == true` in `Keypad-issued`.
    -- This used to look like
    -- > ⊥-elim ([true≠false] [true=false]) where
    -- > [true=false] : true ≡ false
    -- > [true=false] = trans (sym [Kc-old=true]) [Kc-old=false]
    -- > [true≠false] : true ≢ false
    -- > [true≠false] ()
    -- before the introduction of `true-and-false`.

  progress
    (Proper-transfer
      i
      (InControl-nothing [Kc-old=false] [Pc-old=false])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-transfer
      i
      (
        InControl-nothing
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      ([ProperInput] transition-c1)
      (
        trans
        (cong (λ xs → xs ++ (i ∷ [])) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Kcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Pcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Pcom-i])
        )
      )

  progress
    (Proper-Pump-input i
      (InControl-Pump [Kc-old=false] [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-input
      i
      (
        InControl-Pump
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      ([ProperInput] transition-c1)
      (
        trans
        (cong (λ xs → xs ++ (i ∷ [])) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Kcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Pcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Pcom-i])
        )
      )

  progress
    (Proper-Pump-saved
      i
      (InControl-Pump [Kc-old=false] [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Ki-old]
      [Puc-old=i]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-saved
      i
      (
        InControl-Pump
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      ([ProperInput] transition-c1)
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Ki-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=i])
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Kcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Pcon-1])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Recv Keypad new run
          (Observes.can-observe [obs-Pcom-i])
        )
      )

properness-lemma
  (step (Pump ⨾ Recv ⨾ _) _ _)
  _
  (Compliant-step step-ev-sm _) =
  -- We know from the user's guard that Pump` does not emit any
  -- `Recv` events (since it never leaves the init loop).
  -- This was verified by Viper.
  ⊥-elim (Step.guard-holds step-ev-sm)

properness-lemma
  (step (Keypad ⨾ Load64 command-addr ⨾ ret) sm run)
  _
  (Compliant-step step-ev-sm _) =
  -- We know from the user's guard that `Keypad` does not load from
  -- `command_addr`. This was verified by Viper.
  ⊥-elim (impossible (Step.guard-holds step-ev-sm)) where
  old : StateMap
  old = stateMap run
  impossible : KeypadLSM.Guard (Load64 command-addr) (old Keypad) → ⊥
  impossible (inj₁ ())
  impossible (inj₂ ())

properness-lemma
  (step (Keypad ⨾ Load64 control-addr ⨾ ret) new run)
  (Semantics.Permitted-Load64 _ _ [CanLoad64-con-ret] [Permitted-run])
  (Compliant-step [Step-ev-new] [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  -- We extract a bunch of equalities implied by the assumptions.
  -- From here onward, we will often have to do this in two steps:
  -- first, we identify which disjunct of the guard applies, then
  -- we get the equalities out.
  guard-extract :
    KeypadLSM.State.input (old Keypad) ≢ [] ×
    KeypadLSM.State.has-control (old Keypad) ≡ false
  guard-extract = extract (Step.guard-holds [Step-ev-new]) where
    -- we are in the control-addr case of the guard
    extract :
      KeypadLSM.Guard (Load64 control-addr) (old Keypad) →
      KeypadLSM.State.input (old Keypad) ≢ [] ×
      KeypadLSM.State.has-control (old Keypad) ≡ false
    extract (inj₁ (_ , result)) = result
    extract (inj₂ (() , _))
  [Ki-old≠0] : KeypadLSM.State.input (old Keypad) ≢ []
  [Ki-old≠0] = proj₁ guard-extract
  [Kc-old=false] : KeypadLSM.State.has-control (old Keypad) ≡ false
  [Kc-old=false] = proj₂ guard-extract

  -- As with guards, we first have to identify which disjunct of the
  -- transition relation applies.
  transition-extract :
    (
      KeypadLSM.State.input (new Keypad) ≡
      KeypadLSM.State.input (old Keypad)
    ) ×
    KeypadLSM.State.has-control (new Keypad) ≡ (ret == 0) ×
    (
      KeypadLSM.State.issued-cmds (new Keypad) ≡
      KeypadLSM.State.issued-cmds (old Keypad)
    )
  transition-extract = extract (Step.pd-transitions [Step-ev-new]) where
    extract :
      KeypadLSM.Transition
        (Load64 control-addr) ret (old Keypad) (new Keypad) →
      KeypadLSM.State.input (new Keypad) ≡
        KeypadLSM.State.input (old Keypad) ×
      KeypadLSM.State.has-control (new Keypad) ≡ (ret == 0) ×
      KeypadLSM.State.issued-cmds (new Keypad) ≡
        KeypadLSM.State.issued-cmds (old Keypad)
    extract (inj₁ (_ , result)) = result
    extract (inj₂ (() , _))

  [Ki-new=Ki-old] :
    KeypadLSM.State.input (new Keypad) ≡
    KeypadLSM.State.input (old Keypad)
  [Ki-new=Ki-old] = proj₁ transition-extract
  [Kc-new=ret==0] :
    KeypadLSM.State.has-control (new Keypad) ≡ (ret == 0)
  [Kc-new=ret==0] = proj₁ (proj₂ transition-extract)
  [Kic-new=Kic-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad)
  [Kic-new=Kic-old] = proj₂ (proj₂ transition-extract)

  [new-Pump=old-Pump] : new Pump ≡ old Pump
  [new-Pump=old-Pump] =
    sym (Step.world-waits [Step-ev-new] Pump λ ())
  [Pc-new=Pc-old] :
    PumpLSM.State.has-control (new Pump) ≡
    PumpLSM.State.has-control (old Pump)
  [Pc-new=Pc-old] =
    cong PumpLSM.State.has-control [new-Pump=old-Pump]
  [Puc-new=Puc-old] :
    PumpLSM.State.unhandled-cmds (new Pump) ≡
    PumpLSM.State.unhandled-cmds (old Pump)
  [Puc-new=Puc-old] =
    cong PumpLSM.State.unhandled-cmds [new-Pump=old-Pump]
  [Phc-new=Phc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump)
  [Phc-new=Phc-old] =
    cong PumpLSM.State.handled-cmds [new-Pump=old-Pump]

  progress :
    Proper run →
    Proper (step (Keypad ⨾ Load64 control-addr ⨾ ret) new run)
  progress
    (Proper-idle
      (InControl-nothing _ [Pc-old=false])
      [obs-Kcon-0]
      [obs-Pcon-0]
      [ProperInput-old]
      [Phc-old=Kic-old]
      [Puc-old=0]
    ) =
    Proper-Keypad-input
      (
        InControl-Keypad
          (
            trans
            [Kc-new=ret==0]
            (
              -- the most important move!
              cong (λ ret → ret == 0)
              (Observes.must-observe [obs-Kcon-0] ret [CanLoad64-con-ret])
            )
          )
          (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (
        subst ProperInputNE (sym [Ki-new=Ki-old]) (toNE [Ki-old≠0] [ProperInput-old])
      )
      (trans [Phc-new=Phc-old]
        (trans [Phc-old=Kic-old] (sym [Kic-new=Kic-old])))
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (observes-Load64 [obs-Kcon-0])
      (observes-Load64 [obs-Pcon-0])
      where
        toNE :
          {xs : List ℕ} →
          xs ≢ [] →
          ProperInput xs →
          ProperInputNE xs
        toNE [xs≠0] ProperInput-[] = ⊥-elim ([xs≠0] refl)
        toNE [xs≠0] ProperInput-[1] = ProperInputNE-[1]
        toNE [xs≠0] ProperInput-[2] = ProperInputNE-[2]

  progress
    (Proper-Keypad-input
      (InControl-Keypad [Kc-old=true] _)
      _ _ _ _ _
    ) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-Keypad-issued
      _ (InControl-Keypad [Kc-old=true] _) _ _ _ _ _ _
    ) =
    true-and-false [Kc-old=true] [Kc-old=false]
    
  progress
    (Proper-transfer
      i
      (InControl-nothing _ [Pc-old=false])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-transfer
      i
      (
        InControl-nothing
          (
            trans [Kc-new=ret==0]
            (
              cong (λ ret → ret == 0)
              (Observes.must-observe [obs-Kcon-1] ret [CanLoad64-con-ret])
            )
          )
          (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
          (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
          (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (observes-Load64 [obs-Kcon-1])
      (observes-Load64 [obs-Pcon-1])
      (observes-Load64 [obs-Pcom-i])

  progress
    (Proper-Pump-input
      i
      (InControl-Pump _ [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-input
      i
      (
        InControl-Pump
          (
            trans
            [Kc-new=ret==0]
            (
              cong (λ ret → ret == 0)
              (Observes.must-observe [obs-Kcon-1] ret [CanLoad64-con-ret])
            )
          )
          (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (observes-Load64 [obs-Kcon-1])
      (observes-Load64 [obs-Pcon-1])
      (observes-Load64 [obs-Pcom-i])

  progress
    (Proper-Pump-saved
      i
      (InControl-Pump _ [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=i]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-saved
      i
      (
        InControl-Pump
          (
            trans
            [Kc-new=ret==0]
            (
              cong (λ x → x == 0)
              (Observes.must-observe [obs-Kcon-1] ret [CanLoad64-con-ret])
            )
          )
          (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
          (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
          (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=i])
      (observes-Load64 [obs-Kcon-1])
      (observes-Load64 [obs-Pcon-1])
      (observes-Load64 [obs-Pcom-i])

properness-lemma
  (step (Keypad ⨾ Load64 keybuffer-addr ⨾ ret) new run)
  (Semantics.Permitted-DeviceLoad64 _ _ [Permitted-run])
  (Compliant-step [Step-ev-new] [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  -- We extract the equational consequences of compliance. As before,
  -- we must first identify which of the disjuncts applies.
  guard-extract :
    KeypadLSM.State.input (old Keypad) ≢ [] ×
    KeypadLSM.State.has-control (old Keypad) ≡ false
  guard-extract = extract (Step.guard-holds [Step-ev-new]) where
    extract :
      KeypadLSM.Guard (Load64 keybuffer-addr) (old Keypad) →
      KeypadLSM.State.input (old Keypad) ≢ [] ×
      KeypadLSM.State.has-control (old Keypad) ≡ false
    extract (inj₁ (() , _))
    extract (inj₂ (_ , result)) = result
  [Ki-old≠0] : KeypadLSM.State.input (old Keypad) ≢ []
  [Ki-old≠0] = proj₁ guard-extract
  [Kc-old=false] : KeypadLSM.State.has-control (old Keypad) ≡ false
  [Kc-old=false] = proj₂ guard-extract

  transition-extract :
    KeypadLSM.State.input (new Keypad) ≡
      KeypadLSM.State.input (old Keypad) ×
    KeypadLSM.State.has-control (new Keypad) ≡
      KeypadLSM.State.has-control (old Keypad) ×
    KeypadLSM.State.issued-cmds (new Keypad) ≡
      KeypadLSM.State.issued-cmds (old Keypad)
  transition-extract = extract (Step.pd-transitions [Step-ev-new]) where
    extract :
      KeypadLSM.Transition
        (Load64 keybuffer-addr) ret (old Keypad) (new Keypad) →
      KeypadLSM.State.input (new Keypad) ≡
        KeypadLSM.State.input (old Keypad) ×
      KeypadLSM.State.has-control (new Keypad) ≡
        KeypadLSM.State.has-control (old Keypad) ×
      KeypadLSM.State.issued-cmds (new Keypad) ≡
        KeypadLSM.State.issued-cmds (old Keypad)
    extract (inj₁ (() , _))
    extract (inj₂ (_ , _ , result)) = result
  [Ki-new=Ki-old] :
    KeypadLSM.State.input (new Keypad) ≡
    KeypadLSM.State.input (old Keypad)
  [Ki-new=Ki-old] = proj₁ transition-extract
  [Kc-new=Kc-old] :
    KeypadLSM.State.has-control (new Keypad) ≡
    KeypadLSM.State.has-control (old Keypad)
  [Kc-new=Kc-old] = proj₁ (proj₂ transition-extract)
  [Kic-new=Kic-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad)
  [Kic-new=Kic-old] = proj₂ (proj₂ transition-extract)

  [new-Pump=old-Pump] : new Pump ≡ old Pump
  [new-Pump=old-Pump] =
    sym (Step.world-waits [Step-ev-new] Pump λ ())
  [Pc-new=Pc-old] :
    PumpLSM.State.has-control (new Pump) ≡
    PumpLSM.State.has-control (old Pump)
  [Pc-new=Pc-old] =
    cong PumpLSM.State.has-control [new-Pump=old-Pump]
  [Puc-new=Puc-old] :
    PumpLSM.State.unhandled-cmds (new Pump) ≡
    PumpLSM.State.unhandled-cmds (old Pump)
  [Puc-new=Puc-old] =
    cong PumpLSM.State.unhandled-cmds [new-Pump=old-Pump]
  [Phc-new=Phc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump)
  [Phc-new=Phc-old] =
    cong PumpLSM.State.handled-cmds [new-Pump=old-Pump]

  progress :
    Proper run →
    Proper (step (Keypad ⨾ Load64 keybuffer-addr ⨾ ret) new run)
  progress
    (Proper-idle
      (InControl-nothing [Kc-old=false'] [Pc-old=false])
      [obs-Kcon-0]
      [obs-Pcon-0]
      [ProperInput-old]
      [Phc-old=Kic-old]
      [Puc-old=0]
    ) =
    Proper-idle
      (
        InControl-nothing
        (trans [Kc-new=Kc-old] [Kc-old=false'])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (observes-Load64 [obs-Kcon-0])
      (observes-Load64 [obs-Pcon-0])
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (trans [Phc-new=Phc-old]
        (trans [Phc-old=Kic-old] (sym [Kic-new=Kic-old])))
      (trans [Puc-new=Puc-old] [Puc-old=0])

  progress
    (Proper-Keypad-input
      (InControl-Keypad [Kc-old=true] _)
      _ _ _ _ _
    ) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-Keypad-issued
      _ (InControl-Keypad [Kc-old=true] _) _ _ _ _ _ _
    ) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-transfer
      i
      (InControl-nothing [Kc-old=false'] [Pc-old=false])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-transfer
      i
      (
        InControl-nothing
        (trans [Kc-new=Kc-old] [Kc-old=false'])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (observes-Load64 [obs-Kcon-1])
      (observes-Load64 [obs-Pcon-1])
      (observes-Load64 [obs-Pcom-i])

  progress
    (Proper-Pump-input
      i
      (InControl-Pump [Kc-old=false] [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-input
      i
      (
        InControl-Pump
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (observes-Load64 [obs-Kcon-1])
      (observes-Load64 [obs-Pcon-1])
      (observes-Load64 [obs-Pcom-i])

  progress
    (Proper-Pump-saved
      i
      (InControl-Pump [Kc-old=false'] [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=i]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-saved
      i
      (
        InControl-Pump
        (trans [Kc-new=Kc-old] [Kc-old=false'])
        (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=i])
      (observes-Load64 [obs-Kcon-1])
      (observes-Load64 [obs-Pcon-1])
      (observes-Load64 [obs-Pcom-i])

properness-lemma
  (step (Pump ⨾ Load64 control-addr ⨾ ret) new run)
  (Semantics.Permitted-Load64 _ _ [CanLoad64-con-ret] [Permitted-run])
  (Compliant-step [Step-ev-new] [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  -- We extract the equational consequences of compliance.
  guard-extract :
    control-addr ≡ control-addr ×
    PumpLSM.State.has-control (old Pump) ≡ false
  guard-extract = extract (Step.guard-holds [Step-ev-new]) where
    extract :
      (
        control-addr ≡ control-addr ×
        PumpLSM.State.has-control (old Pump) ≡ false
      )
      ⊎
      (
        control-addr ≡ command-addr ×
        PumpLSM.State.has-control (old Pump) ≡ true ×
        PumpLSM.State.unhandled-cmds (old Pump) ≡ []
      ) →
      (
        control-addr ≡ control-addr ×
        PumpLSM.State.has-control (old Pump) ≡ false
      )
    extract (inj₁ x) = x

  [Pc-old=false] : PumpLSM.State.has-control (old Pump) ≡ false
  [Pc-old=false] = proj₂ guard-extract

  transition-extract :
    PumpLSM.State.has-control (new Pump) ≡ (ret == 1) ×
    PumpLSM.State.unhandled-cmds (new Pump) ≡
      PumpLSM.State.unhandled-cmds (old Pump) ×
    PumpLSM.State.handled-cmds (new Pump) ≡
      PumpLSM.State.handled-cmds (old Pump)
  transition-extract = extract (Step.pd-transitions [Step-ev-new]) where
    extract :
      PumpLSM.Transition
        (Load64 control-addr) ret (old Pump) (new Pump) →
      PumpLSM.State.has-control (new Pump) ≡ (ret == 1) ×
      PumpLSM.State.unhandled-cmds (new Pump) ≡
        PumpLSM.State.unhandled-cmds (old Pump) ×
      PumpLSM.State.handled-cmds (new Pump) ≡
        PumpLSM.State.handled-cmds (old Pump)
    extract (inj₁ (_ , result)) = result
    extract (inj₂ (() , _))
  [Pc-new=ret==1] :
    PumpLSM.State.has-control (new Pump) ≡ (ret == 1)
  [Pc-new=ret==1] = proj₁ transition-extract
  [Puc-new=Puc-old] :
    PumpLSM.State.unhandled-cmds (new Pump) ≡
    PumpLSM.State.unhandled-cmds (old Pump)
  [Puc-new=Puc-old] = proj₁ (proj₂ transition-extract)
  [Phc-new=Phc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump)
  [Phc-new=Phc-old] = proj₂ (proj₂ transition-extract)

  [new-Keypad=old-Keypad] : new Keypad ≡ old Keypad
  [new-Keypad=old-Keypad] =
    sym (Step.world-waits [Step-ev-new] Keypad λ ())
  [Ki-new=Ki-old] :
    KeypadLSM.State.input (new Keypad) ≡
    KeypadLSM.State.input (old Keypad)
  [Ki-new=Ki-old] =
    cong KeypadLSM.State.input [new-Keypad=old-Keypad]
  [Kc-new=Kc-old] :
    KeypadLSM.State.has-control (new Keypad) ≡
    KeypadLSM.State.has-control (old Keypad)
  [Kc-new=Kc-old] =
    cong KeypadLSM.State.has-control [new-Keypad=old-Keypad]
  [Kic-new=Kic-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad)
  [Kic-new=Kic-old] =
    cong KeypadLSM.State.issued-cmds [new-Keypad=old-Keypad]

  progress :
    Proper run →
    Proper (step (Pump ⨾ Load64 control-addr ⨾ ret) new run)

  progress
    (Proper-idle
      (InControl-nothing [Kc-old=false] _)
      [obs-Kcon-0]
      [obs-Pcon-0]
      [ProperInput-old]
      [Phc-old=Kic-old]
      [Puc-old=0]
    ) =
    Proper-idle
      (
        InControl-nothing
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (
          trans [Pc-new=ret==1]
          (
            cong (λ x → x == 1)
            (Observes.must-observe [obs-Pcon-0] ret [CanLoad64-con-ret])
          )
        )
      )
      (observes-Load64 [obs-Kcon-0])
      (observes-Load64 [obs-Pcon-0])
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        [Phc-new=Phc-old]
        (trans [Phc-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])

  progress
    (Proper-Keypad-input
      (InControl-Keypad [Kc-old=true] _)
      [ProperInputNE-old]
      [Phc-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-0]
      [obs-Pcon-0]
    ) =
    Proper-Keypad-input
      (
        InControl-Keypad
          (trans [Kc-new=Kc-old] [Kc-old=true])
          (
            trans [Pc-new=ret==1]
            (
              cong (λ x → x == 1)
              (Observes.must-observe [obs-Pcon-0] ret [CanLoad64-con-ret])
            )
          )
      )
      (subst ProperInputNE (sym [Ki-new=Ki-old]) [ProperInputNE-old])
      (trans [Phc-new=Phc-old]
        (trans [Phc-old=Kic-old] (sym [Kic-new=Kic-old])))
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (observes-Load64 [obs-Kcon-0])
      (observes-Load64 [obs-Pcon-0])

  progress
    (Proper-Keypad-issued
      i
      (InControl-Keypad [Kc-old=true] _)
      [ProperInputE-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-0]
      [obs-Pcon-0]
      [obs-Pcom-i]
    ) =
    Proper-Keypad-issued
      i
      (
        InControl-Keypad
        (trans [Kc-new=Kc-old] [Kc-old=true])
        (
          trans
          [Pc-new=ret==1]
          (
            cong (λ x → x == 1)
            (Observes.must-observe [obs-Pcon-0] ret [CanLoad64-con-ret])
          )
        )
      )
      (subst ProperInputE (sym [Ki-new=Ki-old]) [ProperInputE-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (observes-Load64 [obs-Kcon-0])
      (observes-Load64 [obs-Pcon-0])
      (observes-Load64 [obs-Pcom-i])

  progress
    (Proper-transfer
      i
      (InControl-nothing [Kc-old=false] _)
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-input
      i
      (
        InControl-Pump
        (trans [Kc-new=Kc-old] [Kc-old=false])
        (
          trans [Pc-new=ret==1]
          (
            cong (λ x → x == 1)
            (Observes.must-observe [obs-Pcon-1] ret [CanLoad64-con-ret])
          )
        )
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (observes-Load64 [obs-Kcon-1])
      (observes-Load64 [obs-Pcon-1])
      (observes-Load64 [obs-Pcom-i])

  progress
    (Proper-Pump-input
      _ (InControl-Pump _ [Pc-old=true]) _ _ _ _ _ _
    ) =
    true-and-false [Pc-old=true] [Pc-old=false]

  progress
    (Proper-Pump-saved
      _ (InControl-Pump _ [Pc-old=true]) _ _ _ _ _ _
    ) =
    true-and-false [Pc-old=true] [Pc-old=false]

properness-lemma
  (step (Pump ⨾ Load64 command-addr ⨾ ret) new run)
  (Semantics.Permitted-Load64 _ _ [CanLoad64-com-ret] [Permitted-run])
  (Compliant-step [Step-ev-new] [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  guard-extract :
    PumpLSM.State.has-control (old Pump) ≡ true ×
    PumpLSM.State.unhandled-cmds (old Pump) ≡ []
  guard-extract = extract (Step.guard-holds [Step-ev-new]) where
    extract :
      PumpLSM.Guard (Load64 command-addr) (old Pump) →
      PumpLSM.State.has-control (old Pump) ≡ true ×
      PumpLSM.State.unhandled-cmds (old Pump) ≡ []
    extract (inj₁ (() , _))
    extract (inj₂ (_ , result)) = result
  [Pc-old=true] : PumpLSM.State.has-control (old Pump) ≡ true
  [Pc-old=true] = proj₁ guard-extract
  [Puc-old=0] : PumpLSM.State.unhandled-cmds (old Pump) ≡ []
  [Puc-old=0] = proj₂ guard-extract

  transition-extract :
    PumpLSM.State.has-control (new Pump) ≡
      PumpLSM.State.has-control (old Pump) ×
    PumpLSM.State.unhandled-cmds (new Pump) ≡ (ret ∷ []) ×
    PumpLSM.State.handled-cmds (new Pump) ≡
      PumpLSM.State.handled-cmds (old Pump)
  transition-extract = extract (Step.pd-transitions [Step-ev-new]) where
    extract :
      PumpLSM.Transition
        (Load64 command-addr) ret (old Pump) (new Pump) →
      PumpLSM.State.has-control (new Pump) ≡
        PumpLSM.State.has-control (old Pump) ×
      PumpLSM.State.unhandled-cmds (new Pump) ≡ (ret ∷ []) ×
      PumpLSM.State.handled-cmds (new Pump) ≡
        PumpLSM.State.handled-cmds (old Pump)
    extract (inj₁ (() , _))
    extract (inj₂ (_ , result)) = result
  [Pc-new=Pc-old] :
    PumpLSM.State.has-control (new Pump) ≡
    PumpLSM.State.has-control (old Pump)
  [Pc-new=Pc-old] = proj₁ transition-extract
  [Puc-new=ret] :
    PumpLSM.State.unhandled-cmds (new Pump) ≡ (ret ∷ [])
  [Puc-new=ret] = proj₁ (proj₂ transition-extract)
  [Phc-new=Phc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump)
  [Phc-new=Phc-old] = proj₂ (proj₂ transition-extract)

  [new-Keypad=old-Keypad] : new Keypad ≡ old Keypad
  [new-Keypad=old-Keypad] =
    sym (Step.world-waits [Step-ev-new] Keypad λ ())
  [Ki-new=Ki-old] :
    KeypadLSM.State.input (new Keypad) ≡
    KeypadLSM.State.input (old Keypad)
  [Ki-new=Ki-old] =
    cong KeypadLSM.State.input [new-Keypad=old-Keypad]
  [Kc-new=Kc-old] :
    KeypadLSM.State.has-control (new Keypad) ≡
    KeypadLSM.State.has-control (old Keypad)
  [Kc-new=Kc-old] =
    cong KeypadLSM.State.has-control [new-Keypad=old-Keypad]
  [Kic-new=Kic-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad)
  [Kic-new=Kic-old] =
    cong KeypadLSM.State.issued-cmds [new-Keypad=old-Keypad]

  progress :
    Proper run →
    Proper (step (Pump ⨾ Load64 command-addr ⨾ ret) new run)

  progress
    (Proper-idle (InControl-nothing _ [Pc-old=false]) _ _ _ _ _) =
    true-and-false [Pc-old=true] [Pc-old=false]

  progress
    (Proper-Keypad-input (InControl-Keypad _ [Pc-old=false]) _ _ _ _ _) =
    true-and-false [Pc-old=true] [Pc-old=false]

  progress
    (Proper-Keypad-issued _ (InControl-Keypad _ [Pc-old=false]) _ _ _ _ _ _) =
    true-and-false [Pc-old=true] [Pc-old=false]

  progress
    (Proper-transfer _ (InControl-nothing _ [Pc-old=false]) _ _ _ _ _ _) =
    true-and-false [Pc-old=true] [Pc-old=false]

  progress
    (Proper-Pump-input
      i
      (InControl-Pump [Kc-old=false] [Pc-old=true])
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    Proper-Pump-saved
      i
      (
        InControl-Pump
          (trans [Kc-new=Kc-old] [Kc-old=false])
          (trans [Pc-new=Pc-old] [Pc-old=true])
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (
        trans
        [Puc-new=ret]
        (
          cong (λ x → x ∷ [])
          (Observes.must-observe [obs-Pcom-i] ret [CanLoad64-com-ret])
        )
      )
      (observes-Load64 [obs-Kcon-1])
      (observes-Load64 [obs-Pcon-1])
      (observes-Load64 [obs-Pcom-i])

  progress
    (Proper-Pump-saved i _ _ _ [Puc-old=i] _ _ _) =
    ⊥-elim ([0≠i] [0=i]) where
    [0≠i] : [] ≢ (i ∷ [])
    [0≠i] ()
    [0=i] : [] ≡ (i ∷ [])
    [0=i] = trans (sym [Puc-old=0]) [Puc-old=i]

properness-lemma
  (step (Pump ⨾ Load64 keybuffer-addr ⨾ _) _ run)
  _
  (Compliant-step step-ev-sm [Compliant-run]) =
  ⊥-elim (impossible (Step.guard-holds step-ev-sm)) where
  old : StateMap
  old = stateMap run
  impossible : PumpLSM.Guard (Load64 keybuffer-addr) (old Pump) → ⊥
  impossible (inj₁ ())
  impossible (inj₂ ())

properness-lemma
  (step (Keypad ⨾ Store64 control-addr val ⨾ ret) new run)
  (Semantics.Permitted-Store64 x [Permitted-run])
  (Compliant-step [Step-ev-new] [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  extract-guard :
    val ≡ 1 ×
    KeypadLSM.State.input (old Keypad) ≡ [] ×
    KeypadLSM.State.has-control (old Keypad) ≡ true
  extract-guard = extract (Step.guard-holds [Step-ev-new]) where
    extract :
      KeypadLSM.Guard (Store64 control-addr val) (old Keypad) →
      val ≡ 1 ×
      KeypadLSM.State.input (old Keypad) ≡ [] ×
      KeypadLSM.State.has-control (old Keypad) ≡ true
    extract (inj₁ (_ , result)) = result
    extract (inj₂ (() , _))
  [val=1] : val ≡ 1
  [val=1] = proj₁ extract-guard
  [Ki-old=0] : KeypadLSM.State.input (old Keypad) ≡ []
  [Ki-old=0] = proj₁ (proj₂ extract-guard)
  [Kc-old=true] : KeypadLSM.State.has-control (old Keypad) ≡ true
  [Kc-old=true] = proj₂ (proj₂ extract-guard)

  extract-transition :
    KeypadLSM.State.input (new Keypad) ≡
      KeypadLSM.State.input (old Keypad) ×
    KeypadLSM.State.has-control (new Keypad) ≡ false ×
    KeypadLSM.State.issued-cmds (new Keypad) ≡
      KeypadLSM.State.issued-cmds (old Keypad)
  extract-transition = extract (Step.pd-transitions [Step-ev-new]) where
    extract :
      KeypadLSM.Transition
        (Store64 control-addr val) ret (old Keypad) (new Keypad) →
      KeypadLSM.State.input (new Keypad) ≡
        KeypadLSM.State.input (old Keypad) ×
      KeypadLSM.State.has-control (new Keypad) ≡ false ×
      KeypadLSM.State.issued-cmds (new Keypad) ≡
        KeypadLSM.State.issued-cmds (old Keypad)
    extract (inj₁ (_ , _ , result)) = result
    extract (inj₂ (() , _))
  [Ki-new=Ki-old] :
    KeypadLSM.State.input (new Keypad) ≡
    KeypadLSM.State.input (old Keypad)
  [Ki-new=Ki-old] = proj₁ extract-transition
  [Kc-new=false] : KeypadLSM.State.has-control (new Keypad) ≡ false
  [Kc-new=false] = proj₁ (proj₂ extract-transition)
  [Kic-new=Kic-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad)
  [Kic-new=Kic-old] = proj₂ (proj₂ extract-transition)

  [new-Pump=old-Pump] : new Pump ≡ old Pump
  [new-Pump=old-Pump] =
    sym (Step.world-waits [Step-ev-new] Pump λ ())
  [Pc-new=Pc-old] :
    PumpLSM.State.has-control (new Pump) ≡
    PumpLSM.State.has-control (old Pump)
  [Pc-new=Pc-old] =
    cong PumpLSM.State.has-control [new-Pump=old-Pump]
  [Puc-new=Puc-old] :
    PumpLSM.State.unhandled-cmds (new Pump) ≡
    PumpLSM.State.unhandled-cmds (old Pump)
  [Puc-new=Puc-old] =
    cong PumpLSM.State.unhandled-cmds [new-Pump=old-Pump]
  [Phc-new=Phc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump)
  [Phc-new=Phc-old] =
    cong PumpLSM.State.handled-cmds [new-Pump=old-Pump]

  progress :
    Proper run →
    Proper (step (Keypad ⨾ Store64 control-addr val ⨾ ret) new run)

  progress
    (Proper-idle (InControl-nothing [Kc-old=false] _) _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-Keypad-input _ [ProperInputNE-old] _ _ _ _) =
    ⊥-elim ([Ki-old≠0] [Ki-old=0]) where
    [ProperInputNE-xs→xs≠0] :
      ∀ {xs : List ℕ} → ProperInputNE xs → xs ≢ []
    [ProperInputNE-xs→xs≠0] ProperInputNE-[1] ()
    [ProperInputNE-xs→xs≠0] ProperInputNE-[2] ()
    [Ki-old≠0] : KeypadLSM.State.input (old Keypad) ≢ []
    [Ki-old≠0] = [ProperInputNE-xs→xs≠0] [ProperInputNE-old]

  progress
    (Proper-Keypad-issued
      i
      (InControl-Keypad _ [Pc-old=false])
      _
      [Phc++i-old=Kic-old]
      [Puc-old=0]
      _
      _
      [obs-Pcom-i]
    ) =
    Proper-transfer
      i
      (
        InControl-nothing
        [Kc-new=false]
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (
        subst ProperInput
        (sym (trans [Ki-new=Ki-old] [Ki-old=0]))
        ProperInput-[]
      )
      (
        trans
        (cong (λ xs → xs ++ i ∷ []) [Phc-new=Phc-old])
        (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
      )
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (
        observes
        (
          Semantics.CanLoad64-Store64
          Keypad control-addr val refl [val=1] new run
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Store64
          Keypad control-addr val refl [val=1] new run
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Store64-skip
          Keypad control-addr val (λ ()) new run
          (Observes.can-observe [obs-Pcom-i])
        )
      )

  progress
    (Proper-transfer _ (InControl-nothing [Kc-old=false] _) _ _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-Pump-input _ (InControl-Pump [Kc-old=false] _) _ _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-Pump-saved _ (InControl-Pump [Kc-old=false] _) _ _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]

properness-lemma
  (step (Keypad ⨾ Store64 command-addr val ⨾ ret) new run)
  (Semantics.Permitted-Store64 _ [Permitted-run])
  (Compliant-step [Step-ev-new] [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  extract-guard :
    KeypadLSM.State.input (old Keypad) ≡ (val ∷ []) ×
    KeypadLSM.State.has-control (old Keypad) ≡ true
  extract-guard = extract (Step.guard-holds [Step-ev-new]) where
    extract :
      KeypadLSM.Guard (Store64 command-addr val) (old Keypad) →
      KeypadLSM.State.input (old Keypad) ≡ (val ∷ []) ×
      KeypadLSM.State.has-control (old Keypad) ≡ true
    extract (inj₁ (() , _))
    extract (inj₂ (_ , result)) = result
  [Ki-old=val] : KeypadLSM.State.input (old Keypad) ≡ (val ∷ [])
  [Ki-old=val] = proj₁ extract-guard
  [Kc-old=true] : KeypadLSM.State.has-control (old Keypad) ≡ true
  [Kc-old=true] = proj₂ extract-guard

  extract-transition :
    KeypadLSM.State.input (new Keypad) ≡ [] ×
    (
      KeypadLSM.State.has-control (new Keypad) ≡
      KeypadLSM.State.has-control (old Keypad)
    ) ×
    (
      KeypadLSM.State.issued-cmds (new Keypad) ≡
      KeypadLSM.State.issued-cmds (old Keypad) ++
      KeypadLSM.State.input (old Keypad)
    )
  extract-transition = extract (Step.pd-transitions [Step-ev-new]) where
    extract :
      KeypadLSM.Transition
        (Store64 command-addr val) ret (old Keypad) (new Keypad) →
      KeypadLSM.State.input (new Keypad) ≡ [] ×
      KeypadLSM.State.has-control (new Keypad) ≡
        KeypadLSM.State.has-control (old Keypad) ×
      KeypadLSM.State.issued-cmds (new Keypad) ≡
        KeypadLSM.State.issued-cmds (old Keypad) ++
        KeypadLSM.State.input (old Keypad)
    extract (inj₁ (() , _))
    extract (inj₂ (_ , result)) = result
  [Ki-new=0] : KeypadLSM.State.input (new Keypad) ≡ []
  [Ki-new=0] = proj₁ extract-transition
  [Kc-new=Kc-old] :
    KeypadLSM.State.has-control (new Keypad) ≡
    KeypadLSM.State.has-control (old Keypad)
  [Kc-new=Kc-old] = proj₁ (proj₂ extract-transition)
  [Kic-new=Kic-old++Ki-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad) ++
    KeypadLSM.State.input (old Keypad)
  [Kic-new=Kic-old++Ki-old] = proj₂ (proj₂ extract-transition)

  [new-Pump=old-Pump] : new Pump ≡ old Pump
  [new-Pump=old-Pump] =
    sym (Step.world-waits [Step-ev-new] Pump λ ())
  [Pc-new=Pc-old] :
    PumpLSM.State.has-control (new Pump) ≡
    PumpLSM.State.has-control (old Pump)
  [Pc-new=Pc-old] =
    cong PumpLSM.State.has-control [new-Pump=old-Pump]
  [Puc-new=Puc-old] :
    PumpLSM.State.unhandled-cmds (new Pump) ≡
    PumpLSM.State.unhandled-cmds (old Pump)
  [Puc-new=Puc-old] =
    cong PumpLSM.State.unhandled-cmds [new-Pump=old-Pump]
  [Phc-new=Phc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump)
  [Phc-new=Phc-old] =
    cong PumpLSM.State.handled-cmds [new-Pump=old-Pump]

  progress :
    Proper run →
    Proper (step (Keypad ⨾ Store64 command-addr val ⨾ ret) new run)

  progress
    (Proper-idle (InControl-nothing [Kc-old=false] _) _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-Keypad-input
      (InControl-Keypad _ [Pc-old=false])
      _
      [Phc-old=Kic-old]
      [Puc-old=0]
      [obs-Kcon-0]
      [obs-Pcon-0]
    ) =
    Proper-Keypad-issued
      val
      (
        InControl-Keypad
        (trans [Kc-new=Kc-old] [Kc-old=true])
        (trans [Pc-new=Pc-old] [Pc-old=false])
      )
      (subst ProperInputE (sym [Ki-new=0]) ProperInputE-[])
      [Phc++val-new=Ki-new]
      (trans [Puc-new=Puc-old] [Puc-old=0])
      (
        observes
        (
          Semantics.CanLoad64-Store64-skip
          Keypad command-addr val (λ ()) new run
          (Observes.can-observe [obs-Kcon-0])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Store64-skip
          Keypad command-addr val (λ ()) new run
          (Observes.can-observe [obs-Pcon-0])
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Store64
          Keypad command-addr val refl refl new run
        )
      ) where
    -- We prove [Phc++val-new=Ki-new] by 3 steps of equational reasoning.
    substep1 :
      PumpLSM.State.handled-cmds (new Pump) ++ (val ∷ []) ≡
      PumpLSM.State.handled-cmds (old Pump) ++ (val ∷ [])
    substep1 = cong (λ xs → xs ++ val ∷ []) [Phc-new=Phc-old]
    substep2 :
      PumpLSM.State.handled-cmds (old Pump) ++ (val ∷ []) ≡
      KeypadLSM.State.issued-cmds (old Keypad) ++ (val ∷ [])
    substep2 = cong (λ xs → xs ++ val ∷ []) [Phc-old=Kic-old]
    substep3 :
      (
        KeypadLSM.State.issued-cmds (old Keypad) ++
        (val ∷ [])
      )
      ≡
      (
        KeypadLSM.State.issued-cmds (old Keypad) ++
        KeypadLSM.State.input (old Keypad)
      )
    substep3 =
      cong
      (λ xs → KeypadLSM.State.issued-cmds (old Keypad) ++ xs)
      (sym [Ki-old=val])
    [Phc++val-new=Ki-new] :
      PumpLSM.State.handled-cmds (new Pump) ++ (val ∷ []) ≡
      KeypadLSM.State.issued-cmds (new Keypad)
    [Phc++val-new=Ki-new] =
      trans substep1
      (
        trans substep2
        (
          trans substep3
          (sym [Kic-new=Kic-old++Ki-old])
        )
      )

  progress (Proper-Keypad-issued _ _ [ProperInputE-old] _ _ _ _ _) =
    ⊥-elim ([val≠0] [val=0]) where
    [ProperInputE-xs→xs=0] :
      {xs : List ℕ} → ProperInputE xs → xs ≡ []
    [ProperInputE-xs→xs=0] ProperInputE-[] = refl
    [Ki-old=0] : Keypad-input run ≡ []
    [Ki-old=0] = [ProperInputE-xs→xs=0] [ProperInputE-old]
    [val≠0] : (val ∷ []) ≢ []
    [val≠0] ()
    [val=0] : (val ∷ []) ≡ []
    [val=0] = trans (sym [Ki-old=val]) [Ki-old=0]

  progress
    (Proper-transfer _ (InControl-nothing [Kc-old=false] _) _ _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-Pump-input _ (InControl-Pump [Kc-old=false] _) _ _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]

  progress
    (Proper-Pump-saved _ (InControl-Pump [Kc-old=false] _) _ _ _ _ _ _) =
    true-and-false [Kc-old=true] [Kc-old=false]

properness-lemma
  (step (Keypad ⨾ Store64 keybuffer-addr _ ⨾ _) _ _)
  (Semantics.Permitted-Store64 perm-r _)
  _ = ⊥-elim perm-r

properness-lemma
  (step (Pump ⨾ Store64 control-addr val ⨾ ret) new run)
  (Semantics.Permitted-Store64 x [Permitted-run])
  (Compliant-step [Step-ev-new] [Compliant-run]) =
  progress [Proper-run] where
  -- By induction, the rest of the run landed us in a proper state.
  [Proper-run] : Proper run
  [Proper-run] = properness-lemma run [Permitted-run] [Compliant-run]
  old : StateMap
  old = stateMap run

  -- The usual extraction, fortunately we don't need to extract from any
  -- guards here.
  [val=0] : val ≡ 0
  [val=0] = proj₁ (proj₂ (Step.guard-holds [Step-ev-new]))
  [Pc-old=true] : PumpLSM.State.has-control (old Pump) ≡ true
  [Pc-old=true] = proj₁ (proj₂ (proj₂ (Step.guard-holds [Step-ev-new])))
  [Puc-old≠0] : PumpLSM.State.unhandled-cmds (old Pump) ≢ []
  [Puc-old≠0] = proj₂ (proj₂ (proj₂ (Step.guard-holds [Step-ev-new])))

  [Pc-new=val==1] :
    PumpLSM.State.has-control (new Pump) ≡ (val == 1)
  [Pc-new=val==1] = proj₁ (Step.pd-transitions [Step-ev-new])
  [Puc-new=0] : PumpLSM.State.unhandled-cmds (new Pump) ≡ []
  [Puc-new=0] = proj₁ (proj₂ (Step.pd-transitions [Step-ev-new]))
  [Phc-new=Phc-old++Puc-old] :
    PumpLSM.State.handled-cmds (new Pump) ≡
    PumpLSM.State.handled-cmds (old Pump) ++
    PumpLSM.State.unhandled-cmds (old Pump)
  [Phc-new=Phc-old++Puc-old] = proj₂ (proj₂ (Step.pd-transitions [Step-ev-new]))

  [new-Keypad=old-Keypad] : new Keypad ≡ old Keypad
  [new-Keypad=old-Keypad] =
    sym (Step.world-waits [Step-ev-new] Keypad λ ())
  [Ki-new=Ki-old] :
    KeypadLSM.State.input (new Keypad) ≡
    KeypadLSM.State.input (old Keypad)
  [Ki-new=Ki-old] =
    cong KeypadLSM.State.input [new-Keypad=old-Keypad]
  [Kc-new=Kc-old] :
    KeypadLSM.State.has-control (new Keypad) ≡
    KeypadLSM.State.has-control (old Keypad)
  [Kc-new=Kc-old] =
    cong KeypadLSM.State.has-control [new-Keypad=old-Keypad]
  [Kic-new=Kic-old] :
    KeypadLSM.State.issued-cmds (new Keypad) ≡
    KeypadLSM.State.issued-cmds (old Keypad)
  [Kic-new=Kic-old] =
    cong KeypadLSM.State.issued-cmds [new-Keypad=old-Keypad]

  progress :
    Proper run →
    Proper (step (Pump ⨾ Store64 control-addr val ⨾ ret) new run)
  -- Most of these cannot happen.
  progress
    (Proper-idle (InControl-nothing _ [Pc-old=false]) _ _ _ _ _) =
    true-and-false [Pc-old=true] [Pc-old=false]
    
  progress
    (Proper-Keypad-input (InControl-Keypad _ [Pc-old=false]) _ _ _ _ _) =
    true-and-false [Pc-old=true] [Pc-old=false]

  progress
    (Proper-Keypad-issued _ (InControl-Keypad _ [Pc-old=false]) _ _ _ _ _ _) =
    true-and-false [Pc-old=true] [Pc-old=false]

  progress
    (Proper-transfer _ (InControl-nothing _ [Pc-old=false]) _ _ _ _ _ _) =
    true-and-false [Pc-old=true] [Pc-old=false]

  progress
    (Proper-Pump-input _ _ _ _ [Puc-old=0] _ _ _) =
    ⊥-elim ([Puc-old≠0] [Puc-old=0])

  progress
    (Proper-Pump-saved
      i
      (InControl-Pump [Kc-old=false] _)
      [ProperInput-old]
      [Phc++i-old=Kic-old]
      [Puc-old=i]
      [obs-Kcon-1]
      [obs-Pcon-1]
      [obs-Pcom-i]
    ) =
    -- We finally made it. The `Pump` PD only ever writes to `control-addr`
    -- when it is done with handling, and transitions the system back to
    -- idle... at least until the next keypress.
    Proper-idle
      (
        InControl-nothing
          (trans [Kc-new=Kc-old] [Kc-old=false])
          (
            trans
            [Pc-new=val==1]
            (cong (λ x → x == 1) [val=0])
          )
      )
      (
        observes
        (
          Semantics.CanLoad64-Store64
          Pump control-addr val refl [val=0] new run
        )
      )
      (
        observes
        (
          Semantics.CanLoad64-Store64
          Pump control-addr val refl [val=0] new run
        )
      )
      (subst ProperInput (sym [Ki-new=Ki-old]) [ProperInput-old])
      (
        trans
        [Phc-new=Phc-old++Puc-old]
        (
          trans
          (
            cong
            (λ xs → PumpLSM.State.handled-cmds (old Pump) ++ xs)
            [Puc-old=i]
          )
          (trans [Phc++i-old=Kic-old] (sym [Kic-new=Kic-old]))
        )
      )
      [Puc-new=0]

properness-lemma
  (step (Pump ⨾ Store64 command-addr _ ⨾ _) _ _)
  (Semantics.Permitted-Store64 perm-r _)
  _ = ⊥-elim perm-r

properness-lemma
  (step (Pump ⨾ Store64 keybuffer-addr _ ⨾ _) _ _)
  (Semantics.Permitted-Store64 perm-none _)
  _ = ⊥-elim perm-none

global-correctness :
  ∀ run →
  Semantics.Permitted run →
  Compliant run →
  Correct run
global-correctness run perun corun =
  correctness-lemma (properness-lemma run perun corun)
