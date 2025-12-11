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
