module PumpLSM where

--23456789012345678901234567890123456789001234567890123456789012345678901234567890

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
  Here we define the local state machines of the Pump PD, along with its guard
  and transition relations. These should correspond to (and ideally be derived
  automatically from) the contents of `spec-for-pump.vpr`.
-}

record State : Set where
  field
    has-control : Bool
    unhandled-cmds : List ℕ
    handled-cmds : List ℕ

Init : State → Set
--^ Holds if Pump may have the given state as its initial state.
Init s =
  has-control ≡ false ×
  unhandled-cmds ≡ [] ×
  handled-cmds ≡ []
  where open State s

Guard : Call → State → Set
--^ Holds if Pump may emit the given Microkit event in the given state.
Guard (Notify _) _ = ⊤
Guard Recv _ = ⊥
Guard (Load64 address) s =
  (
    address ≡ control-addr ×
    has-control ≡ false
  )
  ⊎
  (
    address ≡ command-addr ×
    has-control ≡ true ×
    unhandled-cmds ≡ []
  )
  where open State s
Guard (Store64 address value) s =
  address ≡ control-addr ×
  value ≡ 0 ×
  has-control ≡ true ×
  unhandled-cmds ≢ []
  where open State s

Transition : (c : Call) → Ret c → (old : State) → (new : State) → Set
--^ Holds if Pump may transition from the first given state to the second one
--upon emitting the given event call.
Transition (Notify _) tt old new =
  has-control new ≡ has-control old ×
  unhandled-cmds new ≡ unhandled-cmds old ×
  handled-cmds new ≡ handled-cmds old
  where open State
Transition Recv tt old new = ⊥
Transition (Load64 address) retval old new =
  (
    address ≡ control-addr ×
    has-control new ≡ (retval == 1) ×
    unhandled-cmds new ≡ unhandled-cmds old ×
    handled-cmds new ≡ handled-cmds old
  )
  ⊎
  (
    address ≡ command-addr ×
    has-control new ≡ has-control old ×
    unhandled-cmds new ≡ (retval ∷ []) ×
    handled-cmds new ≡ handled-cmds old
  )
  where open State
Transition (Store64 address value) r old new =
  has-control new ≡ (value == 1) ×
  unhandled-cmds new ≡ [] ×
  handled-cmds new ≡ handled-cmds old ++ unhandled-cmds old
  where open State
