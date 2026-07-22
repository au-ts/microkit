module Types where

--23456789012345678901234567890123456789001234567890123456789012345678901234567890

open import Data.Nat
open import Data.Unit
open import Data.Bool

{-
  Here  we give the raw data types required to define Microkit event traces.
  We specialize these types to the actual values that may occur in Microkit
  events.
-}

data PD : Set where
  Keypad : PD
  Pump : PD

data Channel : Set where
--^ We only support channels 1 and 42, since only these ones occur in the PDs of
--this example system. One could replace this with a full Ch00-Ch63 enum without
--any difficulty.
  Ch01 : Channel
  Ch42 : Channel

data End : Set where
  Keypad01 : End
  Pump42 : End

endPD : End → PD
endPD Keypad01 = Keypad
endPD Pump42 = Pump

endChannel : End → Channel
endChannel Keypad01 = Ch01
endChannel Pump42 = Ch42

data Address : Set where
--^ We only support addresses that can occur in Microkit events. Note that one
--can easily replace this with ℕ or some Word64 type, we only use this finite
--type as a minor convenience in this example proof.
  control-addr : Address
  command-addr : Address
  keybuffer-addr : Address

Word : Set
--^ We let Word = ℕ for now, so we do not have to define a Word64 type. Keep in
--mind that only 3 words occur in the Microkit events of this system, so one
--could also just use a finite type for the purposes of this demonstration.
Word = ℕ

_==_ : Word → Word → Bool
zero == zero = true
zero == suc _ = false
suc _ == zero = false
suc x == suc y = x == y

data Call : Set where
--^ We model only the relevant Microkit calls. Since no ppcalls or deferred
--notify calls occur, we omit Ppcall, ReplyRecv and NBSendRecv. Since we have no
--IRQs either, we can omit the IRQ api too.
  Notify : (ch : Channel) → Call
  Recv : Call
  Load64 : (addr : Address) → Call
  Store64 : (addr : Address) → (val : Word) → Call

Ret : Call → Set
Ret (Notify _) = ⊤
Ret Recv = ⊤
--^ Normally, this would return a badge or (pp, pf, ns), but in our example we
--only have notifications.
Ret (Load64 _) = Word
Ret (Store64 _ _) = ⊤

record Event : Set where
  constructor _⨾_⨾_
  field
    pd : PD
    call : Call
    ret : Ret call
