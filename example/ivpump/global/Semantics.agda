module Semantics where

open import Types
open import Trace
import SDF

open import Data.Empty
open import Data.Maybe
open import Data.Product
open import Data.Sum
open import Data.Unit

open import Relation.Binary.PropositionalEquality

{-
  In this file we give an STC  memory semantics for Microkit events.
  
  By STC we mean Sequential Trace Consistency, defined for read events as follows:

  > Each `pd1 ⨾ Load64 ra ⨾ rv` event must have `rv` equal to the `wv` of the last
  > `pd2 ⨾ Store64 wa wv ⨾ tt` event in the trace where the `wa` of `pd2` aliases
  > the `ra` of `pd1`, or the initial value if no such write exists.

  In a model which supports memory barrier events (which the current version of
  Pancake Microkit does not), one could also define Relaxed Trace Consistency,
  which would allow this model to prove results about weak memory, although only
  in the absence of load-store reorderings (i.e. one needs prefix-closedness).

  Note that the example does not satisfy its spec under weak memory, since we do
  not have any barrier events in its LSM.
-}

data CanRecv (pd : PD) : Run → Set where
  CanRecv-Notify :
    (other : PD) →
    (ch : Channel) →
    SDF.other-end-pd other ch ≡ just pd →
    (sm : StateMap) →
    (rest : Run) →
    CanRecv pd (step (other ⨾ Notify ch ⨾ tt) sm rest)
  CanRecv-Notify-skip :
    (other : PD) →
    (ch : Channel) →
    SDF.other-end-pd other ch ≢ just pd →
    (sm : StateMap) →
    (rest : Run) →
    CanRecv pd rest →
    CanRecv pd (step (other ⨾ Notify ch ⨾ tt) sm rest)
  CanRecv-Recv :
    (other : PD) →
    other ≢ pd →
    (sm : StateMap) →
    (rest : Run) →
    CanRecv pd rest →
    CanRecv pd (step (other ⨾ Recv ⨾ tt) sm rest)
  CanRecv-Load64 :
    (other : PD) →
    (addr : Address) →
    (val : Word) →
    (sm : StateMap) →
    (rest : Run) →
    CanRecv pd rest →
    CanRecv pd (step (other ⨾ Load64 addr ⨾ val) sm rest)
  CanRecv-Store64 :
    (other : PD) →
    (addr : Address) →
    (val : Word) →
    (sm : StateMap) →
    (rest : Run) →
    CanRecv pd rest →
    CanRecv pd (step (other ⨾ Store64 addr val ⨾ tt) sm rest)

data CanLoad64 (pd : PD) (addr : Address) (val : Word) : Run → Set where
  CanLoad64-init :
    (sm : StateMap) →
    val ≡ 0 →
    --^ Omitting this will permit two subsequent loads to yield different
    --results. If you want non-zero-initialized memory, you need to provide
    --an initial memory map.
    CanLoad64 pd addr val (init sm)
  CanLoad64-Notify :
    (other : PD) →
    (ch : Channel) →
    (sm : StateMap) →
    (rest : Run) →
    CanLoad64 pd addr val rest →
    CanLoad64 pd addr val (step (other ⨾ Notify ch ⨾ tt) sm rest)
  CanLoad64-Recv :
    (other : PD) →
    (sm : StateMap) →
    (rest : Run) →
    CanLoad64 pd addr val rest →
    CanLoad64 pd addr val (step (other ⨾ Recv  ⨾ tt) sm rest)
  CanLoad64-Load64 :
    (other : PD) →
    (oaddr : Address) →
    (oval : Word) →
    (sm : StateMap) →
    (rest : Run) →
    CanLoad64 pd addr val rest →
    CanLoad64 pd addr val (step (other ⨾ Load64 oaddr ⨾ oval) sm rest)
  CanLoad64-Store64 :
    (other : PD) →
    (oaddr : Address) →
    (oval : Word) →
    SDF.mem-alias (pd , addr) (other , oaddr) →
    oval ≡ val →
    (sm : StateMap) →
    (rest : Run) →
    CanLoad64 pd addr val (step (other ⨾ Store64 oaddr oval ⨾ tt) sm rest)
  CanLoad64-Store64-skip :
    (other : PD) →
    (oaddr : Address) →
    (oval : Word) →
    (SDF.mem-alias (pd , addr) (other , oaddr) → ⊥) →
    (sm : StateMap) →
    (rest : Run) →
    CanLoad64 pd addr val rest →
    CanLoad64 pd addr val (step (other ⨾ Store64 oaddr oval ⨾ tt) sm rest)


sequential-consistency :
  {pd : PD} →
  {addr : Address} →
  {v1 v2 : Word} →
  {run : Run} →
  CanLoad64 pd addr v1 run →
  CanLoad64 pd addr v2 run →
  v1 ≡ v2
--^ We prove that the sequential trace consistency semantics, as its name suggests,
--satisfies sequential consistency: each load can return exactly one possible value
--at a given time. This follows from an induction on the two `CanLoad64`s.
sequential-consistency
  (CanLoad64-init sm1 [v1=0])
  (CanLoad64-init .sm1 [v2=0]) =
  trans [v1=0] (sym [v2=0])
sequential-consistency
  (CanLoad64-Notify other ch sm rest l1)
  (CanLoad64-Notify .other .ch .sm .rest l2) =
  sequential-consistency l1 l2
sequential-consistency
  (CanLoad64-Recv other sm rest l1)
  (CanLoad64-Recv .other .sm .rest l2) =
  sequential-consistency l1 l2
sequential-consistency
  (CanLoad64-Load64 other oaddr oval sm rest l1)
  (CanLoad64-Load64 .other .oaddr .oval .sm .rest l2) =
  sequential-consistency l1 l2
sequential-consistency
  (CanLoad64-Store64 other oaddr oval _ [oval=v1] sm rest)
  (CanLoad64-Store64 .other .oaddr .oval _ [oval=v2] .sm .rest) =
  trans (sym [oval=v1]) [oval=v2]
sequential-consistency
  (CanLoad64-Store64 _ _ _ [alias-pd-addr-other-oaddr] _ _ _)
  (CanLoad64-Store64-skip _ _ _ [¬alias-pd-addr-other-oaddr] _ _ _) =
  ⊥-elim ([¬alias-pd-addr-other-oaddr] [alias-pd-addr-other-oaddr])
sequential-consistency
  (CanLoad64-Store64-skip _ _ _ [¬alias-pd-addr-other-oaddr] _ _ _)
  (CanLoad64-Store64 _ _ _ [alias-pd-addr-other-oaddr] _ _ _) =
  ⊥-elim ([¬alias-pd-addr-other-oaddr] [alias-pd-addr-other-oaddr])
sequential-consistency
  (CanLoad64-Store64-skip other oaddr oval x sm rest l1)
  (CanLoad64-Store64-skip .other .oaddr .oval .x .sm .rest l2) =
  sequential-consistency l1 l2


data DeviceLoad64 : PD → Address → Word → Set where
--^ A simple input device model.
  dl1 : DeviceLoad64 Keypad keybuffer-addr 1
  dl2 : DeviceLoad64 Keypad keybuffer-addr 2

data Permitted : Run → Set where
--^ A `Run` is permitted if it is allowed by the sequential trace consistency
--semantics. A permitted run need not comply with the user-provided guard and
--transition specs.
  Permitted-init : {sm : StateMap} → Permitted (init sm)
  Permitted-Notify :
    {pd : PD} →
    {ch : Channel} →
    SDF.is-notify-target pd ch ->
    {sm : StateMap} →
    {rest : Run} →
    Permitted rest →
    Permitted (step (pd ⨾ (Notify ch) ⨾ tt) sm rest) 
  Permitted-Recv :
    {pd : PD} →
    {sm : StateMap} →
    {rest : Run} →
    CanRecv pd rest →
    Permitted rest →
    Permitted (step (pd ⨾ Recv ⨾ tt) sm rest) 
  Permitted-Load64 :
    {pd : PD} →
    {addr : Address} →
    {val : Word} →
    {sm : StateMap} →
    {rest : Run} →
    SDF.mem-is-regular pd addr →
    SDF.mem-readable pd addr →
    CanLoad64 pd addr val rest →
    Permitted rest →
    Permitted (step (pd ⨾ Load64 addr ⨾ val) sm rest)
  Permitted-DeviceLoad64 :
    {pd : PD} →
    {addr : Address} →
    {val : Word} →
    {sm : StateMap} →
    {rest : Run} →
    SDF.mem-readable pd addr →
    DeviceLoad64 pd addr val →
    Permitted rest →
    Permitted (step (pd ⨾ Load64 addr ⨾ val) sm rest)
  Permitted-Store64 :
    {pd : PD} →
    {addr : Address} →
    {val : Word} →
    {sm : StateMap} →
    {rest : Run} →
    SDF.mem-writeable pd addr →
    Permitted rest →
    Permitted (step (pd ⨾ Store64 addr val ⨾ tt) sm rest)
