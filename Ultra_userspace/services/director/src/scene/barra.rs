//! **LA BARRA**: la pastilla flotante y sus widgets (2026-09-13).
//!
//! [consumo] LATE      los widgets se recalculan UNA vez por segundo y solo se
//!                     repintan si su texto cambio; la pastilla, solo cuando se
//!                     repinta el escritorio entero (L6h)
//!
//! Eddi, con tres capturas de Hyprland: *"que mi DIRECTOR mejore en apariencia,
//! mas elegante"*.
//!
//! ## Lo que se toma, y lo que no
//!
//! ```text
//!    se toma    la barra como PASTILLA separada del borde (`barra_hueco`),
//!               bordes redondeados, y los widgets a la derecha: cpu, memoria,
//!               vatios del paquete y el reloj -- numeros que el kernel YA mide
//!    no         transparencia y desenfoque (no hay alfa por ventana), y los
//!               escritorios 1..5 (este escritorio no tiene escritorios virtuales:
//!               pintar cinco numeros que no hacen nada seria mentir con estilo)
//! ```
//!
//! ## ** UN modelo de color para pintar y para borrar
//!
//! `scene_color` responde que color hay en cada pixel cuando el cursor se va, y
//! [`color_en`] es su respuesta para la barra. Pintar la pastilla y contestar por
//! ella viven en el MISMO fichero y leen la MISMA geometria ([`caja`]): si se
//! separaran, mover el raton por encima dejaria esquinas de otro color.

use bmo_userland as bmo;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicU32, Ordering};

use super::estilo::estilo;
use super::{background_at, inside_rounded, rounded_rect, INK, INK_DIM, TASKBAR_H, TASKBAR_TOP};

/// El ancho de la pantalla, apuntado al pintar el fondo. `color_en` lo necesita
/// y quien la llama no lo tiene a mano.
static ANCHO: AtomicU32 = AtomicU32::new(0);

/// `(x, y, ancho, alto)` de la barra: la pastilla, o la tira entera si no flota.
pub(crate) fn caja() -> (u32, u32, u32, u32) {
    let ancho = ANCHO.load(Ordering::Relaxed);
    let e = estilo();
    if !e.barra_flotante {
        return (0, 0, ancho, TASKBAR_H);
    }
    let h = e.barra_hueco.min(12);
    // En vertical, como mucho 3: las fichas van de la fila 8 a la 32 y tienen
    // que seguir cabiendo dentro de la pastilla.
    let v = h.min(3);
    (h, v, ancho.saturating_sub(2 * h), TASKBAR_H - 2 * v)
}

/// Donde acaba la barra por la derecha. Quien borra una tira de fichas no pasa
/// de aqui: detras esta el hueco, que es fondo de escritorio.
pub(crate) fn derecha() -> u32 {
    let (x, _, w, _) = caja();
    x + w
}

/// El color de dentro de la barra. Lo usa todo lo que BORRA dentro de ella.
pub(crate) fn fondo() -> u32 {
    estilo().barra_fondo
}

/// Pinta la barra. El degradado del escritorio ya esta pintado debajo.
pub(crate) fn pintar(p: &bmo::Pantalla) {
    ANCHO.store(p.ancho, Ordering::Relaxed);
    let e = estilo();
    if !e.barra_flotante {
        p.rect(0, 0, p.ancho, TASKBAR_H, e.barra_fondo);
        p.rect(0, 0, p.ancho, 1, TASKBAR_TOP);
        p.rect(0, TASKBAR_H - 1, p.ancho, 1, e.barra_borde);
    } else {
        let (x, y, w, h) = caja();
        rounded_rect(p, x, y, w, h, e.barra_borde);
        rounded_rect(p, x + 1, y + 1, w - 2, h - 2, e.barra_fondo);
    }
    olvidar();
}

/// La marca y el nombre de la izquierda.
pub(crate) fn logo(p: &bmo::Pantalla) {
    p.rect(16, 13, 14, 14, estilo().acento);
    p.texto(38, 14, "BMO-X", INK);
}

/// **Que color hay en `(x, y)` de la barra**, para quien restaura lo que tapo el
/// cursor. `alto` es el de la pantalla, para el degradado de los huecos.
pub(crate) fn color_en(x: u32, y: u32, alto: u32) -> u32 {
    let e = estilo();
    if x >= 16 && x < 30 && y >= 13 && y < 27 {
        return e.acento;
    }
    if !e.barra_flotante {
        return if y == TASKBAR_H - 1 { e.barra_borde } else { e.barra_fondo };
    }
    let (bx, by, bw, bh) = caja();
    if !inside_rounded(x, y, bx, by, bw, bh) {
        return background_at(y, alto);
    }
    if !inside_rounded(x, y, bx + 1, by + 1, bw - 2, bh - 2) {
        return e.barra_borde;
    }
    e.barra_fondo
}

// ===================================================================
//  Los widgets
// ===================================================================

const TEXTO: usize = 48;
/// El sitio que se reserva a la derecha. Fijo, para que un numero que crece una
/// cifra no deje restos del anterior.
const ZONA: u32 = TEXTO as u32 * bmo::GLIFO_ANCHO;

static mut ULTIMO: [u8; TEXTO] = [0; TEXTO];
static mut ULTIMO_N: usize = 0;
static mut FORZAR: bool = true;
static mut PROXIMO: u64 = 0;
static mut PREV_TSC: u64 = 0;
static mut PREV_REPOSO: u64 = 0;
static mut CPU: Option<u64> = None;

/// Lo pintado se da por perdido: la barra se repinto debajo.
pub(crate) fn olvidar() {
    unsafe {
        FORZAR = true;
    }
}

fn pega(t: &mut [u8; TEXTO], n: &mut usize, s: &[u8]) {
    for &b in s {
        if *n < TEXTO {
            t[*n] = b;
            *n += 1;
        }
    }
}

fn cifra(t: &mut [u8; TEXTO], n: &mut usize, v: u64) {
    let mut b = [0u8; 10];
    let k = crate::text::decimal(v, &mut b);
    pega(t, n, &b[..k]);
}

/// Pinta los widgets si toca. `mw` son los milivatios del paquete que midio el
/// escritorio (`Tick::consumo`), si los hay.
pub(crate) fn widgets(p: &bmo::Pantalla, mw: Option<u64>) {
    let e = estilo();
    let ahora = bmo::ciclos();
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    unsafe {
        if !FORZAR && ahora < PROXIMO {
            return;
        }
        PROXIMO = ahora + hz;

        // ** CPU: la parte del ultimo segundo que el nucleo principal NO estuvo
        // dormido. Contadores que solo crecen, restados aqui: el mismo idioma
        // que los vatios.
        let reposo = bmo::info(bmo::INFO_BSP_TICKS_REPOSO);
        if PREV_TSC != 0 && ahora > PREV_TSC && reposo >= PREV_REPOSO {
            let dt = ahora - PREV_TSC;
            let dr = (reposo - PREV_REPOSO).min(dt);
            CPU = Some(100 - dr * 100 / dt);
        }
        PREV_TSC = ahora;
        PREV_REPOSO = reposo;

        let mut t = [0u8; TEXTO];
        let mut n = 0usize;
        let sep = b"   ";
        if e.cpu {
            if let Some(c) = CPU {
                pega(&mut t, &mut n, b"cpu ");
                cifra(&mut t, &mut n, c);
                pega(&mut t, &mut n, b"%");
                pega(&mut t, &mut n, sep);
            }
        }
        if e.memoria {
            let total = bmo::info(bmo::INFO_RAM_TOTAL);
            let usada = total.saturating_sub(bmo::info(bmo::INFO_RAM_LIBRE)) / (1024 * 1024);
            if total > 0 {
                pega(&mut t, &mut n, b"mem ");
                cifra(&mut t, &mut n, usada);
                pega(&mut t, &mut n, b"M");
                pega(&mut t, &mut n, sep);
            }
        }
        if e.vatios {
            if let Some(m) = mw.filter(|&m| m > 0) {
                cifra(&mut t, &mut n, m / 1000);
                pega(&mut t, &mut n, b".");
                cifra(&mut t, &mut n, (m % 1000) / 100);
                pega(&mut t, &mut n, b"W");
                pega(&mut t, &mut n, sep);
            }
        }
        if e.reloj {
            if let Some(f) = bmo_rtc::desempaquetar(bmo::info(bmo::INFO_FECHA)) {
                let dos = |t: &mut [u8; TEXTO], n: &mut usize, v: u8| {
                    pega(t, n, &[b'0' + v / 10, b'0' + v % 10]);
                };
                dos(&mut t, &mut n, f.hora);
                pega(&mut t, &mut n, b":");
                dos(&mut t, &mut n, f.minuto);
            }
        }
        // Sin separador colgando al final.
        while n > 0 && t[n - 1] == b' ' {
            n -= 1;
        }
        let ultimo = &mut *addr_of_mut!(ULTIMO);
        if !FORZAR && ULTIMO_N == n && ultimo[..n] == t[..n] {
            return;
        }
        FORZAR = false;
        ultimo[..n].copy_from_slice(&t[..n]);
        ULTIMO_N = n;

        // Se borra la zona entera y se escribe pegado a la derecha.
        let (_, by, _, bh) = caja();
        let fin = derecha().saturating_sub(14);
        let x0 = fin.saturating_sub(ZONA);
        p.rect(x0, by + 2, ZONA, bh.saturating_sub(4), fondo());
        let ty = (TASKBAR_H - bmo::GLIFO_ALTO) / 2;
        let mut x = fin.saturating_sub(n as u32 * bmo::GLIFO_ANCHO);
        // Las etiquetas (letras) apagadas y las cifras en claro: se lee el
        // numero antes que la palabra, que es para lo que se mira.
        let mut k = 0usize;
        while k < n {
            let mut j = k;
            let digito = t[k].is_ascii_digit() || t[k] == b'.' || t[k] == b':';
            while j < n && (t[j].is_ascii_digit() || t[j] == b'.' || t[j] == b':') == digito {
                j += 1;
            }
            x = p.texto_bytes(x, ty, &t[k..j], if digito { INK } else { INK_DIM });
            k = j;
        }
    }
}
