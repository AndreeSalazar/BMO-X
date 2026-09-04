//! **CARRIL VERDE** -- se cambia solo: son un buffer y unos colores.
//!
//! [carril]  VERDE     el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
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
///
/// # *** CORTABA EN 80 BYTES Y NO LO DECIA (2026-09-02)
///
/// El dueno fotografio una pantalla azul cuyo veredicto acababa asi:
///
/// ```text
///    rsp=0xFFFF800000B8DC50   pila de HILO DEL KERNEL -- de NADIE VIVO marco OCUPADO,
/// ```
///
/// La coma es el byte 80. Detras iba **de quien es ahora ese marco**, que es el
/// dato entero por el que existe esa linea. Y la de al lado, `iq: ... El fallo
/// no es de un cambio de contexto`, mide 84 y perdia las cuatro ultimas.
///
/// ** Se creyo que era el borde de la pantalla, y no lo era: `CHAR_W` son 10
/// pixeles y el informe empieza en `w/12`, o sea que a 1920 caben **176
/// caracteres**. Sobraba media pantalla. El limite era este array.
///
/// > `Informe::push` ya habia pasado por esto --paso de 12 filas a 16 porque
/// > *"descarta en silencio"*-- y el renglon hacia lo mismo un piso mas abajo
/// > sin que nadie lo mirara. La misma enfermedad, dos veces.
///
/// Se arregla en dos mitades, porque una sola no basta:
///
/// * **112 y no 80.** Cabe desde 1280 de ancho (117 caracteres) hacia arriba.
///   Por debajo de eso el que corta vuelve a ser el cristal, y entonces el
///   limite es visible -- que es exactamente la diferencia que importa.
/// * **Y si aun asi se pasa, LO DICE.** Los tres ultimos bytes pasan a `>>>`.
///
/// *** Un instrumento que descarta la respuesta en silencio es peor que uno que
/// no mira: el que no mira se nota. Este contestaba, y la contestacion no
/// llegaba al papel.
#[derive(Clone, Copy)]
pub(super) struct Line {
    pub(super) b: [u8; 112],
    pub(super) n: usize,
}

impl Line {
    pub(super) fn new() -> Self {
        Self { b: [0; 112], n: 0 }
    }
    /// Un byte, o la marca de desbordado. **Todo lo que escribe pasa por aqui**
    /// -- si `hex` volviera a llevar su propia copia del limite, la mitad de los
    /// renglones seguirian cortando callados.
    fn byte(&mut self, c: u8) {
        if self.n < self.b.len() {
            self.b[self.n] = c;
            self.n += 1;
            return;
        }
        // [!] La marca se pisa a si misma cuando sobran muchos bytes, y da
        // igual: lo que hay que saber es **que falta algo**, no cuanto.
        let fin = self.b.len();
        self.b[fin - 3] = b'>';
        self.b[fin - 2] = b'>';
        self.b[fin - 1] = b'>';
    }
    pub(super) fn s(&mut self, s: &str) {
        for &c in s.as_bytes() {
            self.byte(c);
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
            self.byte(tmp[i]);
        }
    }
    /// **UN ORDINAL, en base diez.**
    ///
    /// *** POR QUE HIZO FALTA (2026-09-04)
    ///
    /// La pantalla azul es casi toda direcciones, asi que todo se escribia con
    /// `hex`. El 04-09 salio este renglon:
    ///
    /// ```text
    ///    DESMONTANDO pid=00 estacion 11 destroy_address_space (van 02)
    /// ```
    ///
    /// La estacion 11 de la tabla es `console`, y el nombre decia
    /// `destroy_address_space`, que es la 17. **Los dos tenian razon**: `11` era
    /// hexadecimal, o sea 17. Nadie miente y aun asi el renglon se lee mal.
    ///
    /// ** Una direccion en base 16 es lo correcto; un ORDINAL en base 16 es una
    /// trampa. Es la misma familia que todo lo de este mes -- algo que dice una
    /// cosa cierta de una forma que se entiende como otra-- y esta vez cayo el
    /// que escribio el instrumento.
    pub(super) fn dec(&mut self, mut v: u64) {
        if v == 0 {
            self.byte(b'0');
            return;
        }
        let mut tmp = [0u8; 20];
        let mut n = 0;
        while v > 0 {
            tmp[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
        while n > 0 {
            n -= 1;
            self.byte(tmp[n]);
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

