module Trace where

open import Relation.Binary.PropositionalEquality

open import Types
import LSM

{-
  We define `Run`s: traces of events in logical order, along with the current
  combined state of all the PD LSMs. These `Run`s need not uphold the Microkit
  semantics, nor do they need to comply with the user's specification.

  We separately define what it means for a `Run` to be compliant with a spec in
  this file, and later on what it means for it to obey the Microkit semantics.
-}

StateMap : Set
StateMap = (pd : PD) → LSM.State pd

data Run : Set where
  init : (i : StateMap) → Run
  step : (ev : Event) → (sm : StateMap) → (rest : Run) → Run

stateMap : Run → StateMap
stateMap (init i) = i
stateMap (step _ sm _) = sm

Init : StateMap → Set
Init sm = ∀ (pd : PD) → LSM.Init pd (sm pd)

record Step
--^ A step is valid if it upholds the user-provided guards and transition.
  (ev : Event)
  (old : StateMap)
  (new : StateMap) : Set where
  open Event ev
  field
    guard-holds :
      LSM.Guard pd call (old pd)
    pd-transitions :
      LSM.Transition pd call ret (old pd) (new pd)
    world-waits :
      ∀ (other : PD) → other ≢ pd → old other ≡ new other

data Compliant : Run → Set where
--^ A `Run` is compliant if it complies with the user-provided
--specification consisting of guards, transitions and initial conditions
--at every step.
  Compliant-init :
    {sm : StateMap} →
    (init-sm : Init sm) →
    Compliant (init sm)
  Compliant-step :
    {ev : Event} →
    {sm : StateMap} →
    {rest : Run} →
    (step-ev-sm : Step ev (stateMap rest) sm) →
    (compliant-rest : Compliant rest) →
    Compliant (step ev sm rest)
