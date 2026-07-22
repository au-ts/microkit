module KeypadLSM where

open import Data.Bool
open import Data.Empty
open import Data.List
open import Data.Nat
open import Data.Product
open import Data.Sum
open import Data.Unit

open import Types
open import Relation.Binary.PropositionalEquality

{-
  Here we define the local state machines of the Keypad PD, along with its guard
  and transition relations. These should correspond to (and ideally be derived
  automatically from) the contents of `spec-for-keypad.vpr`.
-}

record State : Set where
  field
    input : List ℕ
    has-control : Bool
    issued-cmds : List ℕ

Init : State → Set
--^ Holds if Keypad may have the given state as its initial state.
Init s =
  input ≡ [] ×
  has-control ≡ false ×
  issued-cmds ≡ []
  where open State s

Guard : Call → State → Set
--^ Holds if Keypad may emit the given microkit event in the given state.
Guard (Notify _) _ = ⊥
Guard Recv s =
  input ≡ [] ×
  has-control ≡ false
  where open State s
Guard (Load64 address) s =
  (
    address ≡ control-addr ×
    input ≢ [] ×
    has-control ≡ false
  )
  ⊎
  (
    address ≡ keybuffer-addr ×
    input ≢ [] ×
    has-control ≡ false
  )
  where open State s
Guard (Store64 address value) s =
  (
    address ≡ control-addr ×
    value ≡ 1 ×
    input ≡ [] ×
    has-control ≡ true
  )
  ⊎
  (
    address ≡ command-addr ×
    input ≡ (value ∷ []) ×
    has-control ≡ true
  )
  where open State s

Transition : (c : Call) → Ret c → (old : State) → (new : State) → Set
--^ Holds if Keypad may transition from the first given state to the second one
--upon emitting the given event call.
Transition (Notify _) tt _ _ = ⊥
Transition Recv tt old new =
  (input new ≡ (1 ∷ []) ⊎ input new ≡ (2 ∷ [])) ×
  has-control new ≡ has-control old ×
  issued-cmds new ≡ issued-cmds old
  where open State
Transition (Load64 address) retval old new =
  (
    address ≡ control-addr ×
    input new ≡ input old ×
    has-control new ≡ (retval == 0) ×
    issued-cmds new ≡ issued-cmds old
  )
  ⊎
  (
    address ≡ keybuffer-addr ×
    (retval ∷ []) ≡ input new ×
    input new ≡ input old ×
    has-control new ≡ has-control old ×
    issued-cmds new ≡ issued-cmds old
  )
  where open State
Transition (Store64 address value) tt old new =
  (
    address ≡ control-addr ×
    value ≡ 1 ×
    input new ≡ input old ×
    has-control new ≡ false ×
    issued-cmds new ≡ issued-cmds old
  )
  ⊎
  (
    address ≡ command-addr ×
    input new ≡ [] ×
    has-control new ≡ has-control old ×
    issued-cmds new ≡ issued-cmds old ++ input old
  )
  where open State
