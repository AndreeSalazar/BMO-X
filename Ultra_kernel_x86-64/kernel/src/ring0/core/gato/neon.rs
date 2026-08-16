//! **EL BLOOM DEL GATO** -- lo que convierte un trazo en un tubo de neon.
//!
//! === El fallo, visto en video ===
//!
//! El video del 2026-08-15 en el Ryzen: el gato es una linea blanca de un pixel
//! sobre un cielo violeta claro, y **no se despega de la escena**. Cuando el
//! degradado del cielo llega a su parte clara, el gato casi desaparece. El dueno
//! lo dijo asi: *"la capa estan mezcladas"*.
//!
//! El pedido original era *"el gato se ve neon"*, y lo que habia era el gato
//! **encendido**: el trazo pasa de gris oscuro a blanco. Eso es una bombilla, no
//! un neon. Lo que hace que algo se lea como tubo de gas no es que brille: es que
//! **derrama luz de su color en lo que tiene alrededor**. Sin ese derrame, un
//! trazo brillante es una pegatina, por muy blanco que sea.
//!
//! === Como se hace sin leer el framebuffer ===
//!
//! La forma normal seria desenfocar el trazo. Un desenfoque de verdad pide leer
//! lo ya pintado, y **leer el framebuffer esta prohibido aqui**: es memoria
//! write-combining y va lentisimo, la misma trampa anotada en
//! `draw_gato_encendido` y la que costo cara en el blit de DOOM.
//!
//! Asi que el derrame se calcula **en la mascara, no en la pantalla**: se mide,
//! para cada pixel, a que distancia esta del trazo mas cercano.
//!
//! ```text
//!    distancia 0   el trazo. Nucleo blanco.
//!    distancia 1   halo cercano. Cian fuerte.
//!    distancia 2   halo lejano.  Cian tenue.
//!    mas lejos     nada.
//! ```
//!
//! Los tres conjuntos son **disjuntos**, asi que al pintar cada pixel cae en un
//! caso y en uno solo: el dibujo sigue siendo un test de bit por pixel, igual que
//! antes. No se pinta nada dos veces y no se lee nada.
//!
//! === Y se calcula UNA vez ===
//!
//! La dilatacion cuesta una ventana de 5x5 por pixel: unos 680.000 tests de bit
//! sobre las 27.360 casillas de la mascara. Hacerlo en cada fotograma seria
//! pagarlo sesenta veces por segundo por un resultado que **no cambia nunca** --
//! la mascara es constante. Se hace al empezar la intro y se guarda en dos
//! mascaras de 1 bit, que son 6.840 bytes de `.bss`.
//!
//! [!] Sin `alloc` y sin coma flotante, como todo lo que corre aqui.

use super::{ALTO, ANCHO, KANJI, KANJI_ALTO, KANJI_ANCHO, OJOS, TRAZO};

/// Hasta que distancia del trazo llega el derrame, en pixeles de mascara.
///
/// [!] **Estaba en 2 y el previsualizador lo tumbo.** Con dos pixeles el halo
/// era un contorno grueso, no un resplandor, y hacia falta un ovalo enorme
/// detras para que el gato se separase -- que salia como un globo turquesa
/// compitiendo con el propio gato.
///
/// Con cuatro, **la luz sigue la forma del trazo**, que es lo que hace un tubo de
/// neon de verdad: ilumina lo que tiene alrededor con SU silueta. Entonces el
/// ovalo de detras puede quedarse en un lavado suave, que es su papel.
///
/// A escala x2 son ocho pixeles de pantalla.
pub(crate) const RADIO: u8 = 4;

/// Cuantos bytes ocupa una mascara de 1 bit de `ancho x alto`.
const fn bytes(ancho: u32, alto: u32) -> usize {
    (ancho as usize * alto as usize).div_ceil(8)
}

const BYTES_GATO: usize = bytes(ANCHO, ALTO);
const BYTES_KANJI: usize = bytes(KANJI_ANCHO, KANJI_ALTO);

/// **El derrame de una pieza**: una mascara de 1 bit por nivel de distancia.
///
/// `niveles[0]` es la distancia 1 --lo pegado al trazo, lo mas encendido-- y
/// `niveles[RADIO-1]` la mas lejana.
///
/// Cuatro mascaras de 1 bit del gato son 13.680 bytes de `.bss`. La alternativa
/// --un byte por pixel-- serian 27.360 para guardar un numero que nunca pasa de
/// cuatro.
struct Halo<const N: usize> {
    niveles: [[u8; N]; RADIO as usize],
}

impl<const N: usize> Halo<N> {
    const fn vacio() -> Self {
        Halo { niveles: [[0; N]; RADIO as usize] }
    }

    /// A que nivel esta el pixel `i`: `1..=RADIO`, o `RADIO + 1` si a nada.
    fn nivel(&self, i: usize) -> u8 {
        for (n, m) in self.niveles.iter().enumerate() {
            if bit(m, i) {
                return n as u8 + 1;
            }
        }
        RADIO + 1
    }
}

/// El derrame del gato (trazo + ojos) y el del kanji. Son dos piezas del mismo
/// letrero y las dos tienen que encender el aire: **un kanji plano al lado de un
/// gato con halo se lee como dos dibujos pegados**, no como una marca.
static mut HALO_GATO: Halo<BYTES_GATO> = Halo::vacio();
static mut HALO_KANJI: Halo<BYTES_KANJI> = Halo::vacio();
/// Ya se calcularon? La dilatacion es cara y el resultado no cambia nunca.
static mut LISTO: bool = false;

/// Lee el bit `i` de una mascara.
fn bit(m: &[u8], i: usize) -> bool {
    m[i / 8] >> (i % 8) & 1 == 1
}

/// Enciende el bit `i` de una mascara.
fn poner(m: &mut [u8], i: usize) {
    m[i / 8] |= 1 << (i % 8);
}

/// **La dilatacion.** Rellena `halo` con la distancia de cada pixel apagado al
/// pixel encendido mas cercano de `hay_luz`.
///
/// Generica sobre el tamano porque el gato y el kanji son dos mascaras
/// distintas y **la operacion es la misma**. Escrita dos veces serian dos sitios
/// donde ajustar el radio, que es como se acaba con un kanji que brilla distinto
/// del gato.
fn dilatar<const N: usize>(
    ancho: u32,
    alto: u32,
    hay_luz: impl Fn(i32, i32) -> bool,
    halo: &mut Halo<N>,
) {
    let r = RADIO as i32;
    for y in 0..alto as i32 {
        for x in 0..ancho as i32 {
            if hay_luz(x, y) {
                // El trazo mismo no es halo: es el nucleo. Los conjuntos tienen
                // que quedar disjuntos para que dibujar siga siendo un caso por
                // pixel.
                continue;
            }
            // Distancia EUCLIDEA (comparando cuadrados, sin raiz). Con radio 2
            // valia la de Chebyshev --la diferencia eran cuatro esquinas que
            // nadie ve--, pero a radio 4 la de Chebyshev dibuja un resplandor
            // con **esquinas cuadradas**, y una luz con esquinas no es una luz.
            let mut d2 = i32::MAX;
            for dy in -r..=r {
                for dx in -r..=r {
                    if hay_luz(x + dx, y + dy) {
                        let m = dx * dx + dy * dy;
                        if m < d2 {
                            d2 = m;
                        }
                    }
                }
            }
            if d2 == i32::MAX {
                continue;
            }
            // `nivel*nivel >= d2` es lo mismo que `nivel >= raiz(d2)` sin
            // calcular la raiz.
            let i = (y as u32 * ancho + x as u32) as usize;
            for nivel in 1..=r {
                if nivel * nivel >= d2 {
                    poner(&mut halo.niveles[(nivel - 1) as usize], i);
                    break;
                }
            }
        }
    }
}

/// Hay trazo (o un ojo) del gato en `(x, y)`? Fuera de la mascara, no.
fn luz_gato(x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= ANCHO as i32 || y >= ALTO as i32 {
        return false;
    }
    let i = (y as u32 * ANCHO + x as u32) as usize;
    bit(&TRAZO, i) || bit(&OJOS, i)
}

/// Lo mismo para el kanji.
fn luz_kanji(x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= KANJI_ANCHO as i32 || y >= KANJI_ALTO as i32 {
        return false;
    }
    bit(&KANJI, (y as u32 * KANJI_ANCHO + x as u32) as usize)
}

/// **Calcula los dos halos. Una vez por arranque.**
///
/// Idempotente a proposito: la intro se dibuja desde muchos sitios del arranque
/// y ninguno tiene por que saber si es el primero.
pub(crate) fn preparar() {
    if unsafe { LISTO } {
        return;
    }
    // SAFETY: arranque de un solo hilo, antes de que exista el planificador. La
    // bandera de arriba es lo unico que protege esto, y es suficiente porque
    // aqui todavia no hay dos ejecuciones posibles.
    unsafe {
        dilatar(ANCHO, ALTO, luz_gato, &mut *core::ptr::addr_of_mut!(HALO_GATO));
        dilatar(
            KANJI_ANCHO,
            KANJI_ALTO,
            luz_kanji,
            &mut *core::ptr::addr_of_mut!(HALO_KANJI),
        );
        LISTO = true;
    }
}

/// **A que distancia del trazo del GATO esta este pixel.**
///
/// `0` = nucleo (trazo u ojo). `1..=RADIO` = derrame, mas debil cuanto mayor.
/// `RADIO + 1` = nada, no se pinta. Es lo unico que el dibujante necesita
/// preguntar, y por eso es lo unico publico.
pub(crate) fn distancia(i: usize) -> u8 {
    if bit(&OJOS, i) || bit(&TRAZO, i) {
        return 0;
    }
    // SAFETY: solo lectura de un estatico que `preparar` deja escrito antes de
    // que nadie dibuje.
    unsafe { (*core::ptr::addr_of!(HALO_GATO)).nivel(i) }
}

/// Lo mismo para el kanji.
pub(crate) fn distancia_kanji(i: usize) -> u8 {
    if bit(&KANJI, i) {
        return 0;
    }
    unsafe { (*core::ptr::addr_of!(HALO_KANJI)).nivel(i) }
}
