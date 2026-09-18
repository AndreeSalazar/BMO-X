//! **QUE SE HACE CON CADA TRAMA QUE LLEGA** -- el juez del sondeo de recepcion.
//!
//! [carril]  ROJO    -- decide que bytes de un DESCONOCIDO pasan al buzon de un
//!           proceso, y cuantos
//!
//! [cuesta]  APARATO -- el anillo es memoria que escribe la tarjeta; un
//!           descriptor que no se devuelve la deja esperando para siempre
//!
//! [riesgo]  SILENCIO -- un fallo aqui no da fault: da `red rx 0` con la red
//!           hablando, o una trama que cruza a la de al lado
//!
//! ## Por que existe
//!
//! Hasta el 2026-09-17 esta decision vivia ESCRITA DENTRO DEL KERNEL, en el
//! bucle de `ring0/red/mod.rs`, y el kernel no se puede probar en el anfitrion.
//! O sea que la unica parte del sistema que lee bytes de fuera tenia su politica
//! sin una sola fila de banco -- y no era una politica hipotetica: ya fallo una
//! vez.
//!
//! > *EL ATASCO DEL 13-09.* Habia un `break` para cualquier cosa que no fuera
//! > una trama limpia, y una sola con error dejaba el anillo parado para
//! > siempre: `red rx` decia 0 y la red seguia hablando.
//!
//! Ese fallo ya estaba arreglado, pero **no tenia prueba**. Reintroducir el
//! `break` habria compilado y pasado el banco entero. Ahora la regla es un
//! metodo con nombre ([`Veredicto::devuelve`]) y tiene su fila.
//!
//! ## Lo que decide, en el orden en que una trama lo cruza
//!
//! ```text
//!    la tiene la tarjeta           -> PARAR       (el UNICO que para)
//!    la tarjeta la marco mala      -> MALA        se cuenta, se devuelve, sigue
//!    su largo no cabe en SU bufer  -> NO CABE     no se lee NI UN BYTE
//!    cabe y no llega a cabecera    -> CORTA       se cuenta, se devuelve, sigue
//!    limpia                        -> ENTREGAR    al buzon, con su tipo
//! ```
//!
//! ** "No se lee ni un byte" no es un comentario: lo garantiza la FORMA de
//! [`juzgar`]. Los bytes se piden con una funcion que solo se llama despues de
//! que el largo quepa, asi que para una trama que no cabe es imposible haberlos
//! mirado. Hay una fila que lo comprueba contando las llamadas.

use crate::anillo::Plan;
use crate::{EthHeader, Llegada, RxDesc};

/// El tipo de lo que llego, en las cuatro casillas que cuenta el kernel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tipo {
    Arp,
    Ipv4,
    Ipv6,
    /// Todo lo demas: VLAN, 802.3 con largo, lo que sea. Se entrega igual --
    /// Ring 0 no filtra protocolos, eso es de Ring 3 -- pero se cuenta aparte.
    Otro,
}

impl Tipo {
    /// Del `ethertype`, que ya viene en el orden de la maquina
    /// ([`EthHeader::parse`] lo leyo big-endian).
    pub const fn de(ethertype: u16) -> Tipo {
        match ethertype {
            0x0806 => Tipo::Arp,
            0x0800 => Tipo::Ipv4,
            0x86DD => Tipo::Ipv6,
            _ => Tipo::Otro,
        }
    }

    /// Su casilla en [`Contadores::tipos`]. Es el orden que ensena `red rx`.
    pub const fn casilla(self) -> usize {
        match self {
            Tipo::Arp => 0,
            Tipo::Ipv4 => 1,
            Tipo::Ipv6 => 2,
            Tipo::Otro => 3,
        }
    }
}

/// **Lo que se hace con UN descriptor del anillo.**
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Veredicto<'a> {
    /// Sigue siendo de la tarjeta: no hay nada mas que leer. El sondeo para.
    Parar,
    /// La tarjeta la marco (error, partida o enana). Lleva el caso para poder
    /// decir cual en pantalla.
    Mala(Llegada),
    /// Declara un largo que NO cabe en su propio bufer. No se ha leido nada.
    NoCabe(u16),
    /// Cabe, pero no llega a los catorce bytes de una cabecera Ethernet.
    Corta(u16),
    /// Limpia: se entrega entera.
    Entregar { cabecera: EthHeader, trama: &'a [u8] },
}

impl Veredicto<'_> {
    /// **Todo menos `Parar` devuelve el descriptor a la tarjeta y avanza.**
    ///
    /// *** Es la regla que rompio el atasco del 13-09, dicha con un nombre en
    /// vez de con un `match` escrito en el kernel. Un descriptor que ya es
    /// nuestro y no se devuelve deja a la tarjeta esperandolo: el anillo no
    /// falla, SE CALLA.
    pub const fn devuelve(&self) -> bool {
        !matches!(self, Veredicto::Parar)
    }
}

/// **Juzga el descriptor `i` del anillo.**
///
/// `leer(fisica, largo)` da los bytes de la trama. **Solo se llama si el largo
/// cabe en el bufer `i`** --no en el corral: en ESE bufer, ver
/// [`Plan::recibida`]-- asi que para una trama que no cabe no se toca memoria.
pub fn juzgar<'a>(
    plan: &Plan,
    i: usize,
    d: RxDesc,
    leer: impl FnOnce(u64, u16) -> &'a [u8],
) -> Veredicto<'a> {
    let largo = match d.llegada() {
        Llegada::DeLaTarjeta => return Veredicto::Parar,
        Llegada::Trama(l) => l,
        mala => return Veredicto::Mala(mala),
    };
    let Some(fisica) = plan.recibida(i, largo) else {
        return Veredicto::NoCabe(largo);
    };
    let trama = leer(fisica, largo);
    match EthHeader::parse(trama) {
        Some(cabecera) => Veredicto::Entregar { cabecera, trama },
        None => Veredicto::Corta(largo),
    }
}

/// **Lo que ha pasado por el anillo desde que se armo**, contado por veredicto.
///
/// Es lo que ensena `red rx`. Antes eran CINCO `static mut` sueltos en el
/// kernel, sumados a mano en cinco sitios del bucle; ahora es uno, y se suma
/// en un solo sitio que tiene banco.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Contadores {
    /// Tramas entregadas.
    pub tramas: u64,
    /// Sus bytes, sin la FCS.
    pub bytes: u64,
    /// Por tipo, en el orden de [`Tipo::casilla`].
    pub tipos: [u64; 4],
    /// Cabian y no llegaban a cabecera: un cable o un filtro, no una trama.
    pub cortas: u64,
    /// Marcadas por la tarjeta, o con un largo que no cabia.
    pub malas: u64,
}

impl Contadores {
    pub const fn nuevo() -> Self {
        Contadores { tramas: 0, bytes: 0, tipos: [0; 4], cortas: 0, malas: 0 }
    }

    /// Apunta un veredicto. `Parar` no es un suceso: no cuenta nada.
    ///
    /// Suma envolviendo, como hacia el kernel: un contador de tramas que da la
    /// vuelta es un dato raro; uno que PARA el kernel por desbordar es un
    /// apagon provocado desde el cable.
    pub fn apuntar(&mut self, v: &Veredicto<'_>) {
        match v {
            Veredicto::Parar => {}
            Veredicto::Mala(_) | Veredicto::NoCabe(_) => self.malas = self.malas.wrapping_add(1),
            Veredicto::Corta(_) => self.cortas = self.cortas.wrapping_add(1),
            Veredicto::Entregar { cabecera, trama } => {
                self.tramas = self.tramas.wrapping_add(1);
                self.bytes = self.bytes.wrapping_add(trama.len() as u64);
                let c = Tipo::de(cabecera.ethertype).casilla();
                self.tipos[c] = self.tipos[c].wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::anillo::{bytes_necesarios, BUFER};
    use crate::{rx, FCS_LEN};
    use std::cell::Cell;

    const BASE: u64 = 0x10_0000;

    fn plan() -> Plan {
        Plan::nuevo(BASE, bytes_necesarios()).unwrap()
    }

    /// Un descriptor como lo deja la tarjeta: entero, sin error, `largo` bytes
    /// de trama mas su FCS.
    fn llego(largo: u16) -> RxDesc {
        RxDesc { opts1: rx::FS | rx::LS | (largo as u32 + FCS_LEN as u32), ..RxDesc::default() }
    }

    fn trama(ethertype: u16, largo: usize) -> Vec<u8> {
        let mut t = vec![0u8; largo];
        t[0..6].copy_from_slice(&[0xFF; 6]);
        t[6..12].copy_from_slice(&[0x2C, 0xF0, 0x5D, 1, 2, 3]);
        t[12..14].copy_from_slice(&ethertype.to_be_bytes());
        t
    }

    #[test]
    fn la_que_tiene_la_tarjeta_para_el_sondeo_y_es_la_unica() {
        let d = RxDesc { opts1: rx::OWN, ..RxDesc::default() };
        let v = juzgar(&plan(), 0, d, |_, _| unreachable!("no hay nada que leer"));
        assert_eq!(v, Veredicto::Parar);
        assert!(!v.devuelve());
    }

    /// *** EL ATASCO DEL 13-09, con fila por fin. Una trama con error, una
    /// partida y una enana: las tres son NUESTRAS y la tarjeta las espera de
    /// vuelta. Si alguna parara el sondeo, el anillo se quedaria callado para
    /// siempre. Reintroducir aquel `break` pone esta fila roja.
    #[test]
    fn una_trama_mala_no_para_el_anillo() {
        let malas = [
            RxDesc { opts1: rx::FS | rx::LS | rx::RES | 64, ..RxDesc::default() },
            RxDesc { opts1: rx::FS | 64, ..RxDesc::default() },
            RxDesc { opts1: rx::FS | rx::LS | 2, ..RxDesc::default() },
        ];
        for d in malas {
            let v = juzgar(&plan(), 0, d, |_, _| unreachable!("una mala no se lee"));
            assert!(matches!(v, Veredicto::Mala(_)), "{v:?}");
            assert!(v.devuelve(), "{v:?} tiene que devolverse a la tarjeta");
        }
    }

    /// ** "No se lee ni un byte" contado, no prometido. La tarjeta escribe el
    /// largo y puede declarar hasta 16.383 bytes en un bufer de 2.048.
    #[test]
    fn la_que_no_cabe_en_su_bufer_no_se_lee() {
        let leidas = Cell::new(0);
        let grande = BUFER as u16 + 1;
        let v = juzgar(&plan(), 0, llego(grande), |_, _| {
            leidas.set(leidas.get() + 1);
            &[]
        });
        assert_eq!(v, Veredicto::NoCabe(grande));
        assert_eq!(leidas.get(), 0, "se miraron bytes de una trama que no cabia");
        assert!(v.devuelve(), "tambien se devuelve: parar aqui era el mismo atasco");
    }

    #[test]
    fn la_que_cabe_se_lee_una_vez_y_se_entrega_entera() {
        let t = trama(0x0806, 60);
        let leidas = Cell::new(0);
        let v = juzgar(&plan(), 3, llego(60), |fis, largo| {
            leidas.set(leidas.get() + 1);
            assert_eq!(fis, plan().bufer(3).unwrap(), "se leyo de otro bufer");
            assert_eq!(largo, 60);
            &t[..]
        });
        assert_eq!(leidas.get(), 1);
        match v {
            Veredicto::Entregar { cabecera, trama } => {
                assert_eq!(cabecera.ethertype, 0x0806);
                assert_eq!(trama.len(), 60);
            }
            otro => panic!("{otro:?}"),
        }
    }

    #[test]
    fn la_que_no_llega_a_cabecera_es_corta_y_no_se_entrega() {
        let t = trama(0x0800, 14);
        let v = juzgar(&plan(), 0, llego(13), |_, largo| &t[..largo as usize]);
        assert_eq!(v, Veredicto::Corta(13));
        assert!(v.devuelve());
    }

    #[test]
    fn cada_tipo_cae_en_su_casilla() {
        assert_eq!(Tipo::de(0x0806), Tipo::Arp);
        assert_eq!(Tipo::de(0x0800), Tipo::Ipv4);
        assert_eq!(Tipo::de(0x86DD), Tipo::Ipv6);
        // Al reves de orden de bytes: 0x0608 NO es ARP. Es el fallo que
        // `EthHeader::parse` evita, y si se colara aqui se veria como "otro".
        assert_eq!(Tipo::de(0x0608), Tipo::Otro);
        let casillas: Vec<usize> =
            [Tipo::Arp, Tipo::Ipv4, Tipo::Ipv6, Tipo::Otro].iter().map(|t| t.casilla()).collect();
        assert_eq!(casillas, vec![0, 1, 2, 3]);
    }

    /// Los contadores suman lo mismo que sumaban los cinco `static mut` del
    /// kernel: malas y "no cabe" juntas, cortas aparte, y `Parar` no cuenta.
    #[test]
    fn los_contadores_apuntan_cada_veredicto_en_su_sitio() {
        let arp = trama(0x0806, 60);
        let ip = trama(0x0800, 100);
        let h = |t: &[u8]| EthHeader::parse(t).unwrap();
        let mut c = Contadores::nuevo();
        for v in [
            Veredicto::Parar,
            Veredicto::Mala(Llegada::ConError),
            Veredicto::NoCabe(9000),
            Veredicto::Corta(10),
            Veredicto::Entregar { cabecera: h(&arp), trama: &arp },
            Veredicto::Entregar { cabecera: h(&ip), trama: &ip },
        ] {
            c.apuntar(&v);
        }
        assert_eq!(
            c,
            Contadores { tramas: 2, bytes: 160, tipos: [1, 1, 0, 0], cortas: 1, malas: 2 }
        );
    }

    #[test]
    fn un_contador_da_la_vuelta_en_vez_de_parar_la_maquina() {
        let mut c = Contadores { malas: u64::MAX, ..Contadores::nuevo() };
        c.apuntar(&Veredicto::Mala(Llegada::Enana));
        assert_eq!(c.malas, 0);
    }
}
