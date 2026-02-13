#include <microkit.h>
#include <string.h>
#include <stdint.h>
// i need to create the following things:

/**
 * frame table
 * sys brk/mmap munmap
 * shadow page tables
 * page file.
 */

static unsigned long long unmapped_frames_addr;

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





