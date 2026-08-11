//! **Comprueba los hashes de seccion de los `.bex` que se van a desplegar.**
//!
//! === Por que esto es una prueba y no un script suelto ===
//!
//! El 2026-08-11, `doom.bex` no pasaba la admision en el Ryzen y el kernel no
//! decia por que --diez caminos de fallo hablaban por el klog, que con el
//! compositor en pantalla no se lee--. La pregunta *"es la firma o no?"* se
//! podia contestar **en el anfitrion, en un segundo**, y en vez de eso costo
//! varios arranques.
//!
//! > **Si una pregunta sobre un fichero se puede contestar sin encender la
//! > maquina, encender la maquina para contestarla es tiempo tirado.**
//!
//! Lo que hace: por cada `.bex` de `staging`, lee su seccion `Signature` y
//! comprueba **cada digest contra los bytes reales de su seccion**, que es
//! exactamente lo que hace `aterrizaje` en Ring 0 al cargar. Si aqui cuadra y
//! en metal no, el problema es el transporte; si aqui no cuadra, el fichero
//! sale mal del build y no hay que mirar el kernel.
//!
//! [!] No falla si no hay `staging`: es un artefacto del build, no del
//! repositorio, y una prueba que exige artefactos rompe el `cargo test` de
//! quien acaba de clonar.

use std::path::{Path, PathBuf};

/// Kind de la seccion Signature en la tabla de BEF.
const SECTION_SIGNATURE: u8 = 0x0F;
const SECTION_ENTRY: usize = 48;
const TABLA_EN: usize = 48;

fn u16_en(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn u32_en(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}
fn u64_en(d: &[u8], o: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[o..o + 8]);
    u64::from_le_bytes(b)
}

struct Seccion {
    kind: u8,
    file_offset: usize,
    file_size: usize,
}

fn secciones(d: &[u8]) -> Vec<Seccion> {
    let n = u32_en(d, 40) as usize;
    let mut v = Vec::new();
    for i in 0..n.min(16) {
        let o = TABLA_EN + i * SECTION_ENTRY;
        if o + SECTION_ENTRY > d.len() {
            break;
        }
        v.push(Seccion {
            kind: d[o],
            file_offset: u64_en(d, o + 8) as usize,
            file_size: u64_en(d, o + 16) as usize,
        });
    }
    v
}

/// `Err(motivo)` en el primer digest que no cuadre.
fn comprobar(d: &[u8]) -> Result<usize, String> {
    if d.len() < 48 || &d[..4] != b"BEF1" {
        return Err("no es un BEF1".into());
    }
    let secs = secciones(d);
    let firma = match secs.iter().find(|s| s.kind == SECTION_SIGNATURE) {
        Some(s) => s,
        // Sin seccion Signature no hay nada que comprobar, y eso es legitimo:
        // las imagenes que el kernel EMBEBE no pasan por el escritor.
        None => return Ok(0),
    };
    if firma.file_offset + 8 > d.len() {
        return Err("la seccion Signature cae fuera del fichero".into());
    }
    let cuantos = u32_en(d, firma.file_offset) as usize;
    let mut ok = 0usize;
    for j in 0..cuantos {
        let base = firma.file_offset + 8 + j * 40;
        if base + 40 > d.len() {
            return Err(format!("el hash {j} cae fuera del fichero"));
        }
        let idx = u16_en(d, base) as usize;
        let esperado = &d[base + 8..base + 40];
        let Some(s) = secs.get(idx) else {
            return Err(format!(
                "el hash {j} apunta a la seccion {idx}, que no existe (hay {})",
                secs.len()
            ));
        };
        // La seccion Signature no se hashea a si misma. Si aparece, el fichero
        // esta mal armado y decirlo aqui es mejor que un digest que nunca cuadra.
        if s.kind == SECTION_SIGNATURE {
            return Err(format!("el hash {j} apunta a la propia seccion Signature"));
        }
        let fin = s.file_offset + s.file_size;
        if fin > d.len() {
            return Err(format!("la seccion {idx} cae fuera del fichero"));
        }
        let bytes = &d[s.file_offset..fin];
        let calculado = bmo_abi::bef::signing::blake3_256(bytes);
        if &calculado[..] != esperado {
            return Err(format!(
                "seccion {idx} (kind {:#04x}, {} B): el digest NO cuadra\n     \
                 dice     {}\n     y mide   {}",
                s.kind,
                s.file_size,
                hex8(esperado),
                hex8(&calculado[..])
            ));
        }
        ok += 1;
    }
    Ok(ok)
}

fn hex8(b: &[u8]) -> String {
    b.iter().take(8).map(|x| format!("{x:02x}")).collect()
}

fn recoger(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            recoger(&p, out);
        } else if p.extension().map(|x| x == "bex").unwrap_or(false) {
            out.push(p);
        }
    }
}

/// **Todos los `.bex` de `staging`, contra sus propios hashes.**
#[test]
fn los_bex_desplegables_cuadran_con_su_firma() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../Ultra_kernel_x86-64/staging/BMO-DATA");
    if !raiz.exists() {
        eprintln!("sin staging: no hay nada que comprobar (build.ps1 no ha corrido)");
        return;
    }
    let mut ficheros = Vec::new();
    recoger(&raiz, &mut ficheros);
    ficheros.sort();
    assert!(!ficheros.is_empty(), "hay staging pero ni un .bex dentro");

    let mut malos = Vec::new();
    for f in &ficheros {
        let d = std::fs::read(f).expect("se puede leer");
        let nombre = f.file_name().unwrap().to_string_lossy().to_string();
        match comprobar(&d) {
            Ok(0) => eprintln!("  {nombre:16} sin seccion Signature"),
            Ok(n) => eprintln!("  {nombre:16} {n} hashes OK"),
            Err(e) => {
                eprintln!("  {nombre:16} ** {e}");
                malos.push(format!("{nombre}: {e}"));
            }
        }
    }
    assert!(
        malos.is_empty(),
        "hay .bex desplegables cuya firma NO cuadra con su contenido:\n{}",
        malos.join("\n")
    );
}
