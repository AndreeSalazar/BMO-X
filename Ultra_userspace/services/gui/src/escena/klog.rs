//! **La consola del KERNEL** — F11, lo que Ring 0 tiene que contar.
//!
//! ═══ Por qué existe ═══
//!
//! Desde que **el escritorio es el arranque**, el panel del kernel deja de
//! pintarse en cuanto el compositor reclama la pantalla. Con él desaparecía el
//! relato entero de cómo arrancó la máquina: qué encontró el USB, qué dijo el
//! disco, si el doble búfer consiguió su bloque. Todo eso se escribía y nadie
//! podía volver a mirarlo.
//!
//! Y bloqueó una sesión de depuración entera: la línea que decidía entre dos
//! culpables se escribía en un sitio que ya nadie miraba. **Un dato que existe
//! y no se puede leer no está.**
//!
//! ═══ ★ Esto NO es "ir a Ring 0" ═══
//!
//! Y la distinción no es un tecnicismo, es el sistema entero.
//!
//! Aquí no se ejecuta nada privilegiado. El compositor sigue siendo un proceso
//! de Ring 3 con sus capabilities contadas, y lo que hace es **preguntar**:
//! `¿cuántas líneas hay?` y `dame los bytes de la número N`. El kernel contesta
//! texto y no cede nada — exactamente igual que con `info`.
//!
//! En un sistema de capabilities, **ver y poder son cosas separadas**. Un
//! "terminal privilegiado" que de verdad ejecutara en Ring 0 tiraría el modelo
//! a la basura para conseguir algo que se puede tener sin romper nada: mirar.
//! Que se pueda mirar TODO sin poder tocar nada es la mitad interesante de la
//! transparencia total que declara este proyecto.
//!
//! ═══ Por qué F11 y no un comando ═══
//!
//! Una tecla de función no produce carácter en ninguna distribución, así que no
//! puede chocar con escribir — el mismo motivo que F12 (ver [`super::datos`]).
//!
//! Pero además hay una razón de hoy: **no hace falta teclear nada para
//! abrirla**. Cuando lo que falla es justo el campo donde se escribe, un
//! diagnóstico que exige escribir un comando no sirve para nada. Una ventana
//! que se abre con una tecla y se lee funciona aunque el terminal no.

use bmo_userland as bmo;

use super::*;
use crate::texto::decimal;

pub(crate) const KLOG_ANCHO: u32 = 900;
pub(crate) const KLOG_ALTO: u32 = 420;

const KLOG_FONDO: u32 = 0x0009_1420;
const KLOG_BORDE: u32 = 0x0037_86C8;
const KLOG_TITULO: u32 = 0x0080_C8FF;

/// Cuánto mide la línea más larga que se enseña. El anillo del kernel guarda 96
/// bytes; aquí se recorta a lo que cabe en la ventana.
const MAX_COLS: usize = 104;

/// Dónde va, centrada sobre el panel.
pub(crate) struct CajaKlog {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) ancho: u32,
    pub(crate) alto: u32,
}

impl CajaKlog {
    pub(crate) fn nueva(p: &bmo::Pantalla) -> Self {
        let ancho = KLOG_ANCHO.min(p.ancho.saturating_sub(40));
        let alto = KLOG_ALTO.min(p.alto.saturating_sub(40));
        Self {
            x: (p.ancho.saturating_sub(ancho)) / 2,
            y: (p.alto.saturating_sub(alto)) / 2,
            ancho,
            alto,
        }
    }

    pub(crate) fn contiene(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.ancho && py >= self.y && py < self.y + self.alto
    }
}

/// El color de una línea **por quien la dice**, no por lo que dice.
///
/// Es la misma idea que el log del kernel usa en su panel: el emisor se
/// reconoce por el prefijo, y una columna de colores alineada se lee de un
/// vistazo sin tener que leer el texto. Lo que NO se hace es buscar palabras
/// como "error" dentro de la línea — eso pinta de rojo un mensaje que dice
/// "sin errores", que es la clase de ayuda que estorba.
fn color_de(linea: &[u8]) -> u32 {
    let empieza = |p: &[u8]| linea.len() >= p.len() && &linea[..p.len()] == p;
    if empieza(b"[uhid]") || empieza(b"[usb]") || empieza(b"[xhci]") {
        0x0070_D8FF // azul: el bus
    } else if empieza(b"[ahci]") || empieza(b"[fs]") || empieza(b"[estratos]") {
        0x00C8_A0FF // violeta: el almacenamiento
    } else if empieza(b"[s1_cpu]") || empieza(b"[s2_mem]") || empieza(b"[kernel]") {
        0x00F6_C445 // ámbar: el arranque
    } else if empieza(b"gui") || empieza(b"[ring3]") {
        TEXTO_BIEN // verde: lo que dice Ring 3
    } else {
        TEXTO
    }
}

/// Pinta la consola del kernel entera.
///
/// Se redibuja completa en cada invocación y no por fotograma, igual que la de
/// datos: el log del arranque no cambia solo, y repintarlo sesenta veces por
/// segundo para enseñar las mismas líneas es tirar el fotograma.
///
/// **`desplazamiento`** es cuántas líneas hacia atrás empieza la ventana. Con 0
/// se ve lo último; subiéndolo se llega al principio del arranque, que es donde
/// están las respuestas de por qué algo no arrancó.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &CajaKlog, desplazamiento: u64) {
    p.rect(c.x, c.y, c.ancho, c.alto, KLOG_BORDE);
    p.rect(c.x + 2, c.y + 2, c.ancho - 4, c.alto - 4, KLOG_FONDO);

    let tx = c.x + 16;
    let mut ty = c.y + 14;
    p.texto(tx, ty, "RING 0 // lo que dice el kernel", KLOG_TITULO);
    ty += bmo::GLIFO_ALTO + 8;

    let hay = bmo::klog_lineas();
    let total = bmo::klog_total();

    if hay == 0 {
        p.texto(tx, ty, "el kernel no ha dicho nada todavia.", TEXTO_TENUE);
        return;
    }

    // La cabecera dice **cuántas se perdieron**, y eso vale tanto como las que
    // se ven: un anillo que ha dado la vuelta y no lo dice hace creer que el
    // arranque empezó donde empieza la primera línea que queda.
    let mut cab = [0u8; 72];
    let mut n = 0usize;
    fn poner(s: &[u8], dst: &mut [u8], n: &mut usize) {
        for &b in s {
            if *n < dst.len() {
                dst[*n] = b;
                *n += 1;
            }
        }
    }
    fn num(v: u64, dst: &mut [u8], n: &mut usize) {
        let mut d = [0u8; 10];
        let k = decimal(v, &mut d);
        poner(&d[..k], dst, n);
    }
    poner(b"guardadas ", &mut cab, &mut n);
    num(hay, &mut cab, &mut n);
    poner(b" de ", &mut cab, &mut n);
    num(total, &mut cab, &mut n);
    if total > hay {
        poner(b"  (se cayeron ", &mut cab, &mut n);
        num(total - hay, &mut cab, &mut n);
        poner(b")", &mut cab, &mut n);
    }
    p.texto_bytes(tx, ty, &cab[..n], TEXTO_TENUE);
    ty += bmo::GLIFO_ALTO + 6;

    // Cuántas caben, dejando el margen de abajo.
    let alto_util = c.alto.saturating_sub(ty - c.y + 14);
    let filas = (alto_util / (bmo::GLIFO_ALTO + 2)) as u64;

    // ★ Se pintan de la MÁS VIEJA a la más nueva dentro de la ventana, para que
    // se lea como se lee un log: hacia abajo en el tiempo. El anillo numera al
    // revés (0 = la más reciente), así que hay que darle la vuelta aquí — y
    // hacerlo al pintar y no al guardar es lo correcto: el orden de lectura es
    // una decisión de presentación, y ésas viven en Ring 3.
    let primera = desplazamiento.min(hay.saturating_sub(1));
    let ultima = (primera + filas).min(hay);
    let mut i = ultima;
    let mut linea = [0u8; MAX_COLS];
    while i > primera {
        i -= 1;
        let n = bmo::klog_texto(i, &mut linea);
        if n > 0 {
            p.texto_bytes(tx, ty, &linea[..n], color_de(&linea[..n]));
        }
        ty += bmo::GLIFO_ALTO + 2;
    }
}
