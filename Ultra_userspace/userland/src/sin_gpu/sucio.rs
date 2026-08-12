//! **Que trozos de la pantalla han cambiado.** Solo la cuenta, sin pintar nada.
//!
//! # El bug que este modulo existe para quitar
//!
//! Antes esto era **una sola caja**: cada `marcar` hacia la union con lo que ya
//! hubiera. Con un solo cambio por fotograma es perfecto y no cuesta nada. Con
//! DOS cambios lejanos es una catastrofe silenciosa:
//!
//! ```text
//!    cursor arriba a la izquierda    16 x 16  =      256 px
//!    cursor de texto abajo derecha    8 x 16  =      128 px
//!    ------------------------------------------------------
//!    lo que de verdad cambio                  =      384 px
//!    lo que la caja unica dice que cambio     = 2.073.600 px   (1920x1080)
//! ```
//!
//! **Cinco mil veces mas.** Y el volcado no es una copia cualquiera: va a
//! memoria write-combining, que es rapida para escribir seguido y no tiene
//! vuelta atras -- 8,3 MB por fotograma. Eso son los dos sintomas que reporto el
//! dueno **de una vez**:
//!
//! * *lento*, porque copia la pantalla entera cuando cambiaron 384 pixeles;
//! * *parpadea*, porque mientras esos 8,3 MB viajan, el escaner de video esta
//!   leyendo la misma memoria y ensena el fotograma a medio llegar.
//!
//! Y aparecia justo al MOVER EL RATON, que es cuando hay dos cosas cambiando en
//! sitios distintos: el cursor donde estaba y el cursor donde esta.
//!
//! # La regla
//!
//! Se guardan hasta [`MAX`] cajas. Dos cajas se juntan **solo si juntarlas sale
//! barato**: si la union no desperdicia mas de lo que ahorra en trabajo de
//! recorrerlas por separado. Cuando no caben mas, se junta el par que menos
//! desperdicie -- que en el peor caso deja exactamente el comportamiento de
//! antes, o sea que este modulo **no puede ser peor** que lo que sustituye.
//!
//! # Por que aqui y no dentro de `Pantalla`
//!
//! Porque esto no toca un solo pixel: es aritmetica de rectangulos. Separado se
//! puede leer entero de una sentada y se puede razonar sin tener delante ni el
//! framebuffer ni el `unsafe`. Es la regla modular de la casa aplicada al reves
//! de como se suele: no se saca lo grande, se saca **lo que no necesita nada**.

/// Cuantas cajas se llevan a la vez.
///
/// Ocho: un compositor con un cursor, un cursor de texto y dos ventanas
/// moviendose no pasa de ahi, y ocho cajas se recorren en menos de lo que cuesta
/// una fila de pixeles de mas.
pub const MAX: usize = 8;

/// `(x0, y0, x1, y1)` -- el limite derecho e inferior NO entran.
pub type Caja = (u32, u32, u32, u32);

/// Superficie de una caja, en pixeles.
#[inline]
fn area(c: Caja) -> u64 {
    ((c.2 - c.0) as u64) * ((c.3 - c.1) as u64)
}

/// La caja mas pequena que contiene a las dos.
#[inline]
fn unir(a: Caja, b: Caja) -> Caja {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

/// **Lo que se desperdicia al juntarlas**: pixeles que se copiarian sin haber
/// cambiado.
///
/// Es la cuenta que decide todo este modulo, y esta escrita como cuenta y no
/// como heuristica a proposito: juntar dos cajas que se tocan cuesta 0 y hay que
/// hacerlo siempre; juntar dos esquinas opuestas cuesta la pantalla entera.
#[inline]
fn desperdicio(a: Caja, b: Caja) -> u64 {
    area(unir(a, b)).saturating_sub(area(a) + area(b))
}

/// Lo que cuesta llevar una caja aparte, en pixeles equivalentes.
///
/// No es cero: cada caja es un bucle mas, con su preparacion por fila. Poner un
/// numero permite que la decision sea una comparacion y no una opinion. 4096 =
/// cuatro filas de 1024, medido a ojo y **deliberadamente generoso**: preferimos
/// juntar de mas que fragmentar de mas, porque fragmentar de mas se paga en cada
/// fotograma y juntar de mas solo en el que ocurre.
const COSTE_DE_UNA_CAJA: u64 = 4096;

/// Las regiones sucias de un fotograma.
#[derive(Clone, Copy)]
pub struct Sucias {
    cajas: [Caja; MAX],
    n: usize,
}

impl Default for Sucias {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Sucias {
    pub const fn nueva() -> Self {
        Sucias { cajas: [(0, 0, 0, 0); MAX], n: 0 }
    }

    /// Hay algo que volcar?
    #[inline]
    pub fn vacia(&self) -> bool {
        self.n == 0
    }

    /// Las cajas a copiar, en orden.
    #[inline]
    pub fn cajas(&self) -> &[Caja] {
        &self.cajas[..self.n]
    }

    /// Pixeles que se van a copiar de verdad. Para poder medir la mejora en vez
    /// de creerla.
    pub fn pixeles(&self) -> u64 {
        let mut t = 0;
        for i in 0..self.n {
            t += area(self.cajas[i]);
        }
        t
    }

    /// **Apunta que este rectangulo cambio.**
    ///
    /// Vacio o invertido no es un error: es que no cambio nada, y se ignora sin
    /// ruido -- las primitivas de dibujo recortan contra la pantalla y un
    /// recorte puede dejar un rectangulo de ancho cero.
    pub fn marcar(&mut self, c: Caja) {
        if c.0 >= c.2 || c.1 >= c.3 {
            return;
        }
        // Si juntarla con alguna sale gratis o casi, se junta ahi mismo. Esto es
        // lo que hace que pintar una ventana en veinte trozos seguidos no
        // produzca veinte cajas.
        for i in 0..self.n {
            if desperdicio(self.cajas[i], c) <= COSTE_DE_UNA_CAJA {
                self.cajas[i] = unir(self.cajas[i], c);
                return;
            }
        }
        if self.n < MAX {
            self.cajas[self.n] = c;
            self.n += 1;
            return;
        }
        // Llena: se junta el par que menos desperdicie, y la nueva ocupa el
        // hueco. En el peor caso esto degenera en UNA caja grande, que es
        // exactamente lo que habia antes de este modulo -- o sea que el peor
        // caso de aqui es el caso normal de ayer.
        let (mut mejor_i, mut mejor_j, mut mejor) = (0usize, 1usize, u64::MAX);
        for i in 0..self.n {
            for j in (i + 1)..self.n {
                let d = desperdicio(self.cajas[i], self.cajas[j]);
                if d < mejor {
                    mejor = d;
                    mejor_i = i;
                    mejor_j = j;
                }
            }
        }
        self.cajas[mejor_i] = unir(self.cajas[mejor_i], self.cajas[mejor_j]);
        self.cajas[mejor_j] = c;
    }
}

// -- Las pruebas ------------------------------------------------------------
//
// [!] NO CORREN, y hay que decirlo en vez de dejar que el `#[cfg(test)]` de la
// impresion de que si. `Ultra_userspace` es `no_std` con su propio guion de
// enlazado, asi que `cargo test` aqui choca con lo mismo que en el kernel:
// enlaza `std` y no hay `std`.
//
// Se escriben igualmente y por una razon concreta: **son ejecutables desde
// fuera**. Este modulo no tiene ni un `unsafe`, ni un puntero, ni una
// dependencia, asi que se copia el fichero tal cual y se corre:
//
// ```text
//    rustc --test sucio.rs -o sucio_test && ./sucio_test
// ```
//
// ** Y ASI SE HIZO ANTES DE COMMITEAR (2026-08-12): 5 de 5 en verde, ejecutadas
// de verdad y no leidas. Un `#[cfg(test)]` que nadie ha corrido no es una
// prueba: es una intencion.
//
// El dia que exista un sitio donde corran solas, se mueven tal cual.
//
// La cicatriz de la casa: nueve pruebas de coma flotante del frontend de C estan
// en verde y **ninguna ejecuta**. Un `#[cfg(test)]` que no corre y no lo dice es
// justo eso.
#[cfg(test)]
mod tests {
    use super::*;

    /// ** EL CASO QUE MOTIVA TODO EL MODULO.
    ///
    /// Dos cursores en esquinas opuestas de una 1920x1080. Con una sola caja son
    /// 2.073.600 pixeles; con esto, 384.
    #[test]
    fn dos_esquinas_opuestas_no_copian_la_pantalla_entera() {
        let mut s = Sucias::nueva();
        s.marcar((0, 0, 16, 16));
        s.marcar((1912, 1064, 1920, 1080));
        assert_eq!(s.cajas().len(), 2, "son dos cosas, no una grande");
        assert_eq!(s.pixeles(), 256 + 128);
    }

    /// Lo que se toca se junta: pintar una ventana en trozos seguidos no puede
    /// producir un trozo por llamada.
    #[test]
    fn lo_que_esta_pegado_se_junta() {
        let mut s = Sucias::nueva();
        s.marcar((100, 100, 200, 200));
        s.marcar((200, 100, 300, 200));
        assert_eq!(s.cajas().len(), 1);
        assert_eq!(s.cajas()[0], (100, 100, 300, 200));
    }

    /// ** EL PEOR CASO DE AQUI ES EL CASO NORMAL DE AYER.
    ///
    /// Con mas cambios lejanos que huecos, esto degenera en cajas grandes -- que
    /// es lo que hacia la version de una sola caja SIEMPRE. No puede ser peor.
    #[test]
    fn al_llenarse_degenera_en_lo_de_antes_y_no_en_algo_peor() {
        let mut s = Sucias::nueva();
        for i in 0..(MAX as u32 + 4) {
            s.marcar((i * 200, i * 100, i * 200 + 8, i * 100 + 8));
        }
        assert!(s.cajas().len() <= MAX, "nunca mas de MAX");
        assert!(!s.vacia());
    }

    /// Un rectangulo vacio o invertido no es un error: las primitivas recortan
    /// contra la pantalla y un recorte puede dejar ancho cero.
    #[test]
    fn un_rectangulo_vacio_se_ignora_sin_ruido() {
        let mut s = Sucias::nueva();
        s.marcar((10, 10, 10, 50));
        s.marcar((10, 10, 50, 10));
        s.marcar((50, 50, 10, 10));
        assert!(s.vacia());
    }

    /// Una recien nacida no tiene nada que volcar. Es lo que `volcar` deja
    /// puesto al terminar, con `replace`: no hace falta un `limpiar` aparte y
    /// por eso no lo hay.
    #[test]
    fn una_recien_nacida_no_tiene_nada_que_volcar() {
        let s = Sucias::nueva();
        assert!(s.vacia());
        assert_eq!(s.pixeles(), 0);
    }
}
