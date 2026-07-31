//! La rejilla de salida y su historial con scroll.
//!
//! 200 filas guardadas, ventana de 16. Lo que sale por arriba **no se pierde**:
//! se mira con la rueda o con RePag/AvPag.

use bmo_userland as bmo;

use super::*;

// ── La salida ───────────────────────────────────────────────────────────

/// Rejilla de caracteres con desplazamiento. Es lo mínimo que se puede llamar
/// terminal: sin colores por celda, sin secuencias de escape, sin scrollback
/// más allá de lo que se ve.
///
/// Deliberado. Un emulador de terminal completo —ANSI, cursor direccionable,
/// regiones— es una pila entera, y hoy lo único que hay al otro lado son
/// programas que escriben líneas. Cuando algo pida más, se añade; adivinarlo
/// ahora sería escribir código que nadie ejercita.
// ── La tinta ────────────────────────────────────────────────────────────
//
// El color va POR LÍNEA y no por celda. Una rejilla de atributos costaría
// 88x16 bytes para decir dieciséis veces lo mismo: este terminal escribe
// líneas enteras —el eco de un comando, un mensaje de error, la salida de un
// programa— y nunca media línea de un color y media de otro.
//
// Antes el único color lo daba `f == s.fila`, o sea "la fila donde está el
// cursor". Eso pinta de otro color la ÚLTIMA línea escrita, que casi nunca es
// la que te interesa: si un programa imprime diez líneas, la marcada es la
// décima y no el comando que lo lanzó.

/// Salida corriente de un programa.
pub(crate) const TINTA_NORMAL: u8 = 0;
/// El comando que escribiste. Es el ancla para leer hacia abajo.
pub(crate) const TINTA_ECO: u8 = 1;
/// Algo salió mal.
pub(crate) const TINTA_MAL: u8 = 2;
/// Algo salió bien y merece verse.
pub(crate) const TINTA_BIEN: u8 = 3;

pub(crate) fn color_tinta(t: u8) -> u32 {
    match t {
        TINTA_ECO => SAL_ECO,
        TINTA_MAL => TEXTO_MAL,
        TINTA_BIEN => TEXTO_BIEN,
        _ => SAL_TEXTO,
    }
}

pub(crate) struct Salida {
    /// El historial entero. Se ESCRIBE aquí; la ventana visible es un trozo.
    pub(crate) celdas: [[u8; SAL_COLS]; SAL_HIST],
    /// Con qué color se pinta cada fila.
    pub(crate) tinta: [u8; SAL_HIST],
    /// Con qué color se escribe a partir de ahora.
    pub(crate) tinta_actual: u8,
    pub(crate) fila: usize,
    pub(crate) col: usize,
    /// Cuántas filas se ha subido el usuario. 0 = pegado abajo, viendo lo
    /// último. Escribir algo nuevo vuelve abajo, como cualquier terminal: si no,
    /// el programa hablaría y nadie lo vería.
    pub(crate) vista: usize,
    /// Hay algo nuevo que pintar. Repintar la rejilla entera cada fotograma
    /// serían 88x16 glifos por vuelta sobre memoria de vídeo sin caché.
    pub(crate) sucia: bool,
}

impl Salida {
    pub(crate) fn nueva() -> Self {
        Self {
            celdas: [[b' '; SAL_COLS]; SAL_HIST],
            tinta: [TINTA_NORMAL; SAL_HIST],
            vista: 0,
            tinta_actual: TINTA_NORMAL,
            fila: 0,
            col: 0,
            sucia: true,
        }
    }

    /// A partir de aquí se escribe con esta tinta. La fila en curso se marca
    /// ya: quien cambia el color antes de escribir espera que valga para lo
    /// que va a escribir, no para lo siguiente.
    pub(crate) fn con_tinta(&mut self, t: u8) {
        self.tinta_actual = t;
        self.tinta[self.fila] = t;
    }

    /// Sube todo una línea y deja la última en blanco. La tinta viaja con su
    /// línea: si no, al desplazarse el color se quedaría marcando la fila de
    /// otro.
    pub(crate) fn desplazar(&mut self) {
        for f in 1..SAL_HIST {
            self.celdas[f - 1] = self.celdas[f];
            self.tinta[f - 1] = self.tinta[f];
        }
        self.celdas[SAL_HIST - 1] = [b' '; SAL_COLS];
        self.tinta[SAL_HIST - 1] = self.tinta_actual;
    }

    /// Sube o baja la ventana. Positivo = hacia atrás en el tiempo.
    ///
    /// Se topa sola en los dos extremos: no se puede subir más allá de lo
    /// guardado ni bajar más allá de lo último. Un scroll que se sale enseña
    /// filas en blanco y parece que se ha perdido todo.
    pub(crate) fn mover_vista(&mut self, filas: i32) {
        let tope = SAL_HIST - SAL_ROWS;
        let nueva = (self.vista as i32 + filas).clamp(0, tope as i32) as usize;
        if nueva != self.vista {
            self.vista = nueva;
            self.sucia = true;
        }
    }

    pub(crate) fn salto(&mut self) {
        self.col = 0;
        // Escribir devuelve la vista abajo: si no, el programa hablaria y el
        // usuario seguiria mirando el pasado sin enterarse.
        self.vista = 0;
        if self.fila + 1 >= SAL_HIST {
            self.desplazar();
        } else {
            self.fila += 1;
        }
        self.tinta[self.fila] = self.tinta_actual;
        self.sucia = true;
    }

    pub(crate) fn byte(&mut self, b: u8) {
        match b {
            b'\n' => self.salto(),
            // El retorno de carro solo, sin avance: se ignora. Un programa que
            // escribe "\r\n" no debe producir dos saltos.
            b'\r' => {}
            // Tabulador a la siguiente parada de 8.
            b'\t' => {
                let siguiente = (self.col / 8 + 1) * 8;
                while self.col < siguiente.min(SAL_COLS) {
                    self.celdas[self.fila][self.col] = b' ';
                    self.col += 1;
                }
                if self.col >= SAL_COLS {
                    self.salto();
                }
                self.sucia = true;
            }
            // Los no imprimibles se tiran en vez de dibujarse como basura.
            c if c < 0x20 => {}
            c => {
                if self.col >= SAL_COLS {
                    self.salto();
                }
                self.celdas[self.fila][self.col] = c;
                self.col += 1;
                self.sucia = true;
            }
        }
    }

    pub(crate) fn texto(&mut self, s: &[u8]) {
        for &b in s {
            self.byte(b);
        }
    }

    pub(crate) fn limpiar(&mut self) {
        self.celdas = [[b' '; SAL_COLS]; SAL_HIST];
        self.tinta = [TINTA_NORMAL; SAL_HIST];
        self.vista = 0;
        self.tinta_actual = TINTA_NORMAL;
        self.fila = 0;
        self.col = 0;
        self.sucia = true;
    }

    // ── Lo que hace falta para PINTAR un informe ────────────────────────
    //
    // Todo esto es de Ring 3 a propósito. El kernel contesta enteros crudos y
    // no sabe nada de KiB, de porcentajes ni de barras: un kernel que decide
    // cómo se ve un número es un kernel con opiniones sobre la interfaz.

    /// Un entero en decimal.
    pub(crate) fn dec(&mut self, mut v: u64) {
        if v == 0 {
            self.byte(b'0');
            return;
        }
        let mut d = [0u8; 20];
        let mut n = 0;
        while v > 0 {
            d[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
        while n > 0 {
            n -= 1;
            self.byte(d[n]);
        }
    }

    /// Un entero alineado a la DERECHA en `ancho` columnas. Es lo que hace que
    /// una columna de números se lea de un vistazo en vez de bailar.
    pub(crate) fn dec_der(&mut self, v: u64, ancho: usize) {
        let mut cifras = 1;
        let mut t = v;
        while t >= 10 {
            t /= 10;
            cifras += 1;
        }
        for _ in cifras..ancho {
            self.byte(b' ');
        }
        self.dec(v);
    }

    /// Bytes en la unidad que toca, con un decimal: `15.9 GiB`.
    ///
    /// Se hace con enteros, dividiendo y sacando el resto — igual que el
    /// decimal de COBOL. Aquí no hay coma flotante y no hace falta.
    pub(crate) fn tamano(&mut self, bytes: u64) {
        const U: [&[u8]; 5] = [b"B", b"KiB", b"MiB", b"GiB", b"TiB"];
        let mut i = 0;
        let mut v = bytes;
        while v >= 1024 && i < 4 {
            v /= 1024;
            i += 1;
        }
        // El primer decimal, sin flotantes: el resto de la última división.
        let frac = if i == 0 {
            0
        } else {
            let divisor = 1u64 << (10 * i);
            ((bytes % divisor) * 10 / divisor) % 10
        };
        self.dec(v);
        if i > 0 {
            self.byte(b'.');
            self.byte(b'0' + frac as u8);
        }
        self.byte(b' ');
        self.texto(U[i]);
    }

    /// Un porcentaje entero, sin dividir por cero.
    pub(crate) fn pct(&mut self, parte: u64, total: u64) {
        if total == 0 {
            self.texto(b"--");
        } else {
            self.dec(parte.saturating_mul(100) / total);
        }
        self.byte(b'%');
    }

    /// Una barra de ocupación de `ancho` columnas: `[####------]`.
    ///
    /// Con caracteres y no con píxeles porque la salida ES una rejilla de
    /// caracteres: una barra dibujada aparte se quedaría quieta cuando el log
    /// desplaza, y una barra que miente de sitio es peor que no tenerla.
    pub(crate) fn barra(&mut self, parte: u64, total: u64, ancho: usize) {
        let llenas = if total == 0 {
            0
        } else {
            (parte.saturating_mul(ancho as u64) / total) as usize
        };
        self.byte(b'[');
        for i in 0..ancho {
            self.byte(if i < llenas { b'#' } else { b'-' });
        }
        self.byte(b']');
    }
}


pub(crate) fn pintar_salida(p: &bmo::Pantalla, c: &Caja, s: &Salida) {
    // Fondo entero de la rejilla y encima las filas. Es un rectángulo de
    // 704x256 px: nada comparado con la pantalla, y evita tener que llevar la
    // cuenta de qué celda cambió.
    p.rect(
        c.salida_x,
        c.salida_y,
        SAL_COLS as u32 * bmo::GLIFO_ANCHO,
        c.salida_alto(),
        CAJA_FONDO,
    );
    // La ventana: las ultimas SAL_ROWS filas, corridas hacia atras por `vista`.
    let base = SAL_HIST - SAL_ROWS - s.vista;
    for f in 0..SAL_ROWS {
        let color = color_tinta(s.tinta[base + f]);
        p.texto_bytes(
            c.salida_x,
            c.salida_y + f as u32 * bmo::GLIFO_ALTO,
            &s.celdas[base + f],
            color,
        );
    }
    // Y si se ha subido, DECIRLO. Una ventana que ensena el pasado sin avisar
    // se confunde con una que se quedo colgada.
    if s.vista > 0 {
        let x = c.salida_x + (SAL_COLS as u32 - 18) * bmo::GLIFO_ANCHO;
        p.texto(x, c.salida_y, "-- historial --", ACENTO);
    }
}
