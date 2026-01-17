/* kernel_compat.h */
#ifndef KERNEL_COMPAT_H
#define KERNEL_COMPAT_H

typedef unsigned long size_t;
typedef long intptr_t;
typedef unsigned long uintptr_t;

// Remove 'const' to match DOOM's r_data.h
int R_TextureNumForName(char *name);
int R_CheckTextureNumForName(char *name);

// Ensure this returns a 64-bit pointer
char *strupr(char *s);
char *strchr(const char *s, int c);
char *strncpy(char *dest, const char *src, size_t n);
void *malloc(size_t size);
void *memcpy(void *dest, const void *src, size_t n);
void *memset(void *s, int c, size_t n);

// 3. DOOM Specifics that return pointers
char *DEH_String(char *s);

static inline int system(const char *command)
{
    (void)command; // Silence unused warning
    return -1;     // Return -1 to indicate the command failed/unsupported
}
#endif