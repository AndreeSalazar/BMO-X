//! **Traducir un desplazamiento a `funcion+N`, leyendo el `.bex` del muerto.**
//!
//! # Por que esto vive en Ring 3 y no en el kernel
//!
//! La autopsia del kernel da un `rip` y su desplazamiento en la imagen
//! (`+0x815f2`). Ponerle nombre exige **abrir el binario y leer su tabla de
//! simbolos**, o sea tocar el disco.
//!
//! Y el manejador de fallos no puede tocar el disco. La razon esta escrita en
//! `ring0/core/autopsy.rs` y no es prudencia general: **el fallo puede ser del
//! disco**. Un informe que necesita el subsistema que acaba de caerse no es un
//! informe -- es un segundo fallo encima del primero.
//!
//! Asi que el reparto queda como el resto del sistema:
//!
//! ```text
//!   el kernel   CAPTURA numeros crudos, dentro del fallo, sin tocar nada
//!   Ring 3      los INTERPRETA despues, vivo, con el disco disponible
//! ```
//!
//! # Y por que se lee A MANO, sin `bmo-abi`
//!
//! `bmo-userland` no tiene dependencias, y eso esta escrito en su `Cargo.toml`
//! como decision: *"un proceso Ring 3 no comparte structs con el kernel"*.
//! El compositor ya lee el formato BEF a mano para sacar los iconos
//! (`scene/launcher.rs`), con el mismo argumento que usa el kernel: **dos
//! lectores del mismo formato es lo que obliga a que el formato este escrito.**
//!
//! Los offsets salen de `bef/header.rs`, `bef/sections.rs` y `bef/symbols.rs`.
//! Si alguno cambia, esto deja de resolver nombres -- y se nota en la primera
//! autopsia, que es lo mejor que le puede pasar a una divergencia.
//!
//! # No se carga la seccion entera, se RECORRE
//!
//! Un `.bex` grande tiene miles de funciones: la tabla de DOOM pasa de 50 KiB y
//! aqui no hay `alloc`. Se leen las entradas **por tandas** en un buffer fijo y
//! se para en cuanto una contiene el desplazamiento buscado. Memoria constante,
//! y en el caso normal ni se llega al final.

use bmo_userland as bmo;

// [!] 0x08, y este numero se comprobo contra un `.bex` de verdad antes de
// creerselo. La primera version puso 0x06 --que es `Exports`-- y el lector
// devolvia `None` siempre: indistinguible de "este binario no trae tabla".
// Un desplazamiento inventado en un lector escrito a mano no da error, da
// SILENCIO. Ver `scratchpad/valida_lector.py`.
const SECTION_SYMBOLS: u8 = 0x08;
/// Una entrada de la tabla de secciones.
const SECTION_ENTRY: usize = 48;
/// Un `Symbol`: `name_off` u32, `name_hash` u32, `virt_addr` u64, `size` u64,
/// `kind` u8, `binding` u8, `visibility` u8, `section_idx` u8, `_reserved` u32.
const SYMBOL: usize = 32;
/// `TablaCadenas`: `count` u32 + reservado u32.
const TABLA_CADENAS: usize = 8;
/// `SymbolKind::Function`.
const KIND_FUNCTION: u8 = 0x01;

/// Cuantos simbolos se leen de una vez. Dieciseis entradas son 512 bytes: cabe
/// de sobra en la pila de Ring 3 y son 16 candidatos por lectura de disco.
const POR_TANDA: usize = 16;

/// Lo mas largo que se copia de un nombre. Los de DOOM mas largos rondan los
/// treinta; con cuarenta y ocho no se corta ninguno de los que importan, y si
/// alguno se corta se nota porque el nombre queda a medias, no porque mienta.
pub const NOMBRE_MAX: usize = 48;

fn u32_en(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn u64_en(b: &[u8], o: usize) -> u64 {
    let mut v = 0u64;
    for i in (0..8).rev() {
        v = (v << 8) | b[o + i] as u64;
    }
    v
}

/// Donde vive la tabla de simbolos de un `.bex`, o `None` si no la trae.
///
/// Devuelve `(offset_de_la_seccion, cuantos_simbolos, offset_de_las_cadenas)`.
fn localizar(f: &bmo::Archivo) -> Option<(u64, usize, u64)> {
    let mut cab = [0u8; 48];
    if f.read(&mut cab) < 48 || &cab[0..4] != b"BEF1" {
        return None;
    }
    let tabla = u64_en(&cab, 32);
    let count = u32_en(&cab, 40) as usize;
    if count == 0 || count > 255 {
        return None;
    }

    let mut sec_off = 0u64;
    let mut sec_len = 0u64;
    for i in 0..count {
        f.saltar(tabla + (i * SECTION_ENTRY) as u64);
        let mut e = [0u8; SECTION_ENTRY];
        if f.read(&mut e) < SECTION_ENTRY {
            return None;
        }
        if e[0] == SECTION_SYMBOLS {
            sec_off = u64_en(&e, 8);
            sec_len = u64_en(&e, 16);
            break;
        }
    }
    if sec_len < TABLA_CADENAS as u64 {
        return None;
    }

    // La cabecera dice CUANTAS entradas hay. Sin ella no se sabe donde acaban
    // las entradas y empiezan las cadenas -- que es el fallo que `TablaCadenas`
    // vino a cerrar el 2026-08-14.
    f.saltar(sec_off);
    let mut h = [0u8; TABLA_CADENAS];
    if f.read(&mut h) < TABLA_CADENAS {
        return None;
    }
    let n = u32_en(&h, 0) as usize;
    let fin_entradas = TABLA_CADENAS as u64 + (n * SYMBOL) as u64;
    // Un `count` inventado haria recorrer cadenas creyendo que son entradas.
    if n == 0 || fin_entradas > sec_len {
        return None;
    }
    Some((sec_off, n, sec_off + fin_entradas))
}

/// **El nombre de la funcion que contiene `desp`, y cuanto le sobra.**
///
/// `desp` es el desplazamiento DENTRO de la imagen, que es justo lo que la
/// autopsia imprime como `+0x815f2`.
///
/// `None` si el binario no se puede abrir, no trae tabla, o **si el
/// desplazamiento no cae dentro de ninguna funcion** -- y ese ultimo caso es
/// una respuesta, no un fallo: significa que ese `rip` esta en tierra de nadie,
/// que es un diagnostico en si mismo.
pub fn resolver(ruta: &[u8], desp: u64, nombre: &mut [u8; NOMBRE_MAX]) -> Option<u64> {
    let Ok(f) = bmo::Archivo::leer_de(ruta) else {
        return None;
    };
    let (sec_off, n, cadenas) = localizar(&f)?;

    let mut tanda = [0u8; POR_TANDA * SYMBOL];
    let mut i = 0usize;
    while i < n {
        let cuantos = POR_TANDA.min(n - i);
        f.saltar(sec_off + TABLA_CADENAS as u64 + (i * SYMBOL) as u64);
        if f.read(&mut tanda[..cuantos * SYMBOL]) < cuantos * SYMBOL {
            return None;
        }
        for k in 0..cuantos {
            let e = k * SYMBOL;
            if tanda[e + 24] != KIND_FUNCTION {
                continue;
            }
            let addr = u64_en(&tanda, e + 8);
            let size = u64_en(&tanda, e + 16);
            if desp < addr || desp >= addr + size {
                continue;
            }
            // Encontrada. El nombre esta en el blob, acabado en cero.
            let name_off = u32_en(&tanda, e) as u64;
            f.saltar(cadenas + name_off);
            let leidos = f.read(&mut nombre[..]);
            if leidos == 0 {
                return None;
            }
            // Cortar en el cero. Si no hay ninguno, el nombre era mas largo que
            // el buffer: se queda lo que cupo, y se ve que esta cortado.
            let fin = nombre[..leidos].iter().position(|&b| b == 0).unwrap_or(leidos);
            for b in nombre[fin..].iter_mut() {
                *b = 0;
            }
            return Some(desp - addr);
        }
        i += cuantos;
    }
    None
}

/// **Donde buscar el `.bex` de un programa del que solo se sabe el nombre.**
///
/// La autopsia dice `programa  doom.bex`, no su ruta: el kernel guarda la ruta
/// de lanzamiento por pid y la suelta al morir el proceso. Asi que aqui se
/// prueban los dos sitios de los que se lanza algo en esta maquina.
///
/// [!] Si algun dia hay un tercero, esto deja de resolver y **no miente**:
/// devuelve `None` y el informe se queda con el numero crudo, que es lo que
/// tenia antes.
pub fn rutas_probables(nombre: &[u8], buf: &mut [u8; 64]) -> [(usize, usize); 2] {
    let mut fin = [(0usize, 0usize); 2];
    let mut p = 0usize;
    for (i, prefijo) in [b"apps/".as_slice(), b"sys/".as_slice()].iter().enumerate() {
        let ini = p;
        for &c in prefijo.iter().chain(nombre.iter()) {
            if p < buf.len() {
                buf[p] = c;
                p += 1;
            }
        }
        fin[i] = (ini, p);
    }
    fin
}

/// **El nombre del programa, sacado del renglon `programa` del informe.**
///
/// El renglon es `programa  doom.bex   pid 2 tid 4`, asi que el nombre es la
/// segunda palabra. Se corta en el primer espacio: `pid` no forma parte de
/// ninguna ruta.
pub fn programa_de(linea: &[u8]) -> Option<(usize, usize)> {
    let etiqueta = b"programa";
    if linea.len() < etiqueta.len() || &linea[..etiqueta.len()] != etiqueta {
        return None;
    }
    let mut i = etiqueta.len();
    while i < linea.len() && linea[i] == b' ' {
        i += 1;
    }
    let ini = i;
    while i < linea.len() && linea[i] != b' ' {
        i += 1;
    }
    if i == ini || linea[ini] == b'(' {
        return None; // "(desconocido)": no hay binario que abrir
    }
    Some((ini, i))
}

/// **La linea de resolucion de un renglon del informe**, o `0` si no hay nada
/// que resolver.
///
/// Busca todos los `+0x...` del renglon --que es como la autopsia escribe un
/// desplazamiento en la imagen-- y los cambia por `nombre+0xN`.
///
/// ** Va en su PROPIA linea y no sustituye a la del kernel, y eso es
/// deliberado: lo que escribio el kernel es la PRUEBA, y esto es una
/// INTERPRETACION que hace Ring 3 leyendo un fichero que puede estar
/// desactualizado. Si un dia el `.bex` del disco no es el que se ejecuto, se
/// vera que las dos lineas no cuadran -- y eso es informacion. Machacando la
/// original no quedaria nada con que discrepar.
pub fn anotar(linea: &[u8], ruta: &[u8], out: &mut [u8]) -> usize {
    let mut n = 0usize;
    let mut pon = |b: &[u8], n: &mut usize| {
        for &c in b {
            if *n < out.len() {
                out[*n] = c;
                *n += 1;
            }
        }
    };

    let mut i = 0usize;
    let mut hallados = 0usize;
    while i + 3 <= linea.len() {
        if &linea[i..i + 3] != b"+0x" {
            i += 1;
            continue;
        }
        // *** UN `+0x` PEGADO A UN CERO NO ES UN DESPLAZAMIENTO EN LA IMAGEN.
        //
        // ** El 2026-08-30 DOOM murio con esto en pantalla:
        //
        // ```text
        //    veredicto *** PUNTERO NULO en 0+0x2c
        //              -> bmo_valor+0x2c
        // ```
        //
        // Las dos lineas se contradicen y la segunda la escribio ESTA funcion.
        // `0+0x2c` es *"base cero, campo 0x2c"* -- el desplazamiento de un
        // CAMPO dentro de una estructura, no una posicion dentro del binario.
        // Se resolvio igual porque aqui se buscaba `+0x` sin mirar a que estaba
        // pegado.
        //
        // *** Y no fallo por casualidad, falla SIEMPRE: en todo `.bex` de esta
        // casa las primeras funciones son las del runtime, linkadas antes que
        // nada:
        //
        // ```text
        //    0x0    bmo_valor    0x53 bytes
        //    0x53   bmo_codigo   0x50
        //    0xA3   bmo_pid      0x4E
        // ```
        //
        // Asi que **cualquier puntero nulo con campo menor de 0x53, en
        // cualquier programa, decia `bmo_valor`** y mandaba a leer el runtime.
        // Un nombre equivocado no es ruido: es una tarde.
        //
        // [!] Se mira que el cero sea el numero ENTERO --que lo de antes no sea
        // otro digito-- para no tragarse un `0x40075030+0x2c` legitimo.
        if i > 0 && linea[i - 1] == b'0' {
            let anterior = if i >= 2 { linea[i - 2] } else { b' ' };
            let es_digito = anterior.is_ascii_hexdigit() || anterior == b'x';
            if !es_digito {
                i += 3;
                continue;
            }
        }
        let mut j = i + 3;
        let mut desp = 0u64;
        while j < linea.len() {
            let d = match linea[j] {
                c @ b'0'..=b'9' => c - b'0',
                c @ b'a'..=b'f' => c - b'a' + 10,
                c @ b'A'..=b'F' => c - b'A' + 10,
                _ => break,
            };
            desp = (desp << 4) | d as u64;
            j += 1;
        }
        if j == i + 3 {
            i = j;
            continue;
        }
        let mut nombre = [0u8; NOMBRE_MAX];
        if let Some(sobra) = resolver(ruta, desp, &mut nombre) {
            if hallados == 0 {
                pon(b"          -> ", &mut n);
            } else {
                pon(b"   ", &mut n);
            }
            let fin = nombre.iter().position(|&b| b == 0).unwrap_or(NOMBRE_MAX);
            pon(&nombre[..fin], &mut n);
            if sobra > 0 {
                pon(b"+0x", &mut n);
                // El sobrante en hexadecimal, sin ceros de delante.
                let mut visto = false;
                for k in (0..16).rev() {
                    let d = ((sobra >> (k * 4)) & 0xF) as u8;
                    if d != 0 {
                        visto = true;
                    }
                    if visto || k == 0 {
                        pon(&[if d < 10 { b'0' + d } else { b'a' + d - 10 }], &mut n);
                    }
                }
            }
            hallados += 1;
        }
        i = j;
    }
    if hallados == 0 {
        0
    } else {
        n
    }
}
