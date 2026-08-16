//! **VISTA CIUDAD** -- la intro de arranque, vista sin arrancar la maquina.
//!
//! ## Por que existe
//!
//! La cabecera de `bmo-ciudad` lleva escrita esta frase desde el primer dia:
//!
//! > *"Un arranque animado que solo se puede juzgar reiniciando la maquina es un
//! > arranque que nadie va a ajustar nunca."*
//!
//! Y hasta hoy la regla se cumplia a medias. Los NUMEROS del guion se prueban
//! desde siempre --que nada de un salto, que los actos entreguen-- pero **la
//! imagen no se podia ver**. Para juzgar si el gato se separa del fondo habia que
//! grabar el disco, reiniciar el Ryzen y grabarlo con el movil. El dueno lo hizo
//! tres veces en dos dias, y en la tercera dijo lo que estaba mal: *"la capa
//! estan mezcladas"*.
//!
//! Un ciclo de ajuste que dura un reinicio es un ciclo que se usa dos veces y se
//! abandona. Esto lo baja a un segundo.
//!
//! ## Que dibuja, y por que es lo MISMO que el kernel
//!
//! El cielo, las torres, la niebla, el aura y el encuadre salen de `bmo-ciudad`
//! -- **el mismo codigo que corre en Ring 0**, no una imitacion. Si aqui el logo
//! cae sobre los tejados, en el metal tambien; y si aqui se ve separado, alli
//! tambien. Es el argumento de `bmo-hash` otra vez: las dos orillas calculan lo
//! mismo porque ejecutan lo mismo.
//!
//! Lo unico que se trae de fuera son **las mascaras del gato**, que son datos
//! generados (`docs/arte/gato_a_mascara.py`) y no logica. Se incluyen con
//! `include!` del fichero del kernel: una copia de esos bytes aqui si seria una
//! mentira esperando a divergir.
//!
//! [!] Lo que NO se comparte es la fuente del kernel, asi que el titulo `BMO-X`
//! sale como un bloque macizo del tamano que ocupa. Para juzgar el ENCUADRE --que
//! es lo que se rompio-- eso basta y sobra: lo que importa es donde cae y cuanto
//! mide, no que letra es.
//!
//! ## Uso
//!
//! ```text
//!   cargo run -p bmo-vista-ciudad -- [ancho] [alto] [ms] [salida.ppm]
//! ```
//!
//! Sin argumentos: 1920x1080, el instante en que el gato esta entero, a
//! `vista.ppm`. PPM porque son diez lineas y cero dependencias; cualquier visor
//! lo abre, y `ffmpeg -i vista.ppm vista.png` lo convierte si hace falta.

use bmo_ciudad::paleta::{mezcla, Color, NEGRO, NEON_CIAN};
use bmo_ciudad::{Camara, Ciudad, Medidas};
use std::io::Write;

// -- LAS MASCARAS DEL GATO ---------------------------------------------------
//
// Datos generados, tomados del kernel TAL CUAL. Ver la cabecera: copiarlos aqui
// seria crear una segunda verdad que diverge el dia que se regenere el logo.
//
// Con `#[path]` y no con `include!`: el fichero empieza con comentarios `//!` de
// modulo, y esos solo son validos si el fichero SE TRATA como un modulo.
const ANCHO: u32 = 152;
const ALTO: u32 = 180;
#[path = "../../../../Ultra_kernel_x86-64/kernel/src/ring0/core/gato/masks.rs"]
mod gato;

/// El lienzo. Un `Vec` de pixeles y un `fill_rect` que se parece al del kernel
/// lo justo para que el codigo de dibujo se lea igual en los dos sitios.
struct Lienzo {
    w: u32,
    h: u32,
    px: Vec<Color>,
}

impl Lienzo {
    fn nuevo(w: u32, h: u32) -> Self {
        Lienzo { w, h, px: vec![NEGRO; (w * h) as usize] }
    }

    fn rect(&mut self, x: i32, y: i32, rw: i32, rh: i32, c: Color) {
        if rw <= 0 || rh <= 0 {
            return;
        }
        for fy in y.max(0)..(y + rh).min(self.h as i32) {
            for fx in x.max(0)..(x + rw).min(self.w as i32) {
                self.px[(fy as u32 * self.w + fx as u32) as usize] = c;
            }
        }
    }

    /// PPM binario (P6). Diez lineas y ninguna dependencia.
    fn guardar(&self, ruta: &str) -> std::io::Result<()> {
        let mut f = std::io::BufWriter::new(std::fs::File::create(ruta)?);
        write!(f, "P6\n{} {}\n255\n", self.w, self.h)?;
        for p in &self.px {
            f.write_all(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8])?;
        }
        f.flush()
    }
}

fn bit(m: &[u8], i: usize) -> bool {
    m[i / 8] >> (i % 8) & 1 == 1
}

fn hay_luz(x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= ANCHO as i32 || y >= ALTO as i32 {
        return false;
    }
    let i = (y as u32 * ANCHO + x as u32) as usize;
    bit(&gato::TRAZO, i) || bit(&gato::OJOS, i)
}

fn luz_kanji(x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= gato::KANJI_ANCHO as i32 || y >= gato::KANJI_ALTO as i32 {
        return false;
    }
    bit(&gato::KANJI, (y as u32 * gato::KANJI_ANCHO + x as u32) as usize)
}

/// La misma dilatacion, generica sobre que se considera "luz". Igual que en
/// `ring0::core::gato::neon::dilatar`.
fn nivel(x: i32, y: i32, luz: impl Fn(i32, i32) -> bool) -> u8 {
    if luz(x, y) {
        return 0;
    }
    let mut d2 = i32::MAX;
    for dy in -RADIO..=RADIO {
        for dx in -RADIO..=RADIO {
            if luz(x + dx, y + dy) {
                let m = dx * dx + dy * dy;
                if m < d2 {
                    d2 = m;
                }
            }
        }
    }
    if d2 == i32::MAX {
        return RADIO as u8 + 1;
    }
    for n in 1..=RADIO {
        if n * n >= d2 {
            return n as u8;
        }
    }
    RADIO as u8 + 1
}

/// El mismo radio que `ring0::core::gato::neon::RADIO`. Si aqui se calculara
/// distinto no se estaria previsualizando lo que va a pasar.
const RADIO: i32 = 4;

fn main() {
    let arg: Vec<String> = std::env::args().collect();
    let leer = |i: usize, por_defecto: u32| -> u32 {
        arg.get(i).and_then(|s| s.parse().ok()).unwrap_or(por_defecto)
    };
    let w = leer(1, 1920);
    let h = leer(2, 1080);
    // Por defecto, el instante en que el gato ya esta entero y los ojos
    // encendidos: es el fotograma que hay que juzgar.
    let ms = leer(3, bmo_ciudad::acto::FIN_GATO);
    let salida = arg.get(4).cloned().unwrap_or_else(|| "vista.ppm".into());

    let f = bmo_ciudad::fotograma(ms);
    let mut l = Lienzo::nuevo(w, h);
    let mut c = Ciudad::nueva(w as i32, h as i32, ((w as u64) << 20) | h as u64);
    c.encender(100);

    // -- LA CIUDAD, detras de todo. Igual que en `pintar_escena`.
    let cam = Camara::nueva(f.avance);
    let mut rects = Vec::new();
    c.dibujar(cam, |x, y, cw, ch, color| rects.push((x, y, cw, ch, color)));
    for (x, y, cw, ch, color) in rects {
        l.rect(x, y, cw, ch, color);
    }

    // -- EL ENCUADRE, pedido al mismo sitio que lo pide el kernel.
    let escala = if h >= 900 { 2 } else { 1 };
    let escala_t = if h >= 900 { 5 } else { 4 };
    let medidas = Medidas {
        pantalla_w: w,
        pantalla_h: h,
        techo: c.techo().max(0) as u32,
        gato_w: ANCHO * escala,
        gato_h: ALTO * escala,
        kanji_w: gato::KANJI_ANCHO * escala,
        kanji_h: gato::KANJI_ALTO * escala,
        hueco_kanji: 22 * escala,
        // La fuente del kernel es de 8x16; `BMO-X` son cinco caracteres.
        titulo_w: 5 * 8 * escala_t,
        titulo_h: 16 * escala_t,
        linea_h: 16,
    };
    let enc = bmo_ciudad::componer(&medidas);

    // -- EL AURA.
    let mut auras = Vec::new();
    bmo_ciudad::aura(
        |y| c.color_cielo(y),
        enc.aura_cx,
        enc.aura_cy,
        enc.aura_rx,
        enc.aura_ry,
        NEON_CIAN,
        50 * f.gato_alfa / 255,
        |x, y, aw, ah, color| auras.push((x, y, aw, ah, color)),
    );
    for (x, y, aw, ah, color) in auras {
        l.rect(x, y, aw, ah, color);
    }

    // -- EL GATO, con nucleo y derrame.
    const TRAZO_APAGADO: Color = 0xFF1A1730;
    const HALO_MAX: u32 = 150;
    const FUERZA_AURA: u32 = 50;
    let blanco: Color = 0xFFFFFFFF;
    let gy = (enc.gato_y as i32 + f.gato_flote).max(0) as u32;
    let c_trazo = mezcla(TRAZO_APAGADO, blanco, f.gato_alfa, 255);
    let ojos_a = (f.ojos_alfa + f.ojos_pulso).min(255);
    let c_ojos = mezcla(TRAZO_APAGADO, NEON_CIAN, ojos_a, 255);
    for fy in 0..ALTO {
        let y = gy + fy * escala;
        // El fondo bajo el gato es el aura, reconstruida con la misma
        // aritmetica que en el kernel.
        let ry = enc.aura_ry as u32;
        let dy = (y as i32 - enc.aura_cy).unsigned_abs().min(ry);
        let cerca = ry - dy;
        let f_aura = (FUERZA_AURA * f.gato_alfa / 255) * cerca * cerca / (ry * ry).max(1);
        let bg = mezcla(c.color_cielo(y as i32), NEON_CIAN, f_aura, 255);
        let r = RADIO as u32;
        let mut halo = [0u32; RADIO as usize];
        for (n, hc) in halo.iter_mut().enumerate() {
            let queda = r - n as u32;
            let fh = HALO_MAX * queda * queda / (r * r) * f.gato_alfa / 255;
            *hc = mezcla(bg, NEON_CIAN, fh, 255);
        }
        for fx in 0..ANCHO {
            let i = (fy * ANCHO + fx) as usize;
            let d = nivel(fx as i32, fy as i32, hay_luz);
            let color = if d == 0 {
                if bit(&gato::OJOS, i) { c_ojos } else { c_trazo }
            } else if d <= RADIO as u8 {
                halo[d as usize - 1]
            } else {
                continue;
            };
            l.rect((enc.gato_x + fx * escala) as i32, y as i32, escala as i32, escala as i32, color);
        }
    }

    // -- EL KANJI.
    let ky = (enc.kanji_y as i32 + f.gato_flote).max(0) as u32;
    let c_kanji = mezcla(TRAZO_APAGADO, NEON_CIAN, ojos_a, 255);
    for fy in 0..gato::KANJI_ALTO {
        let y = ky + fy * escala;
        let ry = enc.aura_ry as u32;
        let dy = (y as i32 - enc.aura_cy).unsigned_abs().min(ry);
        let cerca = ry - dy;
        let f_aura = (FUERZA_AURA * f.gato_alfa / 255) * cerca * cerca / (ry * ry).max(1);
        let bg = mezcla(c.color_cielo(y as i32), NEON_CIAN, f_aura, 255);
        let r = RADIO as u32;
        let mut halo = [0u32; RADIO as usize];
        for (n, hc) in halo.iter_mut().enumerate() {
            let queda = r - n as u32;
            *hc = mezcla(bg, NEON_CIAN, HALO_MAX * queda * queda / (r * r) * ojos_a / 255, 255);
        }
        for fx in 0..gato::KANJI_ANCHO {
            let d = nivel(fx as i32, fy as i32, luz_kanji);
            let color = if d == 0 {
                c_kanji
            } else if d <= RADIO as u8 {
                halo[d as usize - 1]
            } else {
                continue;
            };
            l.rect(
                (enc.kanji_x + fx * escala) as i32,
                y as i32,
                escala as i32,
                escala as i32,
                color,
            );
        }
    }

    // -- EL TITULO, como bloque: aqui no hay fuente del kernel. Lo que se juzga
    // es DONDE cae y CUANTO mide, no que letra es.
    l.rect(
        enc.titulo_x as i32,
        enc.titulo_y as i32,
        medidas.titulo_w as i32,
        medidas.titulo_h as i32,
        mezcla(NEGRO, blanco, f.gato_alfa, 255),
    );
    l.rect(
        enc.titulo_x as i32,
        (enc.titulo_y + medidas.titulo_h + 10) as i32,
        medidas.titulo_w as i32,
        3,
        mezcla(NEGRO, NEON_CIAN, f.gato_alfa, 255),
    );

    match l.guardar(&salida) {
        Ok(()) => println!(
            "{} -- {}x{} en el ms {} (acto {:?}); techo de la ciudad en y={}, el logo acaba en y={}",
            salida,
            w,
            h,
            ms,
            f.acto,
            medidas.techo,
            enc.gato_y + enc.alto_total
        ),
        Err(e) => eprintln!("no se pudo escribir {}: {}", salida, e),
    }
}
