//! **La consola del KERNEL** -- F11, lo que Ring 0 tiene que contar.
//!
//! === Por que existe ===
//!
//! Desde que **el escritorio es el arranque**, el panel del kernel deja de
//! pintarse en cuanto el compositor reclama la pantalla. Con el desaparecia el
//! relato entero de como arranco la maquina: que encontro el USB, que dijo el
//! disco, si el doble bufer consiguio su bloque. Todo eso se escribia y nadie
//! podia volver a mirarlo.
//!
//! Y bloqueo una sesion de depuracion entera: la linea que decidia entre dos
//! culpables se escribia en un sitio que ya nadie miraba. **Un dato que existe
//! y no se puede leer no esta.**
//!
//! === * Esto NO es "ir a Ring 0" ===
//!
//! Y la distincion no es un tecnicismo, es el sistema entero.
//!
//! Aqui no se ejecuta nada privilegiado. El compositor sigue siendo un proceso
//! de Ring 3 con sus capabilities contadas, y lo que hace es **preguntar**:
//! `cuantas lineas hay?` y `dame los bytes de la numero N`. El kernel contesta
//! texto y no cede nada -- exactamente igual que con `info`.
//!
//! En un sistema de capabilities, **ver y poder son cosas separadas**. Un
//! "terminal privilegiado" que de verdad ejecutara en Ring 0 tiraria el modelo
//! a la basura para conseguir algo que se puede tener sin romper nada: mirar.
//! Que se pueda mirar TODO sin poder tocar nada es la mitad interesante de la
//! transparencia total que declara este proyecto.
//!
//! === Por que F11 y no un comando ===
//!
//! Una tecla de funcion no produce caracter en ninguna distribucion, asi que no
//! puede chocar con escribir -- el mismo motivo que F12 (ver [`super::datos`]).
//!
//! Pero ademas hay una razon de hoy: **no hace falta teclear nada para
//! abrirla**. Cuando lo que falla es justo el campo donde se escribe, un
//! diagnostico que exige escribir un comando no sirve para nada. Una ventana
//! que se abre con una tecla y se lee funciona aunque el terminal no.

use bmo_userland as bmo;

use super::*;
use crate::texto::decimal;

pub(crate) const KLOG_ANCHO: u32 = 900;
pub(crate) const KLOG_ALTO: u32 = 420;

// El azul es de esta ventana igual que el verde es de ESTRATOS: el color dice
// cual es antes de leer el titulo. Solo se rebajo el tono -- un borde de neon
// alrededor de novecientos pixeles era lo que mas cansaba de mirar.
const KLOG_FONDO: u32 = 0x000E_1520;
const KLOG_TITULO_FONDO: u32 = 0x0016_2030;
const KLOG_BORDE: u32 = 0x002B_3A50;
const KLOG_TITULO: u32 = 0x0060_A5FA;

/// Cuanto mide la linea mas larga que se ensena. El anillo del kernel guarda 96
/// bytes; aqui se recorta a lo que cabe en la ventana.
const MAX_COLS: usize = 104;

/// Donde va, centrada sobre el panel.
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

/// El color de una linea **por quien la dice**, no por lo que dice.
///
/// Es la misma idea que el log del kernel usa en su panel: el emisor se
/// reconoce por el prefijo, y una columna de colores alineada se lee de un
/// vistazo sin tener que leer el texto. Lo que NO se hace es buscar palabras
/// como "error" dentro de la linea -- eso pinta de rojo un mensaje que dice
/// "sin errores", que es la clase de ayuda que estorba.
// =============== EL FILTRO ===============
//
// * Filtra por FAMILIA DE MODULO y no por severidad, y el motivo es que las
// lineas **no llevan severidad**: el klog es la transcripcion tal cual, texto
// plano de 96 bytes (ver `ring0/core/klog.rs`). Quien lleva severidad es CABINA,
// que es otra cosa y todavia no se asoma a Ring 3. Inventar aqui un "nivel"
// adivinandolo por palabras seria un filtro que miente en cuanto alguien
// escriba un mensaje que no encaje con la corazonada.
//
// Lo que si existe y es fiable es la etiqueta con la que cada modulo empieza su
// linea. Y hay un motivo mas para usar esa: **es la misma taxonomia que ya
// pinta los colores**, asi que la guia se explica sola -- cada opcion se pinta
// del color de sus lineas y no hace falta memorizar nada.

/// Cuantas familias hay, contando `TODO`.
pub(crate) const FAMILIAS: u8 = 5;

/// Aparece `aguja` en algun sitio de `linea`?
///
/// * **AQUI ESTABA EL FALLO, y llevaba puesto desde antes del filtro.** Esto
/// comparaba contra el PRINCIPIO de la linea (`&linea[..p.len()] == p`), y
/// ninguna linea del klog empieza por su etiqueta: **todas empiezan por la
/// hora**, porque `klog::guardar_con_hora` antepone `[     0ms] ` a lo que le
/// den. Lo que se comparaba era un `[` contra un `[`, y nada mas.
///
/// Consecuencia, que se vio en las fotos del dueno: el filtro no dejaba pasar
/// **ni una linea** en ninguna familia, y el coloreado --que ya existia y usaba
/// la misma comparacion-- **nunca habia pintado un solo color**. Un fallo que
/// llevaba ahi sin que nadie lo notara porque su sintoma era "todo blanco", que
/// es exactamente lo que uno espera de un log.
///
/// Se busca como SUBCADENA y no saltando el prefijo de la hora a proposito: asi
/// da igual si manana alguien cambia ese formato, y una linea que mencione
/// `[usb]` en mitad del texto tambien es del bus -- que es la respuesta correcta.
fn contiene(linea: &[u8], aguja: &[u8]) -> bool {
    if aguja.len() > linea.len() {
        return false;
    }
    let mut i = 0usize;
    while i + aguja.len() <= linea.len() {
        if &linea[i..i + aguja.len()] == aguja {
            return true;
        }
        i += 1;
    }
    false
}

/// A que familia pertenece una linea. `0` es "ninguna conocida".
fn familia_de(linea: &[u8]) -> u8 {
    let empieza = |p: &[u8]| contiene(linea, p);
    if empieza(b"[uhid]") || empieza(b"[usb]") || empieza(b"[xhci]") {
        1 // el bus
    } else if empieza(b"[ahci]") || empieza(b"[fs]") || empieza(b"[estratos]") {
        2 // el almacenamiento
    } else if empieza(b"[s1_cpu]") || empieza(b"[s2_mem]") || empieza(b"[kernel]")
        || empieza(b"[ring0]") || empieza(b"[cpu]") || empieza(b"[smp]")
    {
        3 // el arranque y el silicio
    } else if empieza(b"gui.bex>") || empieza(b"[ring3]") {
        // `gui.bex>` con el `>` incluido y no un `gui` suelto: buscando como
        // subcadena, un `gui` a secas se comeria la linea de la entrega
        // (`se cede ... a sys/gui.bex`), que es del arranque y no de Ring 3.
        4 // lo que dice Ring 3
    } else {
        0
    }
}

fn color_familia(f: u8) -> u32 {
    match f {
        1 => 0x0070_D8FF, // azul
        2 => 0x00C8_A0FF, // violeta
        3 => 0x00F6_C445, // ambar
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

/// Pasa esta linea el filtro? `0` = no filtrar.
fn pasa(linea: &[u8], filtro: u8) -> bool {
    filtro == 0 || familia_de(linea) == filtro
}

fn color_de(linea: &[u8]) -> u32 {
    color_familia(familia_de(linea))
}

/// Pinta la consola del kernel entera.
///
/// Se redibuja completa en cada invocacion y no por fotograma, igual que la de
/// datos: el log del arranque no cambia solo, y repintarlo sesenta veces por
/// segundo para ensenar las mismas lineas es tirar el fotograma.
///
/// **`desplazamiento`** es cuantas lineas hacia atras empieza la ventana. Con 0
/// se ve lo ultimo; subiendolo se llega al principio del arranque, que es donde
/// estan las respuestas de por que algo no arranco.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &CajaKlog, desplazamiento: u64, filtro: u8) {
    sombra(p, c.x, c.y, c.ancho, c.alto);
    rect_redondeado(p, c.x, c.y, c.ancho, c.alto, KLOG_BORDE);
    rect_redondeado(p, c.x + 1, c.y + 1, c.ancho - 2, c.alto - 2, KLOG_FONDO);

    // La barra de titulo, con la misma curva que la ventana.
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

    // La cabecera dice **cuantas se perdieron**, y eso vale tanto como las que
    // se ven: un anillo que ha dado la vuelta y no lo dice hace creer que el
    // arranque empezo donde empieza la primera linea que queda.
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

    // -- LA GUIA DEL FILTRO --------------------------------------------
    //
    // Se pinta SIEMPRE, tambien con el filtro en TODO. Un atajo que solo se
    // anuncia cuando ya lo estas usando no se descubre nunca -- y este es el
    // caso exacto que lo motivo: habia un comando que el dueno no podia
    // ejecutar porque no sabia que existia.
    let mut gx = p.texto(tx, ty, "F filtra:", TEXTO_TENUE);
    let mut f = 0u8;
    while f < FAMILIAS {
        gx += bmo::GLIFO_ANCHO;
        // El activo va en su color y con corchetes; los demas, tenues. Cada
        // opcion se pinta del color de SUS lineas: la guia y el log se leen con
        // el mismo codigo de color y no hay nada que memorizar.
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

    // Cuantas caben, dejando el margen de abajo.
    let alto_util = c.alto.saturating_sub(ty - c.y + 14);
    let filas = (alto_util / (bmo::GLIFO_ALTO + 2)) as u64;

    // * Con filtro, `desplazamiento` ya no puede indexar la pantalla: se
    // RECOGEN las que pasan y luego se pintan. Saltarlas al vuelo dejaria
    // huecos en blanco donde habia lineas descartadas, que es la forma mas
    // rapida de que un filtro parezca un fallo de pintado.
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

    // Y cuantas hay EN TOTAL con este filtro. El anillo son 64 lineas: contarlas
    // enteras cuesta nada y evita la duda de "es que no hay mas, o es que no
    // caben?" -- que es justo lo que un filtro provoca si solo ensena una pagina.
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

    // Se pintan de la MAS VIEJA a la mas nueva dentro de la ventana, para que
    // se lea como se lee un log: hacia abajo en el tiempo. El anillo numera al
    // reves (0 = la mas reciente), asi que hay que darle la vuelta aqui -- y
    // hacerlo al pintar y no al guardar es lo correcto: el orden de lectura es
    // una decision de presentacion, y esas viven en Ring 3.
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
