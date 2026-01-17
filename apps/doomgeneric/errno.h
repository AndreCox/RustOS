#ifndef _ERRNO_H
#define _ERRNO_H

// The global error variable
extern int errno;

// Common error codes
#define ENOENT  2   /* No such file or directory */
#define EIO     5   /* I/O error */
#define ENOMEM 12   /* Out of memory */
#define EACCES 13   /* Permission denied */
#define EISDIR 21

#endif