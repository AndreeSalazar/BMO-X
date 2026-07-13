// uefi_chain Linker Script — UEFI application with 5 hand-off layers.
ENTRY(efi_main)

TARGET x86_64-unknown-uefi

SECTIONS
{
    . = ALIGN(4K);
    .text : {
        *(.text .text.*)
    }

    . = ALIGN(4K);
    .rodata : {
        *(.rodata .rodata.*)
    }

    . = ALIGN(4K);
    .data : {
        *(.data .data.*)
    }

    . = ALIGN(4K);
    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }
}
