//! **El mapa de la memoria baja** que usa el bring-up, y la GDT del AP.
//!
//! Está en su propio fichero porque es *el dato*, no el código: son las
//! direcciones que el trampolín lleva escritas a mano —no puede llamar a nada—
//! y que el BSP rellena desde Rust. Los dos lados tienen que decir lo mismo, y
//! un fichero con nombre propio es lo que hace que se vean juntos.
//!
//! ⚠️ Y es exactamente lo que el código viejo hizo mal: la PML4 en `0x7000`
//! ocupa 4 KiB —hasta `0x8000`— y ponía el PDPT en `0x7100`, dentro. Aquí las
//! páginas están separadas a propósito y cada una dice para qué es.

/// El trampolín copiado. La SIPI lleva vector `0x08` → el AP empieza aquí.
pub const TRAMPOLIN: u64 = 0x8000;
/// Los datos que el BSP deja para el AP. Página distinta de la del código.
pub const DATOS: u64 = 0x9000;
/// Pila temporal de 32 bits, en la cola de la página de datos.
///
/// Sólo la usa el trampolín, que la lleva escrita a mano. Está aquí para que el
/// mapa esté completo en un sitio: si alguien mueve `DATOS`, esto se mueve.
#[allow(dead_code)]
pub const PILA_TMP: u32 = 0x9FF0;
/// Primera pila de AP. 4 KiB cada una, hacia arriba.
pub const PILAS: u64 = 0xA000;

// Desplazamientos dentro de `DATOS`. Los mismos números están escritos a mano en
// el trampolín: si se toca uno, se tocan los dos.
pub const OFF_GDTR: u64 = 0x00; // limit u16 + base u64
pub const OFF_IDTR: u64 = 0x10; // limit u16 + base u64
pub const OFF_CR3: u64 = 0x20;
pub const OFF_ENTRADA: u64 = 0x28;
pub const OFF_PILA: u64 = 0x30;
pub const OFF_GDT: u64 = 0x40; // la tabla en sí

/// La GDT que usa el AP para subir de 16 a 64 bits.
///
/// Cinco entradas y **cada una hace falta**. La de datos de 32 bits (`0x18`) es
/// la que no estaba en la versión vieja: aquel código cargaba `0x18` creyendo
/// que era datos y en su tabla era el código de 64 bits.
pub const GDT: [u64; 5] = [
    0,                     // 0x00 null
    0x0000_9B00_0000_FFFF, // 0x08 código 16-bit
    0x00CF_9A00_0000_FFFF, // 0x10 código 32-bit
    0x00CF_9200_0000_FFFF, // 0x18 datos  32-bit
    0x0020_9B00_0000_0000, // 0x20 código 64-bit (L=1)
];
