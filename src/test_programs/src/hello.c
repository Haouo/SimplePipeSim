#include <io.h>

int main(void) {
    /* print the string "Hello, World!\n" and end the program */
    // a0 = 1 : print a1 to stdout
    // a0 = 0 : 
    asm volatile("li a0, 1\n"
                 "li a1, 0x48\n" // 'H'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x65\n" // 'e'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x6c\n" // 'l'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x6c\n" // 'l'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x6f\n" // 'o'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x2c\n" // ','
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x20\n" // ' '
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x57\n" // 'W'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x6f\n" // 'o'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x72\n" // 'r'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x6c\n" // 'l'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x64\n" // 'd'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0x21\n" // '!'
                 "ecall\n"
                 "li a0, 1\n"
                 "li a1, 0xa\n" // '\n'
                 "ecall\n"
                 "li a0, 0\n"
                 "ecall\n" // end the program
    );
}