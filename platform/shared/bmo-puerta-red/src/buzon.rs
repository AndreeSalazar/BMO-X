//! **EL BUZON** -- la memoria que el GATE RED presta, y como se lee sin creerla.
//!
//! [carril]  ROJO      el unico sitio donde el kernel lee indices que escribio Ring 3
//! [cuesta]  DATO      una trama mal copiada sale al cable, o se pierde una recibida
//! [riesgo]  AJENO     esta memoria la escribe el proceso, y puede cambiar un indice
//!                     o un largo MIENTRAS el kernel lo lee
//!
//! # La forma
//!
//! ```text
//!    0         cabecera, 64 bytes (ver `campo`)
//!    64        8 casillas de SALIDA   (el proceso escribe, el kernel saca)
//!    12352     8 casillas de ENTRADA  (el kernel mete, el proceso lee)
//!    24640     fin: 7 paginas
//!
//!    casilla   [largo u16][trama hasta 1514][relleno] = 1536 bytes
//! ```
//!
//! Cada lado es dueno de UN indice por anillo y solo lee el del otro:
//!
//! ```text
//!                  lo ESCRIBE       lo LEE
//!    tx_escrito    el proceso       el kernel
//!    tx_leido      el kernel        el proceso
//!    rx_escrito    el kernel        el proceso
//!    rx_leido      el proceso       el kernel
//! ```
//!
//! # *** Las tres reglas del lado del kernel ([`Lado`])
//!
//! 1. **Su propio indice lo guarda en SU memoria.** El que esta en el buzon es
//!    informativo: si el proceso lo pisa, el kernel ni se entera ni le importa.
//! 2. **Lo que escribe el proceso se lee UNA vez** y se trabaja con la copia. Un
//!    largo leido dos veces puede valer dos cosas, y la segunda es la que ataca.
//! 3. **Un indice mas lejos que las casillas que hay es una mentira**, no un
//!    estado. No se "arregla": se devuelve [`Mal`] y el radar revoca.

/// `BRED`, leido en little-endian. Un buzon revocado lo tiene a cero.
pub const MAGIA: u32 = u32::from_le_bytes(*b"BRED");
pub const VERSION: u16 = 1;
/// Casillas por anillo. Una potencia de dos no hace falta: el modulo se hace
/// siempre con `%`, y ocho tramas en 4 ms son 2.000 por segundo de techo.
pub const CASILLAS: usize = 8;
/// La trama mas larga, sin FCS. Tiene que valer lo mismo que
/// `bmo_net::tx::MAXIMA`, y el kernel lo comprueba al compilar.
pub const TRAMA_MAXIMA: usize = 1514;
pub const CASILLA: usize = 1536;
pub const CABECERA: usize = 64;
pub const BYTES: usize = CABECERA + 2 * CASILLAS * CASILLA;
pub const PAGINA: usize = 4096;
pub const PAGINAS: usize = (BYTES + PAGINA - 1) / PAGINA;

/// Donde vive cada campo de la cabecera. Todos little-endian.
pub mod campo {
    pub const MAGIA: usize = 0;
    pub const VERSION: usize = 4;
    pub const CASILLAS: usize = 6;
    pub const TX_ESCRITO: usize = 8;
    pub const TX_LEIDO: usize = 12;
    pub const RX_ESCRITO: usize = 16;
    pub const RX_LEIDO: usize = 20;
    /// Sube con cada trama metida. Es el testigo de `WAIT`.
    pub const RX_SECUENCIA: usize = 24;
    /// `0` = abierto. Otro numero = revocado, y el numero es un `radar::Motivo`.
    pub const ESTADO: usize = 32;
    pub const SALIERON: usize = 36;
    pub const NEGADAS: usize = 40;
    /// El ultimo `NoSale` del grifo, `0` si la ultima salio.
    pub const ULTIMO_NO: usize = 44;
    /// Tramas que llegaron y no cabian: buzon lleno.
    pub const TIRADAS: usize = 48;
}

/// Donde empieza la casilla de salida `i` (el modulo va dentro).
pub const fn casilla_tx(i: usize) -> usize {
    CABECERA + (i % CASILLAS) * CASILLA
}

/// Donde empieza la casilla de entrada `i` (el modulo va dentro).
pub const fn casilla_rx(i: usize) -> usize {
    CABECERA + CASILLAS * CASILLA + (i % CASILLAS) * CASILLA
}

/// **La memoria del buzon, vista byte a byte.**
///
/// En el anfitrion es un `[u8]`. En el kernel es un puntero al espejo fisico
/// leido con `read_volatile`: el proceso puede estar escribiendo ahi, y un
/// `&[u8]` sobre memoria que cambia sola no es algo que Rust permita suponer.
pub trait Memoria {
    fn tam(&self) -> usize;
    fn leer8(&self, off: usize) -> u8;
    fn escribir8(&mut self, off: usize, v: u8);
}

impl Memoria for [u8] {
    fn tam(&self) -> usize {
        self.len()
    }
    fn leer8(&self, off: usize) -> u8 {
        self[off]
    }
    fn escribir8(&mut self, off: usize, v: u8) {
        self[off] = v;
    }
}

pub fn leer16<M: Memoria + ?Sized>(m: &M, off: usize) -> u16 {
    u16::from_le_bytes([m.leer8(off), m.leer8(off + 1)])
}

pub fn leer32<M: Memoria + ?Sized>(m: &M, off: usize) -> u32 {
    u32::from_le_bytes([m.leer8(off), m.leer8(off + 1), m.leer8(off + 2), m.leer8(off + 3)])
}

pub fn leer64<M: Memoria + ?Sized>(m: &M, off: usize) -> u64 {
    (leer32(m, off) as u64) | ((leer32(m, off + 4) as u64) << 32)
}

fn escribir_bytes<M: Memoria + ?Sized>(m: &mut M, off: usize, b: &[u8]) {
    for (k, &v) in b.iter().enumerate() {
        m.escribir8(off + k, v);
    }
}

pub fn escribir16<M: Memoria + ?Sized>(m: &mut M, off: usize, v: u16) {
    escribir_bytes(m, off, &v.to_le_bytes());
}

pub fn escribir32<M: Memoria + ?Sized>(m: &mut M, off: usize, v: u32) {
    escribir_bytes(m, off, &v.to_le_bytes());
}

pub fn escribir64<M: Memoria + ?Sized>(m: &mut M, off: usize, v: u64) {
    escribir_bytes(m, off, &v.to_le_bytes());
}

/// **Lo que el buzon no puede decir sin mentir.** Uno por motivo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mal {
    /// Un indice del proceso esta mas lejos que las casillas que hay.
    IndiceImposible,
    /// El largo de una casilla de salida no es el de una trama.
    LargoImposible,
    /// La memoria no llega a [`BYTES`]. Es del kernel, no del proceso.
    BuzonCorto,
}

impl Mal {
    pub fn texto(self) -> &'static str {
        match self {
            Mal::IndiceImposible => "un indice del proceso apunta mas lejos que las casillas",
            Mal::LargoImposible => "una casilla de salida declara un largo que no es trama",
            Mal::BuzonCorto => "el buzon es mas corto que su propia forma",
        }
    }
}

/// **EL LADO DEL KERNEL.** Guarda sus dos indices en SU memoria (regla 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Lado {
    tx_leido: u32,
    rx_escrito: u32,
    rx_seq: u64,
    pub sacadas: u64,
    pub metidas: u64,
    pub tiradas: u64,
}

impl Default for Lado {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Lado {
    pub const fn nuevo() -> Self {
        Self { tx_leido: 0, rx_escrito: 0, rx_seq: 0, sacadas: 0, metidas: 0, tiradas: 0 }
    }

    /// **Pone el buzon a cero y escribe la cabecera.** Antes de mapearlo: el
    /// proceso no puede ver nunca lo que hubiera en esos marcos antes.
    pub fn preparar<M: Memoria + ?Sized>(&mut self, m: &mut M) -> Result<(), Mal> {
        if m.tam() < BYTES {
            return Err(Mal::BuzonCorto);
        }
        for i in 0..BYTES {
            m.escribir8(i, 0);
        }
        *self = Self::nuevo();
        escribir32(m, campo::MAGIA, MAGIA);
        escribir16(m, campo::VERSION, VERSION);
        escribir16(m, campo::CASILLAS, CASILLAS as u16);
        Ok(())
    }

    /// El testigo de `WAIT`: cuantas tramas se han metido.
    pub fn secuencia(&self) -> u64 {
        self.rx_seq
    }

    /// **Saca la siguiente trama de salida** y la COPIA a `dst`, que es memoria
    /// del kernel. `Ok(None)` = no hay nada.
    ///
    /// [!] La casilla se da por consumida ANTES de juzgar su largo: una casilla
    /// mentirosa no puede bloquear el anillo, y de todas formas el radar revoca.
    pub fn sacar<M: Memoria + ?Sized>(&mut self, m: &mut M, dst: &mut [u8]) -> Result<Option<usize>, Mal> {
        if m.tam() < BYTES {
            return Err(Mal::BuzonCorto);
        }
        // Regla 2: UNA lectura, y se trabaja con la copia.
        let escrito = leer32(m, campo::TX_ESCRITO);
        let pendientes = escrito.wrapping_sub(self.tx_leido) as usize;
        if pendientes == 0 {
            return Ok(None);
        }
        if pendientes > CASILLAS {
            return Err(Mal::IndiceImposible);
        }
        let base = casilla_tx(self.tx_leido as usize);
        let largo = leer16(m, base) as usize;
        self.tx_leido = self.tx_leido.wrapping_add(1);
        escribir32(m, campo::TX_LEIDO, self.tx_leido);
        if largo > TRAMA_MAXIMA || largo > dst.len() {
            return Err(Mal::LargoImposible);
        }
        for (k, d) in dst.iter_mut().take(largo).enumerate() {
            *d = m.leer8(base + 2 + k);
        }
        self.sacadas += 1;
        Ok(Some(largo))
    }

    /// **Mete una trama recibida.** `Ok(false)` = no cabia y se tiro (se cuenta).
    pub fn meter<M: Memoria + ?Sized>(&mut self, m: &mut M, trama: &[u8]) -> Result<bool, Mal> {
        if m.tam() < BYTES {
            return Err(Mal::BuzonCorto);
        }
        let leido = leer32(m, campo::RX_LEIDO);
        let ocupadas = self.rx_escrito.wrapping_sub(leido) as usize;
        if ocupadas > CASILLAS {
            return Err(Mal::IndiceImposible);
        }
        if ocupadas == CASILLAS || trama.len() > TRAMA_MAXIMA {
            self.tiradas += 1;
            escribir32(m, campo::TIRADAS, self.tiradas as u32);
            return Ok(false);
        }
        let base = casilla_rx(self.rx_escrito as usize);
        escribir16(m, base, trama.len() as u16);
        escribir_bytes(m, base + 2, trama);
        // El indice se publica DESPUES de la trama: el proceso no puede ver una
        // casilla anunciada y a medio escribir.
        self.rx_escrito = self.rx_escrito.wrapping_add(1);
        escribir32(m, campo::RX_ESCRITO, self.rx_escrito);
        self.rx_seq = self.rx_seq.wrapping_add(1);
        escribir64(m, campo::RX_SECUENCIA, self.rx_seq);
        self.metidas += 1;
        Ok(true)
    }

    /// Los contadores del grifo, para que el proceso vea por que no sale algo.
    pub fn publicar<M: Memoria + ?Sized>(&self, m: &mut M, salieron: u64, negadas: u64, ultimo_no: u32) {
        if m.tam() < BYTES {
            return;
        }
        escribir32(m, campo::SALIERON, salieron as u32);
        escribir32(m, campo::NEGADAS, negadas as u32);
        escribir32(m, campo::ULTIMO_NO, ultimo_no);
    }
}

/// Por que el proceso no pudo dejar una trama en el buzon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoEnvia {
    /// El pase se revoco o nunca se abrio. [`Cliente::estado`] dice por que.
    Revocado,
    /// Las 8 casillas estan esperando al kernel. Vuelve en 4 ms.
    Llena,
    /// Mas de 1514 bytes.
    Larga,
}

/// **EL LADO DEL PROCESO.** Sin estado propio: sus dos indices viven en el buzon
/// porque es el unico que los escribe.
pub struct Cliente;

impl Cliente {
    /// Abierto y con la forma que este codigo conoce.
    pub fn vivo<M: Memoria + ?Sized>(m: &M) -> bool {
        m.tam() >= BYTES
            && leer32(m, campo::MAGIA) == MAGIA
            && leer16(m, campo::CASILLAS) as usize == CASILLAS
            && leer32(m, campo::ESTADO) == 0
    }

    /// `0` abierto; otro = el `radar::Motivo` de la revocacion.
    pub fn estado<M: Memoria + ?Sized>(m: &M) -> u32 {
        if m.tam() < BYTES {
            return 0;
        }
        leer32(m, campo::ESTADO)
    }

    pub fn secuencia<M: Memoria + ?Sized>(m: &M) -> u64 {
        if m.tam() < BYTES {
            return 0;
        }
        leer64(m, campo::RX_SECUENCIA)
    }

    /// **Deja una trama para el cable.** Sale en el siguiente latido, si el
    /// grifo la deja.
    pub fn enviar<M: Memoria + ?Sized>(m: &mut M, trama: &[u8]) -> Result<(), NoEnvia> {
        if !Self::vivo(m) {
            return Err(NoEnvia::Revocado);
        }
        if trama.len() > TRAMA_MAXIMA {
            return Err(NoEnvia::Larga);
        }
        let escrito = leer32(m, campo::TX_ESCRITO);
        let leido = leer32(m, campo::TX_LEIDO);
        if escrito.wrapping_sub(leido) as usize >= CASILLAS {
            return Err(NoEnvia::Llena);
        }
        let base = casilla_tx(escrito as usize);
        escribir16(m, base, trama.len() as u16);
        escribir_bytes(m, base + 2, trama);
        escribir32(m, campo::TX_ESCRITO, escrito.wrapping_add(1));
        Ok(())
    }

    /// **Recoge la siguiente trama recibida** en `dst`. `None` = no hay.
    pub fn recibir<M: Memoria + ?Sized>(m: &mut M, dst: &mut [u8]) -> Option<usize> {
        if m.tam() < BYTES || leer32(m, campo::MAGIA) != MAGIA {
            return None;
        }
        let escrito = leer32(m, campo::RX_ESCRITO);
        let leido = leer32(m, campo::RX_LEIDO);
        if escrito == leido {
            return None;
        }
        let base = casilla_rx(leido as usize);
        let largo = (leer16(m, base) as usize).min(TRAMA_MAXIMA).min(dst.len());
        for (k, d) in dst.iter_mut().take(largo).enumerate() {
            *d = m.leer8(base + 2 + k);
        }
        escribir32(m, campo::RX_LEIDO, leido.wrapping_add(1));
        Some(largo)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn buzon() -> (Vec<u8>, Lado) {
        let mut m = vec![0xAAu8; PAGINAS * PAGINA];
        let mut l = Lado::nuevo();
        l.preparar(&mut m[..]).unwrap();
        (m, l)
    }

    fn trama(n: usize, semilla: u8) -> Vec<u8> {
        (0..n).map(|i| semilla.wrapping_add(i as u8)).collect()
    }

    #[test]
    fn la_forma_cabe_y_no_se_pisa() {
        assert_eq!(BYTES, 24_640);
        assert_eq!(PAGINAS, 7);
        let fin_tx = casilla_tx(CASILLAS - 1) + 2 + TRAMA_MAXIMA;
        assert!(fin_tx <= casilla_rx(0), "la ultima de salida no pisa la primera de entrada");
        assert!(casilla_rx(CASILLAS - 1) + 2 + TRAMA_MAXIMA <= BYTES);
        assert_eq!(MAGIA.to_le_bytes(), *b"BRED");
    }

    #[test]
    fn preparar_borra_lo_que_hubiera() {
        let (m, _) = buzon();
        assert!(m[CABECERA..BYTES].iter().all(|&b| b == 0), "ni un byte del dueno anterior");
        assert!(Cliente::vivo(&m[..]));
        let mut corto = vec![0u8; BYTES - 1];
        assert_eq!(Lado::nuevo().preparar(&mut corto[..]), Err(Mal::BuzonCorto));
    }

    #[test]
    fn de_ida_y_de_vuelta_tres_vueltas_enteras() {
        let (mut m, mut l) = buzon();
        let mut dst = [0u8; TRAMA_MAXIMA];
        for vuelta in 0..3u8 {
            for i in 0..CASILLAS {
                let t = trama(60 + i * 100, vuelta ^ i as u8);
                Cliente::enviar(&mut m[..], &t).unwrap();
            }
            assert_eq!(Cliente::enviar(&mut m[..], &trama(60, 0)), Err(NoEnvia::Llena));
            for i in 0..CASILLAS {
                let n = l.sacar(&mut m[..], &mut dst).unwrap().unwrap();
                assert_eq!(&dst[..n], &trama(60 + i * 100, vuelta ^ i as u8)[..]);
            }
            assert_eq!(l.sacar(&mut m[..], &mut dst), Ok(None));

            for i in 0..CASILLAS {
                assert_eq!(l.meter(&mut m[..], &trama(42 + i, vuelta)), Ok(true));
            }
            assert_eq!(l.meter(&mut m[..], &trama(42, 9)), Ok(false), "lleno se TIRA y se cuenta");
            for i in 0..CASILLAS {
                let n = Cliente::recibir(&mut m[..], &mut dst).unwrap();
                assert_eq!(&dst[..n], &trama(42 + i, vuelta)[..]);
            }
            assert_eq!(Cliente::recibir(&mut m[..], &mut dst), None);
        }
        assert_eq!((l.sacadas, l.metidas, l.tiradas), (24, 24, 3));
        assert_eq!(Cliente::secuencia(&m[..]), 24);
        assert_eq!(leer32(&m[..], campo::TIRADAS), 3);
    }

    /// *** El proceso MIENTE con su indice de salida: se dice, no se arregla.
    #[test]
    fn un_indice_de_salida_imposible_es_mal() {
        let (mut m, mut l) = buzon();
        let mut dst = [0u8; TRAMA_MAXIMA];
        escribir32(&mut m[..], campo::TX_ESCRITO, CASILLAS as u32 + 1);
        assert_eq!(l.sacar(&mut m[..], &mut dst), Err(Mal::IndiceImposible));
        escribir32(&mut m[..], campo::TX_ESCRITO, u32::MAX);
        assert_eq!(l.sacar(&mut m[..], &mut dst), Err(Mal::IndiceImposible), "por detras tambien");
    }

    #[test]
    fn un_indice_de_entrada_imposible_es_mal() {
        let (mut m, mut l) = buzon();
        escribir32(&mut m[..], campo::RX_LEIDO, 5);
        assert_eq!(l.meter(&mut m[..], &trama(60, 1)), Err(Mal::IndiceImposible), "leyo lo que no se escribio");
    }

    #[test]
    fn un_largo_imposible_es_mal_y_consume_la_casilla() {
        let (mut m, mut l) = buzon();
        let mut dst = [0u8; TRAMA_MAXIMA];
        Cliente::enviar(&mut m[..], &trama(100, 0)).unwrap();
        escribir16(&mut m[..], casilla_tx(0), 2000);
        assert_eq!(l.sacar(&mut m[..], &mut dst), Err(Mal::LargoImposible));
        assert_eq!(l.sacar(&mut m[..], &mut dst), Ok(None), "no se queda atascado en ella");
    }

    /// ** Regla 1: pisar el indice PUBLICADO del kernel no cambia lo que hace.
    #[test]
    fn pisar_el_indice_del_kernel_no_le_cambia_nada() {
        let (mut m, mut l) = buzon();
        let mut dst = [0u8; TRAMA_MAXIMA];
        Cliente::enviar(&mut m[..], &trama(70, 3)).unwrap();
        escribir32(&mut m[..], campo::TX_LEIDO, 1234);
        assert_eq!(l.sacar(&mut m[..], &mut dst), Ok(Some(70)));
        assert_eq!(leer32(&m[..], campo::TX_LEIDO), 1, "y lo vuelve a publicar bien");
    }

    #[test]
    fn un_buzon_revocado_no_admite_ni_devuelve() {
        let (mut m, _) = buzon();
        escribir32(&mut m[..], campo::ESTADO, 6);
        assert_eq!(Cliente::enviar(&mut m[..], &trama(60, 0)), Err(NoEnvia::Revocado));
        assert_eq!(Cliente::estado(&m[..]), 6);
        let mut lapida = vec![0u8; BYTES];
        assert!(!Cliente::vivo(&lapida[..]));
        assert_eq!(Cliente::recibir(&mut lapida[..], &mut [0u8; 64]), None);
    }

    /// Cincuenta mil cabeceras pisadas al azar: el kernel nunca lee fuera.
    #[test]
    fn cincuenta_mil_cabeceras_pisadas_no_revientan() {
        let (mut m, mut l) = buzon();
        let mut semilla = 0xB0B0_CAFEu64;
        let mut azar = || {
            semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semilla >> 33) as u32
        };
        let mut dst = [0u8; TRAMA_MAXIMA];
        for _ in 0..50_000 {
            let campo_ = [campo::TX_ESCRITO, campo::RX_LEIDO][(azar() % 2) as usize];
            escribir32(&mut m[..], campo_, azar() % 20);
            let c = (azar() as usize) % CASILLAS;
            escribir16(&mut m[..], casilla_tx(c), (azar() % 4000) as u16);
            let _ = l.sacar(&mut m[..], &mut dst);
            let _ = l.meter(&mut m[..], &dst[..(azar() as usize) % (TRAMA_MAXIMA + 1)]);
            let _ = l.meter(&mut m[..], &[0u8; TRAMA_MAXIMA + 86][..]);
        }
    }
}
