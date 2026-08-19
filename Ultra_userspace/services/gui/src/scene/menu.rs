//! **EL MENU DEL CLIC DERECHO** -- lo que se puede hacer con lo que senalas.
//!
//! === Por que existe, y por que no es una lista de botones ===
//!
//! El explorador ya sabia mirar y navegar. Lo que no habia era **como se pide
//! una accion sobre algo concreto**: borrar, renombrar, verificar. Las ordenes
//! existen desde el 19-08 y habia que escribirlas enteras a mano, incluido el
//! nombre que tienes delante y senalado.
//!
//! Una barra de herramientas arriba habria sido lo otro, y es peor: sus botones
//! valen para lo que este seleccionado --que a veces es nada-- y ocupan sitio
//! siempre. El menu aparece donde miras y **se construye a partir de lo que hay
//! debajo del puntero**.
//!
//! === ** LO QUE ELIGES SE ESCRIBE EN LA CONSOLA, NO EN UN DIALOGO ===
//!
//! Esa es la decision de este fichero, y no es pereza por no hacer cuadros de
//! dialogo. Es la misma que ya tomo el lanzador de iconos:
//!
//! > pulsar un icono y teclear su nombre son la misma cosa, y por eso comparten
//! > camino entero.
//!
//! Un `borrar` del menu escribe `borra nota.txt` en la consola y lo ejecuta. Un
//! `renombrar` escribe `renombra nota.txt ` y **te deja el cursor puesto**,
//! porque falta el nombre nuevo y eso solo lo sabes tu.
//!
//! Tres cosas se ganan a la vez: no hay un segundo sitio donde se escriba en el
//! disco --la consola sigue siendo el unico--, se ve la orden que se acaba de
//! ejecutar, y **se aprende el terminal usando el raton**.
//!
//! === Y por que NO hay F5 ===
//!
//! Lo pregunto el dueno y tiene razon en que suena raro que falte. En Windows
//! el F5 existe porque **el shell no se entera** de que el disco cambio: lo
//! cambio otro proceso, y la ventana ensena lo de antes hasta que alguien la
//! empuja.
//!
//! Aqui el que escribe ES el compositor, asi que sabe exactamente cuando pasa:
//! cada gesto manda `recargar` al terminar. Un F5 seria un boton para pedir algo
//! que ya se ha hecho.
//!
//! === El menu se hace del CONTEXTO ===
//!
//! No hay una lista fija con opciones apagadas. Se construyen las que valen para
//! lo que hay debajo:
//!
//! ```text
//!   sobre una CARPETA   entrar, renombrar, borrar
//!   sobre un ARCHIVO    verificar firma, renombrar, borrar
//!   sobre el FONDO      carpeta nueva, fichero nuevo, subir, sellar
//! ```
//!
//! Una opcion que no se puede hacer no se pinta en gris: no se pinta. Un menu
//! con la mitad de las entradas apagadas obliga a leerlas todas para encontrar
//! las dos que sirven.

use bmo_userland as bmo;

use super::zonas::Zona;
use super::{INK, INK_BAD, INK_DIM};

/// Alto de una entrada.
const FILA_H: u32 = 22;
const ANCHO: u32 = 168;
/// Lo mas que puede tener un menu de los de aqui.
const MAX: usize = 6;

/// Sobre que cayo el clic derecho.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sobre {
    /// El hijo `i` del nodo actual.
    Hijo(usize),
    /// El fondo: el sitio donde estas.
    Aqui,
}

/// Que hace una entrada cuando se pulsa.
///
/// * `Orden` lleva la orden ENTERA y se ejecuta; `Empezar` lleva el principio y
/// deja el cursor puesto. La diferencia es si falta algo que solo sabe la
/// persona -- y por eso son dos variantes y no una bandera: leyendo la tabla de
/// abajo se ve cual te va a preguntar antes de pulsarla.
#[derive(Clone, Copy)]
pub(crate) enum Hace {
    /// Se escribe en la consola y se ejecuta.
    Orden,
    /// Se escribe en la consola y se espera a que termines de teclear.
    Empezar,
    /// No pasa por la consola: mueve el cursor y ya.
    Entrar,
    Subir,
    Verificar,
}

#[derive(Clone, Copy)]
pub(crate) struct Entrada {
    pub texto: &'static str,
    /// El verbo que se escribe en la consola, si pasa por ella.
    pub verbo: &'static str,
    pub hace: Hace,
    /// Destruye algo? Se pinta distinto. Ver la nota de `paint`.
    pub pesa: bool,
}

pub(crate) struct Menu {
    pub(crate) visible: bool,
    pub(crate) sobre: Sobre,
    x: u32,
    y: u32,
    entradas: [Entrada; MAX],
    n: usize,
}

const VACIA: Entrada = Entrada { texto: "", verbo: "", hace: Hace::Orden, pesa: false };

impl Menu {
    pub(crate) const fn nuevo() -> Self {
        Self {
            visible: false,
            sobre: Sobre::Aqui,
            x: 0,
            y: 0,
            entradas: [VACIA; MAX],
            n: 0,
        }
    }

    /// **Abre el menu en `(x, y)` con lo que se puede hacer sobre `sobre`.**
    pub(crate) fn abrir(&mut self, x: u32, y: u32, sobre: Sobre) {
        self.visible = true;
        self.sobre = sobre;
        self.x = x;
        self.y = y;
        self.n = 0;
        match sobre {
            Sobre::Hijo(i) => {
                let es_dir = bmo::estratos::hijo_tipo(i as u64) == bmo::estratos::DIRECTORIO;
                if es_dir {
                    self.mete("entrar", "", Hace::Entrar, false);
                } else {
                    // Solo para archivos: un directorio tambien puede llevar
                    // `:firma`, pero comprobarla es leer su lista de entradas y
                    // eso contesta a otra pregunta.
                    self.mete("verificar firma", "", Hace::Verificar, false);
                }
                self.mete("renombrar", "renombra", Hace::Empezar, false);
                self.mete("borrar", "borra", Hace::Orden, true);
            }
            Sobre::Aqui => {
                self.mete("carpeta nueva", "carpeta", Hace::Empezar, false);
                self.mete("fichero nuevo", "nuevo", Hace::Empezar, false);
                if bmo::estratos::hondo() > 0 {
                    self.mete("subir", "", Hace::Subir, false);
                }
                self.mete("sellar", "sella", Hace::Orden, true);
            }
        }
    }

    fn mete(&mut self, texto: &'static str, verbo: &'static str, hace: Hace, pesa: bool) {
        if self.n < MAX {
            self.entradas[self.n] = Entrada { texto, verbo, hace, pesa };
            self.n += 1;
        }
    }

    pub(crate) fn cerrar(&mut self) {
        self.visible = false;
    }

    /// El rectangulo que ocupa. Lo necesita quien repinta el fondo al cerrarlo.
    pub(crate) fn caja(&self) -> Zona {
        Zona { x: self.x, y: self.y, w: ANCHO, h: self.n as u32 * FILA_H + 2 }
    }

    /// Sobre que entrada cayo el puntero.
    pub(crate) fn entrada_en(&self, px: u32, py: u32) -> Option<Entrada> {
        if !self.visible {
            return None;
        }
        let c = self.caja();
        if !c.contiene(px, py) {
            return None;
        }
        let k = ((py - c.y) / FILA_H) as usize;
        if k < self.n {
            Some(self.entradas[k])
        } else {
            None
        }
    }
}

/// Pinta el menu. Va EL ULTIMO de todo: es lo unico que puede taparlo todo.
pub(crate) fn paint(p: &bmo::Pantalla, m: &Menu, fondo: u32, borde: u32) {
    if !m.visible || m.n == 0 {
        return;
    }
    let c = m.caja();
    // La sombra primero, un par de pixeles abajo y a la derecha: sin ella el
    // menu se lee como parte del panel que tapa.
    p.rect(c.x + 3, c.y + 3, c.w, c.h, 0x000B_100E);
    p.rect(c.x, c.y, c.w, c.h, borde);
    p.rect(c.x + 1, c.y + 1, c.w - 2, c.h - 2, fondo);

    let mut y = c.y + 1;
    for e in m.entradas.iter().take(m.n) {
        // ** LO QUE DESTRUYE VA EN OTRO COLOR, y solo eso.
        //
        // `borrar` y `sellar` escriben en el disco; el resto no. Es la misma
        // regla que el aspa roja del marco: el color de aviso aparece donde
        // algo cambia de verdad, y por eso significa algo cuando aparece.
        let ink = if e.pesa { INK_BAD } else { INK };
        p.texto(c.x + 10, y + (FILA_H - bmo::GLIFO_ALTO) / 2, e.texto, ink);
        y += FILA_H;
    }
    // Y la pista de que esto escribe la orden abajo, no la hace por su cuenta.
    p.texto(c.x + 10, c.y + c.h + 4, "-> a la consola", INK_DIM);
}
