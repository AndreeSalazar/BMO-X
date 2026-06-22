//! `ring0::snapshot::memory_mark` — Bitmarks de páginas sucias.
//!
//! v1.8.8: stub. En v1.9 usaremos un bitmap (1 bit por página de 4 KB).

#![allow(dead_code)]

const BITMAP_SIZE: usize = 4096; // hasta 16 GB con páginas de 4 KB

static mut BITMAP: [u64; BITMAP_SIZE / 64] = [0; BITMAP_SIZE / 64];

pub fn init() {
    unsafe { for b in &mut BITMAP { *b = 0; } }
}

/// Marca la página que contiene `paddr` como "sucia".
pub fn mark_dirty(paddr: u64) {
    let page = (paddr / 4096) as usize;
    if page >= BITMAP_SIZE { return; }
    unsafe {
        let word = page / 64;
        let bit = page % 64;
        BITMAP[word] |= 1 << bit;
    }
}

/// ¿La página está sucia?
pub fn is_dirty(paddr: u64) -> bool {
    let page = (paddr / 4096) as usize;
    if page >= BITMAP_SIZE { return false; }
    unsafe {
        let word = page / 64;
        let bit = page % 64;
        (BITMAP[word] & (1 << bit)) != 0
    }
}

/// Limpia todas las marcas.
pub fn clear_all() {
    unsafe { for b in &mut BITMAP { *b = 0; } }
}
