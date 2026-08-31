//! Virtual memory: address spaces built on the `s2_mem` physmap.
//!
//! [carril]  ROJO      el reparto, y hereda el color del carril que manda
//!
//! [cuesta]  MAQUINA -- calcula direcciones para el physmap y las
//!           dereferencia. Ya ha parado la maquina DOS veces: el `#GP` del
//!           25-08 y el `#PF` del 30-08, las dos en `destroy_address_space`.
//!
//! [riesgo]  AJENO ESPEJO
//!           AJENO  -- los numeros que camina no los escribe este fichero:
//!                     salen de las tablas de pagina y del `cr3` de una ranura
//!                     de tarea MUERTA. `caminable` existe por eso, y el
//!                     30-08 se vio que el argumento de entrada era el unico
//!                     que no pasaba por el.
//!           ESPEJO -- `phys::free_frame` juzga LA MISMA direccion fisica con
//!                     su propio techo. El 30-08 no coincidian --16 GiB contra
//!                     64 TiB-- y el flojo era el que dereferencia.
//!
//! Every user address space is a private PML4 that shares the kernel half
//! (PML4[256..512] -- the physmap and future kernel higher-half mappings) and
//! owns a private PDPT under PML4[0]. Entry 0 of that PDPT is a copy of the
//! kernel's supervisor identity map (0..32 MiB); everything the user touches
//! lives at PDPT index >= 1 so the shared identity tables are never polluted.
//!
//! Layout contract (BMO ABI, stable):
//! ```text
//!   1 GiB  USER_IMAGE_BASE   BEX sections (code/rodata/data/bss)
//!   2 GiB  USER_STACK_TOP    user stack, grows down (mapped in F2)
//!   3 GiB  CHANNEL_VA_BASE   16 BMO Channel pages, U/S shared with Ring 0
//! ```

//! # ** LOS TRES CARRILES (L6g, 2026-08-30)
//!
//! Este modulo era UN fichero de 718 lineas donde todo se leia igual: los bits
//! que define Intel, el mapeo que ya tiene un cambio escrito y pendiente, y el
//! CR3 del kernel. Ahora cada trozo esta en su carril, y el carril dice **que
//! cuesta cambiarlo**:
//!
//! ```text
//!    roja.rs       si te equivocas, no hay arranque ni autopsia
//!    amarilla.rs   VA a cambiar, y arrastra a cuatro llamantes cuando pase
//!    verde.rs      se cambia solo. Son numeros del manual y dos lectores
//! ```
//!
//! *** El verde SI tiene fichero aqui, al reves que en `ring0/critic/`. Dentro
//! de un modulo critico, saber que algo es verde **tambien es saber**: es la
//! diferencia entre "toca esto con cuidado" y "toca esto".
//!
//! [!] Fuera no cambia nada: `pub use` deja el modulo con la misma cara que
//! tenia, asi que ningun llamante se entera de la mudanza.

pub mod amarilla;
pub mod roja;
pub mod verde;

pub use amarilla::{
    map_page, map_page_propia, map_page_wc, unmap_page, PTE_NUESTRA, PTE_NX, PTE_PAT_4K,
};
pub use roja::{
    destroy_address_space, init, kernel_half_holes, kernel_pml4, new_address_space, read_cr3,
    self_test, switch_to,
};
pub use verde::{
    fisica_exacta, translate, CHANNEL_VA_BASE, FRAMEBUFFER_VA_BASE, MEMORIA_VA_BASE, PTE_HUGE,
    PTE_PRESENT, PTE_USER, PTE_WRITABLE, USER_IMAGE_BASE, USER_STACK_BOTTOM, USER_STACK_SIZE,
    USER_STACK_TOP,
};

