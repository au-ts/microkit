#include <microkit.h>
#include <stdint.h>
// i need to create the following things:

/**
 * frame table
 * sys brk/mmap munmap
 * shadow page tables
 * page file.
 */

static uint64_t unmapped_frames_addr;
static uint64_t num_frames;

static *FrameInfo frame_table;

typedef struct FrameInfo {
    Cap cap;
    uint32_t frame;
    bool ws;
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

void init(void)
{
    // TODO:
    // each child has a frame table associated to it. this ft should also keep track of the wsclock algo
    // each child has a shadow page table for the heap.
    // need to initialise these

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
    // TODO: this is when the child has a vm fault...
}






