//! **La ventana del SONIDO** -- F10, el aparato y quien lo tiene.
//!
//! === Lo que ensena, y por que ese orden ===
//!
//! Arriba el APARATO, porque es la pregunta que decide todo lo demas: si no hay
//! camino, el volumen y las notas no significan nada. Debajo el volumen, y
//! abajo un teclado para probarlo -- porque un control de sonido que no deja
//! hacer ruido no se puede comprobar, y entonces no se sabe si funciona.
//!
//! === * EL COMPOSITOR RECLAMA EL SONIDO AL ABRIRLA Y LO SUELTA AL CERRARLA ===
//!
//! Y esto es lo interesante de la ventana, mas que lo que pinta.
//!
//! `KIND_AUDIO` es **exclusivo**: un solo proceso lo tiene a la vez. Si el
//! escritorio lo reclamara al arrancar --como hace con la pantalla y la
//! entrada-- **ningun programa lanzado desde el podria volver a sonar jamas**,
//! y el sintoma seria `c/musica.bex` diciendo "lo tiene otro proceso" para
//! siempre.
//!
//! Eso ya paso una vez, con la pantalla: `gui.bex` la reclamaba al arrancar y no
//! la soltaba nunca, asi que `ray.bex` no podia pintar. Costo escribir
//! `PANTALLA_SOLTAR` despues, con el fallo delante.
//!
//! Aqui se hace al reves desde el primer dia: **se toma al abrir y se devuelve
//! al cerrar**. La ventana es un huesped del aparato, no su dueno. Por eso
//! `Sonido::release` existia antes de que hubiera nadie que lo llamara.
//!
//! === Por que F10 ===
//!
//! Una tecla de funcion no produce caracter en ninguna distribucion, asi que no
//! choca con escribir -- el mismo motivo que F11 y F12. Y va pegada a ellas
//! porque es la misma familia: ventanas que se abren con una tecla y se leen.

use bmo_userland as bmo;

use super::*;
use crate::texto::decimal;

pub(crate) const SON_ANCHO: u32 = 620;
pub(crate) const SON_ALTO: u32 = 300;

// El ambar es de esta ventana igual que el azul es del kernel y el verde de
// ESTRATOS: el color dice cual es antes de leer el titulo.
const SON_FONDO: u32 = 0x0018_1105;
const SON_TITULO_FONDO: u32 = 0x0026_1A08;
const SON_BORDE: u32 = 0x004A_3418;
const SON_TITULO: u32 = 0x00F0_A860;
const SON_BARRA: u32 = 0x00D8_8C3A;
const SON_BARRA_HUECO: u32 = 0x0032_2410;
const TECLA_BLANCA: u32 = 0x00C8_C0B0;
const TECLA_NEGRA: u32 = 0x0020_1C18;
const TECLA_PULSADA: u32 = 0x00F0_A860;

/// Las siete notas del teclado de abajo, y la letra que las toca.
///
/// Es una octava y no mas: caben siete teclas legibles en el ancho de la
/// ventana, y el objeto de esto es **comprobar que suena**, no tocar una pieza.
/// Para una pieza esta `c/musica.bex`, que es un programa y no una ventana.
pub(crate) const NOTAS: [(u8, u32, &str); 7] = [
    (b'z', 262, "DO"),
    (b'x', 294, "RE"),
    (b'c', 330, "MI"),
    (b'v', 349, "FA"),
    (b'b', 392, "SOL"),
    (b'n', 440, "LA"),
    (b'm', 494, "SI"),
];

/// Donde va, centrada sobre el panel.
pub(crate) struct CajaSonido {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) ancho: u32,
    pub(crate) alto: u32,
}

impl CajaSonido {
    pub(crate) fn nueva(p: &bmo::Pantalla) -> Self {
        let ancho = SON_ANCHO.min(p.ancho.saturating_sub(40));
        let alto = SON_ALTO.min(p.alto.saturating_sub(40));
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

/// Escribe `s` en `dst` desde `n`, sin salirse. Igual que en `klog`.
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

/// Pinta la ventana entera.
///
/// `aparatos` es la mascara que contesto el kernel (0 = no se pudo preguntar
/// porque el sonido es de otro), `volumen` el que esta puesto, y `pulsada` la
/// nota que se esta tocando ahora mismo, si alguna.
///
/// No recuerda nada entre llamadas **a proposito**: el estado de la sesion vive
/// en `main.rs`, igual que el desplazamiento y el filtro del klog. Un modulo que
/// pinta y ademas recuerda acaba teniendo dos verdades sobre lo mismo.
pub(crate) fn pintar(
    p: &bmo::Pantalla,
    c: &CajaSonido,
    tengo: bool,
    aparatos: u64,
    volumen: u8,
    pulsada: Option<usize>,
) {
    sombra(p, c.x, c.y, c.ancho, c.alto);
    rect_redondeado(p, c.x, c.y, c.ancho, c.alto, SON_BORDE);
    rect_redondeado(p, c.x + 1, c.y + 1, c.ancho - 2, c.alto - 2, SON_FONDO);

    // La barra de titulo, con la misma curva que la ventana.
    for i in 0..RADIO {
        let s = super::curva(i);
        p.rect(c.x + s, c.y + 1 + i, c.ancho - 2 * s, 1, SON_TITULO_FONDO);
    }
    p.rect(c.x + 1, c.y + 1 + RADIO, c.ancho - 2, TITULO_ALTO - 2 - RADIO, SON_TITULO_FONDO);
    p.rect(c.x + 1, c.y + TITULO_ALTO - 1, c.ancho - 2, 1, SON_TITULO);

    let tx = c.x + 16;
    p.rect(tx, c.y + 9, 8, 8, SON_TITULO);
    let px = p.texto(tx + 16, c.y + 8, "Sonido", TEXTO);
    p.texto(px + 2 * bmo::GLIFO_ANCHO, c.y + 8, "KIND_AUDIO", TEXTO_TENUE);

    let mut ty = c.y + TITULO_ALTO + 10;

    // -- 1. EL APARATO -------------------------------------------------
    //
    // Primero porque decide lo demas. Y con la salvedad dicha: un bit puesto
    // significa que hay CAMINO, no que se vaya a oir algo -- el puerto del
    // altavoz existe en todo x86 y el zumbador fisico no.
    if !tengo {
        p.texto(tx, ty, "el sonido lo tiene OTRO proceso.", TEXTO);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "cierra esta ventana y vuelve a abrirla cuando lo suelte.", TEXTO_TENUE);
        return;
    }

    let mut lin = [0u8; 80];
    let mut n = 0usize;
    poner(b"aparato   ", &mut lin, &mut n);
    if aparatos & 1 != 0 {
        poner(b"altavoz del PC", &mut lin, &mut n);
    } else {
        poner(b"ninguno", &mut lin, &mut n);
    }
    if aparatos & 2 != 0 {
        poner(b" + HD Audio", &mut lin, &mut n);
    }
    p.texto_bytes(tx, ty, &lin[..n], TEXTO);
    ty += bmo::GLIFO_ALTO + 2;

    // La salvedad, en tenue y siempre. Sin ella, un altavoz que no suena parece
    // un sistema roto -- y la mitad de las placas modernas no traen zumbador.
    p.texto(
        tx,
        ty,
        "(hay camino; que suene depende de la placa)",
        TEXTO_TENUE,
    );
    ty += bmo::GLIFO_ALTO + 2;
    if aparatos & 2 == 0 {
        p.texto(tx, ty, "HD Audio: sin driver todavia -- casilla 5.1", TEXTO_TENUE);
    }
    ty += bmo::GLIFO_ALTO + 10;

    // -- 2. EL VOLUMEN --------------------------------------------------
    let mut lin = [0u8; 40];
    let mut n = 0usize;
    poner(b"volumen   ", &mut lin, &mut n);
    num(volumen as u64, &mut lin, &mut n);
    poner(b" / 100", &mut lin, &mut n);
    p.texto_bytes(tx, ty, &lin[..n], TEXTO);
    ty += bmo::GLIFO_ALTO + 6;

    let barra_ancho = c.ancho - 32 - 16;
    p.rect(tx, ty, barra_ancho, 10, SON_BARRA_HUECO);
    let lleno = (barra_ancho * volumen as u32) / 100;
    if lleno > 0 {
        p.rect(tx, ty, lleno, 10, SON_BARRA);
    }
    // La marca del escalon: en el altavoz del PC el volumen NO es continuo --
    // son dos modos del temporizador, y el corte esta en 50. Dibujarlo evita
    // que parezca que la barra no hace nada entre 51 y 100.
    p.rect(tx + barra_ancho / 2, ty - 3, 1, 16, SON_TITULO);
    ty += 16;
    p.texto(
        tx,
        ty,
        "el altavoz del PC tiene DOS escalones, no cien: el corte es la marca",
        TEXTO_TENUE,
    );
    ty += bmo::GLIFO_ALTO + 12;

    // -- 3. EL TECLADO --------------------------------------------------
    //
    // Siete teclas. Existe para poder COMPROBAR: un control de sonido que no
    // deja hacer ruido no se puede probar, y entonces no se sabe si funciona.
    let tecla_w = 52u32;
    let tecla_h = 54u32;
    let sep = 4u32;
    let kx = tx;
    for (i, (letra, _, nombre)) in NOTAS.iter().enumerate() {
        let x = kx + i as u32 * (tecla_w + sep);
        let color = if pulsada == Some(i) { TECLA_PULSADA } else { TECLA_BLANCA };
        p.rect(x, ty, tecla_w, tecla_h, color);
        // El nombre de la nota arriba y la letra que la toca abajo: la ventana
        // se explica sola sin una leyenda aparte.
        p.texto(x + 8, ty + 8, nombre, TECLA_NEGRA);
        let mut l = [0u8; 1];
        l[0] = (*letra as char).to_ascii_uppercase() as u8;
        p.texto_bytes(x + 8, ty + tecla_h - bmo::GLIFO_ALTO - 8, &l, TECLA_NEGRA);
    }
    ty += tecla_h + 8;

    // -- 4. La barra de atajos, del mismo estilo que las otras ventanas --
    p.texto(
        tx,
        ty,
        "flechas volumen   Z..M notas   P la frase   ESC cierra y DEVUELVE el aparato",
        TEXTO_TENUE,
    );
}
