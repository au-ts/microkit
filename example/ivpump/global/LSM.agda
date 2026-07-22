--
-- Copyright 2026, UNSW
--
-- SPDX-License-Identifier: BSD-2-Clause
--

module LSM where

open import Data.Bool
open import Data.Empty
open import Data.List
open import Data.Nat
open import Data.Product
open import Data.Sum
open import Data.Unit

open import Types
open import Relation.Binary.PropositionalEquality

import KeypadLSM
import PumpLSM

{-
  Here we define the init, guard and transition relations generically over PDs.
-}

State : PD → Set
State Keypad = KeypadLSM.State
State Pump = PumpLSM.State

Init : (pd : PD) → State pd → Set
Init Keypad = KeypadLSM.Init
Init Pump = PumpLSM.Init

Guard : (pd : PD) → Call → State pd → Set
Guard Keypad = KeypadLSM.Guard
Guard Pump = PumpLSM.Guard

Transition : (pd : PD) → (c : Call) → Ret c → State pd → State pd → Set
Transition Keypad = KeypadLSM.Transition
Transition Pump = PumpLSM.Transition
