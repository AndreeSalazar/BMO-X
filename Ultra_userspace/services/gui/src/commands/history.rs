//! El historial de COMANDOS -- el de la flecha arriba.
//!
//! No confundir con el historial de SALIDA (`scene::output`): aquel guarda lo
//! que los programas imprimieron, este lo que tu escribiste.

use crate::scene::PATH_MAX;

/// Historial de lo escrito. Lo que un terminal sin esto obliga a hacer es
/// reteclear la ruta entera cada vez que te equivocas en una letra -- y eso es
/// justo lo que pasaba: seis intentos de `apps/COBOL.bex` escritos a mano.
///
/// Anillo de ocho. No guarda duplicados seguidos: repetir `ls` cinco veces no
/// debe llenar el historial de `ls`.
pub(crate) struct History {
    pub(crate) lineas: [[u8; PATH_MAX]; 8],
    pub(crate) lengths: [usize; 8],
    /// Cuantas hay guardadas (tope 8).
    pub(crate) n: usize,
    /// Por donde va el paseo con las flechas. `n` = "estoy escribiendo algo
    /// nuevo", que es distinto de "estoy en la mas reciente".
    pub(crate) cursor: usize,
}

impl History {
    pub(crate) fn new() -> Self {
        Self { lineas: [[0u8; PATH_MAX]; 8], lengths: [0; 8], n: 0, cursor: 0 }
    }

    pub(crate) fn push(&mut self, line: &[u8]) {
        if line.is_empty() {
            return;
        }
        if self.n > 0 && &self.lineas[self.n - 1][..self.lengths[self.n - 1]] == line {
            self.cursor = self.n;
            return;
        }
        if self.n == self.lineas.len() {
            // Lleno: se va la mas vieja.
            for i in 1..self.n {
                self.lineas[i - 1] = self.lineas[i];
                self.lengths[i - 1] = self.lengths[i];
            }
            self.n -= 1;
        }
        let k = line.len().min(PATH_MAX);
        self.lineas[self.n][..k].copy_from_slice(&line[..k]);
        self.lengths[self.n] = k;
        self.n += 1;
        self.cursor = self.n;
    }

    /// Hacia atras. Devuelve el nuevo largo de la linea, o `None` si no hay.
    pub(crate) fn back(&mut self, dst: &mut [u8; PATH_MAX]) -> Option<usize> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        let k = self.lengths[self.cursor];
        dst[..k].copy_from_slice(&self.lineas[self.cursor][..k]);
        Some(k)
    }

    /// Hacia adelante. Al pasar de la mas reciente se vuelve a linea en
    /// blanco, que es lo que espera cualquiera que haya usado un shell.
    pub(crate) fn forward(&mut self, dst: &mut [u8; PATH_MAX]) -> Option<usize> {
        if self.cursor + 1 > self.n {
            return None;
        }
        self.cursor += 1;
        if self.cursor == self.n {
            return Some(0);
        }
        let k = self.lengths[self.cursor];
        dst[..k].copy_from_slice(&self.lineas[self.cursor][..k]);
        Some(k)
    }
}

