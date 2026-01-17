#ifndef _UNISTD_H
#define _UNISTD_H

#include <stddef.h>
#include <stdint.h>

// DOOM might use these to check for terminal/file status
int usleep(unsigned int usec);
unsigned int sleep(unsigned int seconds);
int getpid(void);

// These are often expected in unistd.h for file offsets
#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

#endif