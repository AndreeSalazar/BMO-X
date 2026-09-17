//! **HOSTILE DISK** -- ESTRATOS reads its superblocks, its estratos, its nodes
//! and its directory entries from a disk that anyone with the machine in their
//! hands can rewrite. Every decoder, fed mutations of objects it encoded
//! itself, and the tree walk fed a disk that answers with garbage.
//!
//! Checked: nothing panics, and the walk always ends.

use bmo_estratos::objects::{ATTR_DATOS, ATTR_FIRMA, BLOQUE};
use bmo_estratos::read::{descender, Fuente};
use bmo_estratos::{pick_superblock, Attr, Autor, BlockPtr, Entrada, Estrato, Nodo, Superblock, Tipo};
use bmo_hostile::{attack, Hostile, DEFAULT_SEED};

fn samples() -> Vec<Vec<u8>> {
    let ptr = BlockPtr::nuevo(500, 0, &[1u8; 64]);
    let node = Nodo::nuevo(Tipo::Archivo)
        .con(Attr::en_bloques(ATTR_DATOS, 12_376, 1, ptr).unwrap())
        .unwrap()
        .con(Attr::residente(ATTR_FIRMA, &[0xCD; 32]).unwrap())
        .unwrap();
    vec![
        Superblock::new([7u8; 32], 1_000_000).encode().to_vec(),
        Estrato::new(ptr, BlockPtr::NULO, 42, Autor::Proceso(9), "guardar").encode().to_vec(),
        node.encode().to_vec(),
        Entrada::nueva("hola.txt", ptr).unwrap().encode().to_vec(),
        ptr.encode().to_vec(),
    ]
}

#[test]
fn hostile_objects_never_panic() {
    let owned = samples();
    let refs: Vec<&[u8]> = owned.iter().map(|v| v.as_slice()).collect();
    attack("estratos decode", DEFAULT_SEED, 30_000, &refs, BLOQUE + 16, |x| {
        let _ = Superblock::decode(x);
        let _ = Estrato::decode(x);
        if let Ok(n) = Nodo::decode(x) {
            let _ = (n.tiene_firma(), n.attrs().count());
        }
        if let Ok(e) = Entrada::decode(x) {
            let _ = (e.nombre_str(), e.se_llama("HOLA.TXT"));
        }
        let _ = BlockPtr::decode(x);
        let _ = Attr::decode(x);
        let half = x.len() / 2;
        let _ = pick_superblock(&x[..half], &x[half..]);
    });
}

/// A disk whose every block is garbage derived from the case.
struct LyingDisk {
    rng: Hostile,
    reads: u32,
}

impl Fuente for LyingDisk {
    fn bloque(&mut self, _lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
        self.reads += 1;
        for b in dst.iter_mut() {
            *b = self.rng.byte();
        }
        self.rng.below(16) != 0
    }
}

#[test]
fn the_tree_walk_ends_on_a_lying_disk() {
    let good_ptr = BlockPtr::nuevo(500, 0, &[1u8; 64]).encode().to_vec();
    attack("estratos descender", DEFAULT_SEED ^ 1, 3_000, &[&good_ptr], 128, |x| {
        let Ok(root) = BlockPtr::decode(x) else { return };
        let seed = x.iter().fold(1u64, |a, &b| a.wrapping_mul(31).wrapping_add(b as u64));
        for levels in [0u8, 1, 3, 255] {
            let mut disk = LyingDisk { rng: Hostile::new(seed), reads: 0 };
            let mut scratch = vec![[0u8; BLOQUE]; 4];
            let mut chunks = 0u32;
            let _ = descender(&mut disk, &root, levels, &mut scratch, &mut |_| {
                chunks += 1;
                chunks < 10_000
            });
            assert!(disk.reads < 100_000, "the walk read {} blocks from a lying disk", disk.reads);
        }
    });
}
