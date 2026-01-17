#ifndef _INTTYPES_H
#define _INTTYPES_H

#include <stdint.h>

// DOOM might use these for printf, but we'll handle the strings manually in Rust
#define PRId64 "lld"
#define PRIu64 "llu"
#define PRIx64 "llx"
#define PRIu32 "u"
#define PRIx32 "x"

#endif