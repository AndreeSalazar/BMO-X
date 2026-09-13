//! **EL ESTILO que se lee de `sys/director.cfg`** (2026-09-13).
//!
//! [consumo] NADA      lee el fichero UNA vez, al arrancar (L6h)
//!
//! Eddi: *"puede haber su propia configuracion?"*. Los valores de partida son los
//! de siempre (`tema.maqueta`); el fichero pisa los que diga, y lo que no se
//! entienda sale en la consola de Ejecutar con su linea. La lectura y sus reglas
//! viven en `bmo-config`, probadas contra ficheros rotos.
//!
//! [!] No todo el escritorio lee de aqui todavia: el acento de las VENTANAS
//! sigue siendo la constante `ACCENT`. Lo que ya es configurable es el fondo, la
//! barra y sus widgets. Se dice aqui para que nadie cambie `acento` y crea que el
//! fichero no funciona.

use bmo_config::{Estilo, Informe};
use bmo_userland as bmo;
use core::ptr::{addr_of, addr_of_mut};

use super::output::Output;
use super::{ACCENT, BG_BOTTOM, BG_TOP, TASKBAR, TASKBAR_LINE};

pub(crate) const POR_DEFECTO: Estilo = Estilo {
    acento: ACCENT,
    fondo_arriba: BG_TOP,
    fondo_abajo: BG_BOTTOM,
    barra_fondo: TASKBAR,
    barra_borde: TASKBAR_LINE,
    barra_flotante: true,
    barra_hueco: 6,
    reloj: true,
    vatios: true,
    memoria: true,
    cpu: true,
};

pub(crate) const RUTA: &[u8] = b"sys/director.cfg";
const TOPE: usize = 4096;

static mut ESTILO: Estilo = POR_DEFECTO;
static mut INFORME: Informe = Informe::VACIO;
static mut LEIDO: bool = false;

pub(crate) fn estilo() -> &'static Estilo {
    unsafe { &*addr_of!(ESTILO) }
}

/// Cambia el estilo en vivo. Lo usa el editor de aspecto; quien llame repinta.
pub(crate) fn poner(e: Estilo) {
    unsafe { *addr_of_mut!(ESTILO) = e };
}

/// Lee `sys/director.cfg`. Si no esta, se queda lo de siempre: no tener fichero
/// no es un fallo, es no haber cambiado nada.
pub(crate) fn cargar() {
    let Ok(a) = bmo::Archivo::leer_de(RUTA) else { return };
    let mide = (a.tamano() as usize).min(TOPE);
    let mut buf = [0u8; TOPE];
    let n = a.read(&mut buf[..mide]);
    let mut e = POR_DEFECTO;
    let inf = e.aplicar(&buf[..n]);
    unsafe {
        *addr_of_mut!(ESTILO) = e;
        *addr_of_mut!(INFORME) = inf;
        LEIDO = true;
    }
}

/// **Cuenta en la consola lo que dijo el fichero**: cuantos ajustes, y cada
/// linea que no se entendio con su motivo.
pub(crate) fn contar_en(grid: &mut Output) {
    if !unsafe { LEIDO } {
        return;
    }
    let inf = unsafe { &*addr_of!(INFORME) };
    let mut b = [0u8; 10];
    let n = crate::text::decimal(inf.aplicadas as u64, &mut b);
    grid.text(b"director.cfg: ");
    grid.text(&b[..n]);
    grid.text(b" ajuste(s)\n");
    for f in inf.fallos() {
        let n = crate::text::decimal(f.linea as u64, &mut b);
        grid.text(b"  director.cfg linea ");
        grid.text(&b[..n]);
        grid.text(b": ");
        grid.text(f.motivo.texto().as_bytes());
        grid.text(b"\n");
    }
    if inf.recortado {
        grid.text(b"  (y mas lineas mal: solo se ensenan ocho)\n");
    }
}
