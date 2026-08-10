//! **CABINA** -- F11, lo que el kernel ve, con su gravedad.
//!
//! === Que cambia respecto al klog ===
//!
//! El klog es la transcripcion del kernel en texto plano: 96 bytes por linea y
//! **sin severidad**. Servia, y por eso existio primero. Pero la linea que dice
//! si el SMP levanto los doce nucleos llegaba aqui igual que las veinte lineas
//! verdes que la rodean, y **un dato que existe y no se puede distinguir no
//! esta**.
//!
//! CABINA lleva por evento su severidad, su capa, su modulo y su valor. Esta
//! ventana los pinta: una columna de colores alineada se lee de un vistazo sin
//! tener que leer el texto.
//!
//! === * Esto NO es "ir a Ring 0" ===
//!
//! Y la distincion no es un tecnicismo, es el sistema entero.
//!
//! Aqui no se ejecuta nada privilegiado. El compositor sigue siendo un proceso
//! de Ring 3 con sus capabilities contadas, y lo unico que hace es **preguntar**
//! (`TASK_OP_CABINA_INFO` y `_TEXTO`). Ninguna de las dos escribe nada.
//!
//! En un sistema de capabilities, **ver y poder son cosas separadas**. Que se
//! pueda mirar TODO sin poder tocar nada es la mitad interesante de la
//! transparencia total que declara este proyecto.
//!
//! === Se mueve, como todas ===
//!
//! Lleva [`Marco`], igual que la ventana de ESTRATOS: se arrastra por la barra
//! de titulo, se redimensiona por la esquina, y tiene sus tres botones. Una
//! ventana clavada en el centro tapa justo lo que uno quiere comparar con ella.

use bmo_userland as bmo;

use super::marco::Marco;
use super::*;
use crate::texto::decimal;

// Proporcion de la pantalla, no un tamano fijo: ver `docs/LIDERES.md`.
const CAB_PCT_ANCHO: u32 = 70;
const CAB_PCT_ALTO: u32 = 55;
const CAB_MIN_ANCHO: u32 = 520;
const CAB_MIN_ALTO: u32 = 260;

// -- El CIAN del gato -----------------------------------------------------
//
// Es el color de BMO-X: los ojos del gato del logo y el kanji. Aqui hace de
// acento --titulo, subrayado y el bloque del icono-- sobre un fondo casi negro
// con un punto de azul, para que el cian resalte sin quemar.
//
// El neon se usa **solo en el acento**. Novecientos pixeles de borde de neon
// era lo que mas cansaba de mirar en la version anterior de esta ventana, y ya
// se rebajo una vez: la leccion esta pagada.
const CAB_FONDO: u32 = 0x0007_0B0E;
const CAB_TITULO_FONDO: u32 = 0x000D_1519;
const CAB_BORDE: u32 = 0x0019_3038;
const CIAN: u32 = 0x0034_E2E4;
const CIAN_TENUE: u32 = 0x0017_6E70;

// -- Los colores de la GRAVEDAD -------------------------------------------
//
// El orden es el de `cabina_core::Severity`, y se pintan de frio a caliente
// porque asi no hay que aprenderse nada: lo que sube de temperatura importa
// mas. `Trace` es el mas apagado a proposito -- es ruido util, no una noticia.
const SEV_COLOR: [u32; 5] = [
    0x00C8_D4DC, // Info    -- gris claro
    0x0055_6673, // Trace   -- gris apagado
    0x00E8_B84B, // Warning -- ambar
    0x00F2_6B4B, // Fault   -- naranja rojo
    0x00FF_3355, // Panic   -- rojo
];

const SEV_NOMBRE: [&str; 5] = ["info", "trace", " AVISO", " FALLO", " PANICO"];

/// Cuantos eventos caben, sabiendo el alto. Se calcula y no se fija: la ventana
/// se puede redimensionar, y una cuenta fija dejaria filas pintadas fuera o
/// hueco vacio dentro.
fn filas_visibles(marco: &Marco) -> usize {
    let util = marco.alto.saturating_sub(TITULO_ALTO + 44);
    (util / (bmo::GLIFO_ALTO + 3)) as usize
}

pub(crate) struct CajaCabina {
    pub(crate) marco: Marco,
    /// Cuantos eventos hacia atras empieza la ventana. `0` = lo ultimo.
    pub(crate) desde: u64,
    /// Gravedad minima que deja pasar. `0` = todas.
    ///
    /// Vive aqui y no en el modulo que pinta porque es estado de la SESION.
    /// Filtrar por gravedad --y no por texto-- se puede **porque CABINA la
    /// lleva**: el klog no, y por eso su filtro tenia que adivinar por el
    /// prefijo de la linea.
    pub(crate) minima: u64,
}

impl CajaCabina {
    pub(crate) fn nueva(p: &bmo::Pantalla) -> Self {
        Self {
            marco: Marco::nuevo(p, CAB_PCT_ANCHO, CAB_PCT_ALTO, CAB_MIN_ANCHO, CAB_MIN_ALTO),
            desde: 0,
            minima: 0,
        }
    }
}

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

pub(crate) fn pintar(p: &bmo::Pantalla, c: &CajaCabina) {
    if c.marco.minimizada {
        return;
    }
    c.marco.pintar_cromo(p, CAB_BORDE, CAB_FONDO, CAB_TITULO_FONDO, CIAN);
    c.marco.pintar_botones(p, CAB_TITULO_FONDO);

    let tx = c.marco.x + 16;
    // El bloque de acento del titulo: el ojo del gato, en pequeno.
    p.rect(tx, c.marco.y + 9, 8, 8, CIAN);
    let px = p.texto(tx + 16, c.marco.y + 8, "CABINA", TEXTO);
    let px = p.texto(
        px + 2 * bmo::GLIFO_ANCHO,
        c.marco.y + 8,
        "lo que el kernel ve",
        CIAN_TENUE,
    );

    // ** LA FECHA Y LA HORA, en el titulo.
    //
    // Cada evento lleva su sello del arranque (`t34A93`), y eso ordena lo que
    // paso **en esta sesion** y nada mas: dos arranques no se pueden comparar y
    // un log no se puede cruzar con nada de fuera. La hora de la placa --que
    // lleva su pila desde antes de que existieramos-- convierte la bitacora en
    // algo que se puede archivar.
    //
    // Si no hay reloj no se pone nada. **No se inventa una fecha**: un log
    // fechado en 1970 miente con mas convicion que uno sin fechar.
    let mut sello = [0u8; 24];
    let sn = fecha_en(&mut sello);
    if sn > 0 {
        p.texto_bytes(px + 2 * bmo::GLIFO_ANCHO, c.marco.y + 8, &sello[..sn], CIAN_TENUE);
    }

    let mut ty = c.marco.y + TITULO_ALTO + 8;

    let hay = bmo::cabina_disponibles();
    let total = bmo::cabina_total();
    let perdidos = bmo::cabina_perdidos();

    if hay == 0 {
        p.texto(tx, ty, "el kernel no ha dicho nada todavia.", TEXTO_TENUE);
        return;
    }

    // La cabecera dice **cuantos se cayeron del anillo**, y eso vale tanto como
    // los que se ven: un anillo que dio la vuelta y no lo dice hace creer que
    // el arranque empezo donde empieza el primero que sobrevive.
    let mut cab = [0u8; 96];
    let mut n = 0usize;
    poner(b"vivos ", &mut cab, &mut n);
    num(hay, &mut cab, &mut n);
    poner(b" de ", &mut cab, &mut n);
    num(total, &mut cab, &mut n);
    if perdidos > 0 {
        poner(b"   (se cayeron ", &mut cab, &mut n);
        num(perdidos, &mut cab, &mut n);
        poner(b")", &mut cab, &mut n);
    }
    if c.minima > 0 {
        poner(b"   filtro: >= ", &mut cab, &mut n);
        poner(
            SEV_NOMBRE[(c.minima as usize).min(4)].trim_start().as_bytes(),
            &mut cab,
            &mut n,
        );
    }
    p.texto_bytes(tx, ty, &cab[..n], TEXTO_TENUE);
    ty += bmo::GLIFO_ALTO + 6;

    // -- LA GUIA DEL FILTRO, cada opcion de su propio color -------------
    //
    // Se pinta siempre, tambien sin filtro: un atajo que solo se descubre
    // pulsandolo no existe. Y cada nombre va del color de sus lineas, asi que
    // la guia se explica sola y no hay que memorizar nada.
    let mut gx = tx;
    gx = p.texto(gx, ty, "G:", TEXTO_TENUE) + bmo::GLIFO_ANCHO;
    for s in 0..5usize {
        let sel = c.minima as usize == s;
        let color = if sel { SEV_COLOR[s] } else { CIAN_TENUE };
        let nom = SEV_NOMBRE[s].trim_start();
        let fin = p.texto(gx, ty, nom, color);
        if sel {
            p.rect(gx, ty + bmo::GLIFO_ALTO + 1, fin - gx, 1, color);
        }
        gx = fin + bmo::GLIFO_ANCHO;
    }
    ty += bmo::GLIFO_ALTO + 8;

    // -- LOS EVENTOS ----------------------------------------------------
    let cuantas = filas_visibles(&c.marco);
    let mut pintadas = 0usize;
    let mut i = c.desde;

    while pintadas < cuantas && i < hay {
        let sev = bmo::cabina_severidad(i);
        // El filtro es por GRAVEDAD y no por texto. Solo se puede porque CABINA
        // la lleva: buscar la palabra "error" dentro de la linea pinta de rojo
        // un mensaje que dice "sin errores", que es la clase de ayuda que
        // estorba.
        if sev < c.minima {
            i += 1;
            continue;
        }
        let s = (sev as usize).min(4);
        let color = SEV_COLOR[s];

        // La barra de gravedad, a la izquierda. Es lo que hace que la columna
        // se lea sin leer: un ojo encuentra un cambio de color mucho antes que
        // una palabra.
        p.rect(tx, ty + 2, 3, bmo::GLIFO_ALTO - 2, color);

        let mut linea = [0u8; 120];
        let mut n = 0usize;
        poner(SEV_NOMBRE[s].as_bytes(), &mut linea, &mut n);
        poner(b"  ", &mut linea, &mut n);

        let mut modulo = [0u8; 16];
        let m = bmo::cabina_texto(i, bmo::CABINA_TXT_MODULO, &mut modulo);
        poner(&modulo[..m], &mut linea, &mut n);
        // El modulo se alinea a ocho para que los mensajes empiecen todos en la
        // misma columna. Con anchos distintos, la vista no encuentra el texto.
        for _ in m..8 {
            poner(b" ", &mut linea, &mut n);
        }
        poner(b" ", &mut linea, &mut n);

        let mut mensaje = [0u8; 72];
        let k = bmo::cabina_texto(i, bmo::CABINA_TXT_MENSAJE, &mut mensaje);
        poner(&mensaje[..k], &mut linea, &mut n);

        // El VALOR solo si dice algo. Un `=0` detras de cada linea es ruido en
        // la mayoria, y justo en las que importa --fugas, choques de cerrojo--
        // el cero ES la respuesta correcta y ya se ve en `info`.
        if let Some(v) = bmo::cabina_campo(bmo::CABINA_VALOR, i) {
            if v != 0 {
                poner(b" =", &mut linea, &mut n);
                num(v, &mut linea, &mut n);
            }
        }

        p.texto_bytes(tx + 10, ty, &linea[..n.min(linea.len())], color);
        ty += bmo::GLIFO_ALTO + 3;
        pintadas += 1;
        i += 1;
    }

    // -- La barra de atajos, del mismo estilo que las demas -------------
    let by = c.marco.y + c.marco.alto - bmo::GLIFO_ALTO - 8;
    p.texto(
        tx,
        by,
        "G gravedad   RePag/AvPag historia   arrastra el titulo   ESC cierra",
        CIAN_TENUE,
    );
}

/// `AAAA-MM-DD HH:MM` de la placa. Devuelve 0 si la maquina no sabe que dia es.
///
/// Los segundos se dejan fuera a proposito: en un titulo no se leen, y un
/// numero que cambia solo hace parpadear una linea que se mira quieta.
fn fecha_en(out: &mut [u8; 24]) -> usize {
    let v = bmo::info(bmo::INFO_FECHA);
    let Some(f) = bmo_rtc::desempaquetar(v) else {
        return 0;
    };
    let mut b = [0u8; 24];
    let n = bmo_rtc::escribir(&f, &mut b);
    if n < 16 {
        return 0;
    }
    // Hasta el minuto: `AAAA-MM-DD HH:MM` son 16.
    out[..16].copy_from_slice(&b[..16]);
    16
}
