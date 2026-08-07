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

// El azul es de esta ventana igual que el verde es de ESTRATOS: el color dice
// cuál es antes de leer el título. Sólo se rebajó el tono — un borde de neón
// alrededor de novecientos píxeles era lo que más cansaba de mirar.
const KLOG_FONDO: u32 = 0x000E_1520;
const KLOG_TITULO_FONDO: u32 = 0x0016_2030;
const KLOG_BORDE: u32 = 0x002B_3A50;
const KLOG_TITULO: u32 = 0x0060_A5FA;

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
// ═══════════════ EL FILTRO ═══════════════
//
// ★ Filtra por FAMILIA DE MÓDULO y no por severidad, y el motivo es que las
// líneas **no llevan severidad**: el klog es la transcripción tal cual, texto
// plano de 96 bytes (ver `ring0/core/klog.rs`). Quien lleva severidad es CABINA,
// que es otra cosa y todavía no se asoma a Ring 3. Inventar aquí un "nivel"
// adivinándolo por palabras sería un filtro que miente en cuanto alguien
// escriba un mensaje que no encaje con la corazonada.
//
// Lo que sí existe y es fiable es la etiqueta con la que cada módulo empieza su
// línea. Y hay un motivo más para usar ésa: **es la misma taxonomía que ya
// pinta los colores**, así que la guía se explica sola — cada opción se pinta
// del color de sus líneas y no hace falta memorizar nada.

/// Cuántas familias hay, contando `TODO`.
pub(crate) const FAMILIAS: u8 = 5;

/// A qué familia pertenece una línea. `0` es "ninguna conocida".
fn familia_de(linea: &[u8]) -> u8 {
    let empieza = |p: &[u8]| linea.len() >= p.len() && &linea[..p.len()] == p;
    if empieza(b"[uhid]") || empieza(b"[usb]") || empieza(b"[xhci]") {
        1 // el bus
    } else if empieza(b"[ahci]") || empieza(b"[fs]") || empieza(b"[estratos]") {
        2 // el almacenamiento
    } else if empieza(b"[s1_cpu]") || empieza(b"[s2_mem]") || empieza(b"[kernel]") {
        3 // el arranque
    } else if empieza(b"gui") || empieza(b"[ring3]") {
        4 // lo que dice Ring 3
    } else {
        0
    }
}

fn color_familia(f: u8) -> u32 {
    match f {
        1 => 0x0070_D8FF, // azul
        2 => 0x00C8_A0FF, // violeta
        3 => 0x00F6_C445, // ámbar
        4 => TEXTO_BIEN,  // verde
        _ => TEXTO,
    }
}

fn nombre_familia(f: u8) -> &'static str {
    match f {
        1 => "bus",
        2 => "disco",
        3 => "arranque",
        4 => "ring3",
        _ => "TODO",
    }
}

/// ¿Pasa esta línea el filtro? `0` = no filtrar.
fn pasa(linea: &[u8], filtro: u8) -> bool {
    filtro == 0 || familia_de(linea) == filtro
}

fn color_de(linea: &[u8]) -> u32 {
    color_familia(familia_de(linea))
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
pub(crate) fn pintar(p: &bmo::Pantalla, c: &CajaKlog, desplazamiento: u64, filtro: u8) {
    sombra(p, c.x, c.y, c.ancho, c.alto);
    rect_redondeado(p, c.x, c.y, c.ancho, c.alto, KLOG_BORDE);
    rect_redondeado(p, c.x + 1, c.y + 1, c.ancho - 2, c.alto - 2, KLOG_FONDO);

    // La barra de título, con la misma curva que la ventana.
    for i in 0..RADIO {
        let s = super::curva(i);
        p.rect(c.x + s, c.y + 1 + i, c.ancho - 2 * s, 1, KLOG_TITULO_FONDO);
    }
    p.rect(c.x + 1, c.y + 1 + RADIO, c.ancho - 2, TITULO_ALTO - 2 - RADIO, KLOG_TITULO_FONDO);
    p.rect(c.x + 1, c.y + TITULO_ALTO - 1, c.ancho - 2, 1, KLOG_TITULO);

    let tx = c.x + 16;
    p.rect(tx, c.y + 9, 8, 8, KLOG_TITULO);
    let px = p.texto(tx + 16, c.y + 8, "Ring 0", TEXTO);
    p.texto(px + 2 * bmo::GLIFO_ANCHO, c.y + 8, "lo que dice el kernel", TEXTO_TENUE);

    let mut ty = c.y + TITULO_ALTO + 8;

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
    ty += bmo::GLIFO_ALTO + 4;

    // ── LA GUÍA DEL FILTRO ────────────────────────────────────────────
    //
    // Se pinta SIEMPRE, también con el filtro en TODO. Un atajo que sólo se
    // anuncia cuando ya lo estás usando no se descubre nunca — y éste es el
    // caso exacto que lo motivó: había un comando que el dueño no podía
    // ejecutar porque no sabía que existía.
    let mut gx = p.texto(tx, ty, "F filtra:", TEXTO_TENUE);
    let mut f = 0u8;
    while f < FAMILIAS {
        gx += bmo::GLIFO_ANCHO;
        // El activo va en su color y con corchetes; los demás, tenues. Cada
        // opción se pinta del color de SUS líneas: la guía y el log se leen con
        // el mismo código de color y no hay nada que memorizar.
        if f == filtro {
            gx = p.texto(gx, ty, "[", color_familia(f));
            gx = p.texto(gx, ty, nombre_familia(f), color_familia(f));
            gx = p.texto(gx, ty, "]", color_familia(f));
        } else {
            gx = p.texto(gx, ty, nombre_familia(f), TEXTO_TENUE);
        }
        f += 1;
    }
    ty += bmo::GLIFO_ALTO + 6;

    // Cuántas caben, dejando el margen de abajo.
    let alto_util = c.alto.saturating_sub(ty - c.y + 14);
    let filas = (alto_util / (bmo::GLIFO_ALTO + 2)) as u64;

    // ★ Con filtro, `desplazamiento` ya no puede indexar la pantalla: se
    // RECOGEN las que pasan y luego se pintan. Saltarlas al vuelo dejaría
    // huecos en blanco donde había líneas descartadas, que es la forma más
    // rápida de que un filtro parezca un fallo de pintado.
    const MAX_FILAS: usize = 64;
    let tope = (filas as usize).min(MAX_FILAS);
    let mut elegidas = [0u64; MAX_FILAS];
    let mut cuantas = 0usize;
    let mut linea = [0u8; MAX_COLS];
    let mut i = desplazamiento.min(hay.saturating_sub(1));
    while i < hay && cuantas < tope {
        let n = bmo::klog_texto(i, &mut linea);
        if n > 0 && pasa(&linea[..n], filtro) {
            elegidas[cuantas] = i;
            cuantas += 1;
        }
        i += 1;
    }

    // Y cuántas hay EN TOTAL con este filtro. El anillo son 64 líneas: contarlas
    // enteras cuesta nada y evita la duda de "¿es que no hay más, o es que no
    // caben?" — que es justo lo que un filtro provoca si sólo enseña una página.
    if filtro != 0 {
        let mut total_f = 0u64;
        let mut k = 0u64;
        while k < hay {
            let n = bmo::klog_texto(k, &mut linea);
            if n > 0 && pasa(&linea[..n], filtro) {
                total_f += 1;
            }
            k += 1;
        }
        let mut m = [0u8; 32];
        let mut mn = 0usize;
        num(total_f, &mut m, &mut mn);
        poner(b" de ", &mut m, &mut mn);
        num(hay, &mut m, &mut mn);
        p.texto_bytes(c.x + c.ancho - 16 - (mn as u32) * bmo::GLIFO_ANCHO,
                      c.y + TITULO_ALTO + 8, &m[..mn], color_familia(filtro));
    }

    // Se pintan de la MÁS VIEJA a la más nueva dentro de la ventana, para que
    // se lea como se lee un log: hacia abajo en el tiempo. El anillo numera al
    // revés (0 = la más reciente), así que hay que darle la vuelta aquí — y
    // hacerlo al pintar y no al guardar es lo correcto: el orden de lectura es
    // una decisión de presentación, y ésas viven en Ring 3.
    let mut j = cuantas;
    while j > 0 {
        j -= 1;
        let n = bmo::klog_texto(elegidas[j], &mut linea);
        if n > 0 {
            p.texto_bytes(tx, ty, &linea[..n], color_de(&linea[..n]));
        }
        ty += bmo::GLIFO_ALTO + 2;
    }

    if cuantas == 0 {
        p.texto(tx, ty, "el filtro no deja pasar ninguna linea.", TEXTO_TENUE);
    }
}
