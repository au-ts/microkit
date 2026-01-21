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

  - first map in the elf contents and then the ipc buffer

   // For each loadable segment in the ELF, map it into the address space of this PD.
        let mut frame_sequence = 0; // For object naming purpose only.
        for (seg_idx, segment) in elf.loadable_segments().iter().enumerate() {
            if segment.data().is_empty() {
                continue;
            }

            let seg_base_vaddr = segment.virt_addr;
            let seg_mem_size: u64 = segment.mem_size();

            let page_size = PageSize::Small;
            let page_size_bytes = page_size as u64;

            // Create and map all frames for this segment.
            let mut cur_vaddr = round_down(seg_base_vaddr, page_size_bytes);
            while cur_vaddr < seg_base_vaddr + seg_mem_size {
                let mut frame_fill = Fill {
                    entries: [].to_vec(),
                };

                // Now compute the ELF file offset to fill in this page.
                let mut dest_offset = 0;
                if cur_vaddr < seg_base_vaddr {
                    // Take care of case where the ELF segment is not aligned on page boundary:
                    //     |   ELF    |   ELF    |   ELF    |
                    // |   Page   |   Page   |   Page   |
                    //  <->
                    dest_offset = seg_base_vaddr - cur_vaddr;
                }

                let target_vaddr_start = cur_vaddr + dest_offset;
                let section_offset = target_vaddr_start - seg_base_vaddr;
                if section_offset < seg_mem_size {
                    // We have data to load
                    let len_to_cpy =
                        min(page_size_bytes - dest_offset, seg_mem_size - section_offset);

                    frame_fill.entries.push(FillEntry {
                        range: Range {
                            start: dest_offset,
                            end: dest_offset + len_to_cpy,
                        },
                        content: FillEntryContent::Data(ElfContent {
                            elf_id,
                            elf_seg_idx: seg_idx,
                            elf_seg_data_range: (section_offset as usize
                                ..((section_offset + len_to_cpy) as usize)),
                        }),
                    });
                }

                // Create the frame object, cap to the object, add it to the spec and map it in.
                let frame_obj_id = capdl_util_make_frame_obj(
                    self,
                    frame_fill,
                    &format!("elf_{pd_name}_{frame_sequence:09}"),
                    None,
                    PageSize::Small.fixed_size_bits(sel4_config) as u8,
                );
                let frame_cap = capdl_util_make_frame_cap(
                    frame_obj_id,
                    segment.is_readable(),
                    segment.is_writable(),
                    segment.is_executable(),
                    true,
                );

                match map_page(
                    self,
                    sel4_config,
                    pd_name,
                    vspace_obj_id,
                    frame_cap,
                    page_size_bytes,
                    cur_vaddr,
                ) {
                    Ok(_) => {
                        frame_sequence += 1;
                        cur_vaddr += page_size_bytes;
                    }
                    Err(map_err_reason) => {
                        return Err(format!(
                            "add_elf_to_spec(): failed to map segment page to ELF because: {map_err_reason}"
                        ))
                    }
                };
            }
        }