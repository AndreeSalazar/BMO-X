//! **LA FUENTE EN INTI DICE LOS MISMOS BYTES QUE LA FUENTE EN C.**
//!
//! N0b de `docs/plan/PLAN_NAVEGAR.md` (2026-09-16). `toolchain/tools/fontgen`
//! escribe la tabla de glifos cuatro veces del mismo arte: Ring 0, C (REX) e
//! INTI (`runtime/fuente/datos.inti`, lo que trae `usa fuente`). Esta prueba
//! no compara los ficheros: **corre el `.ibx`** y le pregunta a `glifo_fila`
//! cada fila de cada glifo, y a `glifo_de` cada byte Latin-1, y lo compara con
//! lo que dice `fuente/datos.h` leido como texto.
//!
//! ** Es la forma concreta de "C e INTI cooperan sin enlazarse": no comparten
//! codigo, comparten la tabla generada, y una prueba exige que la lean igual.
//! Si un dia INTI empaquetara las palabras al reves, o `glifo_de` se saltara
//! un extra, aqui saldria con el glifo y la fila.

use std::path::PathBuf;

use bmo_lower::emu::{run, Machine};

/// El programa: pregunta glifo a glifo y fila a fila, y lo escribe por la
/// consola de ocho en ocho (dos palabras por glifo, las mismas que la tabla).
/// Despues, `glifo_de` para los 256 bytes, un byte por palabra.
const PROGRAMA: &str = "\
perfil llano
usa bmo
usa fuente

funcion palabra(w es natural64)
    invoca(mi_tarea, op_consola_escribir, w, 0, 0)

funcion principal devuelve entero32
    cambiante g es natural64 = 0
    repite mientras g < FUENTE_GLIFOS
        cambiante lo es natural64 = 0
        cambiante hi es natural64 = 0
        cambiante f es natural64 = 0
        repite mientras f < 8
            lo = lo + glifo_fila(g, f) * potencia(f)
            hi = hi + glifo_fila(g, f + 8) * potencia(f)
            f = f + 1
        palabra(lo)
        palabra(hi)
        g = g + 1
    cambiante b es natural64 = 0
    repite mientras b < 256
        palabra(glifo_de(b) + 256)
        b = b + 1
    devuelve 0

funcion potencia(f es natural64) devuelve natural64
    cambiante p es natural64 = 1
    cambiante k es natural64 = 0
    repite mientras k < f
        p = p * 256
        k = k + 1
    devuelve p
";

fn emitido(texto: &str) -> bmo_inti_x86_64::Emitido {
    let arbol = bmo_inti_front::armar(texto);
    assert!(!arbol.hay_errores(), "el programa no se lee: {}", arbol.pintar("fuente.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let nec = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec).valor;
    bmo_inti_x86_64::emitir(&ir)
}

/// La maquina con el codigo Y la tabla congelada en su sitio.
///
/// ** Sin esto, `GLIFOS` se lee de la direccion CERO --o sea, del codigo-- y
/// `glifo_fila` devuelve opcodes: `0x48, 0x8a, 0x8b`. Es la misma trampa que
/// `pruebas.rs::ejecuta_en` ya conto el 23-08 ("una tabla congelada se leia de
/// la direccion cero"), y ninguna prueba de `tests/` la habia cruzado porque
/// ninguna usaba una constante. Aqui no hay `crt0` delante: el hueco esta en
/// `off`, no en `off + 5`.
fn maquina(e: &bmo_inti_x86_64::Emitido) -> Machine {
    let mut m = Machine::new(e.codigo.clone());
    let (rodata, donde) = bmo_inti_x86_64::rodata_de(e);
    if !rodata.is_empty() {
        let base = m.load_data(&rodata);
        for (off, i) in &e.reubicaciones {
            if let Some(d) = donde.get(*i as usize) {
                m.code[*off..*off + 8].copy_from_slice(&(base + d).to_le_bytes());
            }
        }
    }
    m
}

/// Las palabras que el programa escribio por consola, en orden.
fn palabras(m: &Machine) -> Vec<u64> {
    m.syscalls
        .iter()
        .filter(|c| c.operation == 0x06 && c.capability == 0xFFFF_FFFF_FFFF_FFFE)
        .map(|c| c.arg0)
        .collect()
}

/// `fuente/datos.h` leido como texto: los 120 x 16 bytes y los 25 bytes Latin-1.
fn tabla_de_c() -> (Vec<u8>, Vec<u8>) {
    let ruta = PathBuf::from("../../../forge/sem-asm/tables/bmo/fuente/datos.h");
    let texto = std::fs::read_to_string(&ruta).expect("no encuentro fuente/datos.h");
    let hex = |trozo: &str| -> Vec<u8> {
        trozo
            .split(',')
            .map(|s| s.trim())
            .filter(|s| s.starts_with("0x"))
            .map(|s| u8::from_str_radix(&s[2..4], 16).unwrap())
            .collect()
    };
    let glifos = {
        let a = texto.find("bmo_fuente_glifos[").unwrap();
        let a = texto[a..].find('{').unwrap() + a + 1;
        let b = texto[a..].find("};").unwrap() + a;
        // Cada linea lleva un comentario /* ASCII n */ que no empieza por 0x.
        texto[a..b].lines().flat_map(|l| hex(l.split("/*").next().unwrap())).collect::<Vec<u8>>()
    };
    let latin1 = {
        let a = texto.find("bmo_fuente_latin1[").unwrap();
        let a = texto[a..].find('{').unwrap() + a + 1;
        let b = texto[a..].find("};").unwrap() + a;
        hex(&texto[a..b])
    };
    (glifos, latin1)
}

#[test]
fn la_fuente_de_inti_y_la_de_c_son_la_misma() {
    let (glifos_c, latin1_c) = tabla_de_c();
    assert_eq!(glifos_c.len(), 120 * 16, "datos.h tiene 120 glifos de 16 filas");
    assert_eq!(latin1_c.len(), 25, "y 25 extras Latin-1");

    let e = emitido(PROGRAMA);
    assert!(e.arranca);
    let m = run(maquina(&e), 200_000_000);
    let dicho = palabras(&m);
    assert_eq!(dicho.len(), 120 * 2 + 256, "dos palabras por glifo y un indice por byte -- salieron {} (exited {}, rip {} de {}): {:x?}", dicho.len(), m.exited, m.rip, m.code.len(), &dicho[..dicho.len().min(12)]);

    // 1. glifo a glifo, fila a fila.
    let mut mal = Vec::new();
    for g in 0..120 {
        let lo = dicho[g * 2].to_le_bytes();
        let hi = dicho[g * 2 + 1].to_le_bytes();
        for f in 0..16 {
            let inti = if f < 8 { lo[f] } else { hi[f - 8] };
            let c = glifos_c[g * 16 + f];
            if inti != c {
                mal.push(format!("glifo {} fila {}: INTI {:#04x}, C {:#04x}", g, f, inti, c));
            }
        }
    }
    assert!(mal.is_empty(), "la fuente de INTI no es la de C:\n{}", mal.join("\n"));

    // 2. glifo_de: ASCII directo, los extras por su byte, y el HUECO (`?`) para lo demas.
    for b in 0..256u64 {
        let indice = dicho[240 + b as usize] - 256;
        let esperado = if (32..=126).contains(&b) {
            b - 32
        } else if let Some(i) = latin1_c.iter().position(|&x| x as u64 == b) {
            95 + i as u64
        } else {
            31
        };
        assert_eq!(indice, esperado, "glifo_de({:#04x})", b);
    }
}

/// El programa de arriba solo lee DENTRO de la tabla: `glifo_fila` con un
/// glifo que no existe da el HUECO, no un byte de fuera.
#[test]
fn un_glifo_que_no_existe_es_el_hueco() {
    let programa = "\
perfil llano
usa bmo
usa fuente

funcion palabra(w es natural64)
    invoca(mi_tarea, op_consola_escribir, w, 0, 0)

funcion principal devuelve entero32
    cambiante f es natural64 = 0
    repite mientras f < 16
        palabra(glifo_fila(999, f) + 256)
        palabra(glifo_fila(31, f) + 256)
        f = f + 1
    devuelve 0
";
    let e = emitido(programa);
    let m = run(maquina(&e), 10_000_000);
    let d = palabras(&m);
    assert_eq!(d.len(), 32);
    for f in 0..16 {
        assert_eq!(d[f * 2], d[f * 2 + 1], "fila {} del glifo 999 es la del `?`", f);
    }
    assert!(d.iter().any(|&w| w != 256), "el `?` tiene pixeles");
}
