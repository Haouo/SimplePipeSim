        .section .text._start
        .global _start
_start:
        .option push
        .option norelax
        la gp, __global_pointer$
        .option pop
        la sp, __stack_top
        # jump to main() func
        call main
        # end of the program
        li a0, 0
        ecall