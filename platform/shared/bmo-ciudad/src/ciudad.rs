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
        let horizonte = alto * 82 / 100;
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
            let h = az.entre(alto / 6, alto * 45 / 100).max(12);
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
            let c = mezcla(CIELO_ALTO, CIELO_BAJO, i as u32, FRANJAS as u32 - 1);
            rect(0, i * alto_franja, self.ancho, alto_franja, c);
        }
        let resto = self.horizonte - FRANJAS * alto_franja;
        if resto > 0 {
            rect(0, FRANJAS * alto_franja, self.ancho, resto, CIELO_BAJO);
        }
        rect(0, self.horizonte, self.ancho, self.alto - self.horizonte, CIELO_ALTO);

        // -- LAS TORRES, capa a capa y cada una a su velocidad.
        for capa in 0..2u8 {
            let dx = cam.desplazamiento(capa);
            for t in self.torres[..self.n].iter().filter(|t| t.capa == capa) {
                t.dibujar(self.horizonte, dx, &mut rect);
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

    #[test]
    fn encender_cambia_las_ventanas() {
        let contar = |pct: u32| {
            let mut c = Ciudad::nueva(800, 600, 3);
            c.encender(pct);
            let mut n = 0;
            c.dibujar(Camara::default(), |_, _, _, _, color| {
                if color == VENTANA_CALIDA || color == VENTANA_FRIA {
                    n += 1;
                }
            });
            n
        };
        assert_eq!(contar(0), 0, "al 0% no puede haber ni una encendida");
        assert!(contar(100) > 50, "al 100% tiene que haber muchas");
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
        let mut mas_a_la_derecha = 0;
        c.dibujar(Camara::nueva(200), |x, _, w, _, color| {
            if color == TORRE_FRENTE {
                mas_a_la_derecha = mas_a_la_derecha.max(x + w);
            }
        });
        assert!(
            mas_a_la_derecha >= 640,
            "tras avanzar 200 px la ciudad acaba en {} y la pantalla mide 640",
            mas_a_la_derecha
        );
    }

    #[test]
    fn un_lienzo_diminuto_no_rompe_nada() {
        let c = Ciudad::nueva(32, 24, 5);
        let mut n = 0;
        c.dibujar(Camara::default(), |_, _, _, _, _| n += 1);
        assert!(n > 0);
    }
}
