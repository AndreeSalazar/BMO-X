//! **El MARCO**: lo que toda ventana tiene y ninguna deberia escribir dos veces.
//!
//! === Por que existe ===
//!
//! Habia tres ventanas --Ejecutar, Datos y el log del kernel-- y **tres copias
//! de lo mismo**: cada una con su geometria, su pintado de borde y su barra de
//! titulo. En cuanto entro el arrastre, pasaron a ser tres copias de un
//! algoritmo, que es la forma en que dos ventanas del mismo sistema acaban
//! comportandose distinto sin que nadie lo decida.
//!
//! Anadir minimizar, maximizar y cerrar a cada una habria sido escribir tres
//! veces la misma maquina de estados. Aqui se escribe una.
//!
//! * **El criterio no es la estetica: es que la CUARTA ventana salga gratis.**
//!
//! === Lo que el marco SI sabe, y lo que no ===
//!
//! Sabe de rectangulos, de agarres y de botones. **No sabe que hay dentro** --
//! ni una linea de este modulo menciona ESTRATOS, el log ni la caja de lanzar.
//! Es la misma ley que separa `bmo-lower` de los frontends: se comparten
//! contratos, nunca cerebros.

use bmo_userland as bmo;

use super::*;

/// Los tres botones de la esquina, en el orden de todos los escritorios.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Boton {
    Minimizar,
    Maximizar,
    Cerrar,
}

/// Lado de la zona sensible de cada boton. Veinticuatro pixeles se aciertan sin
/// mirar; doce obligan a apuntar, y apuntar para CERRAR una ventana es como se
/// cierra la que no era.
pub(crate) const BOTON_LADO: u32 = 24;
/// Lo que se puede agarrar en la esquina para estirar.
const ASA_ESQUINA: u32 = 16;

/// El rojo de cerrar cuando el puntero esta encima. Es el unico sitio del
/// sistema donde el rojo no significa "algo va mal" sino "esto destruye", y por
/// eso solo aparece al senalarlo: un aspa roja permanente es una alarma de
/// fondo.
const CERRAR_ENCIMA: u32 = 0x00C4_2B1F;
/// El realce de los otros dos: un peldano mas claro, sin color propio.
const BOTON_ENCIMA: u32 = 0x0039_4457;

/// Geometria y estado de una ventana. **Lo unico que hay que llevar.**
pub(crate) struct Marco {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) ancho: u32,
    pub(crate) alto: u32,
    /// Por debajo de esto no se puede encoger. Una ventana que se puede dejar
    /// inservible con el raton es una trampa, no una libertad.
    min_ancho: u32,
    min_alto: u32,
    /// Donde se agarro DENTRO de la ventana, si se esta arrastrando.
    ///
    /// Se guarda el agarre y no la posicion del puntero: si no, la ventana pega
    /// un salto al empezar y se coloca con su esquina bajo el raton en vez de
    /// quedarse donde la cogiste.
    arrastre: Option<(u32, u32)>,
    estirando: bool,
    /// La geometria de antes de maximizar, para poder volver.
    guardada: Option<(u32, u32, u32, u32)>,
    /// Abierta pero escondida. **No es lo mismo que cerrada**: una minimizada
    /// conserva su sitio, su tamano y lo que estuviera mirando.
    pub(crate) minimizada: bool,
    /// Que boton tiene el puntero encima, para realzarlo. Se lleva como estado
    /// porque el realce solo se repinta cuando CAMBIA -- repintarlo cada
    /// fotograma son 1.700 pixeles de memoria de video sin cache para dejarlo
    /// igual.
    pub(crate) encima: Option<Boton>,
}

impl Marco {
    /// Una ventana nueva, **en fracciones de la pantalla**.
    ///
    /// === Por que no en pixeles ===
    ///
    /// Porque `640 x 330` es un tamano correcto en la pantalla del que lo
    /// escribio y ninguna otra. En una 4K es un sello de correos; en una
    /// 1024x768 no cabe. Un tamano en tantos por ciento se adapta solo, y los
    /// minimos --que si son absolutos, porque el texto mide lo que mide--
    /// impiden que en una pantalla pequena quede ilegible.
    pub(crate) fn nuevo(
        p: &bmo::Pantalla,
        pct_ancho: u32,
        pct_alto: u32,
        min_ancho: u32,
        min_alto: u32,
    ) -> Self {
        let ancho = (p.ancho * pct_ancho / 100)
            .max(min_ancho)
            .min(p.ancho.saturating_sub(16));
        let alto = (p.alto * pct_alto / 100)
            .max(min_alto)
            .min(p.alto.saturating_sub(BARRA_ALTO + 16));
        Self {
            x: p.ancho.saturating_sub(ancho) / 2,
            // Centrada en el hueco que queda BAJO la barra del sistema, no en
            // la pantalla: centrarla en la pantalla la deja siempre un poco
            // alta, y con la barra encima parece descolocada.
            y: BARRA_ALTO + (p.alto.saturating_sub(BARRA_ALTO + alto)) / 2,
            ancho,
            alto,
            min_ancho,
            min_alto,
            arrastre: None,
            estirando: false,
            guardada: None,
            minimizada: false,
            encima: None,
        }
    }

    pub(crate) fn contiene(&self, px: u32, py: u32) -> bool {
        !self.minimizada
            && px >= self.x
            && px < self.x + self.ancho
            && py >= self.y
            && py < self.y + self.alto
    }

    pub(crate) fn maximizada(&self) -> bool {
        self.guardada.is_some()
    }

    // -- Los botones -----------------------------------------------------

    /// La `x` donde empieza el boton `i` contando desde la derecha.
    fn boton_x(&self, i: u32) -> u32 {
        self.x + self.ancho - (3 - i) * BOTON_LADO - 6
    }

    /// Que boton hay bajo el puntero, si hay alguno.
    pub(crate) fn boton_en(&self, px: u32, py: u32) -> Option<Boton> {
        if self.minimizada || py < self.y + 2 || py >= self.y + TITULO_ALTO {
            return None;
        }
        for (i, b) in [Boton::Minimizar, Boton::Maximizar, Boton::Cerrar].into_iter().enumerate() {
            let bx = self.boton_x(i as u32);
            if px >= bx && px < bx + BOTON_LADO {
                return Some(b);
            }
        }
        None
    }

    /// Cae en la barra de titulo, o sea en el asa de arrastrar?
    ///
    /// **Los botones NO cuentan como asa**: si contaran, cada clic en cerrar
    /// empezaria ademas un arrastre, y soltar en otro sitio moveria la ventana
    /// justo cuando querias cerrarla.
    pub(crate) fn en_el_asa(&self, px: u32, py: u32) -> bool {
        self.contiene(px, py) && py < self.y + TITULO_ALTO && self.boton_en(px, py).is_none()
    }

    /// Cae en la esquina de estirar, la de abajo a la derecha?
    pub(crate) fn en_la_esquina(&self, px: u32, py: u32) -> bool {
        !self.minimizada
            && !self.maximizada()
            && px + ASA_ESQUINA >= self.x + self.ancho
            && px < self.x + self.ancho
            && py + ASA_ESQUINA >= self.y + self.alto
            && py < self.y + self.alto
    }

    // -- Mover y estirar -------------------------------------------------

    /// Empieza a arrastrar o a estirar. `true` si agarro algo.
    ///
    /// La esquina se mira ANTES que el asa: si se solaparan --una ventana en su
    /// tamano minimo--, gana estirar, porque es la zona mas pequena y la que no
    /// se puede acertar de otra forma.
    pub(crate) fn agarrar(&mut self, px: u32, py: u32) -> bool {
        if self.en_la_esquina(px, py) {
            self.estirando = true;
            return true;
        }
        if !self.en_el_asa(px, py) || self.maximizada() {
            return false;
        }
        self.arrastre = Some((px - self.x, py - self.y));
        true
    }

    pub(crate) fn soltar(&mut self) {
        self.arrastre = None;
        self.estirando = false;
    }

    pub(crate) fn agarrado(&self) -> bool {
        self.arrastre.is_some() || self.estirando
    }

    /// Lleva o estira la ventana hasta el puntero. `true` si cambio algo.
    ///
    /// Las dos cosas van juntas porque **quien llama no tiene por que saber
    /// cual de las dos esta pasando**: agarro, mueve el raton, y el marco sabe
    /// lo que habia agarrado.
    pub(crate) fn seguir_al_puntero(&mut self, p: &bmo::Pantalla, px: u32, py: u32) -> bool {
        if let Some((ax, ay)) = self.arrastre {
            let nx = px.saturating_sub(ax).min(p.ancho.saturating_sub(self.ancho));
            // Nunca por encima de la barra del sistema: una ventana con el asa
            // debajo de la barra no se puede volver a coger.
            let ny = py
                .saturating_sub(ay)
                .max(BARRA_ALTO)
                .min(p.alto.saturating_sub(self.alto));
            if nx == self.x && ny == self.y {
                return false;
            }
            self.x = nx;
            self.y = ny;
            return true;
        }
        if self.estirando {
            let na = (px.saturating_sub(self.x) + 1)
                .max(self.min_ancho)
                .min(p.ancho.saturating_sub(self.x));
            let nl = (py.saturating_sub(self.y) + 1)
                .max(self.min_alto)
                .min(p.alto.saturating_sub(self.y));
            if na == self.ancho && nl == self.alto {
                return false;
            }
            self.ancho = na;
            self.alto = nl;
            return true;
        }
        false
    }

    /// Maximizar, o volver al tamano de antes. Devuelve la geometria VIEJA para
    /// que quien llama sepa que trozo de pantalla tiene que repintar.
    ///
    /// Maximizada NO es "pantalla completa": deja la barra del sistema a la
    /// vista. Una ventana que tapa la barra esconde las fichas de las demas, y
    /// entonces no hay forma de volver a ellas sin adivinar un atajo.
    pub(crate) fn alternar_maximizada(&mut self, p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
        let vieja = (self.x, self.y, self.ancho, self.alto);
        match self.guardada.take() {
            Some((x, y, a, l)) => {
                self.x = x;
                self.y = y;
                self.ancho = a;
                self.alto = l;
            }
            None => {
                self.guardada = Some(vieja);
                self.x = 0;
                self.y = BARRA_ALTO;
                self.ancho = p.ancho;
                self.alto = p.alto.saturating_sub(BARRA_ALTO);
            }
        }
        vieja
    }

    /// Recoloca la ventana si se ha quedado fuera del panel.
    ///
    /// Hace falta al restaurar una minimizada y al cambiar de resolucion: una
    /// geometria guardada puede haber dejado de ser valida mientras no se veia.
    pub(crate) fn encajar(&mut self, p: &bmo::Pantalla) {
        self.ancho = self.ancho.min(p.ancho).max(self.min_ancho.min(p.ancho));
        self.alto = self
            .alto
            .min(p.alto.saturating_sub(BARRA_ALTO))
            .max(self.min_alto.min(p.alto));
        self.x = self.x.min(p.ancho.saturating_sub(self.ancho));
        self.y = self
            .y
            .max(BARRA_ALTO)
            .min(p.alto.saturating_sub(self.alto));
    }

    // -- Pintar ----------------------------------------------------------

    /// El cromo entero: sombra, borde, cuerpo, barra de titulo y los tres
    /// botones. Deja el interior listo para que el contenido se pinte encima.
    ///
    /// Los colores los trae quien llama porque **cada ventana tiene el suyo** y
    /// eso es informacion, no decoracion: el verde dice ESTRATOS y el azul dice
    /// el kernel antes de que nadie lea un titulo.
    pub(crate) fn pintar_cromo(
        &self,
        p: &bmo::Pantalla,
        borde: u32,
        cuerpo: u32,
        titulo_fondo: u32,
        acento: u32,
    ) {
        sombra(p, self.x, self.y, self.ancho, self.alto);
        rect_redondeado(p, self.x, self.y, self.ancho, self.alto, borde);
        rect_redondeado(p, self.x + 1, self.y + 1, self.ancho - 2, self.alto - 2, cuerpo);

        // La barra, con la MISMA curva que la ventana o asomaria por fuera de
        // sus esquinas.
        for i in 0..RADIO {
            let s = curva(i);
            p.rect(self.x + s, self.y + 1 + i, self.ancho - 2 * s, 1, titulo_fondo);
        }
        p.rect(
            self.x + 1,
            self.y + 1 + RADIO,
            self.ancho - 2,
            TITULO_ALTO - 2 - RADIO,
            titulo_fondo,
        );
        p.rect(self.x + 1, self.y + TITULO_ALTO - 1, self.ancho - 2, 1, acento);

        self.pintar_botones(p, titulo_fondo);
        self.pintar_asa_esquina(p, borde);
    }

    /// Los tres, dibujados con rectangulos porque la fuente no trae sus glifos
    /// y meterlos en la fuente por tres iconos seria tocar el generador para
    /// algo que se dibuja con cuatro `rect`.
    pub(crate) fn pintar_botones(&self, p: &bmo::Pantalla, fondo: u32) {
        for (i, b) in [Boton::Minimizar, Boton::Maximizar, Boton::Cerrar].into_iter().enumerate() {
            let bx = self.boton_x(i as u32);
            let by = self.y + 2;
            let alto = TITULO_ALTO - 3;
            let realce = if self.encima == Some(b) {
                if b == Boton::Cerrar { CERRAR_ENCIMA } else { BOTON_ENCIMA }
            } else {
                fondo
            };
            p.rect(bx, by, BOTON_LADO, alto, realce);

            let cx = bx + BOTON_LADO / 2;
            let cy = by + alto / 2;
            let tinta = if self.encima == Some(b) && b == Boton::Cerrar { 0x00FF_FFFF } else { TEXTO };
            match b {
                // Una raya. Es el icono universal y no necesita mas.
                Boton::Minimizar => p.rect(cx - 5, cy, 10, 1, tinta),
                // Un cuadrado hueco; DOS solapados cuando ya esta maximizada,
                // que es como se dice "restaurar" en todas partes.
                Boton::Maximizar => {
                    if self.maximizada() {
                        marco_hueco(p, cx - 5, cy - 3, 8, 8, tinta);
                        marco_hueco(p, cx - 2, cy - 6, 8, 8, tinta);
                    } else {
                        marco_hueco(p, cx - 4, cy - 4, 9, 9, tinta);
                    }
                }
                // El aspa, pixel a pixel: no hay primitiva de linea diagonal y
                // dieciseis puntos no piden una.
                Boton::Cerrar => {
                    for k in 0..9u32 {
                        p.punto(cx - 4 + k, cy - 4 + k, tinta);
                        p.punto(cx - 4 + k, cy + 4 - k, tinta);
                    }
                }
            }
        }
    }

    /// Las tres rayitas en diagonal de la esquina. Un agarre invisible no
    /// existe: nadie prueba a estirar una ventana que no parece estirable.
    fn pintar_asa_esquina(&self, p: &bmo::Pantalla, color: u32) {
        if self.maximizada() {
            return;
        }
        for k in 0..3u32 {
            let d = 4 + k * 4;
            p.rect(self.x + self.ancho - 4 - d, self.y + self.alto - 6 - k * 4, d, 2, color);
        }
    }
}

/// Un rectangulo de un pixel de grosor. Cuatro `rect` y ya.
fn marco_hueco(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, color: u32) {
    p.rect(x, y, w, 1, color);
    p.rect(x, y + h - 1, w, 1, color);
    p.rect(x, y, 1, h, color);
    p.rect(x + w - 1, y, 1, h, color);
}
