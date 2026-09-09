//! **CARRIL VERDE** -- LAS LETRAS. Se puede tocar sin miedo.
//!
//! [carril]  VERDE     no toca memoria cruda ni cuentas de nadie: pone
//!                     `punto` encima de `punto`. Lo peor que puede pasar aqui
//!                     es que una letra salga fea, y eso se ve al instante.
//!
//! [cuesta]  NADA -- un glifo mal no rompe nada y no convence a nadie: se ve.
//!           Es la definicion de verde, y decirlo tambien es informacion. Este
//!           es el fichero al que se viene a cambiar algo deprisa con la
//!           maquina rota.
//!
//! [riesgo]  ESPEJO
//!           ESPEJO -- la fuente esta DUPLICADA: la misma tabla la tiene el
//!                    kernel en `core/splash/`. Salen del mismo generador
//!                    (`toolchain/tools/fontgen`), asi que no son dos fuentes
//!                    que puedan divergir por si solas -- pero editar una a
//!                    mano si las separa, y nadie avisaria.
//!
//! # Por que esta copiada aqui y no se pide por syscall
//!
//! La alternativa era una operacion `DIBUJAR_TEXTO` sobre el framebuffer, y eso
//! es exactamente la linea que `KIND_FRAMEBUFFER` existe para no cruzar: el
//! kernel contesta cuatro preguntas y se aparta. Si Ring 0 dibujara letras
//! tendria que saber de tipografia, de kerning y de colores -- decisiones de
//! aspecto, ninguna suya. Es el mismo argumento que dejo el cursor en Ring 3.
//!
//! # *** Y EL CARRIL VERDE ERA EL SEGUNDO CUELLO DE BOTELLA (2026-09-09)
//!
//! `glifo` llamaba a `punto` por cada bit encendido, y `punto` **marca**. Marcar
//! copia `Sucias` entera --136 bytes-- para leerla y otra vez para escribirla:
//!
//! ```text
//!    la barra de tareas, un fotograma     ~110 letras, ~4.950 pixeles
//!    lo que esos pixeles PINTAN                    19,3 KiB
//!    lo que su contabilidad COPIA               1.315,0 KiB
//!    ------------------------------------------------------
//!    razon papeleo / trabajo                         68 a 1
//! ```
//!
//! ** Sesenta y ocho veces mas memoria movida para APUNTAR el trabajo que para
//! hacerlo. Y estaba **aqui**, en el carril que dice *"no puede pasar nada
//! malo"* -- que seguia siendo verdad: no pasaba nada malo, solo costaba.
//!
//! *** La leccion, y es la que hay que llevarse: **un carril VERDE no es un
//! carril barato.** El color dice lo que arriesgas al tocarlo, no lo que cuesta
//! ejecutarlo. Buscar rendimiento solo en lo rojo es exactamente como se pasan
//! los sesenta y ocho a uno por delante de las narices.
//!
//! `rect` ya lo hacia bien desde agosto: *"se marca UNA vez, con las medidas ya
//! recortadas"*. Este fichero no lo habia copiado.
//!
//! # Lo que este carril NO hace, y es deliberado
//!
//! ```text
//!    [ ] no usa `rep stosd`: un glifo pinta pixeles SUELTOS --solo los bits
//!        encendidos-- y ahi una instruccion de cadena no tiene nada que hacer.
//!        `glifo_escala` si, porque amplia a cuadrados, y por eso llama a `rect`
//!    [ ] no recorta el mismo: `punto_ya_marcado` sigue comprobando los limites.
//!        Lo caro era el papeleo, no el `cmp` -- y quitar el recorte moveria
//!        este fichero a [cuesta] MAQUINA para ahorrar dos instrucciones
//! ```

use crate::*;

/// La fuente 8x16 de BMO, la MISMA que pinta el kernel.
///
/// Aqui hay 4 KiB de tabla duplicada. Sale del mismo generador
/// (`toolchain/tools/fontgen`), asi que no son dos fuentes que puedan
/// divergir: son dos copias de una, y regenerar actualiza las dos.
static FONT16: [[u8; 16]; 120] = include!("../font16_data.rs");
static FONT_EXTRA: [u8; 25] = include!("../font16_extra.rs");
const ASCII_GLYPHS: usize = 95;

/// Ancho y alto de un glifo, en pixeles. El avance horizontal ES el ancho: la
/// fuente ya trae su propio espaciado dentro del mapa de bits.
pub const GLIFO_ANCHO: u32 = 8;
pub const GLIFO_ALTO: u32 = 16;

/// Byte -> indice de glifo. ASCII directo; para el espanol (n, a, ...) se
/// busca el byte Latin-1 en la tabla de extras.
///
/// **Latin-1 y no UTF-8, igual que en Ring 0.** Un caracter es UN byte, asi el
/// teclado, la caja y la fuente hablan el mismo idioma sin decodificador de por
/// medio. Un `&str` de Rust es UTF-8, asi que una `n` escrita en el codigo
/// fuente llega como dos bytes y no se dibuja: para eso estan `texto_bytes` y
/// el hecho de que lo que se teclea ya viene en Latin-1 del kernel.
fn indice_glifo(c: u8) -> Option<usize> {
    if (32..=126).contains(&c) {
        return Some(c as usize - 32);
    }
    let mut i = 0;
    while i < FONT_EXTRA.len() {
        if FONT_EXTRA[i] == c {
            return Some(ASCII_GLYPHS + i);
        }
        i += 1;
    }
    None
}

impl Pantalla {
    /// Un caracter. Solo pinta los pixeles encendidos: el fondo se respeta,
    /// que es lo que permite escribir encima de lo que ya hay sin recuadros.
    ///
    /// ** SE MARCA LA CELDA ENTERA UNA VEZ, y luego se pintan los bits. La
    /// version anterior marcaba pixel a pixel y eso costaba **68 veces mas que
    /// pintar**: ver [`Pantalla::punto_ya_marcado`], donde esta la cuenta.
    ///
    /// Es la misma regla que `rect` llevaba puesta desde agosto --*"se marca UNA
    /// vez, con las medidas ya recortadas, en vez de un pixel por vuelta"*-- y
    /// que este fichero no habia copiado.
    ///
    /// [!] Se marca la celda ENTERA aunque solo se enciendan sus bits: la caja
    /// sucia es un rectangulo, y el rectangulo que contiene a los bits de una
    /// letra es su celda. Marcar de menos deja letras a medio volcar; marcar la
    /// celda es exacto, no generoso.
    pub fn glifo(&self, x: u32, y: u32, c: u8, color: u32) {
        let idx = match indice_glifo(c) {
            Some(i) => i,
            None => return,
        };
        self.marcar(x, y, GLIFO_ANCHO, GLIFO_ALTO);
        let g = &FONT16[idx];
        for (fila, &bits) in g.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            for col in 0..8u32 {
                if bits & (0x80 >> col) != 0 {
                    self.punto_ya_marcado(x + col, y + fila as u32, color);
                }
            }
        }
    }

    /// Un caracter AMPLIADO por un entero: cada pixel del glifo pasa a ser un
    /// cuadrado de `escala`.
    ///
    /// [!] **Esta NO lleva el arreglo de marcar una vez**, y es deliberado: sus
    /// `rect` marcan uno por cuadrado, hasta 45 veces por letra. No se toca
    /// porque no esta en el camino caliente --el texto grande sale en el
    /// arranque, en la calculadora y en poco mas, nunca por fotograma-- y
    /// cambiarla pediria una version de `rect` que no marca, o sea mas API para
    /// nadie. El dia que un titulo se repinte a 60 Hz, esta es la linea.
    ///
    /// Entero y con `rect`, no interpolado: ampliar por 4 un glifo de 8x16 da
    /// bloques limpios de 32x64, y esa estetica es la que tiene esta maquina.
    /// Una interpolacion pediria coma flotante, un buffer intermedio y un gusto
    /// que no es el de aqui -- y con la misma fuente que ya esta cargada.
    pub fn glifo_escala(&self, x: u32, y: u32, c: u8, color: u32, escala: u32) {
        if escala <= 1 {
            self.glifo(x, y, c, color);
            return;
        }
        let idx = match indice_glifo(c) {
            Some(i) => i,
            None => return,
        };
        let g = &FONT16[idx];
        for (fila, &bits) in g.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            for col in 0..8u32 {
                if bits & (0x80 >> col) != 0 {
                    self.rect(x + col * escala, y + fila as u32 * escala, escala, escala, color);
                }
            }
        }
    }

    /// Un `&str` ampliado. Devuelve la x donde acabo.
    pub fn texto_escala(&self, x: u32, y: u32, s: &str, color: u32, escala: u32) -> u32 {
        let mut cx = x;
        for &c in s.as_bytes() {
            self.glifo_escala(cx, y, c, color, escala);
            cx += GLIFO_ANCHO * escala;
        }
        cx
    }

    /// Lo que ocupa un texto ampliado, para poder centrarlo sin adivinar.
    pub fn ancho_escala(s: &str, escala: u32) -> u32 {
        s.len() as u32 * GLIFO_ANCHO * escala
    }

    /// Una tira de bytes Latin-1. Devuelve la x donde acabo, para encadenar.
    pub fn texto_bytes(&self, x: u32, y: u32, s: &[u8], color: u32) -> u32 {
        let mut cx = x;
        for &c in s {
            self.glifo(cx, y, c, color);
            cx += GLIFO_ANCHO;
        }
        cx
    }

    /// Un `&str`. Los bytes que no sean ASCII se saltan en vez de salir como
    /// basura: un literal con acentos viene en UTF-8 y esta fuente es Latin-1.
    pub fn texto(&self, x: u32, y: u32, s: &str, color: u32) -> u32 {
        self.texto_bytes(x, y, s.as_bytes(), color)
    }
}
