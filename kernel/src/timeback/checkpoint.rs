//! `timeback::checkpoint` — Checkpoints con nombre.

extern crate alloc;

use alloc::string::String;
use core::mem::MaybeUninit;

const MAX_CHECKPOINTS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CheckpointId(pub u32);

static mut NEXT_ID: u32 = 1;
static mut COUNT: usize = 0;
static mut NAMES: [MaybeUninit<Option<String>>; MAX_CHECKPOINTS] = [const { MaybeUninit::uninit() }; MAX_CHECKPOINTS];

/// Crea un checkpoint con `name` y retorna su ID.
pub fn create(name: &str) -> CheckpointId {
    unsafe {
        let id = NEXT_ID;
        NEXT_ID += 1;
        let slot = (id as usize - 1) % MAX_CHECKPOINTS;
        NAMES[slot].write(Some(String::from(name)));
        if COUNT < MAX_CHECKPOINTS { COUNT += 1; }
        CheckpointId(id)
    }
}

/// Busca el nombre de un checkpoint por ID.
pub fn name(id: CheckpointId) -> Option<String> {
    unsafe {
        let slot = (id.0 as usize - 1) % MAX_CHECKPOINTS;
        NAMES[slot].assume_init_read().clone()
    }
}

/// # de checkpoints vivos.
pub fn count() -> usize { unsafe { COUNT } }
