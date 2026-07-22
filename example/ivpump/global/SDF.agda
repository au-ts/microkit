--
-- Copyright 2026, UNSW
--
-- SPDX-License-Identifier: BSD-2-Clause
--

module SDF where

open import Types
open import Data.Maybe
open import Data.Product
open import Data.Unit
open import Data.Empty
open import Relation.Binary.PropositionalEquality

{-
  The following data would come from an SDF export, specific to the system. In
  this example, `is-notify-target`, `mem-readable` and `mem-writeable` correspond
  to the actual contents of the `caps-for-PD.vpr` file, modulo the fact that we
  do not model addresses explicitly as words in the Agda code.
-}

other-end : PD → Channel → Maybe (PD × Channel)
other-end Keypad Ch01 = nothing
other-end Keypad Ch42 = nothing
other-end Pump Ch01 = nothing
other-end Pump Ch42 = just (Keypad , Ch01)

other-end-pd : PD → Channel → Maybe PD
other-end-pd Keypad Ch01 = nothing
other-end-pd Keypad Ch42 = nothing
other-end-pd Pump Ch01 = nothing
other-end-pd Pump Ch42 = just Keypad

is-notify-target : PD → Channel → Set
is-notify-target pd ch = other-end pd ch ≢ nothing

mem-readable : PD → Address → Set
mem-readable Keypad control-addr = ⊤
mem-readable Keypad command-addr = ⊤
mem-readable Keypad keybuffer-addr = ⊤
mem-readable Pump control-addr = ⊤
mem-readable Pump command-addr = ⊤
mem-readable Pump keybuffer-addr = ⊥

mem-writeable : PD → Address → Set
mem-writeable Keypad control-addr = ⊤
mem-writeable Keypad command-addr = ⊤
mem-writeable Keypad keybuffer-addr = ⊥
mem-writeable Pump control-addr = ⊤
mem-writeable Pump command-addr = ⊥
mem-writeable Pump keybuffer-addr = ⊥

mem-alias : (PD × Address) → (PD × Address) → Set
--^ Holds if the given PD/Address pairs refer to the same physical memory
-- location when considered in the virtual address spaces of the respective
-- PDs. Reduces to equality in this example, since our SDF maps every MR
-- into a fixed location in all PDs.
mem-alias (_ , a) (_ , b) = a ≡ b

mem-is-regular : PD → Address → Set
--^ Holds only if the given PD/Address pair refers to regular / non-device
-- memory.
mem-is-regular _ control-addr = ⊤
mem-is-regular _ command-addr = ⊤
mem-is-regular _ keybuffer-addr = ⊥
