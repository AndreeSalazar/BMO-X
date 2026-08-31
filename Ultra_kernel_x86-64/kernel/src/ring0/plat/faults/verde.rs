//! **CARRIL VERDE** -- se cambia solo: son un buffer y unos colores.
//!
//! [cuesta]  NADA -- `Line` es un constructor de renglones de capacidad fija
//!           y los demas son colores y un plazo. Equivocarse pinta feo.
//!
//! [riesgo]  -- ninguno declarado.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! ** `Line` esta aqui y no en amarillo aunque lo use el informe: **no decide
//! nada**. Escribe bytes en un array de tamano fijo y se para al llegar al
//! final. Es la pieza mas tocada del fichero y la menos peligrosa, que es
//! justo lo que un carril tiene que poder decir de un vistazo.

use crate::ring0::dev::console::serial_write;

/// Small fixed-capacity line builder (no alloc, exception-context safe).
#[derive(Clone, Copy)]
pub(super) struct Line {
    pub(super) b: [u8; 80],
    pub(super) n: usize,
}

impl Line {
    pub(super) fn new() -> Self {
        Self { b: [0; 80], n: 0 }
    }
    pub(super) fn s(&mut self, s: &str) {
        for &c in s.as_bytes() {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    pub(super) fn hex(&mut self, mut v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        let mut tmp = [0u8; 16];
        for i in 0..digits {
            tmp[digits - 1 - i] = H[(v & 0xF) as usize];
            v >>= 4;
        }
        for i in 0..digits {
            if self.n < self.b.len() {
                self.b[self.n] = tmp[i];
                self.n += 1;
            }
        }
    }
    pub(super) fn as_str(&self) -> &str {
        core::str::from_utf8(&self.b[..self.n]).unwrap_or("")
    }
}


// -- La pantalla de fallo ------------------------------------------------

/// Azul de BMO. No es el de Microsoft ni pretende serlo: una pantalla de
/// panico es una pieza de diseno estandar de cualquier sistema operativo, y
/// esta lleva la cara de este. Lo que si se le copia al mundo entero es la
/// idea buena -- **azul, letra grande, y los numeros que hacen falta**.
pub(super) const FALLO_FONDO: u32 = 0x0011_3A6E;
pub(super) const FALLO_TITULO: u32 = 0x00FF_FFFF;
pub(super) const FALLO_TEXTO: u32 = 0x00C8_DCF0;
pub(super) const FALLO_DATO: u32 = 0x00FF_D2_5A;
pub(super) const FALLO_BARRA: u32 = 0x004C_9BE8;

/// Segundos que el informe se queda en pantalla antes de reiniciar.
///
/// Bastante para leerlo y, sobre todo, para **fotografiarlo**: aqui la foto es
/// el depurador. Poco para no dejar la maquina muerta si esto pasa mientras
/// nadie mira.
pub(super) const FALLO_SEGUNDOS: u64 = 20;


/// Filas del informe, en el orden en que se pintan. `faults.rs` las llena.
pub(super) struct Informe {
    /// * 16 y no 12. Los dos informes llegaron a llenar las doce EXACTAS, y
    /// `push` descarta en silencio a partir del tope: la siguiente fila que
    /// alguien anadiera se perderia sin un solo aviso, justo en la herramienta
    /// que usamos para depurar cuando no hay otra. Un margen de cuatro cuesta
    /// 352 bytes de una pila que ya no va a servir para nada mas.
    pub(super) lineas: [Line; 16],
    pub(super) n: usize,
}

impl Informe {
    pub(super) fn nuevo() -> Self {
        Self { lineas: [Line::new(); 16], n: 0 }
    }
    pub(super) fn push(&mut self, l: Line) {
        if self.n < self.lineas.len() {
            self.lineas[self.n] = l;
            self.n += 1;
        }
        // Todo lo que se pinta va TAMBIEN por serie, que es lo unico que
        // sobrevive a un reinicio automatico.
        serial_write("[fault] ");
        serial_write(l.as_str());
        serial_write("\n");
    }
}

