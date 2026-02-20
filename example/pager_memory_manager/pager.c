#include <microkit.h>
#include <stdint.h>

/**
 * I have made a couple assumptions:
 * - there is a fixed maximum of PD's
 * - 
 */

// i need to create the following things:

#define MAX_PDS 128
#define NUM_PT_ENTRIES 512

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
    // bool dirty; // probably better to keep a bool instead of a pointer to the shadow page table
    pe *page; // the page this frame is mapped to.
    uint32_t next;
    uint32_t prev;
} FrameInfo;

// pub struct Rights {
//     pub read: bool,
//     pub write: bool,
//     pub grant: bool,
//     pub grant_reply: bool,
// }

typedef struct Rights {
    bool read;
    bool write;
    bool grant;
    bool grant_reply;
} Rights;

// pub struct Frame {
//     pub object: ObjectId, u32
//     pub rights: Rights,
//     pub cached: bool,
//     pub executable: bool,
// }
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
} pe; // might need more stuff here.

typedef struct page_table {
    pe *pe[NUM_PT_ENTRIES];
} pt;

typedef struct page_middle_directory {
    pt *pt[NUM_PT_ENTRIES];
} pmd;

typedef struct page_upper_directory {
    pmd *pmd[NUM_PT_ENTRIES];
} pud;


// stuff required for the vm fault handling
pud pgd[MAX_PDS][NUM_PT_ENTRIES]; // page tables for the children.


FrameInfo frame_table[MAX_PDS]; // this functions as a doubly ll.


// TODO: have process vspace ptrs here as well.
unsigned long vspaces[MAX_PDS];
unsigned long long time = 0; // Working set clock.
#define TAU 10 // not too sure what the optimal number for this would be.

#define PGD_INDEX(va) (((va) >> 39) & 0x1FF)
#define PUD_INDEX(va) (((va) >> 30) & 0x1FF)
#define PD_INDEX(va)  (((va) >> 21) & 0x1FF)
#define PT_INDEX(va)  (((va) >> 12) & 0x1FF)

static retrieve_page(uintptr_t fault_addr, uint32_t pd_idx) {
    if (!pgd[pd_idx][PGD_INDEX(fault_addr)]) {
        
    }
}

void init(void)
{
    // TODO:
    // each child has a frame table associated to it. this ft should also keep track of the wsclock algo
    // each child has a shadow page table for the heap.
    // need to initialise these

    for (int i = 0; i < num_frames; ++i) {
        int next = i + 1, prev = i - 1;
        if (i == 0) prev = num_frames - 1;
        if (i == num_frames - 1) next = 0;
        frame_table[frames[i].pd_idx] = {frames[i].frame_cap, frames[i].frame_id, 0, false, next, prev};
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






