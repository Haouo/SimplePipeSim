#include <core.h>
#include <io.h>

#include <stdarg.h>

#define NOT_IMPLEMENTED                                                        \
  do {                                                                         \
    puts("Please implement the function by yourself!\n");                      \
    terminate();                                                               \
  } while (0);

void putchar(char c) { platform_outb(c); }

void puts(char *s) {
  while (*s != '\0') {
    putchar(*(s++));
  }
}

void putint(int numb) {
  if (numb < 0) {
    putchar('-');
    numb = -numb; // convert to positive number
  }

  if (numb / 10) {
    putint(numb / 10);
  }
  putchar((numb % 10) + '0');
}

void printf(char *format, ...) {
  va_list args;
  va_start(args, format);

  while (*format != '\0') {
    if (*format == '%') {
      format++;
      switch (*format) {
      case 'c': { // char
        char c = va_arg(args, int);
        putchar(c);
        break;
      }
      case 'd': { // int
        int numb = va_arg(args, int);
        putint(numb);
        break;
      }
      case 's': { // string
        char *s = va_arg(args, char *);
        puts(s);
        break;
      }
      default:
        break;
      }
    } else {
      putchar(*format);
    }
    format++;
  }

  va_end(args);
}
