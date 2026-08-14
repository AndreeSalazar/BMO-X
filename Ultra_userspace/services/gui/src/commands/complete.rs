//! El TAB: completar una ruta con lo que hay en el disco.

use bmo_userland as bmo;

use crate::scene::output::Output;
use crate::scene::PATH_MAX;
use crate::text::is_dot_entry;

pub(crate) fn complete(path: &mut [u8; PATH_MAX], n: usize, output: &mut Output) -> usize {
    // El ultimo token: lo que hay tras el ultimo espacio. Asi `corre app<TAB>`
    // completa la ruta y no el verbo.
    let start = path[..n].iter().rposition(|&c| c == b' ').map_or(0, |i| i + 1);
    // * La carpeta y el prefijo se COPIAN a locales antes de tocar nada.
    // Tomarlos prestados de `path` y luego escribir en `path` es exactamente
    // lo que el prestamista de Rust no deja -- y hace bien: escribir sobre lo
    // que estas leyendo es como se corrompe un buffer sin enterarse.
    let mut dir = [0u8; PATH_MAX];
    let mut dir_n = 0usize;
    let mut prefix = [0u8; 12];
    let mut prefix_n = 0usize;
    let prefix_start;
    {
        let token = &path[start..n];
        let cut = token.iter().rposition(|&c| c == b'/' || c == b'\\');
        let (d0, pi) = match cut {
            Some(i) => (&token[..i], i + 1),
            None => (&token[0..0], 0),
        };
        prefix_start = pi;
        dir_n = d0.len().min(PATH_MAX);
        dir[..dir_n].copy_from_slice(&d0[..dir_n]);
        let p0 = &token[prefix_start..];
        prefix_n = p0.len().min(prefix.len());
        prefix[..prefix_n].copy_from_slice(&p0[..prefix_n]);
    }
    let dir = &dir[..dir_n];
    let prefix = &prefix[..prefix_n];

    let d = match bmo::Directorio::open(dir) {
        Ok(d) => d,
        Err(_) => return n,
    };

    let baja = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
    let mut how_many = 0usize;
    let mut common = [0u8; 12];
    let mut common_n = 0usize;
    let mut only_is_dir = false;
    // Los candidatos se listan DESPUES, en una segunda pasada: guardarlos
    // todos aqui pediria un vector, y sin `alloc` eso es un array con un tope
    // inventado. Recorrer dos veces cuesta microsegundos y no inventa topes.
    let mut frames = 0u32;
    while frames < 256 {
        let e = match d.next() { Some(e) => e, None => break };
        frames += 1;
        let mut nom = [0u8; 12];
        let length = e.legible(&mut nom);
        // * `.` y `..` FUERA. Eran el motivo de que el TAB no completara
        // NUNCA dentro de una carpeta: entran como candidatos, y el prefijo
        // comun de `.`, `..` y `gui.bex` es la cadena vacia. El TAB listaba
        // todo y no avanzaba ni una letra, que parecia "no busca referencias"
        // cuando lo que hacia era buscarlas y anularse solo.
        if is_dot_entry(&nom[..length]) { continue; }
        if length < prefix.len() { continue; }
        let mut matches = true;
        for k in 0..prefix.len() {
            if baja(nom[k]) != baja(prefix[k]) { matches = false; break; }
        }
        if !matches { continue; }
        if how_many == 0 {
            common[..length].copy_from_slice(&nom[..length]);
            common_n = length;
            only_is_dir = e.es_dir;
        } else {
            // Recortar al prefijo comun con lo que llevabamos.
            let mut k = 0usize;
            while k < common_n && k < length && baja(common[k]) == baja(nom[k]) { k += 1; }
            common_n = k;
            only_is_dir = false;
        }
        how_many += 1;
    }

    if how_many == 0 {
        return n;
    }

    // Escribir el prefijo comun en el sitio del que habia.
    let mut end = start + prefix_start;
    let mut k = 0usize;
    while k < common_n && end < PATH_MAX {
        path[end] = common[k];
        end += 1;
        k += 1;
    }
    if how_many == 1 && only_is_dir && end < PATH_MAX {
        path[end] = b'/';
        end += 1;
    }

    // Con mas de uno, ENSENAR lo que hay. Es la diferencia con ciclar.
    if how_many > 1 {
        let d2 = match bmo::Directorio::open(dir) { Ok(d) => d, Err(_) => return end };
        let mut frames = 0u32;
        while frames < 256 {
            let e = match d2.next() { Some(e) => e, None => break };
            frames += 1;
            let mut nom = [0u8; 12];
            let length = e.legible(&mut nom);
            if length < prefix.len() { continue; }
            let mut matches = true;
            for k in 0..prefix.len() {
                if baja(nom[k]) != baja(prefix[k]) { matches = false; break; }
            }
            if !matches { continue; }
            output.text(b"  ");
            output.text(&nom[..length]);
            if e.es_dir { output.byte(b'/'); }
            output.byte(b'\n');
        }
    }
    end
}

/// El motivo, en una linea que dice que hacer.
///
/// Antes los siete casos se aplanaban en "no puedo crear ahi (nombre 8.3?
/// carpeta?)" -- un mensaje que le pasa la pregunta al usuario en vez de
/// contestarla. El kernel SI sabe cual de los dos fue.
pub(crate) fn file_error_reason(e: u32) -> &'static [u8] {
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

