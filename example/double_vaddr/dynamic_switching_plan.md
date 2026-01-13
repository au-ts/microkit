# Plan for changing PD's dynamically

## Requirements

- There should be a loader that can be run by the controlelr function that loads and returns a CPTR to a valid vspace root,
  which can then be passed into the TCB.

## how does a vspace for a process get loaded by the microkit tool?

- step 1: the running PD transfers control to the controller PD and suspends.
- step 2: the controller PD makes a PPC to the monitor PD, who is in charge of initialising a new vspace for a program that is not in the system description (TODO)
- step 3: the monitor mints the new vspace into the controller PD, which then passes the new vspace into the stopped PD (as a middleman).
  - could the middleman be skipped in this case?
  - the controller PD calls the monitor PD with a specific badge with the name of a process

- notes from talking to julia:
- monitor probably shouldnt be doing all that work because it has been refactored (i fixed up changes from incoming microkit)
- give the controller some pool of untyped memory and a bunch of capabilities to create a vspace
- make a temporary PD that just exists to hold the compiled elf files that will be dynamiaclly loaded
  - guessting that the controller will make a PPC to this PD to grab the elf file (though now i am unsure if this second pd is needed.)
  - i guess it's needed for isolation so someone could ideally just replace the info PD with something else
  - make a pd that just holds elf file info and map in that region into the controller anyway LOL

goal for results: will debug and see if this works at all

5/1:
- spent a while fixing up all of the compile bugs. the build is compiling now! Going to go home later and re write the vspace loading sequencce
tomorrow:
- keep writing up the loading sequence, maybe start debugging cause things will take a while. 

- steps for making a vspace with reference from the builder:
- Create TCB and VSpace with all ELF loadable frames mapped in.
  - for every elf, add possible elf to the spec
    - given an elf file,,,,
    - for every loadable segment in the ELF, map into the given pd's addrespace.
    - need to find out how to get the segment base and size from just the memory blob
    - create frame object and cap for frame
    - map page for all frames
    - at the end, create and map the ipc buffer
  - all memory regions are mapped in
    - this is done with map memory region
- Create and map in the stack from the bottom up
  - config the bottom of the stack and create stack frames. 
  - map in stack frame acap