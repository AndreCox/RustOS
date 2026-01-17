#ifndef _ASSERT_H
#define _ASSERT_H

// Forward declare your Rust panic handler or a C wrapper for it
void kernel_panic(const char *message);

#define assert(expression) \
    ((expression) ? (void)0 : kernel_panic("Assertion failed: " #expression))

#endif