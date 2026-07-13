/* Stage 1 — Arch init (GDT, IDT, SYSCALL) */
ENTRY(_start)

SECTIONS
{
    . = 0x100000;

    .text ALIGN(4K) : {
        *(.text._start)
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
        __bss_end = .;
    }

    __stage_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note.*)
    }
}
