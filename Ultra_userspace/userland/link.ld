/* Guion de enlazado de TODO programa Ring 3 de BMO.
 *
 * La direccion base y el orden de las secciones son un CONTRATO con el
 * kernel: `proc.rs` coloca cada seccion cargable secuencialmente desde
 * USER_IMAGE_BASE, alineando el cursor a lo que la seccion declare (4 KiB,
 * lo pone `bex-link`) y avanzando por paginas enteras.
 *
 * Reproducir esa regla aqui es lo que hace que las direcciones absolutas que
 * escribe el enlazador caigan donde el kernel las mapea. Si esto se desalinea,
 * el programa carga, salta al vacio y muere con un #UD en Ring 3 — por eso
 * `bex-link` compara las dos colocaciones y se planta si no coinciden, en vez
 * de dejar que lo descubra el hardware.
 */
ENTRY(_start)

SECTIONS
{
    . = 0x40000000;   /* vmm::USER_IMAGE_BASE */

    .text ALIGN(4K) : {
        KEEP(*(.text._start))
        *(.text .text.*)
    }

    .rodata ALIGN(4K) : {
        *(.rodata .rodata.*)
    }

    .data ALIGN(4K) : {
        *(.data .data.*)
        *(.got .got.plt)
    }

    .bss ALIGN(4K) : {
        *(.bss .bss.*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.eh_frame*)
        *(.comment)
        *(.note*)
    }
}
