
typedef __builtin_va_list va_list;

#define MAX_PDS 64
#define NUM_PT_ENTRIES 128
#define BRK_START 0x8000000000
#define MMAP_START 0x9000000000
#define ROUND_DOWN_TO_4K(x) ((x) & ~(4096 - 1))
#define INDEX_INTO_MMAP_ARRAY(x) (ROUND_DOWN_TO_4K(x)) / 4096