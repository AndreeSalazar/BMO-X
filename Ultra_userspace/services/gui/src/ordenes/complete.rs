//! El TAB: completar una ruta con lo que hay en el disco.

use bmo_userland as bmo;

use crate::escena::salida::Salida;
use crate::escena::RUTA_MAX;
use crate::texto::es_punto;

pub(crate) fn complete(ruta: &mut [u8; RUTA_MAX], n: usize, salida: &mut Salida) -> usize {
    // El ultimo token: lo que hay tras el ultimo espacio. Asi `corre app<TAB>`
    // completa la ruta y no el verbo.
    let inicio = ruta[..n].iter().rposition(|&c| c == b' ').map_or(0, |i| i + 1);
    // * La carpeta y el prefijo se COPIAN a locales antes de tocar nada.
    // Tomarlos prestados de `ruta` y luego escribir en `ruta` es exactamente
    // lo que el prestamista de Rust no deja -- y hace bien: escribir sobre lo
    // que estas leyendo es como se corrompe un buffer sin enterarse.
    let mut dir = [0u8; RUTA_MAX];
    let mut dir_n = 0usize;
    let mut prefijo = [0u8; 12];
    let mut pref_n = 0usize;
    let pref_ini;
    {
        let token = &ruta[inicio..n];
        let corte = token.iter().rposition(|&c| c == b'/' || c == b'\\');
        let (d0, pi) = match corte {
            Some(i) => (&token[..i], i + 1),
            None => (&token[0..0], 0),
        };
        pref_ini = pi;
        dir_n = d0.len().min(RUTA_MAX);
        dir[..dir_n].copy_from_slice(&d0[..dir_n]);
        let p0 = &token[pref_ini..];
        pref_n = p0.len().min(prefijo.len());
        prefijo[..pref_n].copy_from_slice(&p0[..pref_n]);
    }
    let dir = &dir[..dir_n];
    let prefijo = &prefijo[..pref_n];

    let d = match bmo::Directorio::open(dir) {
        Ok(d) => d,
        Err(_) => return n,
    };

    let baja = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
    let mut cuantos = 0usize;
    let mut comun = [0u8; 12];
    let mut comun_n = 0usize;
    let mut unico_es_dir = false;
    // Los candidatos se listan DESPUES, en una segunda pasada: guardarlos
    // todos aqui pediria un vector, y sin `alloc` eso es un array con un tope
    // inventado. Recorrer dos veces cuesta microsegundos y no inventa topes.
    let mut vueltas = 0u32;
    while vueltas < 256 {
        let e = match d.next() { Some(e) => e, None => break };
        vueltas += 1;
        let mut nom = [0u8; 12];
        let largo = e.legible(&mut nom);
        // * `.` y `..` FUERA. Eran el motivo de que el TAB no completara
        // NUNCA dentro de una carpeta: entran como candidatos, y el prefijo
        // comun de `.`, `..` y `gui.bex` es la cadena vacia. El TAB listaba
        // todo y no avanzaba ni una letra, que parecia "no busca referencias"
        // cuando lo que hacia era buscarlas y anularse solo.
        if es_punto(&nom[..largo]) { continue; }
        if largo < prefijo.len() { continue; }
        let mut cuadra = true;
        for k in 0..prefijo.len() {
            if baja(nom[k]) != baja(prefijo[k]) { cuadra = false; break; }
        }
        if !cuadra { continue; }
        if cuantos == 0 {
            comun[..largo].copy_from_slice(&nom[..largo]);
            comun_n = largo;
            unico_es_dir = e.es_dir;
        } else {
            // Recortar al prefijo comun con lo que llevabamos.
            let mut k = 0usize;
            while k < comun_n && k < largo && baja(comun[k]) == baja(nom[k]) { k += 1; }
            comun_n = k;
            unico_es_dir = false;
        }
        cuantos += 1;
    }

    if cuantos == 0 {
        return n;
    }

    // Escribir el prefijo comun en el sitio del que habia.
    let mut fin = inicio + pref_ini;
    let mut k = 0usize;
    while k < comun_n && fin < RUTA_MAX {
        ruta[fin] = comun[k];
        fin += 1;
        k += 1;
    }
    if cuantos == 1 && unico_es_dir && fin < RUTA_MAX {
        ruta[fin] = b'/';
        fin += 1;
    }

    // Con mas de uno, ENSENAR lo que hay. Es la diferencia con ciclar.
    if cuantos > 1 {
        let d2 = match bmo::Directorio::open(dir) { Ok(d) => d, Err(_) => return fin };
        let mut vueltas = 0u32;
        while vueltas < 256 {
            let e = match d2.next() { Some(e) => e, None => break };
            vueltas += 1;
            let mut nom = [0u8; 12];
            let largo = e.legible(&mut nom);
            if largo < prefijo.len() { continue; }
            let mut cuadra = true;
            for k in 0..prefijo.len() {
                if baja(nom[k]) != baja(prefijo[k]) { cuadra = false; break; }
            }
            if !cuadra { continue; }
            salida.texto(b"  ");
            salida.texto(&nom[..largo]);
            if e.es_dir { salida.byte(b'/'); }
            salida.byte(b'\n');
        }
    }
    fin
}

/// El motivo, en una linea que dice que hacer.
///
/// Antes los siete casos se aplanaban en "no puedo crear ahi (nombre 8.3?
/// carpeta?)" -- un mensaje que le pasa la pregunta al usuario en vez de
/// contestarla. El kernel SI sabe cual de los dos fue.
pub(crate) fn motivo_archivo(e: u32) -> &'static [u8] {
    match e {
        bmo::ERROR_ARCH_CARPETA => b"esa carpeta no existe.",
        bmo::ERROR_ARCH_ES_CARPETA => b"eso es una carpeta, no un archivo: prueba ls.",
        bmo::ERROR_ARCH_NOMBRE => b"el name no cabe en 8.3 (8 letras + 3 de extension).",
        bmo::ERROR_ARCH_NO_ESTA => b"ese archivo no esta.",
        bmo::ERROR_ARCH_GRANDE => b"el archivo pasa de 4 KiB: hoy no cabe.",
        bmo::ERROR_ARCH_SOLO_LECTURA => b"el volumen de datos no se puede write.",
        bmo::ERROR_ARCH_SIN_HUECO => b"hay demasiados archivos abiertos.",
        _ => b"no se pudo.",
    }
}

