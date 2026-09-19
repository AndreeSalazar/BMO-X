//! **El metro del emisor de x86-64** (2026-09-18).
//!
//! La ley de `OPTIMIZACION_MAESTRO.md` pone el orden: CORRECTO, MEDIDO, RAPIDO.
//! El emisor es "calculo puro" -- el unico camino donde la tabla dice que SI se
//! cuentan ciclos --, y hasta hoy no tenia metro: ninguna optimizacion podia
//! demostrar lo que ganaba, ni ningun cambio lo que empeoraba.
//!
//! Esto compila un banco FIJO de programas reales con los emisores de verdad
//! (C y C++ por objeto + enlace, como el build), los ejecuta en el emulador y
//! apunta tres numeros por programa:
//!
//!   pasos    instrucciones que ejecuto hasta `EXIT`. Determinista: sale igual
//!            en cualquier maquina que compile, sin el Ryzen.
//!   codigo   bytes de la seccion de codigo del `.bex` que se entrega.
//!   salida   la huella de lo que imprimio.
//!
//! `LINEA_BASE.txt` es un TRINQUETE: `pasos` y `codigo` solo pueden bajar, y
//! `salida` no puede cambiar NUNCA. Una optimizacion que cambia lo que imprime
//! un programa no es una optimizacion.
//!
//!   metro            la tabla
//!   metro --check    el juicio del build: 0 si nada sube ni cambia
//!   metro --fijar    reescribe la linea base (cuando algo BAJO)
//!   metro --desglose A DONDE se van las instrucciones, por clase y por
//!                    lenguaje (`emu::clases`). No es trinquete: es el dato
//!                    que elige la primera optimizacion, en vez de elegirla
//!                    quien mira
//!   metro --caliente <programa del banco> [salida.bin]
//!                    las direcciones que MAS se ejecutan, con su clase, y
//!                    el codigo volcado a un fichero para desensamblarlo
//!                    (llvm-objcopy -I binary -O elf64-x86-64 + llvm-objdump)
//!
//! [!] Lo que NO mide todavia, dicho: INTI (su cadena son seis pasos y sus
//! programas de ejemplo tocan hardware), y los ciclos del Ryzen -- las
//! instrucciones no son ciclos; eso lo miden `c/coste.bex` y `c/ciclos.bex` en
//! el metal.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::exit;

use bmo_lower::emu::clases::{Censo, Clase};

/// Instrucciones como mucho por programa. Un programa del banco que no termina
/// es un fallo del emisor, no del metro.
const LIMITE: usize = 50_000_000;

/// El banco. **Fijo a proposito**: si cambia, la linea base se rehace entera.
/// Fuera, con su motivo:
///   c/ciclos_C.c, c/coste_C.c   opcodes que el emulador no ejecuta (rdtsc)
const BANCO: &[(&str, &str)] = &[
    ("c", "toolchain/lang/c/examples/hola_C.c"),
    ("c", "toolchain/lang/c/examples/memoria_C.c"),
    ("c", "toolchain/lang/c/examples/vivaldi_C.c"),
    ("c", "toolchain/lang/c/examples/blit_C.c"),
    ("c", "toolchain/lang/c/examples/sonido_C.c"),
    ("c", "toolchain/lang/c/examples/musica_C.c"),
    ("c", "toolchain/lang/c/examples/scroll_C.c"),
    ("c", "toolchain/lang/c/examples/caja_C.c"),
    ("c", "toolchain/lang/c/examples/leer_C.c"),
    ("c", "toolchain/lang/c/examples/cubo_C.c"),
    ("c", "toolchain/lang/c/examples/guia_C.c"),
    ("c", "toolchain/lang/c/examples/imagen_C.c"),
    ("c", "toolchain/lang/c/examples/raycaster_C.c"),
    ("c", "toolchain/lang/c/examples/sonda_C.c"),
    ("cpp", "toolchain/lang/cpp/examples/1-clases/cuentas.cpp"),
    ("cobol", "toolchain/lang/cobol/examples/1-basico/hola.cob"),
    ("cobol", "toolchain/lang/cobol/examples/2-decimal/banco.cob"),
    ("cobol", "toolchain/lang/cobol/examples/2-decimal/hola_COBOL.cob"),
    ("cobol", "toolchain/lang/cobol/examples/3-presentacion/extracto.cob"),
    ("cobol", "toolchain/lang/cobol/examples/5-tablas/conceptos.cob"),
    ("cobol", "toolchain/lang/cobol/examples/6-condiciones/cartera.cob"),
    ("cobol", "toolchain/lang/cobol/examples/7-empaquetado/cuentas.cob"),
    ("cobol", "toolchain/lang/cobol/examples/8-parrafos/cierre.cob"),
    ("cobol", "toolchain/lang/cobol/examples/9-decision/comision.cob"),
    ("ada", "toolchain/lang/ada/examples/1-basico/cierre.adb"),
];

#[derive(Debug, Clone, PartialEq)]
struct Medida {
    pasos: u64,
    codigo: u64,
    salida: String,
    /// A donde se fueron los `pasos`. No va a la linea base: es diagnostico.
    censo: Censo,
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("..")
}

fn compilar(lenguaje: &str, ruta: &Path, fuente: &str) -> Result<Vec<u8>, String> {
    let nombre = ruta.file_name().and_then(|n| n.to_str()).unwrap_or("unidad").to_string();
    let enlazar = |objeto: Vec<u8>| {
        bmo_enlazar::enlazar(&[(nombre.clone(), objeto)]).map_err(|e| format!("enlace: {e:?}"))
    };
    match lenguaje {
        "c" => enlazar(bmo_c_x86_64::compile_object_with_preprocessor(
            fuente, ruta, bmo_c_x86_64::CStandard::C11, bmo_c_x86_64::Libc::Copia)
            .map_err(|e| format!("linea {}: {}", e.line, e.message))?),
        "cpp" => enlazar(bmo_cpp_x86_64::compilar_fichero(fuente, ruta, true)
            .map_err(|e| format!("linea {}: {}", e.line, e.message))?),
        "cobol" => bmo_cobol_x86_64::compile_source_to_bex(fuente)
            .map_err(|e| format!("{e:?}")),
        "ada" => bmo_ada_x86_64::compilar(fuente).map_err(|e| format!("{e:?}")),
        otro => Err(format!("lenguaje desconocido `{otro}`")),
    }
}

/// Los bytes de la seccion de codigo del `.bex`.
fn bytes_de_codigo(bex: &[u8]) -> u64 {
    use bmo_abi::bef::sections::{SectionEntry, SectionKind};
    let tabla = u64::from_le_bytes(bex[32..40].try_into().unwrap()) as usize;
    let cuantas = u32::from_le_bytes(bex[40..44].try_into().unwrap()) as usize;
    (0..cuantas)
        .map(|i| tabla + i * SectionEntry::SIZE)
        .filter(|&e| bex[e] == SectionKind::Code as u8)
        .map(|e| u64::from_le_bytes(bex[e + 16..e + 24].try_into().unwrap()))
        .sum()
}

/// FNV-1a de 64 bits: una huella, no una firma. Solo tiene que cambiar si la
/// salida cambia.
fn huella(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn medir(lenguaje: &str, rel: &str) -> Result<Medida, String> {
    let ruta = raiz().join(rel);
    let fuente = std::fs::read_to_string(&ruta).map_err(|e| format!("no se lee: {e}"))?;
    let bex = compilar(lenguaje, &ruta, &fuente)?;
    let maquina = bmo_lower::emu::cargar_bex(&bex)?;
    let m = std::panic::catch_unwind(move || bmo_lower::emu::run(maquina, LIMITE))
        .map_err(|_| format!("no termina en {LIMITE} instrucciones o el emulador no sabe una"))?;
    if !m.exited {
        return Err("no termino por EXIT".into());
    }
    Ok(Medida { pasos: m.pasos, codigo: bytes_de_codigo(&bex), salida: huella(&m.console), censo: m.censo })
}

fn linea_base() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("LINEA_BASE.txt")
}

fn leer_base() -> Result<BTreeMap<String, Medida>, String> {
    let texto = std::fs::read_to_string(linea_base()).map_err(|e| format!("no hay linea base: {e}"))?;
    let mut out = BTreeMap::new();
    for l in texto.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') { continue; }
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() != 4 { return Err(format!("linea base mal formada: `{l}`")); }
        let n = |s: &str| s.parse::<u64>().map_err(|_| format!("numero mal formado: `{s}`"));
        out.insert(c[0].to_string(), Medida { pasos: n(c[1])?, codigo: n(c[2])?, salida: c[3].to_string(), censo: Censo::default() });
    }
    Ok(out)
}

/// **El juicio**: que subio, que cambio y que falta. `(quejas, cuantos bajaron)`.
fn juzgar(medidas: &BTreeMap<String, Medida>, base: &BTreeMap<String, Medida>) -> (Vec<String>, usize) {
    let mut quejas = Vec::new();
    let mut bajaron = 0;
    for (p, m) in medidas {
        match base.get(p) {
            None => quejas.push(format!("{p}: no esta en la linea base (fijala con --fijar)")),
            Some(b) => {
                if m.salida != b.salida {
                    quejas.push(format!("{p}: LA SALIDA CAMBIO ({} -> {}): una optimizacion no cambia lo que hace un programa", b.salida, m.salida));
                }
                if m.pasos > b.pasos {
                    quejas.push(format!("{p}: instrucciones SUBEN {} -> {}", b.pasos, m.pasos));
                }
                if m.codigo > b.codigo {
                    quejas.push(format!("{p}: codigo SUBE {} -> {} B", b.codigo, m.codigo));
                }
                if m.pasos < b.pasos || m.codigo < b.codigo { bajaron += 1; }
            }
        }
    }
    for p in base.keys() {
        if !medidas.contains_key(p) {
            quejas.push(format!("{p}: esta en la linea base y ya no en el banco"));
        }
    }
    (quejas, bajaron)
}

/// Una fila del desglose: cada clase como porcentaje del total de esa fila.
fn fila_desglose(nombre: &str, c: &Censo) {
    let t = c.total().max(1) as f64;
    print!("{nombre:<12} {:>10}", c.total());
    for k in Clase::TODAS {
        print!(" {:>6.1}", c.de(k) as f64 * 100.0 / t);
    }
    println!();
}

/// `--desglose`: por lenguaje y en total. Por programa NO, a proposito: el
/// banco lo dominan tres programas de C (blit, memoria, vivaldi) y una tabla de
/// 25 filas esconde eso en vez de ensenarlo; por lenguaje es donde se decide
/// que emisor se toca primero.
fn desglose(medidas: &BTreeMap<String, Medida>) {
    let mut por_lenguaje: BTreeMap<&str, Censo> = BTreeMap::new();
    let mut total = Censo::default();
    for (lenguaje, rel) in BANCO {
        if let Some(m) = medidas.get(*rel) {
            por_lenguaje.entry(lenguaje).or_default().sumar(&m.censo);
            total.sumar(&m.censo);
        }
    }
    print!("{:<12} {:>10}", "lenguaje", "pasos");
    for k in Clase::TODAS { print!(" {:>6}", k.nombre()); }
    println!("   (% de los pasos de esa fila)");
    for (l, c) in &por_lenguaje { fila_desglose(l, c); }
    fila_desglose("TOTAL", &total);
    // Y la conclusion en una linea, porque es lo que se lee.
    let (pila, marco) = (total.de(Clase::Pila), total.de(Clase::Marco));
    println!(
        "
marco + pila = {} de {} ({:.1} %): lo que un reparto de registros quitaria como techo",
        pila + marco, total.total(), (pila + marco) as f64 * 100.0 / total.total().max(1) as f64
    );
}

/// `--caliente`: el perfil de UN programa, direccion por direccion.
fn caliente(rel: &str, salida: Option<&str>) {
    let Some((lenguaje, _)) = BANCO.iter().find(|(_, r)| *r == rel) else {
        println!("`{rel}` no esta en el banco del metro");
        exit(1);
    };
    let ruta = raiz().join(rel);
    let fuente = std::fs::read_to_string(&ruta).expect("leer el fuente");
    let bex = compilar(lenguaje, &ruta, &fuente).expect("compilar");
    let maquina = bmo_lower::emu::cargar_bex(&bex).expect("cargar");
    let codigo = maquina.code.clone();
    let mut cuentas: BTreeMap<usize, u64> = BTreeMap::new();
    let m = bmo_lower::emu::run_con(maquina, LIMITE, |rip| *cuentas.entry(rip).or_default() += 1);
    if let Some(s) = salida {
        std::fs::write(s, &codigo).expect("escribir el codigo");
        println!("codigo volcado en {s} ({} B)", codigo.len());
    }
    // Las 40 direcciones mas calientes, en ORDEN DE DIRECCION: asi el bucle
    // se lee de arriba abajo, con su clase al lado.
    let mut top: Vec<(usize, u64)> = cuentas.iter().map(|(&a, &c)| (a, c)).collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    top.truncate(40);
    top.sort();
    println!("{:>8}  {:>10}  {:<10}  bytes        ({} pasos en total)", "rip", "veces", "clase", m.pasos);
    for (a, c) in top {
        let clase = bmo_lower::emu::clases::clasificar(&codigo[a..]).nombre();
        let bytes: Vec<String> = codigo[a..(a + 8).min(codigo.len())].iter().map(|b| format!("{b:02x}")).collect();
        println!("{a:>8x}  {c:>10}  {clase:<10}  {}", bytes.join(" "));
    }
}

fn main() {
    let modo = std::env::args().nth(1).unwrap_or_default();
    if modo == "--caliente" {
        let rel = std::env::args().nth(2).unwrap_or_default();
        let salida = std::env::args().nth(3);
        caliente(&rel, salida.as_deref());
        return;
    }
    let mut medidas = BTreeMap::new();
    let mut rotos = Vec::new();
    for (lenguaje, rel) in BANCO {
        match medir(lenguaje, rel) {
            Ok(m) => { medidas.insert(rel.to_string(), m); }
            Err(e) => rotos.push(format!("{rel}: {e}")),
        }
    }
    if !rotos.is_empty() {
        for r in &rotos { println!("  [X] {r}"); }
        println!("metro: {} programa(s) del banco no se pudieron medir", rotos.len());
        exit(1);
    }
    let (pasos, codigo): (u64, u64) = medidas.values().fold((0, 0), |a, m| (a.0 + m.pasos, a.1 + m.codigo));

    match modo.as_str() {
        "--fijar" => {
            let mut t = String::from(
                "# EL METRO DEL EMISOR -- linea base (toolchain/tools/metro).\n\
                 # TRINQUETE: `pasos` y `codigo` solo bajan; `salida` no cambia nunca.\n\
                 # Se reescribe con `cargo run --release -p bmo-metro -- --fijar`.\n\
                 # programa  pasos  codigo  salida\n");
            for (p, m) in &medidas {
                t.push_str(&format!("{p} {} {} {}\n", m.pasos, m.codigo, m.salida));
            }
            std::fs::write(linea_base(), t).expect("escribir la linea base");
            println!("metro: linea base fijada -- {} programas, {pasos} instrucciones, {codigo} B de codigo", medidas.len());
        }
        "--desglose" => desglose(&medidas),
        "--check" => {
            let base = match leer_base() {
                Ok(b) => b,
                Err(e) => { println!("guardian MUERTO: {e}"); exit(1); }
            };
            let (quejas, bajaron) = juzgar(&medidas, &base);
            if !quejas.is_empty() {
                for q in &quejas { println!("  [X] {q}"); }
                println!("metro: {} incumplimiento(s)", quejas.len());
                exit(1);
            }
            let aviso = if bajaron > 0 { format!("; {bajaron} bajaron: fija con --fijar") } else { String::new() };
            println!("clean: metro del emisor -- {} programas (C, C++, COBOL, Ada), {pasos} instrucciones, {codigo} B de codigo, ninguna salida cambio{aviso}", medidas.len());
        }
        _ => {
            println!("{:<58} {:>12} {:>9}  salida", "programa", "pasos", "codigo");
            for (p, m) in &medidas {
                println!("{p:<58} {:>12} {:>9}  {}", m.pasos, m.codigo, m.salida);
            }
            println!("{:<58} {pasos:>12} {codigo:>9}", "TOTAL");
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(pasos: u64, codigo: u64, salida: &str) -> Medida {
        Medida { pasos, codigo, salida: salida.into(), censo: Censo::default() }
    }
    fn uno(x: Medida) -> BTreeMap<String, Medida> {
        BTreeMap::from([("p".to_string(), x)])
    }

    #[test]
    fn igual_es_limpio() {
        assert_eq!(juzgar(&uno(m(10, 20, "a")), &uno(m(10, 20, "a"))), (vec![], 0));
    }
    #[test]
    fn bajar_es_limpio_y_se_cuenta() {
        let (q, b) = juzgar(&uno(m(9, 19, "a")), &uno(m(10, 20, "a")));
        assert!(q.is_empty());
        assert_eq!(b, 1);
    }
    #[test]
    fn subir_instrucciones_se_caza() {
        assert_eq!(juzgar(&uno(m(11, 20, "a")), &uno(m(10, 20, "a"))).0.len(), 1);
    }
    #[test]
    fn subir_codigo_se_caza() {
        assert_eq!(juzgar(&uno(m(10, 21, "a")), &uno(m(10, 20, "a"))).0.len(), 1);
    }
    /// La que mas importa: mas rapido y DISTINTO no es mas rapido, es roto.
    #[test]
    fn cambiar_la_salida_se_caza_aunque_baje() {
        let (q, _) = juzgar(&uno(m(5, 10, "b")), &uno(m(10, 20, "a")));
        assert!(q.iter().any(|x| x.contains("LA SALIDA CAMBIO")));
    }
    #[test]
    fn un_programa_que_falta_o_sobra_se_caza() {
        assert_eq!(juzgar(&uno(m(1, 1, "a")), &BTreeMap::new()).0.len(), 1);
        assert_eq!(juzgar(&BTreeMap::new(), &uno(m(1, 1, "a"))).0.len(), 1);
    }
}
