//! **EL MARCO** -- las dos masas oscuras que te ponen dentro de la escena.
//!
//! === Que problema resuelve, y por que ninguna cantidad de detalle lo resolvia ===
//!
//! Con cielo, torres lejanas y torres cercanas la escena ya tiene profundidad.
//! Y sigue siendo **una vista**: no hay nada entre el ojo y la ciudad, asi que
//! se lee como un fondo de pantalla por muchas ventanas que se le pongan.
//!
//! Lo que falta es un plano delante. En la referencia que enseno el dueno --y en
//! cualquier plano de callejon, y en el arranque de medio videojuego-- los bordes
//! izquierdo y derecho son masas de edificio casi negras. Eso es lo unico que
//! separa *"una foto de una ciudad"* de *"estas de pie en un callejon
//! mirandola"*.
//!
//! Y no se consigue con detalle. Se consigue con **un valor mas oscuro que todo
//! lo demas, ocupando los bordes, y sin nada dentro**. El propio dueno lo acoto
//! asi: *"NO es necesario tener TANTOS DETALLES ni es como juegos"*.
//!
//! === La perspectiva sale de la FORMA, no del dibujo ===
//!
//! Un rectangulo negro a cada lado tapa, pero no da perspectiva: da barras. Lo
//! que da el pasillo es que cada lado sea **una fila de bloques que retrocede**:
//!
//! ```text
//!    |####|                                        |####|
//!    |####|##|                                  |##|####|
//!    |####|##|#|                              |#|##|####|
//!    |####|##|#|__                          __|#|##|####|
//!         ^   ^  ^                          ^   ^   ^
//!      cerca  |  lejos                  lejos   |   cerca
//!             y cada uno mas bajo y mas claro que el anterior
//! ```
//!
//! Tres pistas a la vez, y las tres son aritmetica: cada bloque que retrocede es
//! **mas estrecho**, **mas bajo** y **mas claro** (perspectiva aerea, la misma
//! que la niebla). El ojo lee esa progresion como distancia sin que haya un solo
//! detalle dibujado.
//!
//! === Por que el marco casi NO se mueve, rompiendo el paralaje ===
//!
//! La regla del paralaje dice que lo cercano pasa mas deprisa. Aqui se
//! desobedece a proposito, y el motivo es el reloj: la intro dura unos 2,4
//! segundos y la camara avanza 120 px/s, o sea **unos 290 px en total**. Un marco
//! de primer plano de verdad se moveria dos o tres veces eso --700 px-- y como
//! esta anclado a los bordes de la pantalla, **se saldria entero y dejaria los
//! bordes al aire** a mitad de la animacion. El plano que existe para cerrar la
//! escena la abriria.
//!
//! Asi que el marco es un **proscenio**: la boca del callejon por la que miras.
//! Deriva lo justo para que se note que hay movimiento (`camara::DERIVA_DEL_MARCO`) y los bloques
//! de fuera se estiran hasta el borde para que el sello no se pueda romper nunca.
//! Hay una prueba que lo recorre entero.

use crate::camara::Camara;
use crate::paleta::*;

/// Cuantos bloques tiene cada lado.
///
/// Cuatro: con dos no hay progresion que leer --son dos barras-- y con seis los
/// ultimos son tan estrechos que se leen como rayas. Cuatro es donde se ve el
/// escalonado y todavia queda calle en medio.
pub const BLOQUES_POR_LADO: usize = 4;

/// Cuanta pantalla ocupa cada lado del marco, en centesimas.
///
/// ** Y ESTE NUMERO NO ES DE GUSTO, AUNQUE LO PAREZCA.
///
/// El marco tapa, y **lo que tapa es la barra de progreso**: las torres no son
/// decoracion, son el arranque ocurriendo (`Ciudad::encender` las va prendiendo
/// de izquierda a derecha, y una torre que se queda negra es un subsistema que no
/// arranco). Un marco ancho se come esa informacion.
///
/// Se probaron los dos a ojo con `bmo-vista-ciudad`, que para eso existe: a 25
/// el pasillo es rotundo y se pierde la mitad de la ciudad; a 19 el pasillo se
/// lee igual y se ve la ciudad entera. Gana 19, y no por bonito.
///
/// [!] Estaba en 27 y una PRUEBA lo tumbo antes de compilar el kernel: con el
/// solape de los bloques cada lado llegaba al 31% real y la calle se quedaba en
/// el 38%. El aura del gato es OPACA, asi que si el marco se comiera su sitio
/// habria que elegir entre uno de los dos. Ver [`Marco::interior`] y
/// `queda_calle_despejada_en_medio`.
const ANCHO_LADO: i32 = 19;

/// Un bloque del marco: donde esta, cuanto mide y de que tono.
#[derive(Clone, Copy, Debug)]
pub struct Bloque {
    pub x: i32,
    pub ancho: i32,
    /// `y` de su techo. `0` = llega al borde de arriba de la pantalla.
    pub techo: i32,
    pub tono: Color,
    /// El canto iluminado esta a la derecha? (bloques del lado izquierdo).
    pub canto_derecho: bool,
}

/// El marco entero: los dos lados.
pub struct Marco {
    bloques: [Bloque; BLOQUES_POR_LADO * 2],
    ancho: i32,
    alto: i32,
}

impl Marco {
    /// Compone el marco para un lienzo de `ancho x alto`.
    ///
    /// No lleva azar: el marco es **composicion**, no reparto. Es la parte de la
    /// escena que se decide, igual que el encuadre del logo -- y por eso sale
    /// igual en todos los arranques mientras la ciudad de detras cambia.
    pub fn nuevo(ancho: i32, alto: i32) -> Self {
        let mut bloques = [Bloque {
            x: 0,
            ancho: 0,
            techo: 0,
            tono: MARCO_CERCA,
            canto_derecho: true,
        }; BLOQUES_POR_LADO * 2];

        let lado = ancho * ANCHO_LADO / 100;
        let mut n = 0;
        for izquierda in [true, false] {
            // Cuanto llevamos metido hacia el centro desde este borde.
            let mut metido = 0;
            for i in 0..BLOQUES_POR_LADO {
                // Cada bloque que retrocede es mas ESTRECHO. Los anchos suman
                // mas que `lado` porque los bloques **se solapan**: una fila de
                // bloques separados parece una valla, no una calle.
                let w = (lado * (10 - 2 * i as i32) / 16).max(6);
                // Mas BAJO: el primero llega al borde de arriba y los demas van
                // descendiendo. Es la mitad vertical de la perspectiva.
                let techo = alto * (i as i32 * 13) / 100;
                // Mas CLARO: perspectiva aerea, la misma que la niebla.
                let tono = mezcla(
                    MARCO_CERCA,
                    MARCO_LEJOS,
                    i as u32,
                    (BLOQUES_POR_LADO - 1) as u32,
                );
                let x = if izquierda { metido } else { ancho - metido - w };
                bloques[n] = Bloque { x, ancho: w, techo, tono, canto_derecho: izquierda };
                n += 1;
                // El siguiente empieza antes de que acabe este: de ahi el
                // solape.
                metido += w * 3 / 5;
            }
        }
        Marco { bloques, ancho, alto }
    }

    /// **Hasta donde llega el marco por dentro**: la `x` del canto interior del
    /// lado izquierdo. El derecho es simetrico (`ancho - interior`).
    ///
    /// Lo pregunta el encuadre del logo: el aura es opaca y no puede meterse
    /// debajo del marco, porque entonces el marco la borraria o ella borraria el
    /// marco. Se contesta con una cuenta y no con un numero copiado.
    pub fn interior(&self) -> i32 {
        self.bloques[..BLOQUES_POR_LADO]
            .iter()
            .map(|b| b.x + b.ancho)
            .max()
            .unwrap_or(0)
    }

    pub fn bloques(&self) -> &[Bloque] {
        &self.bloques
    }

    /// **Dibuja el marco.** Emite rectangulos; no toca ninguna pantalla.
    ///
    /// Va lo ULTIMO de la escena: es lo que esta mas cerca, asi que tapa a todo
    /// lo demas. Y tapa de verdad --es opaco-- que es justamente por lo que
    /// tambien AHORRA: cada pixel que cubre es un pixel de ciudad que ya no hace
    /// falta calcular. Ver `Ciudad::dibujar`.
    /// * SE DIBUJA RECORTANDO, NO SOLAPANDO, y eso arregla dos cosas de una.
    ///
    /// Los bloques se GENERAN solapados a proposito --una fila de bloques
    /// separados parece una valla, no una calle-- y al pintarlos tal cual salian
    /// dos problemas que son el mismo:
    ///
    /// * **La profundidad salia del reves.** Se pintaban del cercano al lejano,
    ///   asi que el ultimo en pintarse era el mas lejano... y tapaba al mas
    ///   cercano. La escalera de valores estaba bien calculada y mal pintada.
    /// * **Y se pagaba dos veces.** El presupuesto lo dijo: 0,75x de pantalla
    ///   escritos para cubrir 0,52x. Un tercio del marco era marco encima de
    ///   marco.
    ///
    /// Recortando cada bloque a lo que asoma por dentro del anterior, la escalera
    /// queda continua, el orden deja de importar y **no se escribe un pixel dos
    /// veces**. El solape sigue estando en la generacion, que es donde hacia
    /// falta: garantiza que no quede una rendija entre bloque y bloque.
    pub fn dibujar(&self, cam: Camara, mut rect: impl FnMut(i32, i32, i32, i32, Color)) {
        // Capa 2 = el marco. Deriva poco a proposito: ver la cabecera.
        let dx = cam.desplazamiento(2);
        for lado in 0..2usize {
            // Hasta donde ha llegado ya el marco por dentro, en este lado.
            let mut borde = if lado == 0 { i32::MIN } else { i32::MAX };
            for i in 0..BLOQUES_POR_LADO {
                let b = self.bloques[lado * BLOQUES_POR_LADO + i];
                let izquierda = b.canto_derecho;
                let mut x = b.x - dx;
                let mut w = b.ancho;
                // ** EL SELLO. El bloque de fuera de cada lado se estira hasta
                // el borde de la pantalla pase lo que pase. Sin esto, la deriva
                // --por poca que sea-- abre una rendija de cielo pegada al
                // canto, y una rendija en el plano que existe para cerrar la
                // escena se ve mas que el marco entero.
                if i == 0 {
                    if izquierda {
                        w += x.max(0);
                        x = x.min(0);
                    } else {
                        let derecha = x + w;
                        if derecha < self.ancho {
                            w += self.ancho - derecha;
                        }
                    }
                }
                // Recortar a lo que asoma por dentro de lo ya cubierto.
                if izquierda {
                    if borde != i32::MIN && x < borde {
                        w -= borde - x;
                        x = borde;
                    }
                } else {
                    let derecha = x + w;
                    if borde != i32::MAX && derecha > borde {
                        w -= derecha - borde;
                    }
                }
                let alto = self.alto - b.techo;
                if w <= 0 || alto <= 0 {
                    continue;
                }
                rect(x, b.techo, w, alto, b.tono);
                // El canto iluminado: la luz de la calle enganchada en la
                // esquina. Un rectangulo de dos pixeles, y sin el el marco es
                // una barra negra.
                let cx = if izquierda { x + w - 2 } else { x };
                rect(cx, b.techo, 2, alto, MARCO_CANTO);
                borde = if izquierda { x + w } else { x };
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// ** EL SELLO NO SE PUEDE ROMPER. Es la razon de ser del modulo: un marco
    /// con una rendija de cielo pegada al borde se ve mas que el marco entero.
    ///
    /// Se recorre TODO el avance que la camara puede tener en la intro, y de
    /// sobra: si algun dia la animacion dura mas, esta prueba lo dice antes que
    /// el Ryzen.
    #[test]
    fn el_marco_nunca_deja_el_borde_al_aire() {
        let (ancho, alto) = (1920, 1080);
        let m = Marco::nuevo(ancho, alto);
        for avance in (0..2000).step_by(7) {
            let mut cubre_izq = false;
            let mut cubre_der = false;
            m.dibujar(Camara::nueva(avance), |x, y, w, h, _| {
                if y == 0 && h >= alto && w > 4 {
                    if x <= 0 {
                        cubre_izq = true;
                    }
                    if x + w >= ancho {
                        cubre_der = true;
                    }
                }
            });
            assert!(cubre_izq, "con avance {} se abrio el borde izquierdo", avance);
            assert!(cubre_der, "con avance {} se abrio el borde derecho", avance);
        }
    }

    /// ** LA PERSPECTIVA ES ARITMETICA: cada bloque que retrocede es mas
    /// estrecho, mas bajo y mas claro. Las tres a la vez.
    ///
    /// Si una de las tres se rompiera, el lado dejaria de leerse como una calle
    /// que se aleja y volveria a ser una barra con escalones. Y eso no se ve en
    /// el codigo: se ve en el Ryzen, tres dias despues.
    #[test]
    fn cada_bloque_que_retrocede_es_mas_estrecho_mas_bajo_y_mas_claro() {
        let m = Marco::nuevo(1920, 1080);
        for lado in 0..2 {
            let bs = &m.bloques()[lado * BLOQUES_POR_LADO..(lado + 1) * BLOQUES_POR_LADO];
            for par in bs.windows(2) {
                let (cerca, lejos) = (par[0], par[1]);
                assert!(lejos.ancho < cerca.ancho, "no se estrecha");
                assert!(lejos.techo > cerca.techo, "no baja");
                assert!(
                    luminancia(lejos.tono) >= luminancia(cerca.tono),
                    "no se aclara al alejarse"
                );
            }
        }
    }

    /// El primer bloque de cada lado llega al borde de ARRIBA. Es lo que
    /// convierte el marco en marco y no en una fila de edificios bajos.
    #[test]
    fn el_bloque_de_delante_llega_hasta_arriba() {
        let m = Marco::nuevo(1366, 768);
        assert_eq!(m.bloques()[0].techo, 0);
        assert_eq!(m.bloques()[BLOQUES_POR_LADO].techo, 0);
    }

    /// ** Y DEJA LA CALLE EN MEDIO. El logo vive ahi y su aura es OPACA: si el
    /// marco se comiera el centro habria que elegir entre el marco y el gato.
    #[test]
    fn queda_calle_despejada_en_medio() {
        for (w, h) in [(1920, 1080), (1600, 900), (1366, 768), (1280, 720)] {
            let m = Marco::nuevo(w, h);
            let libre = w - 2 * m.interior();
            assert!(
                libre * 100 / w >= 40,
                "a {}x{} solo queda el {}% de calle",
                w,
                h,
                libre * 100 / w
            );
        }
    }

    /// Los dos lados son espejo uno del otro. Un marco asimetrico se lee como un
    /// fallo de centrado, no como composicion.
    #[test]
    fn los_dos_lados_son_simetricos() {
        let m = Marco::nuevo(1920, 1080);
        for i in 0..BLOQUES_POR_LADO {
            let izq = m.bloques()[i];
            let der = m.bloques()[BLOQUES_POR_LADO + i];
            assert_eq!(izq.ancho, der.ancho);
            assert_eq!(izq.techo, der.techo);
            assert_eq!(izq.x, 1920 - der.x - der.ancho, "no estan a la misma distancia del borde");
        }
    }

    /// Un lienzo diminuto no rompe ni genera bloques de ancho cero.
    #[test]
    fn un_lienzo_diminuto_no_rompe_nada() {
        let m = Marco::nuevo(64, 48);
        for b in m.bloques() {
            assert!(b.ancho > 0);
            assert!(b.techo < 48);
        }
    }
}
