//! **LA CIUDAD** -- como se reparten las torres y como se pinta el cielo.
//!
//! Lo que una torre ES vive en `torre.rs`. Aqui se decide cuantas hay, donde, y
//! en que orden se pintan.

use crate::azar::Azar;
use crate::camara::Camara;
use crate::paleta::*;
use crate::torre::Torre;

/// Cuantas torres como mucho.
///
/// El tope existe porque este crate **no reserva memoria** y no puede: corre en
/// un kernel `no_std` sin `alloc`.
///
/// [!] Estaba en 40 y **la prueba del borde pelado lo tumbo**: con la camara
/// avanzada 200 px la ciudad se acababa a media pantalla, porque el tope se
/// alcanzaba antes que el margen y las ultimas torres no llegaban a generarse.
/// El limite que manda no era el ancho del mundo: era este numero.
///
/// 96 cubre un 1080p **mas medio ancho de margen** para que la camara tenga por
/// donde avanzar. Cuesta unos 2 KiB de pila, contra los 64 KiB del kernel.
pub const MAX_TORRES: usize = 96;

/// Cuantas de esas van al fondo. Un tercio: son mas anchas, asi que con menos
/// cubren lo mismo, y dejar dos tercios al frente es lo que hace que la fila de
/// delante no tenga huecos.
const TORRES_DE_FONDO: usize = MAX_TORRES / 3;

/// Franjas del degradado del cielo.
///
/// Un degradado de verdad pediria un color por fila. Por franjas son dieciseis
/// rectangulos y **el escalonado se lee como pixel art**, que es lo que se
/// quiere aqui. Misma decision que el degradado del escritorio.
const FRANJAS: i32 = 16;

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
    /// `semilla` fija el reparto: la misma semilla da la misma ciudad, siempre.
    ///
    /// # Las dos capas, y por que se distinguen por la FORMA
    ///
    /// El fondo son torres **anchas, bajas y sin ventanas**; el frente,
    /// estrechas y altas. Esa diferencia --y no el color-- es lo que da
    /// profundidad: el ojo lee "lejos" en lo que tiene menos detalle, no en lo
    /// que esta mas oscuro.
    ///
    /// [!] Y se generan **mas anchas que el lienzo**, empezando en negativo y
    /// pasandose por la derecha: la camara avanza, asi que lo que hoy esta fuera
    /// entra en pantalla dentro de un segundo.
    pub fn nueva(ancho: i32, alto: i32, semilla: u64) -> Self {
        let mut az = Azar::nuevo(semilla);
        // ** EL HORIZONTE BAJA, y es la tercera cosa que dijo la foto del Ryzen.
        //
        // Estaba en el 82% del alto, y con las torres llegando al 45% la ciudad
        // subia hasta media pantalla: el gato --que se centra en el bloque
        // entero-- caia DENTRO de las torres en vez de delante de ellas. Un
        // primer plano sin aire alrededor no es un primer plano.
        //
        // Al 92% la ciudad se queda en la franja de abajo y le deja al logo el
        // tercio central despejado, que es como esta compuesta la referencia que
        // enseno el dueno.
        let horizonte = alto * 92 / 100;
        let mut torres = [Torre::APAGADA; MAX_TORRES];
        let mut n = 0;

        // Cuanto mundo se genera de mas, para que la camara tenga por donde
        // avanzar sin quedarse sin ciudad.
        let margen = ancho / 2;

        // -- Capa de fondo: siluetas anchas.
        let mut x = -20;
        while x < ancho + margen && n < TORRES_DE_FONDO {
            let w = az.entre(ancho / 22, ancho / 11).max(6);
            let h = az.entre(alto / 9, alto / 4).max(8);
            torres[n] = Torre { x, ancho: w, alto: h, capa: 0, ..Torre::APAGADA };
            n += 1;
            // Se solapan a proposito: una fila de torres separadas parece una
            // valla, no una ciudad.
            x += w - az.entre(2, w / 3 + 2);
        }

        // -- Capa delantera: estrechas, altas, con ventanas.
        let mut x = -12;
        while x < ancho + margen && n < MAX_TORRES {
            let w = az.entre(ancho / 40, ancho / 18).max(5);
            // Y mas bajas: con el horizonte al 92% y torres de hasta el 45% del
            // alto, la ciudad seguiria comiendose el centro. Hasta el 32%.
            let h = az.entre(alto / 8, alto * 32 / 100).max(12);
            let tinte = match az.entre(0, 5) {
                0 => NEON_MAGENTA,
                1 => NEON_CIAN,
                _ => VENTANA_MAGENTA,
            };
            torres[n] = Torre { x, ancho: w, alto: h, capa: 1, encendido: false, tinte };
            n += 1;
            x += w + az.entre(1, 6);
        }

        Ciudad { ancho, alto, horizonte, torres, n }
    }

    pub fn cuantas(&self) -> usize {
        self.n
    }

    pub fn torres(&self) -> &[Torre] {
        &self.torres[..self.n]
    }

    /// **El techo de la ciudad**: la `y` de la punta de torre mas alta.
    ///
    /// Todo lo que quede por encima de esto es cielo despejado. Existe para que
    /// **quien pone el logo no tenga que adivinarlo**, que es lo que estaba
    /// pasando: el bloque del gato se centraba en la pantalla entera y su mitad
    /// de abajo --el titulo, sobre todo-- caia dentro de las torres. En el video
    /// del 08-15 se ve el `BMO-X` escrito justo encima de los tejados y el kanji
    /// pisando una torre.
    ///
    /// Se mide, no se calcula de la formula: si manana alguien sube las torres,
    /// el logo se aparta solo. Un numero copiado a mano en el otro lado es
    /// exactamente la clase de acuerdo que se rompe sin que nadie se entere.
    pub fn techo(&self) -> i32 {
        self.torres[..self.n]
            .iter()
            .map(|t| self.horizonte - t.alto)
            .min()
            .unwrap_or(self.horizonte)
    }

    /// **De que color esta el cielo en la fila `y`.**
    ///
    /// La usa `dibujar` para pintarlo, y la usa el aura del logo
    /// ([`crate::halo`]) para saber sobre que esta encendiendose. Es una sola
    /// funcion y no dos copias a proposito: un aura calculada con otra formula
    /// se nota como un rectangulo de otro tono en cuanto alguien toque el
    /// degradado.
    ///
    /// Debajo del horizonte contesta el suelo.
    pub fn color_cielo(&self, y: i32) -> Color {
        if y >= self.horizonte {
            return CIELO_ALTO;
        }
        let alto_franja = (self.horizonte / FRANJAS).max(1);
        let i = (y.max(0) / alto_franja).min(FRANJAS - 1);
        mezcla(CIELO_ALTO, CIELO_BAJO, i as u32, FRANJAS as u32 - 1)
    }

    /// **Enciende la ciudad al `pct` por ciento.**
    ///
    /// El enganche con el sistema: quien llama pasa lo que quiera que
    /// signifique --nucleos en pie, subsistemas arrancados, tareas vivas-- y las
    /// torres se encienden **de izquierda a derecha**.
    ///
    /// De izquierda a derecha y no al azar porque asi **se lee como una barra de
    /// progreso**: la ciudad a medias dice "voy por la mitad" sin poner un
    /// numero.
    pub fn encender(&mut self, pct: u32) {
        let pct = pct.min(100);
        let frente = self.torres[..self.n].iter().filter(|t| t.capa == 1).count();
        let vivas = frente * pct as usize / 100;
        let mut i = 0;
        for t in self.torres[..self.n].iter_mut() {
            if t.capa != 1 {
                continue;
            }
            t.encendido = i < vivas;
            i += 1;
        }
    }

    /// **Dibuja.** Emite rectangulos; no toca ninguna pantalla.
    ///
    /// El orden es el de un pintor: cielo, fondo, frente. Lo de detras se pinta
    /// antes y lo tapa lo de delante -- que es lo unico que se puede hacer sin
    /// buffer de profundidad, y aqui no hace falta uno porque solo hay dos capas
    /// y se sabe cual va delante.
    pub fn dibujar(&self, cam: Camara, mut rect: impl FnMut(i32, i32, i32, i32, Color)) {
        // -- EL CIELO. No se mueve con la camara: esta infinitamente lejos, que
        // es justo lo que significa un cielo.
        let alto_franja = (self.horizonte / FRANJAS).max(1);
        for i in 0..FRANJAS {
            // El color sale de `color_cielo` y no de la formula suelta: el aura
            // del logo pregunta por ahi, y dos formulas para el mismo cielo se
            // separan el dia que alguien toque una.
            let c = self.color_cielo(i * alto_franja);
            rect(0, i * alto_franja, self.ancho, alto_franja, c);
        }
        let resto = self.horizonte - FRANJAS * alto_franja;
        if resto > 0 {
            rect(0, FRANJAS * alto_franja, self.ancho, resto, CIELO_BAJO);
        }
        rect(0, self.horizonte, self.ancho, self.alto - self.horizonte, CIELO_ALTO);

        // -- LAS TORRES, capa a capa y cada una a su velocidad.
        //
        // ** Y LA NIEBLA VA EN MEDIO, no encima. Una niebla pintada al final
        // tine la escena entera y no separa nada; metida ENTRE las dos capas es
        // aire de verdad -- lo de detras queda al otro lado de ella.
        for capa in 0..2u8 {
            if capa == 1 {
                crate::niebla::bandas(self.ancho, self.horizonte, cam.avance, &mut rect);
            }
            let dx = cam.desplazamiento(capa);
            for t in self.torres[..self.n].iter().filter(|t| t.capa == capa) {
                // La bruma se aplica por FILA, dentro de la torre: su base esta
                // mas lejos que su punta, y velar la torre entera por igual la
                // dejaria plana otra vez.
                t.dibujar(self.horizonte, dx, &mut |x, y, w, h, c| {
                    rect(x, y, w, h, crate::niebla::velo(c, y, self.horizonte, capa));
                });
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_misma_semilla_da_la_misma_ciudad() {
        let a = Ciudad::nueva(1920, 1080, 42);
        let b = Ciudad::nueva(1920, 1080, 42);
        assert_eq!(a.cuantas(), b.cuantas());
        for (t1, t2) in a.torres().iter().zip(b.torres()) {
            assert_eq!((t1.x, t1.ancho, t1.alto), (t2.x, t2.ancho, t2.alto));
        }
    }

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

    /// El cielo cubre el lienzo entero: ni una fila sin pintar. Un hueco se ve
    /// como una banda negra cruzando la pantalla.
    #[test]
    fn el_cielo_y_el_suelo_cubren_todo_el_alto() {
        let c = Ciudad::nueva(640, 480, 7);
        let mut filas = [false; 480];
        c.dibujar(Camara::default(), |_, y, _, h, _| {
            for f in y..(y + h) {
                if (0..480).contains(&f) {
                    filas[f as usize] = true;
                }
            }
        });
        assert!(filas.iter().all(|&f| f), "quedaron filas sin pintar");
    }

    /// [!] Esta prueba comparaba colores EXACTOS (`== VENTANA_CALIDA`) y la
    /// niebla la tumbo: ahora cada color sale velado segun su altura, asi que
    /// ninguno es exactamente el de la paleta. Comparar por igualdad era pedirle
    /// a la escena que no tuviera atmosfera.
    ///
    /// Lo que la prueba queria decir de verdad es **"hay puntos brillantes"**, y
    /// eso se pregunta por luminancia -- que ademas es lo que el ojo hace.
    #[test]
    fn encender_cambia_las_ventanas() {
        let umbral = luminancia(CIELO_BAJO) + 40;
        let contar = |pct: u32| {
            let mut c = Ciudad::nueva(800, 600, 3);
            c.encender(pct);
            let mut n = 0;
            c.dibujar(Camara::default(), |_, _, _, _, color| {
                if luminancia(color) > umbral {
                    n += 1;
                }
            });
            n
        };
        assert_eq!(contar(0), 0, "al 0% no puede haber ni un punto brillante");
        assert!(contar(100) > 50, "al 100% tiene que haber muchos");
    }

    #[test]
    fn se_enciende_de_izquierda_a_derecha() {
        let mut c = Ciudad::nueva(800, 600, 9);
        c.encender(50);
        let mut vista_apagada = false;
        for t in c.torres().iter().filter(|t| t.capa == 1) {
            if !t.encendido {
                vista_apagada = true;
            } else {
                assert!(!vista_apagada, "una encendida DESPUES de una apagada");
            }
        }
    }

    /// ** Con la camara avanzada la ciudad SIGUE cubriendo la pantalla. Es la
    /// prueba de que se genera mundo de sobra: sin el margen, avanzar deja el
    /// borde derecho pelado y se ve el cielo hasta el suelo.
    #[test]
    fn avanzar_no_deja_el_borde_pelado() {
        let mut c = Ciudad::nueva(640, 480, 11);
        c.encender(100);
        // Un CUERPO de torre, y se reconoce por la forma y no por el color: con
        // la niebla puesta ningun color es ya exactamente el de la paleta. Ancho
        // menor que la pantalla descarta el cielo y el suelo (que van de borde a
        // borde), y alto grande descarta ventanas y jirones de niebla.
        let mut mas_a_la_derecha = 0;
        c.dibujar(Camara::nueva(200), |x, _, w, h, _| {
            if w < 640 && h > 30 {
                mas_a_la_derecha = mas_a_la_derecha.max(x + w);
            }
        });
        assert!(
            mas_a_la_derecha >= 640,
            "tras avanzar 200 px la ciudad acaba en {} y la pantalla mide 640",
            mas_a_la_derecha
        );
    }

    /// ** EL TECHO DEJA SITIO AL LOGO, que es el fallo que se ve en el video del
    /// 2026-08-15: el `BMO-X` escrito sobre los tejados y el kanji pisando una
    /// torre.
    ///
    /// El numero que importa no es el techo en si: es **cuanto cielo despejado
    /// queda**. Si esto se quedara corto, el logo no cabria por encima de la
    /// ciudad y volveria a meterse dentro.
    #[test]
    fn por_encima_del_techo_queda_medio_lienzo_despejado() {
        let c = Ciudad::nueva(1920, 1080, 42);
        let techo = c.techo();
        assert!(techo > 0, "las torres se salen por arriba de la pantalla");
        assert!(techo < c.horizonte, "el techo tiene que estar sobre el suelo");
        assert!(
            techo >= c.alto / 2,
            "solo quedan {} px de cielo despejado y el logo no cabe",
            techo
        );
    }

    /// El techo se MIDE de las torres que hay. Si alguien las sube, el logo se
    /// aparta solo -- que es la razon de que esto no sea un porcentaje copiado.
    #[test]
    fn el_techo_es_la_punta_mas_alta_de_verdad() {
        let c = Ciudad::nueva(1280, 720, 5);
        let mas_alta = c.torres().iter().map(|t| t.alto).max().unwrap();
        assert_eq!(c.techo(), c.horizonte - mas_alta);
    }

    /// El cielo que dibuja `dibujar` y el que contesta `color_cielo` son EL
    /// MISMO. Si divergieran, el aura del logo saldria como un ovalo de otro
    /// tono pegado sobre el degradado.
    #[test]
    fn el_cielo_que_se_pinta_es_el_que_se_contesta() {
        let c = Ciudad::nueva(800, 600, 13);
        let mut visto = 0;
        c.dibujar(Camara::default(), |x, y, w, _, color| {
            // Solo las franjas del cielo: de borde a borde y por encima del
            // horizonte. Lo demas son torres, niebla y suelo.
            if x == 0 && w == c.ancho && y < c.horizonte {
                assert_eq!(color, c.color_cielo(y), "la franja de y={} no cuadra", y);
                visto += 1;
            }
        });
        assert!(visto > 0, "no se miro ni una franja de cielo");
    }

    /// Debajo del horizonte contesta el suelo, y no se sale del array de
    /// franjas. Lo pregunta el aura cuando el logo queda bajo en una pantalla
    /// pequena.
    #[test]
    fn preguntar_fuera_del_cielo_no_desborda() {
        let c = Ciudad::nueva(640, 480, 3);
        let _ = c.color_cielo(-100);
        let _ = c.color_cielo(c.alto * 10);
        assert_eq!(c.color_cielo(c.horizonte + 1), CIELO_ALTO);
    }

    #[test]
    fn un_lienzo_diminuto_no_rompe_nada() {
        let c = Ciudad::nueva(32, 24, 5);
        let mut n = 0;
        c.dibujar(Camara::default(), |_, _, _, _, _| n += 1);
        assert!(n > 0);
    }
}
