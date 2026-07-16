/* bmo-kernel linker script — Ring 0 base */
ENTRY(_start)

SECTIONS
{
    . = 0x400000;

    .text ALIGN(4K) : {
        KEEP(*(.text._start))
        *(.text .text.*)
    }

    .rodata ALIGN(4K) : {
        *(.rodata .rodata.*)
    }

    .data ALIGN(4K) : {
        *(.data .data.*)
    }

    .bss ALIGN(4K) : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        . = . + 65536;  /* KERNEL_STACK (64 KiB kernel stack) */
        KERNEL_STACK_END_MARKER = .;
        __bss_end = .;
    }
}
