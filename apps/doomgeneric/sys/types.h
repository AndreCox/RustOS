#ifndef _SYS_TYPES_H
#define _SYS_TYPES_H

#include <stddef.h>
#include <stdint.h>

// These are the types DOOM's system-level code expects
typedef int32_t  pid_t;
typedef int32_t  mode_t;
typedef int64_t  off_t;
typedef int32_t  uid_t;
typedef int32_t  gid_t;
typedef int64_t  ssize_t;

#endif