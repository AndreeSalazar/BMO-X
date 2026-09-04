//! Bitmap physical frame allocator (4 KiB frames, physical < PHYSMAP_SIZE).
//!
//! [carril]  ROJO      el reparto, y hereda el color del carril que manda
//!
//!
//! [cuesta]  MAQUINA -- entregar dos veces el mismo marco no da un fallo: da
//!           dos duenos del mismo byte, y el sintoma tres arranques despues.
//!
//! [riesgo]  ESPEJO -- `MAX_PHYS` es el techo de lo que el physmap alcanza, y
//!           NO es el unico sitio que lo decide: `vmm::caminable` juzga la
//!           misma direccion. El 30-08 los dos numeros no eran el mismo y la
//!           maquina se paro. Si este techo cambia, ese cambia con el.
//!
//! *** Y esa es exactamente la razon de no partirlo. Un `verde.rs` con dos
//! lectores dentro no informa de nada, y **tres ficheros donde solo hay un
//! carril es la aguja mejor escondida**. Un modulo lleva los carriles que
//! TIENE; este tiene uno.
//!
//! Source of truth: the BootContext memory map *after* `s2_mem` already
//! carved out its page-table pool (so the tables that built the physmap can
//! never be handed out). On top of the map we apply the kernel's own
//! reservations: legacy low memory, the kernel image, faggin/UEFI stage
//! images, the BootContext page, the GOP framebuffer, the LAPIC/IOAPIC/HPET
//! window, and the reserved Ring 3 payload/workspace ranges.
//!
//! The 512 KiB bitmap lives in `.bss` (identity-mapped, zeroed by `_start`).
//! Coverage is exactly `mm::PHYSMAP_SIZE` (16 GiB): the allocator must never
//! hand out a frame the physmap cannot reach, because `zero_frame` and every
//! page-table access go through `phys_to_virt`. The 1 TiB path is: s2_mem
//! sizes the physmap from the memory map (1 GiB pages), PHYSMAP_SIZE follows
//! it, and the bitmap moves out of `.bss` into a boot-time carve.
//! # ** LOS CARRILES (L6g)
//!
//! ```text
//!    roja.rs      el BITMAP: init, reserve_range, alloc_frame,
//!                 alloc_frames_contig, free_frame, tramos, stats
//!    amarilla.rs  `zero_frame`: la unica que escribe en la memoria de verdad
//! ```
//!
//! *** DOS carriles y no tres. No hay nada verde: `tramos()` y `stats()` leen,
//! pero leen el mismo bitmap que el resto reparte, y **un modulo lleva los
//! carriles que TIENE**.
//!
//! ** La tabla que contesta *de quien es este marco* --la pregunta que el
//! titulo de `roja.rs` promete y su bitmap no puede dar-- vive en
//! `mm::duenno`, un piso mas arriba. No es un tercer carril: no reparte RAM y
//! no escribe la memoria de nadie. La ley R9 lo dijo antes que nadie cuando se
//! intento meterla aqui.
//!
//! [!] Fuera no cambia nada: `pub use` deja el modulo con la misma cara.

mod amarilla;
mod roja;

pub use amarilla::zero_frame;
pub use super::duenno::{duenno_de, Duenno};
pub use roja::{
    alloc_frame, alloc_frame_de, alloc_frames_contig, esta_libre, free_frame, free_frame_de,
    init, stats, tramos,
};
