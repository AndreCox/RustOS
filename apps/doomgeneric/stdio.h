#ifndef _STDIO_H
#define _STDIO_H

#include <stddef.h>
#include <stdarg.h>

typedef struct { int unused; } FILE;
#define stdout ((FILE*)1)
#define stderr ((FILE*)2)

// Existing declarations...
int printf(const char* format, ...);
int fprintf(FILE* stream, const char* format, ...);
int sprintf(char* str, const char* format, ...);
int snprintf(char* str, size_t size, const char* format, ...);
int puts(const char* s);
int putchar(int c);

// --- ADD THESE FOR G_GAME.C ---
FILE* fopen(const char* filename, const char* mode);
int fclose(FILE* stream);
size_t fread(void* ptr, size_t size, size_t nmemb, FILE* stream);
size_t fwrite(const void* ptr, size_t size, size_t nmemb, FILE* stream);
int fseek(FILE* stream, long offset, int whence);
long ftell(FILE* stream);
int remove(const char* filename);
int rename(const char* oldname, const char* newname);
int fflush(FILE* stream);
int vfprintf(FILE* stream, const char* format, va_list ap);
int sscanf(const char* str, const char* format, ...);
int vsnprintf(char* str, size_t size, const char* format, va_list ap);

#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

#endif