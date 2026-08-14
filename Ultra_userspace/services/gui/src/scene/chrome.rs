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
pub(crate) enum Button {
    Minimize,
    Maximize,
    Close,
}

/// Hacia donde va un gesto de teclado.
///
/// Es un rumbo y no un `(dx, dy)` porque los cuatro gestos NO son simetricos:
/// `Up` maximiza y `Down` restaura, mientras que los lados encajan una
/// mitad. Con un par de numeros habria que reconstruir cual era cual a base de
/// comparaciones, que es como se acaba maximizando al pulsar izquierda.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Heading {
    Left,
    Right,
    Up,
    Down,
}

/// Cuanto se mueve una ventana por pulsacion.
///
/// Veinticuatro pixeles: se ve que se movio sin tener que mirar dos veces, y
/// cruzar una pantalla de 1920 cuesta unas ochenta pulsaciones -- que suena a
/// mucho hasta que se recuerda que para eso estan `Shift` y las mitades.
pub(crate) const KEY_STEP: u32 = 24;

/// Lado de la zona sensible de cada boton. Veinticuatro pixeles se aciertan sin
/// mirar; doce obligan a apuntar, y apuntar para CERRAR una ventana es como se
/// cierra la que no era.
pub(crate) const BTN_SIDE: u32 = 24;
/// Lo que se puede agarrar en la esquina para estirar.
const GRIP_CORNER: u32 = 16;

/// El rojo de cerrar cuando el puntero esta encima. Es el unico sitio del
/// sistema donde el rojo no significa "algo va mal" sino "esto destruye", y por
/// eso solo aparece al senalarlo: un aspa roja permanente es una alarma de
/// fondo.
const CLOSE_HOVER: u32 = 0x00C4_2B1F;
/// El realce de los otros dos: un peldano mas claro, sin color propio.
const BTN_HOVER: u32 = 0x0039_4457;

/// Geometria y estado de una ventana. **Lo unico que hay que llevar.**
pub(crate) struct Chrome {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Por debajo de esto no se puede encoger. Una ventana que se puede dejar
    /// inservible con el raton es una trampa, no una libertad.
    min_w: u32,
    min_h: u32,
    /// Donde se agarro DENTRO de la ventana, si se esta arrastrando.
    ///
    /// Se guarda el agarre y no la posicion del puntero: si no, la ventana pega
    /// un salto al empezar y se coloca con su esquina bajo el raton en vez de
    /// quedarse donde la cogiste.
    drag: Option<(u32, u32)>,
    resizing: bool,
    /// La geometria de antes de maximizar, para poder volver.
    saved: Option<(u32, u32, u32, u32)>,
    /// Abierta pero escondida. **No es lo mismo que cerrada**: una minimizada
    /// conserva su sitio, su tamano y lo que estuviera mirando.
    pub(crate) minimized: bool,
    /// Que boton tiene el puntero encima, para realzarlo. Se lleva como estado
    /// porque el realce solo se repinta cuando CAMBIA -- repintarlo cada
    /// fotograma son 1.700 pixeles de memoria de video sin cache para dejarlo
    /// igual.
    pub(crate) hover: Option<Button>,
}

impl Chrome {
    /// Una ventana nueva, **en fracciones de la pantalla**.
    ///
    /// === Por que no en pixeles ===
    ///
    /// Porque `640 x 330` es un tamano correcto en la pantalla del que lo
    /// escribio y ninguna otra. En una 4K es un sello de correos; en una
    /// 1024x768 no cabe. Un tamano en tantos por ciento se adapta solo, y los
    /// minimos --que si son absolutos, porque el texto mide lo que mide--
    /// impiden que en una pantalla pequena quede ilegible.
    pub(crate) fn new(
        p: &bmo::Pantalla,
        pct_w: u32,
        pct_h: u32,
        min_w: u32,
        min_h: u32,
    ) -> Self {
        let width = (p.ancho * pct_w / 100)
            .max(min_w)
            .min(p.ancho.saturating_sub(16));
        let height = (p.alto * pct_h / 100)
            .max(min_h)
            .min(p.alto.saturating_sub(TASKBAR_H + 16));
        Self {
            x: p.ancho.saturating_sub(width) / 2,
            // Centrada en el hueco que queda BAJO la barra del sistema, no en
            // la pantalla: centrarla en la pantalla la deja siempre un poco
            // alta, y con la barra encima parece descolocada.
            y: TASKBAR_H + (p.alto.saturating_sub(TASKBAR_H + height)) / 2,
            width,
            height,
            min_w,
            min_h,
            drag: None,
            resizing: false,
            saved: None,
            minimized: false,
            hover: None,
        }
    }

    /// Una ventana **del tamano de su contenido**, mas el cromo.
    ///
    /// === Por que esta si va en pixeles, cuando `new` no ===
    ///
    /// Porque aqui el tamano no lo elige el escritorio: **lo eligio la app**. Una
    /// superficie de 640x400 mide eso, y darle un 40 % de la pantalla la
    /// estiraria o la dejaria con un borde muerto alrededor. El argumento contra
    /// los pixeles --que `640x330` solo es correcto en la pantalla del que lo
    /// escribio-- vale para una ventana del sistema y no para una que envuelve
    /// una imagen de medida conocida.
    ///
    /// Se recorta contra el panel: una app puede pedir una superficie mas grande
    /// que la pantalla, y una ventana que no cabe no se puede ni agarrar.
    pub(crate) fn for_content(p: &bmo::Pantalla, width: u32, height: u32) -> Self {
        let width = (width + 2).min(p.ancho.saturating_sub(16)).max(3 * BTN_SIDE + 16);
        let height = (height + TITLE_H + 1).min(p.alto.saturating_sub(TASKBAR_H + 16));
        Self {
            x: p.ancho.saturating_sub(width) / 2,
            y: TASKBAR_H + (p.alto.saturating_sub(TASKBAR_H + height)) / 2,
            width,
            height,
            // El minimo es el cromo: por debajo de eso no quedan ni los botones,
            // y una ventana sin boton de cerrar es una ventana que no se cierra.
            min_w: 3 * BTN_SIDE + 16,
            min_h: TITLE_H + 8,
            drag: None,
            resizing: false,
            saved: None,
            minimized: false,
            hover: None,
        }
    }

    pub(crate) fn contains(&self, px: u32, py: u32) -> bool {
        !self.minimized
            && px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }

    pub(crate) fn is_maximized(&self) -> bool {
        self.saved.is_some()
    }

    // -- Los botones -----------------------------------------------------

    /// La `x` donde empieza el boton `i` contando desde la derecha.
    fn boton_x(&self, i: u32) -> u32 {
        self.x + self.width - (3 - i) * BTN_SIDE - 6
    }

    /// Que boton hay bajo el puntero, si hay alguno.
    pub(crate) fn button_at(&self, px: u32, py: u32) -> Option<Button> {
        if self.minimized || py < self.y + 2 || py >= self.y + TITLE_H {
            return None;
        }
        for (i, b) in [Button::Minimize, Button::Maximize, Button::Close].into_iter().enumerate() {
            let bx = self.boton_x(i as u32);
            if px >= bx && px < bx + BTN_SIDE {
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
    pub(crate) fn on_the_grip(&self, px: u32, py: u32) -> bool {
        self.contains(px, py) && py < self.y + TITLE_H && self.button_at(px, py).is_none()
    }

    /// Cae en la esquina de estirar, la de abajo a la derecha?
    pub(crate) fn on_the_corner(&self, px: u32, py: u32) -> bool {
        !self.minimized
            && !self.is_maximized()
            && px + GRIP_CORNER >= self.x + self.width
            && px < self.x + self.width
            && py + GRIP_CORNER >= self.y + self.height
            && py < self.y + self.height
    }

    // -- Mover y estirar -------------------------------------------------

    /// Empieza a arrastrar o a estirar. `true` si agarro algo.
    ///
    /// La esquina se mira ANTES que el asa: si se solaparan --una ventana en su
    /// tamano minimo--, gana estirar, porque es la zona mas pequena y la que no
    /// se puede acertar de otra forma.
    pub(crate) fn grab(&mut self, px: u32, py: u32) -> bool {
        if self.on_the_corner(px, py) {
            self.resizing = true;
            return true;
        }
        if !self.on_the_grip(px, py) || self.is_maximized() {
            return false;
        }
        self.drag = Some((px - self.x, py - self.y));
        true
    }

    pub(crate) fn release(&mut self) {
        self.drag = None;
        self.resizing = false;
    }

    pub(crate) fn grabbed(&self) -> bool {
        self.drag.is_some() || self.resizing
    }

    /// Lleva o estira la ventana hasta el puntero. `true` si cambio algo.
    ///
    /// Las dos cosas van juntas porque **quien llama no tiene por que saber
    /// cual de las dos esta pasando**: agarro, mueve el raton, y el marco sabe
    /// lo que habia agarrado.
    pub(crate) fn follow_pointer(&mut self, p: &bmo::Pantalla, px: u32, py: u32) -> bool {
        if let Some((ax, ay)) = self.drag {
            let nx = px.saturating_sub(ax).min(p.ancho.saturating_sub(self.width));
            // Nunca por encima de la barra del sistema: una ventana con el asa
            // debajo de la barra no se puede volver a coger.
            let ny = py
                .saturating_sub(ay)
                .max(TASKBAR_H)
                .min(p.alto.saturating_sub(self.height));
            if nx == self.x && ny == self.y {
                return false;
            }
            self.x = nx;
            self.y = ny;
            return true;
        }
        if self.resizing {
            let na = (px.saturating_sub(self.x) + 1)
                .max(self.min_w)
                .min(p.ancho.saturating_sub(self.x));
            let nl = (py.saturating_sub(self.y) + 1)
                .max(self.min_h)
                .min(p.alto.saturating_sub(self.y));
            if na == self.width && nl == self.height {
                return false;
            }
            self.width = na;
            self.height = nl;
            return true;
        }
        false
    }

    // -- Mover y encajar SIN raton ---------------------------------------
    //
    // ** El raton no es la unica mano.** Todo lo de arriba --agarrar, seguir al
    // puntero, la esquina-- exige un puntero, y hay dos momentos en los que no
    // lo hay: cuando la ventana se ha quedado con el asa fuera de la pantalla y
    // cuando el dueno esta escribiendo y no quiere soltar el teclado.
    //
    // Va en el marco y no en el compositor por la misma ley que el resto del
    // modulo: esto son rectangulos y topes. Que tecla lo dispara es politica, y
    // la politica vive arriba.

    /// Mueve la ventana un paso en un rumbo. `true` si de verdad se movio.
    ///
    /// Los topes son los MISMOS que los del arrastre --nunca por encima de la
    /// barra, nunca fuera del panel--, y eso no es economia de codigo sino la
    /// unica forma de que las dos manos dejen la ventana en sitios alcanzables
    /// por la otra. Una ventana que el teclado puede meter donde el raton no
    /// llega es una ventana perdida.
    ///
    /// Una MAXIMIZADA no se mueve, y sale gratis: ocupa el panel entero, asi que
    /// los topes la dejan donde estaba y esto devuelve `false` solo.
    pub(crate) fn push(&mut self, p: &bmo::Pantalla, heading: Heading) -> bool {
        if self.minimized {
            return false;
        }
        let (nx, ny) = match heading {
            Heading::Left => (self.x.saturating_sub(KEY_STEP), self.y),
            Heading::Right => (
                (self.x + KEY_STEP).min(p.ancho.saturating_sub(self.width)),
                self.y,
            ),
            Heading::Up => (
                self.x,
                self.y.saturating_sub(KEY_STEP).max(TASKBAR_H),
            ),
            Heading::Down => (
                self.x,
                (self.y + KEY_STEP)
                    .min(p.alto.saturating_sub(self.height))
                    .max(TASKBAR_H),
            ),
        };
        if nx == self.x && ny == self.y {
            return false;
        }
        self.x = nx;
        self.y = ny;
        true
    }

    /// Encaja la ventana contra un borde: media pantalla a un lado, el panel
    /// entero arriba, y abajo **deshace** el maximizado.
    ///
    /// === Por que `Down` no minimiza ===
    ///
    /// Porque seria una trampa sin salida. Hoy la barra del sistema solo tiene
    /// ficha para Ejecutar y para Datos --`chip_at(.., 2)`--, asi que una
    /// CABINA minimizada no tiene por donde volver: seguiria abierta, sin
    /// pintarse, y sin ningun control que la traiga. Un atajo que puede dejar
    /// una ventana inalcanzable no se da hasta que exista el camino de vuelta.
    ///
    /// Encajar a un lado **deja de ser estar maximizada**: se olvida la
    /// geometria guardada, porque el sitio al que hay que poder volver es este y
    /// no el de hace tres gestos.
    pub(crate) fn snap(&mut self, p: &bmo::Pantalla, heading: Heading) -> bool {
        if self.minimized {
            return false;
        }
        let usable_h = p.alto.saturating_sub(TASKBAR_H);
        let avg = (p.ancho / 2).max(self.min_w).min(p.ancho);
        let dest = match heading {
            Heading::Left => (0, TASKBAR_H, avg, usable_h),
            Heading::Right => (p.ancho - avg, TASKBAR_H, avg, usable_h),
            Heading::Up => (0, TASKBAR_H, p.ancho, usable_h),
            // `take` aunque no se vaya a usar: si no estaba maximizada no hay
            // nada que quitar, y si lo estaba deja de estarlo aqui mismo.
            Heading::Down => match self.saved.take() {
                Some(g) => g,
                None => return false,
            },
        };
        let old = (self.x, self.y, self.width, self.height);
        if dest == old {
            return false;
        }
        match heading {
            // Maximizar por teclado guarda el sitio igual que el boton: son el
            // mismo gesto por dos caminos, y tienen que deshacerse igual.
            Heading::Up => self.saved = Some(old),
            Heading::Left | Heading::Right => self.saved = None,
            Heading::Down => {}
        }
        let (nx, ny, na, nl) = dest;
        self.x = nx;
        self.y = ny;
        self.width = na;
        self.height = nl;
        true
    }

    /// Maximizar, o volver al tamano de antes. Devuelve la geometria VIEJA para
    /// que quien llama sepa que trozo de pantalla tiene que repintar.
    ///
    /// Maximizada NO es "pantalla completa": deja la barra del sistema a la
    /// vista. Una ventana que tapa la barra esconde las fichas de las demas, y
    /// entonces no hay forma de volver a ellas sin adivinar un atajo.
    pub(crate) fn toggle_maximized(&mut self, p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
        let old = (self.x, self.y, self.width, self.height);
        match self.saved.take() {
            Some((x, y, a, l)) => {
                self.x = x;
                self.y = y;
                self.width = a;
                self.height = l;
            }
            None => {
                self.saved = Some(old);
                self.x = 0;
                self.y = TASKBAR_H;
                self.width = p.ancho;
                self.height = p.alto.saturating_sub(TASKBAR_H);
            }
        }
        old
    }

    /// Recoloca la ventana si se ha quedado fuera del panel.
    ///
    /// Hace falta al restaurar una minimizada y al cambiar de resolucion: una
    /// geometria guardada puede haber dejado de ser valida mientras no se veia.
    pub(crate) fn fit(&mut self, p: &bmo::Pantalla) {
        self.width = self.width.min(p.ancho).max(self.min_w.min(p.ancho));
        self.height = self
            .height
            .min(p.alto.saturating_sub(TASKBAR_H))
            .max(self.min_h.min(p.alto));
        self.x = self.x.min(p.ancho.saturating_sub(self.width));
        self.y = self
            .y
            .max(TASKBAR_H)
            .min(p.alto.saturating_sub(self.height));
    }

    // -- Pintar ----------------------------------------------------------

    /// El cromo entero: sombra, borde, cuerpo, barra de titulo y los tres
    /// botones. Deja el interior listo para que el contenido se pinte encima.
    ///
    /// Los colores los trae quien llama porque **cada ventana tiene el suyo** y
    /// eso es informacion, no decoracion: el verde dice ESTRATOS y el azul dice
    /// el kernel antes de que nadie lea un titulo.
    pub(crate) fn paint_chrome(
        &self,
        p: &bmo::Pantalla,
        edge: u32,
        cuerpo: u32,
        title_bg: u32,
        acento: u32,
    ) {
        shadow(p, self.x, self.y, self.width, self.height);
        rounded_rect(p, self.x, self.y, self.width, self.height, edge);
        rounded_rect(p, self.x + 1, self.y + 1, self.width - 2, self.height - 2, cuerpo);

        // La barra, con la MISMA curva que la ventana o asomaria por fuera de
        // sus esquinas.
        for i in 0..RADIUS {
            let s = curve(i);
            p.rect(self.x + s, self.y + 1 + i, self.width - 2 * s, 1, title_bg);
        }
        p.rect(
            self.x + 1,
            self.y + 1 + RADIUS,
            self.width - 2,
            TITLE_H - 2 - RADIUS,
            title_bg,
        );
        p.rect(self.x + 1, self.y + TITLE_H - 1, self.width - 2, 1, acento);

        self.paint_buttons(p, title_bg);
        self.paint_corner_grip(p, edge);
    }

    /// Los tres, dibujados con rectangulos porque la fuente no trae sus glifos
    /// y meterlos en la fuente por tres iconos seria tocar el generador para
    /// algo que se dibuja con cuatro `rect`.
    pub(crate) fn paint_buttons(&self, p: &bmo::Pantalla, fondo: u32) {
        for (i, b) in [Button::Minimize, Button::Maximize, Button::Close].into_iter().enumerate() {
            let bx = self.boton_x(i as u32);
            let by = self.y + 2;
            let height = TITLE_H - 3;
            let realce = if self.hover == Some(b) {
                if b == Button::Close { CLOSE_HOVER } else { BTN_HOVER }
            } else {
                fondo
            };
            p.rect(bx, by, BTN_SIDE, height, realce);

            let cx = bx + BTN_SIDE / 2;
            let cy = by + height / 2;
            let ink = if self.hover == Some(b) && b == Button::Close { 0x00FF_FFFF } else { INK };
            match b {
                // Una raya. Es el icono universal y no necesita mas.
                Button::Minimize => p.rect(cx - 5, cy, 10, 1, ink),
                // Un cuadrado hueco; DOS solapados cuando ya esta maximizada,
                // que es como se dice "restaurar" en todas partes.
                Button::Maximize => {
                    if self.is_maximized() {
                        chrome_gap(p, cx - 5, cy - 3, 8, 8, ink);
                        chrome_gap(p, cx - 2, cy - 6, 8, 8, ink);
                    } else {
                        chrome_gap(p, cx - 4, cy - 4, 9, 9, ink);
                    }
                }
                // El aspa, pixel a pixel: no hay primitiva de linea diagonal y
                // dieciseis puntos no piden una.
                Button::Close => {
                    for k in 0..9u32 {
                        p.punto(cx - 4 + k, cy - 4 + k, ink);
                        p.punto(cx - 4 + k, cy + 4 - k, ink);
                    }
                }
            }
        }
    }

    /// Las tres rayitas en diagonal de la esquina. Un agarre invisible no
    /// existe: nadie prueba a estirar una ventana que no parece estirable.
    fn paint_corner_grip(&self, p: &bmo::Pantalla, color: u32) {
        if self.is_maximized() {
            return;
        }
        for k in 0..3u32 {
            let d = 4 + k * 4;
            p.rect(self.x + self.width - 4 - d, self.y + self.height - 6 - k * 4, d, 2, color);
        }
    }
}

/// Un rectangulo de un pixel de grosor. Cuatro `rect` y ya.
fn chrome_gap(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, color: u32) {
    p.rect(x, y, w, 1, color);
    p.rect(x, y + h - 1, w, 1, color);
    p.rect(x, y, 1, h, color);
    p.rect(x + w - 1, y, 1, h, color);
}
