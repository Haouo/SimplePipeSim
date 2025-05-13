#include "../include/core.h"

void terminate(void) {
    for (;;) {
        SYSCALL_1(SYS_EXIT);
    }
}

void platform_outb(char c) { SYSCALL_2(SYS_PUTC, c); }
