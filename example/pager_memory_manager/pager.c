#include <microkit.h>
#include <stdint.h>
#include "types.h"
/**
 * I have made a couple assumptions:
 * - there is a fixed maximum of PD's
 * - 
 */

// i need to create the following things:



#define INTO_PT(x) // make this macro such that it indexes into the struct page tables.

/**
 * frame table
 * sys brk/mmap munmap
 * shadow page tables
 * page file.
 */

static uint64_t unmapped_frames_addr;
static uint64_t num_frames;


typedef struct FrameInfo {
    Cap cap;
    uint32_t frame_id;
    uint64_t last_accessed; // for working set.
    pe *page; // the page this frame is mapped to.
    uint32_t next;
} FrameInfo;

typedef struct Rights {
    bool read;
    bool write;
    bool grant;
    bool grant_reply;
} Rights;

typedef struct Cap {
    uint32_t object;
    Rights rights;
    bool cached;
    bool executable;
} Cap;

typedef struct microkit_data {
    Cap frame_cap;
    uint32_t frame_id;
    uintptr_t pd_idx;
} frame_pd_id;



frame_pd_id *frames = (frame_pd_id *) unmapped_frames_addr;



typedef struct page_entry {
    uint32_t frame_id;
    Cap frame_cap;
    bool dirty;
} pe; // might need more stuff here.


// stuff required for the vm fault handling
// I cannot have actual page tables due to missing malloc.
pe page_table[MAX_PDS][NUM_PT_ENTRIES]; // page tables for the children. // each pd has 512 total max pages in heap.


FrameInfo frame_table[MAX_PDS][NUM_PT_ENTRIES]; // this functions as a doubly ll.

/**
 * wsclock hand ptr.
 */
FrameInfo *wshand[MAX_PDS] = {NULL};

// TODO: have process vspace ptrs here as well.
unsigned long vspaces[MAX_PDS];
unsigned long long time = 0; // Working set clock.
#define TAU 10 // not too sure what the optimal number for this would be.

static inline void move_hand(uint32_t pd_idx) {
    wshand[pd_idx] = &frame_table[pd_idx][wshand[pd_idx]->next];
}

/**
 * Gets the next frame to allocate, may need to page out the frame
 * currently recursive, may/may not want to change.
 */
static FrameInfo *get_frame(uint32_t pd_idx) {
    if (!wshand[pd_idx]->page) {
        FrameInfo *ret = wshand;
        move_hand(pd_idx);
        return ret;
    }

    if (wshand[pd_idx]->page->dirty) {
        --wshand[pd_idx]->page->dirty;
        move_hand(pd_idx);
    } else if (time - wshand[pd_idx]->last_accessed < TAU)
    {
        move_hand(pd_idx); // this has potential to cause infinite loop if I don't increment time.
    } else {
        FrameInfo *ret = wshand;
        move_hand(pd_idx);
        return ret;
    }

    return get_frame(pd_idx);
}

static inline pe retrieve_page(uintptr_t fault_addr, uint32_t pd_idx) {
    return page_table[pd_idx][INDEX_INTO_MMAP_ARRAY(fault_addr)];
}

void init(void)
{
    int frame_indicies[MAX_PDS] = {0};
    for (int i = 0; i < num_frames; ++i) {
        uint32_t next = i + 1, prev = i - 1;
        frame_pd_id *cur_frame = &frames[i];
        int pd_idx = cur_frame->pd_idx;

        frame_table[pd_idx][frame_indicies[pd_idx]] = { .cap = cur_frame->frame_cap, .frame_id = cur_frame->frame_id, .last_accessed = 0, page = NULL, .next = ++frame_indicies[pd_idx]};
    }

    // set the wshand to the start for every pd
    for (int i = 0; i < num_frames; ++i) {
        wshand[i] = frame_table[i];
        frame_table[i][frame_indicies[i] - 1].next = 0; 
    }
}

void notified(microkit_channel ch)
{
    // TODO: this may not be required 
}

seL4_MessageInfo_t protected(microkit_channel ch, microkit_msginfo msginfo)
{
    // TODO: this may not be required.
}

seL4_Bool fault(microkit_child child, microkit_msginfo msginfo, microkit_msginfo *reply_msginfo)
{
    ++time;
    // TODO: this is when the child has a vm fault...
    uintptr_t fault_addr = microkit_mr_get(1); // I am not sure if this is the right mr number so will need to check later.
    // check if a page in is required.
    
    // check if page out is required.

    // find a unused frame and map

    
}






