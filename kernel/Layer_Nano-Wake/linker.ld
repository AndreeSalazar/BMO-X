/* BMO Layer_Nano-Wake Linker Script — compact two-segment layout */
ENTRY(_start)

SECTIONS
{
    . = 0x2000000;

    .text ALIGN(16) : {
        *(.text._start)
        *(.text .text.*)
        *(.rodata .rodata.*)
        *(.data .data.*)
    }

    .bss ALIGN(16) : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    __layer_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note.*)
        *(.got)
        *(.got.plt)
        *(.plt)
        *(.rela.*)
        *(.dynstr)
        *(.dynsym)
        *(.dynamic)
        *(.gnu.hash)
    }
}
