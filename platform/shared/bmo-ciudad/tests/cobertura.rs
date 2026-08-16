//! # LA PRUEBA QUE FALTABA: ni un pixel del lienzo se queda sin escribir
//!
//! ## Que fallo, y por que las pruebas que habia lo dejaron pasar
//!
//! El video del arranque del 2026-08-15 enseno una franja muerta de 191 px
//! pegada al borde izquierdo: **el 9,9% del ancho de la pantalla** congelado con
//! el ultimo color que le cayo encima, sin repintarse mas aunque el resto de la
//! escena se apagara a negro. Lo que faltaba ahi era el bloque exterior del
//! marco, que se emite en `x = -34` en cuanto la camara deriva y que el kernel
//! descartaba entero por tener la esquina fuera.
//!
//! Y habia pruebas. Tres, y las tres en verde:
//!
//! * `el_marco_nunca_deja_el_borde_al_aire` comprobaba que el crate **emite** un
//!   rectangulo con `x <= 0`. Lo emitia. El consumidor lo tiraba.
//! * `el_cielo_y_el_suelo_cubren_todo_el_alto` comprobaba que ninguna FILA se
//!   queda sin tocar. Ninguna se quedaba: la franja muerta es una columna.
//! * El previsualizador dibujaba la escena entera y salia bien, porque
//!   **ejecutaba otra regla de recorte** que la del kernel.
//!
//! O sea que el fallo cabia entero por el hueco entre "la geometria es
//! correcta" y "los pixeles acaban en la pantalla". Nadie preguntaba lo unico
//! que lo habria cazado: **que quedo sin escribir**.
//!
//! ## Por que es una prueba de integracion y no un `#[cfg(test)]` mas
//!
//! Porque desde aqui **solo se ve la API publica**, que es exactamente lo que
//! se quiere probar. Un test de dentro del crate podria llamar a `emitir` y
//! comprobar geometria; desde fuera la unica forma de sacar pixeles es
//! `dibujar(cam, &mut lienzo)`, que es el camino que recorre el kernel.

use bmo_ciudad::{Camara, Ciudad, Lienzo, Recorte};

/// Un lienzo que no guarda colores: guarda **si se escribio**.
///
/// Es la diferencia entre esta prueba y las otras. Un lienzo que guarda colores
/// contesta "de que color es este pixel"; este contesta "toco alguien este
/// pixel", que es la pregunta cuya respuesta faltaba.
struct Cobertura {
    w: i32,
    h: i32,
    escrito: Vec<bool>,
    /// Rectangulos que llegaron al relleno. Sirve para el contraste con los que
    /// la escena emite: la diferencia son los que el recorte partio.
    pintados: u32,
}

impl Cobertura {
    fn nueva(w: i32, h: i32) -> Self {
        Cobertura { w, h, escrito: vec![false; (w * h) as usize], pintados: 0 }
    }

    /// La columna sin escribir mas ancha pegada a un borde. Es la forma que
    /// tenia el fallo, y darla en pixeles hace que el mensaje de error se pueda
    /// comparar con la foto.
    fn franja_muerta_izquierda(&self) -> i32 {
        let mut n = 0;
        for x in 0..self.w {
            if (0..self.h).any(|y| !self.escrito[(y * self.w + x) as usize]) {
                n += 1;
            } else {
                break;
            }
        }
        n
    }

    fn sin_escribir(&self) -> usize {
        self.escrito.iter().filter(|v| !**v).count()
    }
}

impl Lienzo for Cobertura {
    fn recorte(&self) -> Recorte {
        Recorte::nuevo(0, 0, self.w, self.h)
    }

    fn rect_dentro(&mut self, r: Recorte, _color: bmo_ciudad::Color) {
        self.pintados += 1;
        // El contrato del lienzo dice que esto llega dentro. Si algun dia deja
        // de ser cierto, que reviente aqui y no en el framebuffer del Ryzen.
        assert!(
            r.x0 >= 0 && r.y0 >= 0 && r.x1 <= self.w && r.y1 <= self.h,
            "el lienzo recibio un rectangulo fuera: {:?}",
            r
        );
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                self.escrito[(y * self.w + x) as usize] = true;
            }
        }
    }
}

/// Las resoluciones que la maquina puede dar. La del Ryzen es la primera.
const PANTALLAS: [(i32, i32); 4] = [(1920, 1080), (1600, 900), (1366, 768), (1280, 720)];

/// ** LA PRUEBA. La escena entera cubre el lienzo entero, en todo instante de la
/// intro y en toda resolucion.
///
/// Se recorre la animacion completa y no solo el fotograma final, porque la
/// franja muerta **aparecia con la deriva**: en el milisegundo cero el bloque
/// del marco cae en `x = 0` exacto y se pinta; a partir de los 75 ms la camara
/// lo empuja a negativo y ahi empezaba el agujero. Una prueba del primer
/// fotograma lo habria dejado pasar igual que las otras.
#[test]
fn la_escena_cubre_el_lienzo_entero_en_toda_la_intro() {
    for (w, h) in PANTALLAS {
        let c = Ciudad::nueva(w, h, ((w as u64) << 20) | h as u64);
        // De 40 en 40 ms: sesenta fotogramas repartidos por los cuatro actos,
        // que es de sobra para pillar cualquier avance de camara que abra un
        // hueco. Recorrerlos de uno en uno serian 9.600 lienzos.
        for ms in (0..=bmo_ciudad::DURACION_MS).step_by(40) {
            let f = bmo_ciudad::fotograma(ms);
            let mut lienzo = Cobertura::nueva(w, h);
            c.dibujar(Camara::nueva(f.avance), &mut lienzo);

            let muerta = lienzo.franja_muerta_izquierda();
            assert_eq!(
                muerta, 0,
                "a {}x{} en el ms {} (avance {}) quedo una franja muerta de {} px \
                 pegada al borde izquierdo -- el {:.1}% del ancho",
                w, h, ms, f.avance, muerta,
                muerta as f64 * 100.0 / w as f64
            );
            assert_eq!(
                lienzo.sin_escribir(),
                0,
                "a {}x{} en el ms {} quedaron {} pixeles sin escribir ({:.1}% de la pantalla)",
                w, h, ms,
                lienzo.sin_escribir(),
                lienzo.sin_escribir() as f64 * 100.0 / (w * h) as f64
            );
        }
    }
}

/// ** LA PRUEBA DE QUE LA PRUEBA SIRVE.
///
/// Una prueba que nunca se ha visto fallar no prueba nada: puede estar midiendo
/// otra cosa y no enterarse. Asi que aqui se vuelve a montar **la regla vieja
/// del kernel** --descartar el rectangulo entero si una esquina se sale, en vez
/// de recortarlo-- y se comprueba que con ella el agujero SALE.
///
/// Con la regla vieja, a 1920x1080 y la camara al final de la intro, la medicion
/// sobre el video daba 191 px de franja muerta (el 9,9% del ancho) y 149.376
/// pixeles sin escribir (el 7,2% de la pantalla). Aqui no se clava el numero
/// exacto --depende del ancho del marco y de la deriva, que son de gusto y se
/// pueden tocar-- pero si el orden de magnitud: si alguien vuelve a descartar en
/// vez de recortar, esto lo dice.
#[test]
fn la_regla_vieja_del_kernel_dejaba_el_agujero() {
    /// El lienzo de antes del 2026-08-15, reconstruido tal cual estaba en
    /// `splash.rs`: `if x >= 0 && y >= 0 && x < w && y < h { fill_rect(...) }`.
    struct ComoElKernelViejo {
        w: i32,
        h: i32,
        escrito: Vec<bool>,
    }
    impl Lienzo for ComoElKernelViejo {
        fn recorte(&self) -> Recorte {
            Recorte::nuevo(0, 0, self.w, self.h)
        }
        /// ** Aqui esta el fallo, y esta en la PUERTA y no en el relleno.
        ///
        /// Reimplementar `caja` es exactamente lo que hacia el kernel al tener
        /// su propio `fill_rect` con las comprobaciones a mano. Hoy el trait no
        /// obliga a nadie a no hacerlo --Rust no tiene metodos finales-- pero
        /// hay que escribirlo aposta, que es la diferencia entre un descuido y
        /// una decision.
        fn caja(&mut self, r: &Recorte, color: bmo_ciudad::Color) {
            if r.x0 >= 0 && r.y0 >= 0 && r.x0 < self.w && r.y0 < self.h {
                let dentro = r.interseccion(&self.recorte());
                if !dentro.vacio() {
                    self.rect_dentro(dentro, color);
                }
            }
        }
        fn rect_dentro(&mut self, r: Recorte, _c: bmo_ciudad::Color) {
            for y in r.y0..r.y1 {
                for x in r.x0..r.x1 {
                    self.escrito[(y * self.w + x) as usize] = true;
                }
            }
        }
    }

    let (w, h) = (1920, 1080);
    let c = Ciudad::nueva(w, h, ((w as u64) << 20) | h as u64);
    let f = bmo_ciudad::fotograma(bmo_ciudad::DURACION_MS);
    let mut viejo = ComoElKernelViejo { w, h, escrito: vec![false; (w * h) as usize] };
    c.dibujar(Camara::nueva(f.avance), &mut viejo);

    let mut franja = 0;
    for x in 0..w {
        if (0..h).any(|y| !viejo.escrito[(y * w + x) as usize]) {
            franja += 1;
        } else {
            break;
        }
    }
    let sin_escribir = viejo.escrito.iter().filter(|v| !**v).count();

    assert!(
        franja * 100 / w >= 5,
        "la regla vieja tendria que dejar una franja muerta gorda pegada al borde \
         y solo dejo {} px ({}%). O la geometria del marco cambio, o esta prueba \
         dejo de medir lo que media",
        franja,
        franja * 100 / w
    );
    assert!(
        sin_escribir * 100 / (w * h) as usize >= 3,
        "la regla vieja solo dejo {} pixeles sin escribir",
        sin_escribir
    );
}

/// ** Y LA MITAD QUE LA PRUEBA ANTERIOR NO VE: que el recorte RECORTE, en vez
/// de descartar.
///
/// Cubrir el lienzo entero se puede conseguir mal. Si alguien "arreglara" el
/// sello del marco haciendo que empiece en `x = 0` en vez de en negativo, la
/// cobertura saldria perfecta y la escena estaria rota de otra forma: el marco
/// dejaria de sellar el borde en cuanto la camara derive.
///
/// Asi que aqui se comprueba lo contrario: que la escena **sigue emitiendo
/// geometria que empieza fuera** y que el lienzo la parte en vez de tirarla. Es
/// la prueba de que el arreglo esta donde tiene que estar --en quien pinta-- y
/// no escondido en la geometria.
#[test]
fn la_escena_sigue_emitiendo_desde_fuera_y_el_lienzo_lo_recorta() {
    let (w, h) = (1920, 1080);
    let c = Ciudad::nueva(w, h, 42);
    // Con la camara al final de la intro, que es cuando mas deriva lleva.
    let f = bmo_ciudad::fotograma(bmo_ciudad::DURACION_MS);

    /// Un lienzo que apunta si le llego algo que TOCA el borde izquierdo. Un
    /// rectangulo recortado por la izquierda acaba en `x0 == 0`.
    struct Borde {
        w: i32,
        h: i32,
        pegado_al_borde: bool,
    }
    impl Lienzo for Borde {
        fn recorte(&self) -> Recorte {
            Recorte::nuevo(0, 0, self.w, self.h)
        }
        fn rect_dentro(&mut self, r: Recorte, _c: bmo_ciudad::Color) {
            // Alto completo y pegado al borde: eso es el sello del marco, ya
            // recortado. Si se descartara, esto no llegaria nunca.
            if r.x0 == 0 && r.y0 == 0 && r.y1 == self.h && r.ancho() > 4 {
                self.pegado_al_borde = true;
            }
        }
    }

    let mut l = Borde { w, h, pegado_al_borde: false };
    c.dibujar(Camara::nueva(f.avance), &mut l);
    assert!(
        l.pegado_al_borde,
        "el sello del marco no llego al borde izquierdo: o la geometria dejo de \
         emitirlo desde fuera, o alguien volvio a descartarlo en vez de recortarlo"
    );
}
