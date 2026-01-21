#ifndef KERNEL_COMPAT_H
#define KERNEL_COMPAT_H

#include <stdint.h>
#include <stddef.h>
#include <stdarg.h>

// ==========================================================
// 1. BASE TYPES AND STRUCTURES (Define these FIRST)
// ==========================================================
typedef int32_t pid_t;
typedef uint32_t uid_t;
typedef uint32_t gid_t;
typedef long off_t;
typedef long ssize_t;
typedef uint32_t mode_t;
typedef uint32_t dev_t;
typedef uint32_t ino_t;
typedef uint32_t nlink_t;

typedef struct
{
    int unused;
} FILE;

struct stat
{
    dev_t st_dev;
    ino_t st_ino;
    mode_t st_mode;
    nlink_t st_nlink;
    uid_t st_uid;
    gid_t st_gid;
    off_t st_size;
    // Doom mostly only cares about st_mode and st_size
};

// ==========================================================
// 2. GLOBALS AND MACROS
// ==========================================================
extern FILE *stderr;
extern FILE *stdout;
extern FILE *stdin;

extern int errno;

#define EISDIR 21
#define ENOENT 2
#define ENOMEM 12

#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

#define S_IFMT 0170000
#define S_IFDIR 0040000
#define S_IFREG 0100000
#define S_ISREG(m) (((m) & S_IFMT) == S_IFREG)
#define S_ISDIR(m) (((m) & S_IFMT) == S_IFDIR)

// ==========================================================
// 3. FUNCTION PROTOTYPES
// ==========================================================

// ctype.h
int tolower(int c);
int toupper(int c);
int isspace(int c);
int isalpha(int c);
int isdigit(int c);
int isalnum(int c);

// stdio.h
int printf(const char *format, ...);
int fprintf(FILE *stream, const char *format, ...);
int sprintf(char *str, const char *format, ...);
int snprintf(char *str, size_t size, const char *format, ...);
int vsnprintf(char *str, size_t size, const char *format, va_list ap);
int sscanf(const char *str, const char *format, ...);
int fflush(FILE *stream);
int puts(const char *s);
int putchar(int c);
int vfprintf(FILE *stream, const char *format, va_list ap);

// string.h
void *memset(void *s, int c, size_t n);
void *memcpy(void *dest, const void *src, size_t n);
void *memmove(void *dest, const void *src, size_t n);
int memcmp(const void *s1, const void *s2, size_t n);
char *strcpy(char *dest, const char *src);
char *strncpy(char *dest, const char *src, size_t n);
char *strcat(char *dest, const char *src);
int strcmp(const char *s1, const char *s2);
int strncmp(const char *s1, const char *s2, size_t n);
char *strchr(const char *s, int c);
char *strrchr(const char *s, int c);
char *strstr(const char *haystack, const char *needle);
size_t strlen(const char *s);
int strcasecmp(const char *s1, const char *s2);
int strncasecmp(const char *s1, const char *s2, size_t n);
char *strdup(const char *s);

// stdlib.h
void *malloc(size_t size);
void *calloc(size_t nmemb, size_t size);
void free(void *ptr);
void *realloc(void *ptr, size_t size);
void exit(int status);
char *getenv(const char *name);
int abs(int j);
long atol(const char *nptr);
int atoi(const char *nptr);
double atof(const char *nptr);
int system(const char *command);

// math.h
long labs(long j);
double pow(double x, double y);
double sin(double x);
double cos(double x);
double fabs(double x);
float fabsf(float x);
int abs(int j);
long labs(long j);

// File System / sys/stat.h
FILE *fopen(const char *filename, const char *mode);
int fclose(FILE *stream);
size_t fread(void *ptr, size_t size, size_t nmemb, FILE *stream);
size_t fwrite(const void *ptr, size_t size, size_t nmemb, FILE *stream);
int fseek(FILE *stream, long offset, int whence);
long ftell(FILE *stream);
int feof(FILE *stream);
int remove(const char *filename);
int rename(const char *oldname, const char *newname);
int mkdir(const char *pathname, mode_t mode);
int stat(const char *path, struct stat *buf);

#endif