#ifndef __CORE_H__
#define __CORE_H__

#include <stdint.h>

#define SYS_EXIT 0
#define SYS_PUTC 1

static inline void __internal_syscall(uint64_t arg0, uint64_t arg1) {
    register uint64_t a0 asm("a0") = arg0;
    register uint64_t a1 asm("a1") = arg1;
    asm volatile("ecall" ::"r"(a0), "r"(a1) :);
}

#define SYSCALL_1(A0) __internal_syscall(A0, 0)
#define SYSCALL_2(A0, A1) __internal_syscall(A0, A1)

/* function used to exit the current running program */
extern void terminate(void) __attribute__((noreturn));

/* basic output function (single character as unit) */
extern void platform_outb(char c);

#endif
