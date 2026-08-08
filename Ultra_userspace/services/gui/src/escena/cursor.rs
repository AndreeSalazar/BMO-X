//! El puntero del raton, dibujado en Ring 3.
//!
//! Su forma, su color y su contorno son decisiones de ASPECTO, y ninguna tiene
//! nada que hacer en Ring 0 -- por eso el kernel entrega coordenadas y se aparta.

use bmo_userland as bmo;

// -- El cursor -----------------------------------------------------------

pub(crate) const CUR_ANCHO: usize = 10;
pub(crate) const CUR_ALTO: usize = 16;
/// 0 = transparente, 1 = relleno, 2 = borde.
///
/// Borde oscuro alrededor del relleno claro: es lo que hace que una flecha se
/// vea igual de bien sobre un fondo claro que sobre uno oscuro. No es adorno,
/// es la razon de que todos los cursores del mundo tengan contorno.
pub(crate) const FLECHA: [[u8; CUR_ANCHO]; CUR_ALTO] = [
    [2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 2, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 2, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 1, 2, 0, 0, 0, 0, 0, 0],
    [2, 1, 1, 1, 2, 0, 0, 0, 0, 0],
    [2, 1, 1, 1, 1, 2, 0, 0, 0, 0],
    [2, 1, 1, 1, 1, 1, 2, 0, 0, 0],
    [2, 1, 1, 1, 1, 1, 1, 2, 0, 0],
    [2, 1, 1, 1, 1, 1, 1, 1, 2, 0],
    [2, 1, 1, 1, 1, 1, 2, 2, 2, 2],
    [2, 1, 1, 2, 1, 1, 2, 0, 0, 0],
    [2, 1, 2, 0, 2, 1, 1, 2, 0, 0],
    [2, 2, 0, 0, 2, 1, 1, 2, 0, 0],
    [2, 0, 0, 0, 0, 2, 1, 1, 2, 0],
    [0, 0, 0, 0, 0, 2, 1, 2, 0, 0],
    [0, 0, 0, 0, 0, 0, 2, 2, 0, 0],
];
/// **La barra de texto.** Donde se puede escribir.
///
/// No es adorno: es la unica forma que tiene el escritorio de decir "aqui
/// dentro el clic coloca el cursor de escritura" **antes** de que lo intentes.
/// Un campo de texto que se ve igual que el fondo obliga a probar.
pub(crate) const BARRA: [[u8; CUR_ANCHO]; CUR_ALTO] = [
    [0, 0, 2, 2, 2, 2, 2, 2, 0, 0],
    [0, 0, 2, 1, 1, 1, 1, 2, 0, 0],
    [0, 0, 2, 2, 1, 1, 2, 2, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 0, 2, 1, 1, 2, 0, 0, 0],
    [0, 0, 2, 2, 1, 1, 2, 2, 0, 0],
    [0, 0, 2, 1, 1, 1, 1, 2, 0, 0],
    [0, 0, 2, 2, 2, 2, 2, 2, 0, 0],
];

/// **La mano.** Esto se pulsa.
///
/// La usa lo que reacciona a un clic y no lo parece: los botones de la
/// calculadora. Un boton dibujado es una promesa; la mano es la que la
/// confirma sin gastar un clic en comprobarlo.
pub(crate) const MANO: [[u8; CUR_ANCHO]; CUR_ALTO] = [
    [0, 0, 0, 2, 2, 0, 0, 0, 0, 0],
    [0, 0, 2, 1, 1, 2, 0, 0, 0, 0],
    [0, 0, 2, 1, 1, 2, 0, 0, 0, 0],
    [0, 0, 2, 1, 1, 2, 0, 0, 0, 0],
    [0, 0, 2, 1, 1, 2, 2, 2, 0, 0],
    [0, 0, 2, 1, 1, 1, 1, 1, 2, 0],
    [0, 2, 2, 1, 1, 1, 1, 1, 1, 2],
    [2, 1, 2, 1, 1, 1, 1, 1, 1, 2],
    [2, 1, 1, 1, 1, 1, 1, 1, 1, 2],
    [0, 2, 1, 1, 1, 1, 1, 1, 1, 2],
    [0, 2, 1, 1, 1, 1, 1, 1, 1, 2],
    [0, 0, 2, 1, 1, 1, 1, 1, 1, 2],
    [0, 0, 2, 1, 1, 1, 1, 1, 2, 0],
    [0, 0, 0, 2, 1, 1, 1, 1, 2, 0],
    [0, 0, 0, 2, 1, 1, 1, 1, 2, 0],
    [0, 0, 0, 0, 2, 2, 2, 2, 0, 0],
];

/// Que esta diciendo el puntero ahora mismo.
///
/// * **La forma del cursor es informacion, no decoracion.** Es lo unico del
/// escritorio que contesta "que pasa si pulso aqui?" **sin que haya que
/// pulsar**. Un sistema con una sola forma obliga a probar cada sitio, y probar
/// donde no se debe es exactamente lo que un puntero existe para evitar.
///
/// Las tres son las que de verdad significan algo distinto aqui. No hay reloj
/// de espera a proposito: nada de este escritorio bloquea, asi que un cursor
/// de "espera" seria una forma que nunca es verdad.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Forma {
    /// Lo normal: senalar y elegir.
    Flecha,
    /// Sobre un campo donde se escribe.
    Texto,
    /// Sobre algo que reacciona al clic.
    Mano,
}

impl Forma {
    fn mapa(self) -> &'static [[u8; CUR_ANCHO]; CUR_ALTO] {
        match self {
            Forma::Flecha => &FLECHA,
            Forma::Texto => &BARRA,
            Forma::Mano => &MANO,
        }
    }
}

pub(crate) const CUR_RELLENO: u32 = 0x00FF_FFFF;
pub(crate) const CUR_BORDE: u32 = 0x0000_0000;

fn dibujar_cursor(p: &bmo::Pantalla, x: u32, y: u32, forma: Forma) {
    for (fila, linea) in forma.mapa().iter().enumerate() {
        for (col, &v) in linea.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let color = if v == 1 { CUR_RELLENO } else { CUR_BORDE };
            p.punto(x + col as u32, y + fila as u32, color);
        }
    }
}

/// **Lo que hay debajo del cursor**, guardado pixel a pixel.
///
/// === Por que esto y no preguntarle a la escena ===
///
/// Antes el cursor se borraba repintando `color_escena`: "que deberia haber
/// aqui?". Eso vale mientras la escena conozca **todo** lo que hay en pantalla,
/// y dejo de valer en cuanto aparecieron ventanas que no estan en ese modelo --
/// la consola de datos y el conmutador. Pasar el raton por encima de ellas
/// dejaba un rastro de agujeros con el color del fondo del escritorio, porque
/// la escena contestaba con lo que habia *antes* de que esa ventana existiera.
///
/// Con `save-under` la pregunta desaparece: no hace falta saber que hay debajo
/// porque se guarda. Son 160 pixeles --640 bytes de pila-- y funciona igual con
/// las ventanas de hoy y con las que vengan, sin que ninguna tenga que
/// registrarse en ningun sitio.
///
/// === El precio, dicho entero ===
///
/// Lo guardado **caduca** si alguien pinta ahi mientras el cursor esta puesto:
/// devolverlo taparia lo nuevo con lo viejo. Por eso el compositor lo quita al
/// PRINCIPIO del fotograma y lo pone al FINAL, con todo el dibujo en medio --
/// que es la disciplina de cualquier cursor por software.
pub(crate) struct Bajo {
    px: [u32; CUR_ANCHO * CUR_ALTO],
    x: u32,
    y: u32,
    puesto: bool,
    /// Con que forma esta dibujado ahora mismo. Hace falta guardarla para poder
    /// notar que cambio sin que el puntero se mueva.
    forma: Forma,
}

impl Bajo {
    pub(crate) const fn nuevo() -> Self {
        Self {
            px: [0; CUR_ANCHO * CUR_ALTO],
            x: 0,
            y: 0,
            puesto: false,
            forma: Forma::Flecha,
        }
    }

    /// Guarda lo que hay y dibuja el cursor encima. Al FINAL del fotograma.
    ///
    /// ** **LA SINCRONIZACION DE ANTES DE LEER, Y POR QUE FALTABA.**
    ///
    /// Este es el unico sitio de todo el compositor que **lee** la pantalla. Y
    /// desde que el framebuffer se mapea en **write-combining** (`952681c7`),
    /// leerlo sin barrera no devuelve lo que acabas de pintar: devuelve lo que
    /// habia **antes**.
    ///
    /// * Con el doble bufer activo, `sincronizar_lectura` **no hace nada** -- y
    /// eso es lo mejor que le puede pasar a esta linea. Se lee del lienzo, que
    /// es RAM normal y cacheada: el problema no se arregla, deja de existir.
    ///
    /// Con WC el CPU acumula las escrituras en un bufer y las suelta cuando se
    /// llena. Una lectura de memoria WC **no esta ordenada** contra esas
    /// escrituras pendientes -- el manual lo dice y no hay forma de saltarselo.
    /// Asi que la secuencia del fotograma era:
    ///
    /// ```text
    ///   1. quitar        -> escribe (al bufer)
    ///   2. pintar todo   -> escribe (al bufer)
    ///   3. poner: LEER   -> ve la pantalla de HACE UN FOTOGRAMA
    ///   4. vaciar        -> sfence, ahora si llega todo
    /// ```
    ///
    /// El paso 3 guardaba pixeles caducados, y el `quitar` del fotograma
    /// siguiente los devolvia **encima de lo nuevo**: un rectangulo de 10x16
    /// con contenido viejo persiguiendo al puntero. Eso es el ghosting.
    ///
    /// Es el Ep. 20 otra vez y por el otro lado. Alli se descubrio que **la
    /// pantalla** no veia nuestras escrituras sin `sfence`; lo que nadie miro es
    /// que **nosotros** tampoco las vemos. La barrera hace falta en los dos
    /// sentidos, y va aqui dentro y no en quien llama: la invariante es de la
    /// lectura, no del sitio desde donde se pide.
    pub(crate) fn poner(&mut self, p: &bmo::Pantalla, x: u32, y: u32, forma: Forma) {
        if self.puesto {
            // * Y si la FORMA cambio, hay que redibujar aunque no se haya
            // movido: pasar del raton quieto sobre el escritorio al campo de
            // texto no mueve un pixel el puntero, y aun asi tiene que cambiar.
            //
            // Se quita y se vuelve a poner en vez de dibujar encima: las tres
            // formas ocupan pixeles distintos, asi que pintar la nueva sobre la
            // vieja dejaria los trozos que la nueva no cubre.
            if self.forma == forma {
                return;
            }
            self.quitar(p);
        }
        self.forma = forma;
        p.sincronizar_lectura();
        for fila in 0..CUR_ALTO {
            for col in 0..CUR_ANCHO {
                self.px[fila * CUR_ANCHO + col] = p.leer(x + col as u32, y + fila as u32);
            }
        }
        self.x = x;
        self.y = y;
        self.puesto = true;
        dibujar_cursor(p, x, y, forma);
    }

    /// Devuelve lo guardado. Al PRINCIPIO del fotograma, antes de pintar nada.
    /// Si no estaba puesto no hace nada, asi que se puede llamar siempre.
    pub(crate) fn quitar(&mut self, p: &bmo::Pantalla) {
        if !self.puesto {
            return;
        }
        for fila in 0..CUR_ALTO {
            for col in 0..CUR_ANCHO {
                p.punto(
                    self.x + col as u32,
                    self.y + fila as u32,
                    self.px[fila * CUR_ANCHO + col],
                );
            }
        }
        self.puesto = false;
    }
}

