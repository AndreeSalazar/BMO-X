//! El puntero del raton, dibujado en Ring 3.
//!
//! Su forma, su color y su contorno son decisiones de ASPECTO, y ninguna tiene
//! nada que hacer en Ring 0 — por eso el kernel entrega coordenadas y se aparta.

use bmo_userland as bmo;

// ── El cursor ───────────────────────────────────────────────────────────

pub(crate) const CUR_ANCHO: usize = 10;
pub(crate) const CUR_ALTO: usize = 16;
/// 0 = transparente, 1 = relleno, 2 = borde.
///
/// Borde oscuro alrededor del relleno claro: es lo que hace que una flecha se
/// vea igual de bien sobre un fondo claro que sobre uno oscuro. No es adorno,
/// es la razón de que todos los cursores del mundo tengan contorno.
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
pub(crate) const CUR_RELLENO: u32 = 0x00FF_FFFF;
pub(crate) const CUR_BORDE: u32 = 0x0000_0000;

fn dibujar_cursor(p: &bmo::Pantalla, x: u32, y: u32) {
    for (fila, linea) in FLECHA.iter().enumerate() {
        for (col, &v) in linea.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let color = if v == 1 { CUR_RELLENO } else { CUR_BORDE };
            p.punto(x + col as u32, y + fila as u32, color);
        }
    }
}

/// **Lo que hay debajo del cursor**, guardado píxel a píxel.
///
/// ═══ Por qué esto y no preguntarle a la escena ═══
///
/// Antes el cursor se borraba repintando `color_escena`: "¿qué debería haber
/// aquí?". Eso vale mientras la escena conozca **todo** lo que hay en pantalla,
/// y dejó de valer en cuanto aparecieron ventanas que no están en ese modelo —
/// la consola de datos y el conmutador. Pasar el ratón por encima de ellas
/// dejaba un rastro de agujeros con el color del fondo del escritorio, porque
/// la escena contestaba con lo que había *antes* de que esa ventana existiera.
///
/// Con `save-under` la pregunta desaparece: no hace falta saber qué hay debajo
/// porque se guarda. Son 160 píxeles —640 bytes de pila— y funciona igual con
/// las ventanas de hoy y con las que vengan, sin que ninguna tenga que
/// registrarse en ningún sitio.
///
/// ═══ El precio, dicho entero ═══
///
/// Lo guardado **caduca** si alguien pinta ahí mientras el cursor está puesto:
/// devolverlo taparía lo nuevo con lo viejo. Por eso el compositor lo quita al
/// PRINCIPIO del fotograma y lo pone al FINAL, con todo el dibujo en medio —
/// que es la disciplina de cualquier cursor por software.
pub(crate) struct Bajo {
    px: [u32; CUR_ANCHO * CUR_ALTO],
    x: u32,
    y: u32,
    puesto: bool,
}

impl Bajo {
    pub(crate) const fn nuevo() -> Self {
        Self {
            px: [0; CUR_ANCHO * CUR_ALTO],
            x: 0,
            y: 0,
            puesto: false,
        }
    }

    /// Guarda lo que hay y dibuja el cursor encima. Al FINAL del fotograma.
    pub(crate) fn poner(&mut self, p: &bmo::Pantalla, x: u32, y: u32) {
        if self.puesto {
            return;
        }
        for fila in 0..CUR_ALTO {
            for col in 0..CUR_ANCHO {
                self.px[fila * CUR_ANCHO + col] = p.leer(x + col as u32, y + fila as u32);
            }
        }
        self.x = x;
        self.y = y;
        self.puesto = true;
        dibujar_cursor(p, x, y);
    }

    /// Devuelve lo guardado. Al PRINCIPIO del fotograma, antes de pintar nada.
    /// Si no estaba puesto no hace nada, así que se puede llamar siempre.
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

