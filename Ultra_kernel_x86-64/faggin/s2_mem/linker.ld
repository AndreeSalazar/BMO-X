ENTRY(_start)
SECTIONS
{
    . = 0x200000;
    .text ALIGN(4K) : { *(.text .text.*) }
    .rodata ALIGN(4K) : { *(.rodata .rodata.*) }
    .data ALIGN(4K) : { *(.data .data.*) }
    .bss ALIGN(4K) : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = . + 65536;  /* Safe stack for CR3 switch */
        __bss_end = .;
    }
}
