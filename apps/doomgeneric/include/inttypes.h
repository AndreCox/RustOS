#ifndef _INTTYPES_H
#define _INTTYPES_H

#include <stdint.h>

// Doom specifically wants these for printf formatting
#define PRId64 "lld"
#define PRIu64 "llu"
#define PRIx64 "llx"
#define PRId32 "d"
#define PRIu32 "u"
#define PRIx32 "x"

#endif