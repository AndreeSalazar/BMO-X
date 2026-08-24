//! **`banda` -- cuanto ancho de banda tiene de verdad esta memoria.**
//!
//! ## Por que fichero propio (L6b)
//!
//! Por la razon buena, no por la del tamano: contesta **otra pregunta**.
//! `hardware.rs` ensena aparatos y `smp test` mide **cuanto acelera repartir**.
//! Esto no mide una aceleracion -- mide **un caudal**, en bytes por segundo, y
//! el numero que produce no se compara con ningun otro de este shell: se compara
//! con el tamano de un modelo.
//!
//! ## Lo que sale por pantalla, y por que en ese orden
//!
//! ```text
//!   1. el BANCO      cuanta memoria y cuantas veces el L3 -- si esto no
//!                    convence, lo de abajo no vale nada
//!   2. el BARRIDO    una fila por cuantas partes. La columna que importa
//!                    es MB/s, y lo que se busca es DONDE DEJA DE SUBIR
//!   3. el TECHO      lo de arriba dividido por el tamano de un modelo:
//!                    tokens/s. Es un techo, y lo dice
//! ```
//!
//! *** El orden no es decorativo: **primero se justifica el instrumento y
//! despues se ensena la medida.** Al reves, un numero grande se lee antes de
//! que a nadie le de tiempo a preguntar si el banco cabia en cache.

use super::super::phase::s_log;
use crate::ring0::plat::smp;
use crate::ring0::plat::smp::banda;

/// Modelos con los que se traduce el caudal a tokens/s, en MB.
///
/// [!] Son **tamanos tipicos de cuantizacion a 4 bits**, no ficheros que
/// existan en este disco. Estan aqui para convertir una unidad en otra, y por
/// eso la fila que sale se llama "techo" y no "rendimiento".
const MODELOS: [(&str, u64); 3] = [
    ("1B en 4 bits", 700),
    ("3B en 4 bits", 1700),
    ("7B en 4 bits", 3700),
];

fn txt(b: &mut [u8; 80], o: &mut usize, t: &str) {
    for &ch in t.as_bytes() {
        if *o < b.len() {
            b[*o] = ch;
            *o += 1;
        }
    }
}

fn dec(b: &mut [u8; 80], o: &mut usize, mut v: u64) {
    let mut tmp = [0u8; 20];
    let mut i = 0;
    if v == 0 {
        tmp[0] = b'0';
        i = 1;
    }
    while v > 0 {
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        if *o < b.len() {
            b[*o] = tmp[i];
            *o += 1;
        }
    }
}

/// Un valor x100 como `12.16`. Sin flotantes: **no hay ni uno en Ring 0** y
/// meter el primero para pintar una tabla seria pagar el estado de FPU entero
/// por dos decimales.
fn dec2(b: &mut [u8; 80], o: &mut usize, v: u64) {
    dec(b, o, v / 100);
    txt(b, o, ".");
    let r = v % 100;
    if r < 10 {
        txt(b, o, "0");
    }
    dec(b, o, r);
}

fn pad(b: &mut [u8; 80], o: &mut usize, hasta: usize) {
    while *o < hasta && *o < b.len() {
        b[*o] = b' ';
        *o += 1;
    }
}

fn linea(b: &[u8; 80], o: usize) {
    s_log(core::str::from_utf8(&b[..o]).unwrap_or("?"));
}

/// **`banda` -- el barrido.**
///
/// *** Y AQUI SE RESCATA EL BUS AL SALIR, como en `smp test`. El barrido son
/// seis puntos por tres pasadas y el BSP se pasa decimas de segundo sin bombear
/// el USB; el evento de un endpoint de interrupcion **es el permiso** para
/// volver a encolar, asi que perder uno no pierde una tecla: **para la bomba**.
/// Le paso al dueno el 24-08 con `smp test` y no hace falta que pase dos veces.
pub(crate) fn shell_banda() {
    s_log("== banda: el ancho de banda de la memoria ==");

    let (vivos, _) = smp::alive();
    let mut b = [0u8; 80];

    // --- 1. EL INSTRUMENTO, antes que la medida -------------------------
    let (bytes, veces) = match banda::preparar() {
        Ok(v) => v,
        Err(e) => {
            let mut o = 0;
            txt(&mut b, &mut o, "[banda] no se puede medir: ");
            txt(&mut b, &mut o, e);
            linea(&b, o);
            return;
        }
    };
    let mut o = 0;
    txt(&mut b, &mut o, "  banco    ");
    dec(&mut b, &mut o, bytes / (1024 * 1024));
    txt(&mut b, &mut o, " MiB  (");
    dec(&mut b, &mut o, veces);
    txt(&mut b, &mut o, "x el L3)  -- si cupiera en cache, esto mediria cache");
    linea(&b, o);

    let hz = crate::ring0::cpu_vendor::ryzen_5_5600x::bmo_cpu::tsc_freq_hz();
    let mut o = 0;
    txt(&mut b, &mut o, "  reloj    TSC ");
    if hz == 0 {
        txt(&mut b, &mut o, "DESCONOCIDO -- solo se pueden ensenar ticks");
    } else {
        dec(&mut b, &mut o, hz / 1_000_000);
        txt(&mut b, &mut o, " MHz");
    }
    linea(&b, o);

    let mut o = 0;
    txt(&mut b, &mut o, "  obreros  ");
    dec(&mut b, &mut o, vivos as u64);
    txt(&mut b, &mut o, " despiertos");
    if vivos == 0 {
        txt(&mut b, &mut o, "  [!] solo el BSP: `smp despertar` para el barrido");
    }
    linea(&b, o);

    // --- 2. EL BARRIDO ---------------------------------------------------
    s_log("");
    s_log("  partes        ticks         MB/s     x el de 1 parte");

    let mut base_mb = 0u64;
    let mut techo_mb = 0u64;
    let mut techo_partes = 0u64;

    for &extra in banda::PUNTOS.iter() {
        // Un punto que pide mas obreros de los que hay medirian una carrera
        // incompleta: se salta y se dice, en vez de ensenar el numero bonito
        // que sale cuando falta gente.
        if extra > vivos {
            continue;
        }
        let (ticks, leidos, todos) = banda::medir(extra);
        let partes = extra as u64 + 1;

        let mut o = 0;
        pad(&mut b, &mut o, 4);
        dec(&mut b, &mut o, partes);
        pad(&mut b, &mut o, 14);
        dec(&mut b, &mut o, ticks);
        pad(&mut b, &mut o, 28);

        match banda::mb_por_segundo(leidos, ticks) {
            Some(mb) if banda::creible(mb) => {
                dec(&mut b, &mut o, mb);
                pad(&mut b, &mut o, 37);
                if partes == 1 {
                    base_mb = mb;
                    txt(&mut b, &mut o, "1.00");
                } else if base_mb > 0 {
                    dec2(&mut b, &mut o, mb * 100 / base_mb);
                }
                if mb > techo_mb {
                    techo_mb = mb;
                    techo_partes = partes;
                }
            }
            // *** UN NUMERO IMPOSIBLE SE DENUNCIA SOLO. Por encima de 100 GB/s
            // no hay DDR4 de dos canales: o el banco cabia en cache o el
            // cronometro miente, y las dos cosas invalidan la fila.
            Some(mb) => {
                dec(&mut b, &mut o, mb);
                txt(&mut b, &mut o, "  [!] IMPOSIBLE: no es RAM lo que se midio");
            }
            None => txt(&mut b, &mut o, "   -- (sin frecuencia de TSC)"),
        }
        if !todos {
            txt(&mut b, &mut o, "  [!] falto alguna parte");
        }
        linea(&b, o);
    }

    // El testigo, siempre. Cero = nadie leyo, y entonces los ticks de arriba
    // miden un bucle vacio por muy bonitos que sean.
    let t = banda::testigo();
    let mut o = 0;
    txt(&mut b, &mut o, "  testigo  ");
    if t == 0 {
        txt(&mut b, &mut o, "CERO -- la parte 0 no leyo nada: la tabla no vale");
    } else {
        dec(&mut b, &mut o, t);
    }
    linea(&b, o);

    // --- 3. EL TECHO, que es a lo que se venia ---------------------------
    if techo_mb == 0 {
        crate::ring0::dev::usb::rescatar_el_bus();
        return;
    }
    s_log("");
    let mut o = 0;
    txt(&mut b, &mut o, "  techo    ");
    dec(&mut b, &mut o, techo_mb);
    txt(&mut b, &mut o, " MB/s con ");
    dec(&mut b, &mut o, techo_partes);
    txt(&mut b, &mut o, " partes");
    linea(&b, o);

    // ** LA ECUACION, escrita al lado del numero que la alimenta: un modelo lee
    // sus pesos ENTEROS por cada token, asi que tokens/s = caudal / tamano. No
    // es una regla del pulgar, es la definicion -- y por eso el CPU no es el
    // que manda aqui.
    for &(nombre, mb) in MODELOS.iter() {
        let mut o = 0;
        txt(&mut b, &mut o, "    ");
        txt(&mut b, &mut o, nombre);
        txt(&mut b, &mut o, " (");
        dec(&mut b, &mut o, mb);
        txt(&mut b, &mut o, " MB)");
        pad(&mut b, &mut o, 30);
        txt(&mut b, &mut o, "-> ");
        dec2(&mut b, &mut o, banda::techo_tokens_x100(techo_mb, mb));
        txt(&mut b, &mut o, " tokens/s de TECHO");
        linea(&b, o);
    }
    s_log("  [!] TECHO, no prediccion: supone leer los pesos a la velocidad");
    s_log("      maxima de la maquina. Un motor real se queda en el 60-80%.");

    crate::ring0::dev::usb::rescatar_el_bus();
}
