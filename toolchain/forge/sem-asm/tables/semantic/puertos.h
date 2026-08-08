/* semantic/puertos.h -- el espacio de E/S, que no es memoria.
 *
 * x86 tiene DOS espacios de direcciones: la memoria y los 65.536 puertos de
 * E/S. A un puerto no se llega con un puntero -- no hay direccion que
 * desreferenciar-- sino con `in` y `out`, y por eso hacen falta instrucciones
 * propias.
 *
 * Es lo viejo del PC: el teclado PS/2 (0x60/0x64), el reinicio por 0xCF9, el
 * PIC (0x20/0xA0), la configuracion de PCI (0xCF8/0xCFC). Lo moderno --xHCI,
 * AHCI, NVMe-- vive en MMIO y se toca con punteros normales, sin nada de esto.
 *
 * * Ring 0. En Ring 3 un `out` da #GP salvo que el mapa de permisos de la TSS
 *   diga otra cosa, y BMO no abre ese mapa a nadie.
 */
#ifndef SEMANTIC_PUERTOS_H
#define SEMANTIC_PUERTOS_H

#include <semantic/tipos.h>

/* `out dx, al` -- un byte. */
void puerto_byte(u16 puerto, u8 valor) { __outb(puerto, valor); }

/* `out dx, ax` -- dos bytes. */
void puerto_palabra(u16 puerto, u16 valor) { __outw(puerto, valor); }

/* `out dx, eax` -- cuatro. Es el que usa la configuracion de PCI. */
void puerto_doble(u16 puerto, u32 valor) { __outl(puerto, valor); }

u8 puerto_leer_byte(u16 puerto) { return (u8)__inb(puerto); }
u16 puerto_leer_palabra(u16 puerto) { return (u16)__inw(puerto); }
u32 puerto_leer_doble(u16 puerto) { return (u32)__inl(puerto); }

#endif /* SEMANTIC_PUERTOS_H */
