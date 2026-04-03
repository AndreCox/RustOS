#ifndef FREESTANDING_FIX_H
#define FREESTANDING_FIX_H

#include <stddef.h>
#include <stdarg.h>
#include <stdint.h>

#undef NULL
#define NULL ((void*)0)

typedef void FILE;
typedef unsigned int mode_t;

extern FILE *stdout;
extern FILE *stderr;
extern int errno;

#define EISDIR 21
#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

// Standard functions
size_t strlen(const char* s);
int snprintf(char* str, size_t size, const char* format, ...);
int printf(const char* format, ...);
int fprintf(FILE* stream, const char* format, ...);
int putchar(int c);
int puts(const char* s);
int fflush(FILE* stream);
int toupper(int c);
int tolower(int c);
int isspace(int c);
int isprint(int c);
int abs(int n);
int strcmp(const char* s1, const char* s2);
int strcasecmp(const char* s1, const char* s2);
int strncmp(const char* s1, const char* s2, size_t n);
int strncasecmp(const char* s1, const char* s2, size_t n);
int atoi(const char* nptr);
double atof(const char* nptr);
char* strdup(const char* s);
char* strstr(const char* haystack, const char* needle);
char* strchr(const char* s, int c);
char* strrchr(const char* s, int c);
char* strncpy(char* dest, const char* src, size_t n);

void* memset(void* s, int c, size_t n);
void* memcpy(void* dest, const void* src, size_t n);
void* memmove(void* dest, const void* src, size_t n);

FILE* fopen(const char* pathname, const char* mode);
int fclose(FILE* stream);
long ftell(FILE* stream);
int fseek(FILE* stream, long offset, int whence);
size_t fread(void* ptr, size_t size, size_t nmemb, FILE* stream);
size_t fwrite(const void* ptr, size_t size, size_t nmemb, FILE* stream);

int remove(const char* pathname);
int rename(const char* oldpath, const char* newpath);
int mkdir(const char* pathname, mode_t mode);

int sscanf(const char* str, const char* format, ...);
int vfprintf(FILE* stream, const char* format, va_list ap);
int vsprintf(char* str, const char* format, va_list ap);
int vsnprintf(char* str, size_t size, const char* format, va_list ap);

void exit(int status);

// Memory management
void* malloc(size_t size);
void* calloc(size_t nmemb, size_t size);
void* realloc(void* ptr, size_t size);
void free(void* ptr);

// Math
double fabs(double x);

#endif
