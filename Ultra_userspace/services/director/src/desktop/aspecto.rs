//! **EL EDITOR DE ASPECTO** -- `aspecto` en Ejecutar (2026-09-13).
//!
//! [consumo] NADA      no corre en reposo: solo mientras se pulsan sus teclas
//!                     (L6h)
//!
//! Eddi: *"puede haber su propia configuracion o modo editor?"*. La mitad de
//! leer es `sys/director.cfg` (`scene::estilo`); esta es la de CAMBIAR: se ve el
//! resultado en el acto y se guarda en el mismo fichero.
//!
//! ## Donde se ensena, y por que ahi
//!
//! En la rejilla de salida de Ejecutar, como una lista, y no en un panel
//! flotante nuevo. La rejilla ya es parte del modelo de la escena: el cursor
//! pasa por encima y el escritorio sabe repintarla. Un panel dibujado a mano
//! encima del fondo dejaria rastros al mover el raton -- y lo que se esta
//! editando, la barra y el fondo, se ve EN SU SITIO, que es donde hay que verlo.
//!
//! ```text
//!    flechas arriba/abajo   que ajuste
//!    flechas izq/der        cambiarlo (los colores recorren una paleta)
//!    ENTRAR                 guardar en sys/director.cfg
//!    ESC                    salir DESHACIENDO lo que no se guardo
//! ```
//!
//! [!] Los colores van por PALETA y no tecleando hex: con flechas se prueba
//! rapido, y un color escrito a mano sigue pudiendose poner en el `.cfg`.

use bmo_config::Estilo;
use bmo_userland as bmo;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::desktop::Desktop;
use crate::scene::estilo;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

static ACTIVO: AtomicBool = AtomicBool::new(false);
static mut CAMPO: usize = 0;
static mut ANTES: Estilo = estilo::POR_DEFECTO;
static mut AVISO: &str = "";

/// Los ajustes, en el orden en que se ven.
const CAMPOS: [&str; 11] = [
    "barra_flotante",
    "barra_hueco",
    "acento",
    "barra_fondo",
    "barra_borde",
    "fondo_arriba",
    "fondo_abajo",
    "cpu",
    "memoria",
    "vatios",
    "reloj",
];

/// Acentos: vivos, para una linea o un punto.
const VIVOS: [u32; 9] = [
    0x0060_A5FA, 0x008B_5CF6, 0x00C0_84FC, 0x00F4_72B6, 0x0034_D399, 0x00FB_BF24, 0x0022_D3EE,
    0x00F8_7171, 0x00E6_EDF6,
];
/// Fondos: oscuros, para planos grandes que se miran una hora.
const OSCUROS: [u32; 11] = [
    0x000F_131D, 0x0011_111B, 0x0018_1825, 0x001E_1E2E, 0x000B_0E14, 0x001B_2233, 0x000C_0F17,
    0x0016_1B22, 0x001A_1B26, 0x0023_2136, 0x0026_2F42,
];

pub(crate) fn activo() -> bool {
    ACTIVO.load(Ordering::Relaxed)
}

/// Abre el editor: apunta el estilo de ahora para poder deshacer, y lo ensena.
pub(crate) fn abrir(dsk: &mut Desktop, p: &bmo::Pantalla) {
    unsafe {
        *addr_of_mut!(ANTES) = *estilo::estilo();
        CAMPO = 0;
        AVISO = "";
    }
    ACTIVO.store(true, Ordering::Relaxed);
    ensenar(dsk, p);
}

fn paso(paleta: &[u32], actual: u32, delta: isize) -> u32 {
    let n = paleta.len() as isize;
    let i = paleta.iter().position(|&c| c == actual).map_or(-1, |i| i as isize);
    // Un color que no esta en la paleta (escrito a mano en el `.cfg`) empieza
    // por el primero hacia donde se pulse.
    let j = if i < 0 { if delta > 0 { 0 } else { n - 1 } } else { (i + delta).rem_euclid(n) };
    paleta[j as usize]
}

fn cambiar(e: &mut Estilo, campo: usize, d: isize) {
    match campo {
        0 => e.barra_flotante = !e.barra_flotante,
        1 => e.barra_hueco = (e.barra_hueco as isize + d).clamp(0, 12) as u32,
        2 => e.acento = paso(&VIVOS, e.acento, d),
        3 => e.barra_fondo = paso(&OSCUROS, e.barra_fondo, d),
        4 => e.barra_borde = paso(&OSCUROS, e.barra_borde, d),
        5 => e.fondo_arriba = paso(&OSCUROS, e.fondo_arriba, d),
        6 => e.fondo_abajo = paso(&OSCUROS, e.fondo_abajo, d),
        7 => e.cpu = !e.cpu,
        8 => e.memoria = !e.memoria,
        9 => e.vatios = !e.vatios,
        _ => e.reloj = !e.reloj,
    }
}

/// El valor de un campo, como se escribe en el `.cfg`.
fn valor(e: &Estilo, campo: usize, dst: &mut [u8; 8]) -> usize {
    let hex = |c: u32, dst: &mut [u8; 8]| {
        dst[0] = b'#';
        for i in 0..6 {
            dst[1 + i] = b"0123456789ABCDEF"[((c >> ((5 - i) * 4)) & 0xF) as usize];
        }
        7
    };
    let si = |v: bool, dst: &mut [u8; 8]| {
        dst[..2].copy_from_slice(if v { b"si" } else { b"no" });
        2
    };
    match campo {
        0 => si(e.barra_flotante, dst),
        1 => {
            // El hueco va de 0 a 12: una o dos cifras.
            let h = e.barra_hueco.min(99);
            if h >= 10 {
                dst[0] = b'0' + (h / 10) as u8;
                dst[1] = b'0' + (h % 10) as u8;
                2
            } else {
                dst[0] = b'0' + h as u8;
                1
            }
        }
        2 => hex(e.acento, dst),
        3 => hex(e.barra_fondo, dst),
        4 => hex(e.barra_borde, dst),
        5 => hex(e.fondo_arriba, dst),
        6 => hex(e.fondo_abajo, dst),
        7 => si(e.cpu, dst),
        8 => si(e.memoria, dst),
        9 => si(e.vatios, dst),
        _ => si(e.reloj, dst),
    }
}

/// Escribe la lista en la rejilla y repinta el escritorio con el estilo de ahora.
fn ensenar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let e = *estilo::estilo();
    let campo = unsafe { CAMPO };
    let g = &mut dsk.out.grid;
    g.clear();
    g.with_ink(INK_GOOD);
    g.text(b"ASPECTO -- lo que cambies se ve ya en la barra y el fondo\n");
    g.with_ink(INK_PLAIN);
    for (k, nombre) in CAMPOS.iter().enumerate() {
        let elegido = k == campo;
        g.with_ink(if elegido { INK_ECHO } else { INK_PLAIN });
        g.text(if elegido { b"  > " } else { b"    " });
        g.text(nombre.as_bytes());
        for _ in nombre.len()..18 {
            g.byte(b' ');
        }
        let mut v = [0u8; 8];
        let n = valor(&e, k, &mut v);
        g.text(if elegido { b"< " } else { b"  " });
        g.text(&v[..n]);
        g.text(if elegido { b" >\n" } else { b"\n" });
    }
    g.with_ink(INK_PLAIN);
    g.text(b"\n  arriba/abajo elige   izq/der cambia   ENTRAR guarda   ESC deshace y sale\n");
    let aviso = unsafe { AVISO };
    if !aviso.is_empty() {
        g.with_ink(if aviso.starts_with("guardado") { INK_GOOD } else { INK_ERR });
        g.text(b"  ");
        g.text(aviso.as_bytes());
        g.byte(b'\n');
        g.with_ink(INK_PLAIN);
    }
    crate::repintar_escritorio(p, dsk, "aspecto");
}

/// Guarda el estilo de ahora en `sys/director.cfg`.
fn guardar() -> &'static str {
    let mut buf = [0u8; 1024];
    let n = estilo::estilo().escribir(&mut buf);
    let Ok(a) = bmo::Archivo::create(estilo::RUTA) else {
        return "no se pudo abrir sys/director.cfg para escribir";
    };
    let puestos = a.write(&buf[..n]);
    if a.close() && puestos == n {
        "guardado en sys/director.cfg (el proximo despliegue lo pisa: copialo al repo)"
    } else {
        "NO se guardo: el disco dijo que no al cerrar (motivo en F11)"
    }
}

/// **Las teclas mientras el editor esta abierto.** Se las queda todas: con el
/// editor delante, una flecha no es del historial de Ejecutar.
pub(crate) fn on_key(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8) {
    let mut e = *estilo::estilo();
    let campo = unsafe { CAMPO };
    unsafe { AVISO = "" };
    match c {
        0x80 => unsafe { CAMPO = (campo + CAMPOS.len() - 1) % CAMPOS.len() },
        0x81 | b'\t' => unsafe { CAMPO = (campo + 1) % CAMPOS.len() },
        0x82 | 0x83 => {
            cambiar(&mut e, campo, if c == 0x83 { 1 } else { -1 });
            estilo::poner(e);
        }
        b'\r' | b'\n' => unsafe { AVISO = guardar() },
        0x1B => {
            // ** ESC DESHACE lo que no se guardo. Salir de un editor dejando a
            // medias lo que se probo es como se acaba con un aspecto que nadie
            // eligio.
            estilo::poner(unsafe { *addr_of_mut!(ANTES) });
            ACTIVO.store(false, Ordering::Relaxed);
            dsk.out.grid.clear();
            dsk.out.grid.text(b"aspecto: sin guardar, como estaba\n");
            crate::repintar_escritorio(p, dsk, "listo");
            return;
        }
        _ => return,
    }
    // Lo guardado pasa a ser el punto al que ESC vuelve.
    if unsafe { AVISO }.starts_with("guardado") {
        unsafe { *addr_of_mut!(ANTES) = *estilo::estilo() };
    }
    ensenar(dsk, p);
}
