//! **La cola de avisos de puerto**: lo que el xHC cuenta, sin que se pise.
//!
//! === Por que existe este modulo ===
//!
//! El aviso de "cambio el puerto N" vivia en TRES estaticos sueltos:
//!
//! ```ignore
//! static mut LAST_PORT: u8 = 0;
//! static mut LAST_PORT_CCS: bool = false;
//! static mut PORT_PENDIENTE: bool = false;
//! ```
//!
//! Eso es un buzon de **una sola plaza**. Y quien lo llena es
//! `poll_transfer_event`, que **drena el anillo de eventos en bucle** hasta
//! encontrar un Transfer Event: cada TRB de puerto que se cruza en esa vuelta
//! sobrescribe al anterior y solo sobrevive el ultimo.
//!
//! Dicho con lo que le pasa al dueno delante de la maquina:
//!
//! ```text
//!   desenchufa el teclado   -> el xHC postea (4, desconectado)
//!   lo vuelve a enchufar    -> el xHC postea (4, conectado)   <- PISA al anterior
//!   el kernel pregunta      -> "puerto 4, conectado"
//! ```
//!
//! **La desconexion no ocurrio nunca**, para el kernel. Y sin desconexion no se
//! llama a `soltar_puerto`, asi que el teclado fantasma sigue contando como
//! presente: `completo()` dice que no falta nada y el adoptador se va sin tocar
//! el bus. En CABINA sale como `puerto: ENCHUFADO, nada que adoptar` con un
//! `creo tener teclado:raton =257` al lado -- que es la linea que ya se escribio
//! para que esta mentira se pudiera VER, y se vio.
//!
//! No hace falta desenchufar rapido para caer en esto. Basta con que dos puertos
//! cambien en la misma vuelta del anillo -- que es exactamente lo que pasa cuando
//! el firmware vuelve a dar corriente a todo despues de pasar por la BIOS.
//!
//! === Por que una cola y no un bitmask ===
//!
//! Un bitmask de "puertos que cambiaron" + leer PORTSC al consumir seria mas
//! corto, y estaria mal por lo mismo: el par desenchufe/enchufe del mismo puerto
//! se funde en "esta conectado", y **el hecho que hay que no perder es la
//! desconexion**. Un aparato que se va y vuelve NO es el mismo aparato: hay que
//! olvidarlo antes de volver a mirarlo.
//!
//! === Y si aun asi se desborda ===
//!
//! Se cuenta y se dice ([`Avisos::desbordes`]). Quien consume tiene entonces un
//! deber concreto: **barrer los puertos de verdad** en vez de fiarse de los
//! avisos. Ver `bmo_uhid::barrido`. Perder un aviso deja de ser fatal en cuanto
//! existe alguien que compara lo que se cree con lo que hay.
//!
//! Vive aparte del driver porque es la unica parte de esto que se puede probar
//! sin un xHC delante -- la misma razon por la que `bmo_uhid::puertos` vive
//! aparte, y por la que aquel bug se pudo cazar en una prueba.

/// Cuantos avisos caben sin perder ninguno.
///
/// Diecisiete y no cuatro: el peor caso real es el firmware devolviendo la
/// corriente a todos los puertos raiz a la vez, y esta placa declara 16. Uno mas
/// para que ese barrido entero quepa aunque llegue con algo ya encolado.
pub const MAX_AVISOS: usize = 17;

/// Un aviso: `(puerto 1-based tal cual lo manda el xHC, hay algo conectado)`.
pub type Aviso = (u8, bool);

/// La cola. FIFO, tamano fijo, sin asignacion.
#[derive(Debug, Clone, Copy)]
pub struct Avisos {
    buf: [Aviso; MAX_AVISOS],
    primero: usize,
    largo: usize,
    desbordes: u32,
}

impl Default for Avisos {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Avisos {
    pub const fn nueva() -> Self {
        Self { buf: [(0, false); MAX_AVISOS], primero: 0, largo: 0, desbordes: 0 }
    }

    /// Cuantos avisos esperan.
    pub fn largo(&self) -> usize {
        self.largo
    }

    /// Hay algo que atender?
    pub fn hay(&self) -> bool {
        self.largo != 0
    }

    /// Avisos que no cupieron. **Si esto sube, los avisos ya no bastan** y hay
    /// que ir a mirar los puertos. Ver la cabecera del modulo.
    pub fn desbordes(&self) -> u32 {
        self.desbordes
    }

    /// El puerto 0 no existe en el xHC: los Port ID son 1-based. Un aviso con
    /// puerto 0 es un TRB mal leido, y encolarlo solo serviria para que alguien
    /// mas abajo restara uno y acabara mirando el puerto 255.
    fn valido(puerto: u8) -> bool {
        puerto >= 1
    }

    /// Indice fisico del ultimo encolado.
    fn ultimo(&self) -> Option<Aviso> {
        if self.largo == 0 {
            return None;
        }
        Some(self.buf[(self.primero + self.largo - 1) % MAX_AVISOS])
    }

    /// **Apunta un cambio de puerto.** Devuelve `false` si no cupo.
    ///
    /// Se funden los duplicados EXACTOS seguidos --mismo puerto, mismo estado--
    /// porque son la misma noticia contada dos veces y llenarian la cola en una
    /// tormenta de PORTSC. Lo que jamas se funde es un cambio de estado: `(4,
    /// desconectado)` seguido de `(4, conectado)` son dos hechos distintos y
    /// perder el primero es el bug entero de este modulo.
    pub fn anotar(&mut self, puerto: u8, conectado: bool) -> bool {
        if !Self::valido(puerto) {
            return false;
        }
        if self.ultimo() == Some((puerto, conectado)) {
            return true;
        }
        if self.largo == MAX_AVISOS {
            // Se tira el NUEVO y no el viejo, a proposito. Lo viejo es lo que
            // todavia no se ha atendido, y entre "no me entere del ultimo
            // enchufe" y "no me entere de que se fue el teclado", el que deja la
            // maquina inservible es el segundo.
            self.desbordes = self.desbordes.wrapping_add(1);
            return false;
        }
        let sitio = (self.primero + self.largo) % MAX_AVISOS;
        self.buf[sitio] = (puerto, conectado);
        self.largo += 1;
        true
    }

    /// **Saca el aviso mas antiguo.** `None` si no hay nada nuevo, para que quien
    /// consuma pueda sondear en su bucle sin re-enumerar cien veces el mismo
    /// enchufe.
    pub fn tomar(&mut self) -> Option<Aviso> {
        if self.largo == 0 {
            return None;
        }
        let a = self.buf[self.primero];
        self.primero = (self.primero + 1) % MAX_AVISOS;
        self.largo -= 1;
        Some(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ** EL BUG. Un desenchufe y un enchufe seguidos son DOS noticias.
    ///
    /// Con el buzon de una plaza, el segundo pisaba al primero y el kernel no se
    /// enteraba nunca de que el teclado se habia ido. Sin esa mitad, el teclado
    /// fantasma seguia contando como presente y el re-enchufe no adoptaba nada:
    /// `puerto: ENCHUFADO, nada que adoptar` + `creo tener teclado:raton =257`.
    #[test]
    fn desenchufar_y_volver_a_enchufar_no_se_funde_en_uno() {
        let mut a = Avisos::nueva();
        a.anotar(4, false);
        a.anotar(4, true);

        assert_eq!(a.tomar(), Some((4, false)), "primero se fue");
        assert_eq!(a.tomar(), Some((4, true)), "y despues volvio");
        assert_eq!(a.tomar(), None);
    }

    /// Y el orden importa: si el enchufe se atendiera antes que el desenchufe,
    /// el aparato se adoptaria y acto seguido se soltaria.
    #[test]
    fn el_orden_es_el_de_llegada() {
        let mut a = Avisos::nueva();
        a.anotar(1, true);
        a.anotar(2, true);
        a.anotar(1, false);

        assert_eq!(a.tomar(), Some((1, true)));
        assert_eq!(a.tomar(), Some((2, true)));
        assert_eq!(a.tomar(), Some((1, false)));
    }

    /// ** Y dos puertos en la misma vuelta del anillo tampoco se pisan.
    ///
    /// Este es el caso que no necesita dedos rapidos: el firmware devolviendo la
    /// corriente a todo despues de la BIOS postea varios cambios de golpe, y
    /// `poll_transfer_event` los cruza todos en la MISMA vuelta.
    #[test]
    fn dos_puertos_a_la_vez_llegan_los_dos() {
        let mut a = Avisos::nueva();
        a.anotar(3, true);
        a.anotar(5, true);

        assert_eq!(a.largo(), 2, "el teclado Y el raton");
        assert_eq!(a.tomar(), Some((3, true)));
        assert_eq!(a.tomar(), Some((5, true)));
    }

    /// El duplicado exacto es la misma noticia dos veces: se funde. Sin esto,
    /// una tormenta de PORTSC llenaria la cola de nada.
    #[test]
    fn el_duplicado_exacto_seguido_se_funde() {
        let mut a = Avisos::nueva();
        a.anotar(2, true);
        a.anotar(2, true);
        a.anotar(2, true);

        assert_eq!(a.largo(), 1);
        assert_eq!(a.desbordes(), 0, "fundir no es perder");
    }

    /// Pero fundir es SOLO para el estado repetido. En cuanto cambia, es otro
    /// hecho -- y volver a encolar el estado anterior tampoco se funde con uno
    /// que ya se atendio.
    #[test]
    fn fundir_no_se_come_un_cambio_de_estado() {
        let mut a = Avisos::nueva();
        a.anotar(2, true);
        a.anotar(2, false);
        a.anotar(2, true);

        assert_eq!(a.largo(), 3);
    }

    /// Desbordar tira lo NUEVO y conserva lo viejo: lo viejo es lo que aun no se
    /// ha atendido. Y se cuenta, porque un desborde es la senal de que los
    /// avisos ya no bastan y hay que ir a mirar los puertos.
    #[test]
    fn desbordar_conserva_lo_viejo_y_se_cuenta() {
        let mut a = Avisos::nueva();
        for p in 1..=MAX_AVISOS as u8 {
            assert!(a.anotar(p, true));
        }
        assert!(!a.anotar(200, true), "aqui ya no cabe");

        assert_eq!(a.desbordes(), 1);
        assert_eq!(a.tomar(), Some((1, true)), "el primero sigue estando");
    }

    /// La cola da la vuelta sin corromperse: consumir hace sitio de verdad.
    #[test]
    fn se_puede_dar_la_vuelta_al_buffer() {
        let mut a = Avisos::nueva();
        for p in 1..=MAX_AVISOS as u8 {
            a.anotar(p, true);
        }
        for p in 1..=(MAX_AVISOS as u8 - 1) {
            assert_eq!(a.tomar(), Some((p, true)));
        }
        // Queda uno dentro y sitio de sobra por detras.
        assert!(a.anotar(7, false));
        assert!(a.anotar(8, false));
        assert_eq!(a.tomar(), Some((MAX_AVISOS as u8, true)));
        assert_eq!(a.tomar(), Some((7, false)));
        assert_eq!(a.tomar(), Some((8, false)));
        assert_eq!(a.desbordes(), 0);
    }

    /// El puerto 0 no existe: los Port ID del xHC son 1-based. Encolarlo seria
    /// que alguien mas abajo le reste uno y acabe reseteando el puerto 255.
    #[test]
    fn el_puerto_cero_no_se_encola() {
        let mut a = Avisos::nueva();
        assert!(!a.anotar(0, true));
        assert!(!a.hay());
    }

    /// Una cola vacia no inventa avisos. Si esto fallara, el kernel re-enumeraria
    /// en cada bombeo -- 250 veces por segundo sobre un bus que funciona.
    #[test]
    fn una_cola_vacia_no_dice_nada() {
        let mut a = Avisos::nueva();
        assert!(!a.hay());
        assert_eq!(a.tomar(), None);
        assert_eq!(a.tomar(), None);
    }
}
