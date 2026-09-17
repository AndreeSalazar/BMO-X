//! **HOSTILE BEX** -- the container every program arrives in. Today a `.bex`
//! comes from a disk the owner wrote; with the antenna and the network it can
//! come from anyone. Real binaries from the tree, mutated, through every reader
//! of the format.
//!
//! Checked: nothing panics. The validator's own tests check that it REJECTS
//! what it must.

use bmo_abi::bef::{katanas, paquete, sections, validator};
use bmo_abi::dynobj::{lista, tabla, texto};
use bmo_hostile::{attack, DEFAULT_SEED};

const PAYLOADS: &str = "../../../Ultra_kernel_x86-64/kernel/src/ring0/task/payloads/";

fn samples() -> Vec<Vec<u8>> {
    ["hola_C.bex", "hola_COBOL.bex", "init_hello.bex", "rpc_cli.bex"]
        .iter()
        .map(|n| {
            let p = format!("{}/{}{}", env!("CARGO_MANIFEST_DIR"), PAYLOADS, n);
            std::fs::read(&p).unwrap_or_else(|e| panic!("sample {} is gone ({}): the hostile pass needs a real .bex", p, e))
        })
        .collect()
}

#[test]
fn hostile_bex_never_panics() {
    let owned = samples();
    let refs: Vec<&[u8]> = owned.iter().map(|v| v.as_slice()).collect();
    attack("bef readers", DEFAULT_SEED, 6_000, &refs, 16_000, |x| {
        let _ = validator::validate(x);
        let _ = bmo_abi::bex::validate(x);
        let _ = sections::TablaCadenas::leer(x, 48);
        if let Some(dir) = paquete::directorio(x) {
            for i in 0..dir.len() + 2 {
                let _ = (dir.nombre(i), dir.entrada(i), dir.datos(i));
            }
            let _ = dir.buscar("icono");
        }
        let _ = paquete::seccion_recursos(x);
    });
}

#[test]
fn hostile_sections_never_panic() {
    // The section readers get a section, not a file: random blocks and blocks
    // cut out of real binaries.
    let owned = samples();
    let refs: Vec<&[u8]> = owned.iter().map(|v| &v[..v.len().min(512)]).collect();
    attack("sections", DEFAULT_SEED ^ 1, 30_000, &refs, 600, |x| {
        let _ = katanas::cuantas(x);
        let _ = katanas::revisar(x, 4096);
        for i in [0usize, 1, 7, usize::MAX] {
            let _ = katanas::katana(x, i);
        }
        let _ = lista::leer(x);
        let _ = lista::revisar(x, 8);
        let _ = tabla::leer(x);
        let _ = tabla::revisar(x);
        let _ = texto::leer(x);
        let _ = texto::contenido(x);
        let _ = texto::revisar(x);
    });
}

/// ** The guard: if the samples are not VALID binaries today, every mutation
/// dies at the magic or the version, and the tests above only attack the door.
#[test]
fn the_samples_are_valid_binaries() {
    for (i, s) in samples().iter().enumerate() {
        let r = validator::validate(s);
        assert!(r.is_valid, "sample {} no longer validates: {:?}", i, r.issues.iter().map(|x| format!("{:?}", x)).collect::<Vec<_>>());
    }
}
