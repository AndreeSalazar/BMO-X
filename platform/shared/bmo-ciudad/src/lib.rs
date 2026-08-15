//! # LA CIUDAD -- el fondo del arranque, dibujado por la CPU
//!
//! ## Que es
//!
//! Una skyline de neon en pixel art: torres de fondo, torres delanteras,
//! ventanas encendidas y algun letrero. Va **detras del gato** en la pantalla de
//! arranque, y es lo que el dueno pidio despues de ensenar dos capturas de
//! Geoxor: *"cuando se prenda mi maquina el gato se ve neon y la CPU dibuja
//! todo en pixeles"*.
//!
//! ## Por que se DIBUJA y no se guarda
//!
//! Es la decision entera de este crate, asi que va primero.
//!
//! Una pantalla de 1920x1080 en pixel art guardada como bitmap son megabytes, no
//! cabe en el kernel, y solo sirve para **esa** resolucion: en un panel de otro
//! tamano hay que estirarla, y estirar pixel art lo destruye -- deja de tener
//! pixeles cuadrados, que es justamente lo que lo define.
//!
//! Generada, la ciudad son **cero bytes de datos y unos cientos de lineas**, se
//! compone a la medida exacta del panel que haya, y --lo que de verdad importa--
//! **puede decir cosas**. Ver la seccion siguiente.
//!
//! ## ** LA CIUDAD ES EL ESTADO DEL SISTEMA, y esto no es adorno
//!
//! La idea es del dueno: *"en el fondo se ve el sistema de ciudad con TODO"*.
//!
//! Cada torre es un SUBSISTEMA y sus ventanas encendidas son un numero de
//! verdad: cuantos nucleos estan en pie, cuantas tareas vivas, cuanta RAM. Una
//! maquina a medio arrancar tiene la ciudad a oscuras y se va encendiendo sola
//! segun entra cada pieza. Un subsistema que no arranco **deja su torre negra**,
//! y eso se ve desde el otro lado de la habitacion sin leer una linea de log.
//!
//! Es CABINA dibujada como skyline. La informacion ya existe; lo que faltaba era
//! una forma de mirarla que no fuera leer.
//!
//! ## Y los dientes aqui NO son un defecto
//!
//! El escalon 2.5 del rasterizador (`triangulo_suave`) existe para quitar los
//! bordes escalonados. Aqui **no se usa a proposito**: en pixel art el borde
//! duro ES el dibujo, y suavizarlo lo emborrona. Dicho por el dueno con todas
//! las letras: *"asi con dientes no me importa"*.
//!
//! La cobertura llegara a esta pantalla el dia que el logo sea vectorial y haya
//! GPU, que es cuando tiene sentido. Hoy no.
//!
//! ## Como se dibuja: rectangulos por callback
//!
//! Este crate **no toca ninguna pantalla**. Emite `rect(x, y, w, h, color)` y
//! quien llama decide donde cae eso -- el framebuffer del kernel, el compositor,
//! o un vector en una prueba.
//!
//! Es el mismo patron que `dibujo::triangulo`, y por el mismo motivo: sin
//! destino, esto es aritmetica pura, y la aritmetica **se puede probar en el
//! anfitrion**. Un fondo de arranque que solo se puede juzgar arrancando la
//! maquina es un fondo que nadie va a tocar nunca.
//!
//! [!] Y por eso el crate vive en `platform/shared/` y no dentro del kernel: lo
//! pidio el dueno --*"un crate independiente fuera del Ring 0 y Ring 3"*-- y
//! tiene el precedente exacto de `bmo-hash`, *"el unico BLAKE3 del sistema"*,
//! que comparten las dos orillas porque ejecutan el MISMO codigo.

#![cfg_attr(not(test), no_std)]

/// Un color BGRA de 32 bits, como los quiere el framebuffer.
pub type Color = u32;

// -- La paleta ---------------------------------------------------------------
//
// Sale de las dos capturas que enseno el dueno: fondo casi negro con tinte
// violeta, torres en morados frios, y el neon repartido en cian, magenta y
// ambar. **Pocos tonos y muy separados**, que es lo que hace que el pixel art se
// lea: una paleta de treinta grises no es pixel art, es una foto pequena.

/// El cielo, arriba del todo. Casi negro con violeta.
pub const CIELO_ALTO: Color = 0xFF0B0714;
/// El cielo cerca del horizonte: el resplandor de la ciudad tinendo la niebla.
pub const CIELO_BAJO: Color = 0xFF2A1140;
/// Torres del fondo: apenas siluetas.
pub const TORRE_FONDO: Color = 0xFF191033;
/// Torres delanteras.
pub const TORRE_FRENTE: Color = 0xFF241847;
/// El borde iluminado de una torre delantera, del lado del neon.
pub const TORRE_BORDE: Color = 0xFF3B2A6B;

/// Ventana encendida, la mas comun.
pub const VENTANA_CALIDA: Color = 0xFFFFC96B;
/// Ventana encendida en frio.
pub const VENTANA_FRIA: Color = 0xFF7DE3FF;
/// Ventana encendida en magenta.
pub const VENTANA_MAGENTA: Color = 0xFFFF6BD6;
/// Ventana apagada: no es negra, es la torre un poco mas oscura. Una ventana
/// negra del todo desaparece y la fachada se queda lisa.
pub const VENTANA_APAGADA: Color = 0xFF120B26;

/// El cian de la marca. Es el mismo que el de los ojos del gato.
pub const NEON_CIAN: Color = 0xFF00E5FF;
/// El magenta de los letreros.
pub const NEON_MAGENTA: Color = 0xFFFF3DAE;

/// Cuantas torres como mucho. El tope existe para que la ciudad quepa en la
/// pila: este crate **no reserva memoria** y no puede -- corre en un kernel
/// `no_std` sin `alloc`.
pub const MAX_TORRES: usize = 40;

/// Una torre: donde empieza, cuanto mide, y de que capa es.
#[derive(Clone, Copy)]
pub struct Torre {
    pub x: i32,
    pub ancho: i32,
    pub alto: i32,
    /// `0` = fondo (silueta), `1` = frente (con ventanas).
    pub capa: u8,
    /// Cuantas de sus ventanas estan encendidas, de 0 a 255. **Es el dato del
    /// sistema**: quien construye la ciudad decide que significa.
    pub encendido: u8,
    /// El tono del neon de esta torre.
    pub tinte: Color,
}

/// El generador de numeros: **xorshift de 64 bits**.
///
/// Hace falta uno porque una skyline con todas las torres iguales no es una
/// skyline, y no puede ser el del sistema: tiene que dar **la misma ciudad en
/// cada arranque**. Un fondo que cambia solo cada vez que enciendes es un fondo
/// que no puedes usar para notar que algo cambio.
///
/// Xorshift y no algo mejor porque aqui no se protege nada: son tres
/// desplazamientos, cabe en una linea, y sus numeros no se parecen entre si, que
/// es todo lo que hace falta para repartir alturas.
pub struct Azar(u64);

impl Azar {
    pub fn nuevo(semilla: u64) -> Self {
        // El cero es punto fijo de xorshift: se quedaria en cero para siempre y
        // la ciudad saldria con todas las torres identicas. Se desvia.
        Azar(if semilla == 0 { 0x9E37_79B9_7F4A_7C15 } else { semilla })
    }

    pub fn siguiente(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Un numero en `[desde, hasta]`, los dos incluidos.
    pub fn entre(&mut self, desde: i32, hasta: i32) -> i32 {
        if hasta <= desde {
            return desde;
        }
        let rango = (hasta - desde + 1) as u64;
        desde + (self.siguiente() % rango) as i32
    }
}

/// La ciudad entera: unas cuantas torres y el tamano del lienzo.
pub struct Ciudad {
    pub ancho: i32,
    pub alto: i32,
    /// Donde empieza el suelo. Las torres crecen hacia arriba desde aqui.
    pub horizonte: i32,
    torres: [Torre; MAX_TORRES],
    n: usize,
}

impl Ciudad {
    /// Compone una ciudad para un lienzo de `ancho x alto`.
    ///
    /// `semilla` fija el reparto de torres: la misma semilla da la misma ciudad,
    /// siempre.
    ///
    /// # Las dos capas, y por que en este orden
    ///
    /// El fondo se genera con torres **mas anchas, mas bajas y sin ventanas**, y
    /// el frente con torres estrechas y altas. Esa diferencia --y no el color--
    /// es lo que da profundidad: el ojo lee "lejos" en lo que tiene menos
    /// detalle, no en lo que esta mas oscuro.
    pub fn nueva(ancho: i32, alto: i32, semilla: u64) -> Self {
        let mut az = Azar::nuevo(semilla);
        let horizonte = alto * 82 / 100;
        let mut torres = [Torre {
            x: 0,
            ancho: 0,
            alto: 0,
            capa: 0,
            encendido: 0,
            tinte: NEON_CIAN,
        }; MAX_TORRES];
        let mut n = 0;

        // -- Capa de fondo: siluetas anchas, sin ventanas.
        let mut x = -20;
        while x < ancho && n < MAX_TORRES / 2 {
            let w = az.entre(ancho / 22, ancho / 11).max(6);
            let h = az.entre(alto / 9, alto / 4).max(8);
            torres[n] = Torre { x, ancho: w, alto: h, capa: 0, encendido: 0, tinte: TORRE_FONDO };
            n += 1;
            // Se solapan a proposito: una fila de torres separadas parece una
            // valla, no una ciudad.
            x += w - az.entre(2, w / 3 + 2);
        }

        // -- Capa delantera: estrechas, altas, con ventanas.
        let mut x = -12;
        while x < ancho && n < MAX_TORRES {
            let w = az.entre(ancho / 40, ancho / 18).max(5);
            let h = az.entre(alto / 6, alto * 45 / 100).max(12);
            let tinte = match az.entre(0, 5) {
                0 => NEON_MAGENTA,
                1 => NEON_CIAN,
                _ => VENTANA_CALIDA,
            };
            torres[n] = Torre { x, ancho: w, alto: h, capa: 1, encendido: 0, tinte };
            n += 1;
            x += w + az.entre(1, 6);
        }

        Ciudad { ancho, alto, horizonte, torres, n }
    }

    /// Cuantas torres tiene.
    pub fn cuantas(&self) -> usize {
        self.n
    }

    /// Las torres, para poder encenderlas desde fuera.
    pub fn torres(&self) -> &[Torre] {
        &self.torres[..self.n]
    }

    /// **Enciende la ciudad al `pct` por ciento.**
    ///
    /// Este es el enganche con el sistema: quien llama pasa lo que quiera que
    /// signifique --nucleos en pie, subsistemas arrancados, tareas vivas-- y las
    /// torres se van encendiendo de izquierda a derecha.
    ///
    /// De izquierda a derecha y no al azar **porque se tiene que poder leer como
    /// una barra de progreso**: la ciudad encendida a medias dice "voy por la
    /// mitad" sin poner un numero.
    pub fn encender(&mut self, pct: u32) {
        let pct = pct.min(100);
        let frente: usize = self.torres[..self.n].iter().filter(|t| t.capa == 1).count();
        let vivas = frente * pct as usize / 100;
        let mut i = 0;
        for t in self.torres[..self.n].iter_mut() {
            if t.capa != 1 {
                continue;
            }
            t.encendido = if i < vivas { 255 } else { 0 };
            i += 1;
        }
    }

    /// **Dibuja.** Emite rectangulos; no toca ninguna pantalla.
    ///
    /// El orden es el de un pintor: cielo, torres del fondo, torres del frente y
    /// sus ventanas. Lo de detras se pinta antes y lo tapa lo de delante, que es
    /// lo unico que se puede hacer sin buffer de profundidad -- y aqui no hace
    /// falta uno, porque la escena tiene dos capas y se sabe cual va delante.
    pub fn dibujar(&self, mut rect: impl FnMut(i32, i32, i32, i32, Color)) {
        // -- EL CIELO, por franjas.
        //
        // Un degradado de verdad pediria un color por fila; por franjas son
        // dieciseis rectangulos y **el escalonado se lee como pixel art**, que
        // es lo que se quiere. La misma decision que ya se tomo en el degradado
        // del escritorio.
        const FRANJAS: i32 = 16;
        let alto_franja = (self.horizonte / FRANJAS).max(1);
        for i in 0..FRANJAS {
            let y = i * alto_franja;
            let c = mezcla(CIELO_ALTO, CIELO_BAJO, i as u32, FRANJAS as u32 - 1);
            rect(0, y, self.ancho, alto_franja, c);
        }
        // Lo que quede entre la ultima franja y el horizonte.
        let resto = self.horizonte - FRANJAS * alto_franja;
        if resto > 0 {
            rect(0, FRANJAS * alto_franja, self.ancho, resto, CIELO_BAJO);
        }
        // El suelo.
        rect(0, self.horizonte, self.ancho, self.alto - self.horizonte, CIELO_ALTO);

        // -- LAS TORRES.
        for capa in 0..2u8 {
            for t in self.torres[..self.n].iter().filter(|t| t.capa == capa) {
                let y = self.horizonte - t.alto;
                let color = if capa == 0 { TORRE_FONDO } else { TORRE_FRENTE };
                rect(t.x, y, t.ancho, t.alto, color);
                if capa == 0 {
                    continue;
                }
                // El canto iluminado: una columna de un pixel en el lado
                // izquierdo. Cuesta un rectangulo y **separa dos torres
                // pegadas**, que sin el se leen como una sola mas ancha.
                rect(t.x, y, 1, t.alto, TORRE_BORDE);
                self.ventanas(t, y, &mut rect);
            }
        }
    }

    /// La rejilla de ventanas de una torre.
    fn ventanas(&self, t: &Torre, y0: i32, rect: &mut impl FnMut(i32, i32, i32, i32, Color)) {
        const PASO: i32 = 5;
        const LADO: i32 = 2;
        if t.ancho < PASO * 2 || t.alto < PASO * 2 {
            return;
        }
        // El patron sale de la posicion, no de un generador: asi **la misma
        // torre tiene siempre las mismas ventanas encendidas** aunque se
        // redibuje, y no parpadea entre fotogramas.
        let mut fy = y0 + PASO;
        while fy + LADO < y0 + t.alto - 2 {
            let mut fx = t.x + PASO;
            while fx + LADO < t.x + t.ancho - 1 {
                let h = mezclador(fx as u64, fy as u64);
                let color = if t.encendido == 0 {
                    VENTANA_APAGADA
                } else if h % 5 == 0 {
                    VENTANA_APAGADA
                } else if h % 7 == 0 {
                    t.tinte
                } else if h % 3 == 0 {
                    VENTANA_FRIA
                } else {
                    VENTANA_CALIDA
                };
                rect(fx, fy, LADO, LADO, color);
                fx += PASO;
            }
            fy += PASO;
        }
    }
}

/// Mezcla dos colores por canal. Entera, sin coma flotante.
pub fn mezcla(a: Color, b: Color, parte: u32, total: u32) -> Color {
    if total == 0 {
        return a;
    }
    let parte = parte.min(total);
    let inv = total - parte;
    let canal = |desp: u32| {
        let ca = (a >> desp) & 0xFF;
        let cb = (b >> desp) & 0xFF;
        ((ca * inv + cb * parte) / total) & 0xFF
    };
    0xFF00_0000 | (canal(16) << 16) | (canal(8) << 8) | canal(0)
}

/// Un revoltijo determinista de dos coordenadas. Da el patron de ventanas.
fn mezclador(x: u64, y: u64) -> u64 {
    let mut h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ y.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    h
}

// -- Las pruebas -------------------------------------------------------------
//
// Corren con `cargo test -p bmo-ciudad` **de verdad**, y esa es media razon de
// que este crate exista aparte: no tiene guion de enlazado propio, asi que no
// choca con lo que impide probar `Ultra_userspace` y el kernel.
#[cfg(test)]
mod pruebas {
    use super::*;

    /// La misma semilla da la misma ciudad. Es la propiedad que permite usar el
    /// fondo para notar que algo cambio.
    #[test]
    fn la_misma_semilla_da_la_misma_ciudad() {
        let a = Ciudad::nueva(1920, 1080, 42);
        let b = Ciudad::nueva(1920, 1080, 42);
        assert_eq!(a.cuantas(), b.cuantas());
        for (t1, t2) in a.torres().iter().zip(b.torres()) {
            assert_eq!((t1.x, t1.ancho, t1.alto), (t2.x, t2.ancho, t2.alto));
        }
    }

    /// Y dos semillas distintas dan ciudades distintas -- si no, el azar no
    /// esta haciendo nada y todas las maquinas tendrian el mismo fondo.
    #[test]
    fn dos_semillas_dan_ciudades_distintas() {
        let a = Ciudad::nueva(1920, 1080, 1);
        let b = Ciudad::nueva(1920, 1080, 2);
        let iguales = a
            .torres()
            .iter()
            .zip(b.torres())
            .filter(|(t1, t2)| t1.alto == t2.alto && t1.ancho == t2.ancho)
            .count();
        assert!(iguales < a.cuantas(), "las dos ciudades salieron identicas");
    }

    /// El cielo cubre el lienzo entero: ni una fila sin pintar. Un hueco aqui
    /// se ve como una banda negra cruzando la pantalla.
    #[test]
    fn el_cielo_y_el_suelo_cubren_todo_el_alto() {
        let c = Ciudad::nueva(640, 480, 7);
        let mut filas = [false; 480];
        c.dibujar(|_, y, _, h, _| {
            for f in y..(y + h) {
                if (0..480).contains(&f) {
                    filas[f as usize] = true;
                }
            }
        });
        assert!(filas.iter().all(|&f| f), "quedaron filas sin pintar");
    }

    /// Encender al 0% y al 100% tiene que dar numeros distintos de ventanas
    /// encendidas. Es la prueba de que el enganche con el sistema hace algo.
    #[test]
    fn encender_cambia_las_ventanas() {
        let contar = |pct: u32| {
            let mut c = Ciudad::nueva(800, 600, 3);
            c.encender(pct);
            let mut n = 0;
            c.dibujar(|_, _, _, _, color| {
                if color == VENTANA_CALIDA || color == VENTANA_FRIA {
                    n += 1;
                }
            });
            n
        };
        let apagada = contar(0);
        let encendida = contar(100);
        assert_eq!(apagada, 0, "al 0% no puede haber ni una ventana encendida");
        assert!(encendida > 50, "al 100% tiene que haber muchas, hubo {}", encendida);
    }

    /// Encender a la mitad enciende **por la izquierda**, para que se lea como
    /// una barra de progreso.
    #[test]
    fn se_enciende_de_izquierda_a_derecha() {
        let mut c = Ciudad::nueva(800, 600, 9);
        c.encender(50);
        // Una sola pasada de izquierda a derecha: en cuanto aparece una
        // apagada, ninguna de las siguientes puede estar encendida.
        let mut vista_apagada = false;
        for t in c.torres().iter().filter(|t| t.capa == 1) {
            if t.encendido == 0 {
                vista_apagada = true;
            } else {
                assert!(
                    !vista_apagada,
                    "hay una encendida DESPUES de una apagada: no se lee como progreso"
                );
            }
        }
    }

    /// Un lienzo minusculo no puede hacer que se salga ni entre en bucle.
    #[test]
    fn un_lienzo_diminuto_no_rompe_nada() {
        let c = Ciudad::nueva(32, 24, 5);
        let mut n = 0;
        c.dibujar(|_, _, _, _, _| n += 1);
        assert!(n > 0);
    }
}
