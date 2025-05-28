OUTPUT_ARCH(riscv)
ENTRY(_start)

MEMORY
{
    RAM: ORIGIN = 0x00000000, LENGTH = 128K
}

SECTIONS
{
    .text : ALIGN(4)
    {
        *(.text._start)
        *(.text)
        *(.text.*)
    } > RAM

    .rodata : ALIGN(4)
    {
        *(.rodata)
        *(.rodata.*)
    } > RAM

    .data : AT(ADDR(.rodata) + SIZEOF(.rodata)), ALIGN(4)
    {
        _sidata = LOADADDR(.data);
        _sdata = .;
        PROVIDE(__global_pointer$ = _sdata + 0x800);
        *(.data)
        *(.data.*)
        _edata = .;
    } > RAM

    .bss (NOLOAD) : ALIGN(4)
    {
        _sbss = .;
        *(.bss)
        *(.bss.*)
        _ebss = .;
        PROVIDE(sbss = _sbss);
        PROVIDE(ebss = _ebss);
    } > RAM

    .heap (NOLOAD) :
    {
        _sheap = .;
        . += HEAP_SIZE;
        _eheap = .;
        PROVIDE(heap_start = _sheap);
        PROVIDE(heap_end = _eheap);
    } > RAM

    .stack (NOLOAD) :
    {
        _estack = .;
        . += STACK_SIZE;
        _sstack = .;
        PROVIDE(stack_top = _sstack);
    } > RAM
}
