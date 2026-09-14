//! **TRANSMITIR** -- el anillo de salida, el registro de vuelos y el GRIFO.
//!
//! [carril]  ROJO      el corral de donde la tarjeta LEE lo que pone en el cable,
//!                     y el grifo que decide que sale
//! [cuesta]  APARATO   programa el anillo de transmision de una tarjeta de red
//! [riesgo]  SILENCIO  una trama mal armada no falla aqui: sale al cable, o la
//!                     tarjeta lee memoria que no es del corral
//!
//! # E1 de `docs/plan/PLAN_RED_TX.md` (2026-09-13)
//!
//! Eddi: *"dale, eso es buen plan pero mas maduro"*. Maduro quiere decir que la
//! primera trama que salga por el cable del Ryzen no sea la primera vez que corre
//! este codigo. Aqui vive TODO lo que se puede equivocar sin fallo y se puede
//! probar sin tarjeta; el kernel, en E3, solo copia lo que esto aprueba -- la
//! misma forma que [`crate::anillo`] ya tiene para recibir.
//!
//! ```text
//!    el PLANO     donde cae cada descriptor y cada bufer dentro del corral,
//!                 con UN solo EOR, y ningun largo que salga del bufer
//!    los VUELOS   que casilla tiene la tarjeta, en que orden vuelven, y cual
//!                 lleva demasiado tiempo fuera. Es lo que el kernel cruza con
//!                 el titular del DMA (`mm/titular`, APARATO_NIC)
//!    el GRIFO     que puede salir: cerrado por defecto, se abre A MANO con
//!                 caducidad y cupo, con tope por segundo, solo con nuestra MAC
//!                 de origen, y solo ARP o IPv4
//! ```
//!
//! # *** Por que el grifo vive AQUI y no en la pila de Ring 3
//!
//! `bmo-pila` ya rechaza lo que no debe salir. Pero la pila es de USUARIO: un
//! proceso con `KIND_RED` podria saltarsela y escribir la trama a mano. El grifo
//! es la ultima puerta antes del cable, y la aplica el kernel -- **que no sabe lo
//! que es una IP** (`RED_MAESTRO.md`) y por eso solo mira lo que es de Ethernet:
//! quien la firma (la MAC de origen), cuanto mide, de que tipo es y a que ritmo.
//!
//! [!] Una MAC de origen ajena es exactamente la suplantacion que un proceso
//! comprometido intentaria primero. Se rechaza por su nombre.

use crate::anillo::Falta;
use crate::Mac;

/// Descriptores del anillo de salida. Los mismos que el de entrada, por la misma
/// razon: una sola reserva contigua y una sola pagina de descriptores.
pub const ANILLO: usize = 16;
/// Bytes por bufer. Una trama sin jumbo cabe con sitio y en una pagina.
pub const BUFER: u64 = 2048;
/// Bytes de un descriptor. Contrato de hardware.
pub const DESC: u64 = 16;
/// `TNPDS` pide la base del anillo alineada a 256, igual que `RDSAR`.
pub const ALINEACION: u64 = 256;
/// La trama mas corta que se entrega a la tarjeta, sin FCS (la FCS la pone ella).
pub const MINIMA: usize = 60;
/// La mas larga, sin FCS: 14 de cabecera y 1500 de carga.
pub const MAXIMA: usize = 1514;

/// Registros de la familia 8169/8168 para transmitir. **Se escriben en E3**, no
/// antes: aqui estan para que la prueba y el kernel usen el MISMO numero.
pub mod reg_tx {
    /// `TxDescStartAddrLow`: donde empieza el anillo de prioridad normal.
    pub const TNPDS_LO: usize = 0x20;
    /// `TxDescStartAddrHigh`.
    pub const TNPDS_HI: usize = 0x24;
    /// `TxPoll` de la 8168: escribir `NPQ` es tocar la campana.
    pub const TPPOLL: usize = 0x38;
    /// `TxConfig`, 32 bits.
    pub const TCR: usize = 0x40;
}

/// **Lo que se escribe en `TCR`**: rafaga de DMA sin tope (`7 << 8`) y el hueco
/// entre tramas estandar de 96 bits (`3 << 24`). Son los numeros del driver
/// r8169 de Linux para esta familia: aqui no hay nada que decidir, y un hueco
/// distinto es una trama que el otro extremo no separa de la anterior.
pub const TCR_VALOR: u32 = (0x03 << 24) | (0x07 << 8);

/// Bits de `TxPoll`.
pub mod tppoll {
    /// Hay algo nuevo en la cola de prioridad normal.
    pub const NPQ: u8 = 0x40;
}

/// Bits de `opts1` en un descriptor de salida.
pub mod tx {
    /// Puesto por nosotros = la TARJETA lo tiene y lo va a enviar. Lo quita ella
    /// cuando ha terminado.
    pub const OWN: u32 = 1 << 31;
    /// Fin del anillo. En el ultimo, y solo en el ultimo.
    pub const EOR: u32 = 1 << 30;
    /// Primer segmento de la trama.
    pub const FS: u32 = 1 << 29;
    /// Ultimo segmento. Con `FS`, la trama cabe entera en este descriptor.
    pub const LS: u32 = 1 << 28;
    /// El largo de la trama, 16 bits.
    pub const LEN_MASK: u32 = 0xFFFF;
}

/// Un descriptor de salida: 16 bytes con el mismo contrato que el de entrada.
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct TxDesc {
    pub opts1: u32,
    pub opts2: u32,
    pub addr_lo: u32,
    pub addr_hi: u32,
}

impl TxDesc {
    /// Un descriptor QUIETO: con su bufer y sin `OWN`, asi que la tarjeta no lo
    /// envia. Asi se arma el anillo entero antes de encender nada.
    fn quieto(buf: u64, ultimo: bool) -> Self {
        TxDesc {
            opts1: if ultimo { tx::EOR } else { 0 },
            opts2: 0,
            addr_lo: (buf & 0xFFFF_FFFF) as u32,
            addr_hi: (buf >> 32) as u32,
        }
    }

    /// Un descriptor LISTO PARA ENVIAR `largo` bytes: `OWN | FS | LS | largo`.
    fn para_enviar(buf: u64, largo: u16, ultimo: bool) -> Self {
        let mut d = Self::quieto(buf, ultimo);
        d.opts1 |= tx::OWN | tx::FS | tx::LS | (largo as u32 & tx::LEN_MASK);
        d
    }

    /// La tiene todavia la tarjeta? `false` = ya la envio, se puede recoger.
    pub fn lo_tiene_la_tarjeta(&self) -> bool {
        self.opts1 & tx::OWN != 0
    }
}

/// Bytes que hay que reservar para el corral de salida.
pub const fn bytes_necesarios() -> u64 {
    DESC * ANILLO as u64 + BUFER * ANILLO as u64
}

/// **El plano del corral de salida.** Se valida una vez y despues solo se
/// consulta, igual que [`crate::anillo::Plan`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Plan {
    base: u64,
    bytes: u64,
}

impl Plan {
    pub fn nuevo(base: u64, bytes: u64) -> Result<Plan, Falta> {
        if base % ALINEACION != 0 {
            return Err(Falta::NoAlineada);
        }
        if base.checked_add(bytes).is_none() {
            return Err(Falta::Desborda);
        }
        if bytes < bytes_necesarios() {
            return Err(Falta::Pequena);
        }
        Ok(Plan { base, bytes })
    }

    /// Donde empieza el anillo: lo que va a `TNPDS`.
    pub fn descriptores(&self) -> u64 {
        self.base
    }

    /// Direccion fisica del bufer `i`, o `None` si `i` no existe.
    pub fn bufer(&self, i: usize) -> Option<u64> {
        if i >= ANILLO {
            return None;
        }
        Some(self.base + DESC * ANILLO as u64 + BUFER * i as u64)
    }

    /// Esta este trozo dentro del corral? Con `checked_add`, como el de entrada.
    pub fn contiene(&self, dir: u64, largo: u64) -> bool {
        match dir.checked_add(largo) {
            None => false,
            Some(fin) => dir >= self.base && fin <= self.base + self.bytes,
        }
    }

    /// El descriptor `i` QUIETO, para armar el anillo sin enviar nada.
    pub fn quieto(&self, i: usize) -> Option<TxDesc> {
        let buf = self.bufer(i)?;
        if !self.contiene(buf, BUFER) {
            return None;
        }
        Some(TxDesc::quieto(buf, i == ANILLO - 1))
    }

    /// El descriptor `i` listo para enviar `largo` bytes, o `None` si el largo no
    /// es el de una trama que puede salir. **El largo lo decide el grifo antes**;
    /// esto lo vuelve a comprobar porque es el numero que la tarjeta obedece.
    pub fn para_enviar(&self, i: usize, largo: usize) -> Option<TxDesc> {
        let buf = self.bufer(i)?;
        if !(MINIMA..=MAXIMA).contains(&largo) || !self.contiene(buf, largo as u64) {
            return None;
        }
        Some(TxDesc::para_enviar(buf, largo as u16, i == ANILLO - 1))
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

// =====================================================================
//  LOS VUELOS -- quien tiene cada casilla, y desde cuando
// =====================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Casilla {
    Libre,
    EnVuelo { desde: u64 },
}

/// Por que una casilla no puede despegar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoDespega {
    /// Las 16 las tiene la tarjeta.
    AnilloLleno,
    /// No es la casilla que toca: el anillo se recorre en orden o la tarjeta se
    /// salta tramas.
    NoToca,
    /// Ya estaba en vuelo: reprogramar un bufer que la tarjeta esta leyendo.
    YaEnVuelo,
}

/// Lo que paso al mirar la casilla mas vieja.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Recogida {
    /// No hay nada en vuelo, o la mas vieja aun la tiene la tarjeta.
    Nada,
    /// Volvio: `casilla` queda libre y `tardo` ticks estuvo fuera.
    Aterrizo { casilla: usize, tardo: u64 },
}

/// **El registro de vuelos del anillo de salida.** Determinista: el tiempo lo
/// pone quien llama, y el orden es el del anillo.
pub struct Vuelos {
    casillas: [Casilla; ANILLO],
    siguiente: usize,
    mas_vieja: usize,
    en_vuelo: usize,
    pub despegues: u64,
    pub aterrizajes: u64,
}

impl Default for Vuelos {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Vuelos {
    pub const fn nuevo() -> Self {
        Self { casillas: [Casilla::Libre; ANILLO], siguiente: 0, mas_vieja: 0, en_vuelo: 0, despegues: 0, aterrizajes: 0 }
    }

    pub fn en_vuelo(&self) -> usize {
        self.en_vuelo
    }

    pub fn casilla(&self, i: usize) -> Option<Casilla> {
        self.casillas.get(i).copied()
    }

    /// La casilla que toca usar ahora, sin marcarla.
    pub fn proxima(&self) -> Result<usize, NoDespega> {
        if self.en_vuelo == ANILLO {
            return Err(NoDespega::AnilloLleno);
        }
        Ok(self.siguiente)
    }

    /// **Despega `casilla`.** El kernel lo llama DESPUES de apuntarla en el
    /// titular del DMA y ANTES de tocar la campana.
    pub fn despegar(&mut self, casilla: usize, ahora: u64) -> Result<(), NoDespega> {
        let toca = self.proxima()?;
        if casilla != toca {
            return Err(NoDespega::NoToca);
        }
        if self.casillas[casilla] != Casilla::Libre {
            return Err(NoDespega::YaEnVuelo);
        }
        self.casillas[casilla] = Casilla::EnVuelo { desde: ahora };
        self.siguiente = (self.siguiente + 1) % ANILLO;
        self.en_vuelo += 1;
        self.despegues += 1;
        Ok(())
    }

    /// La casilla mas vieja en vuelo, si hay alguna: la que el kernel tiene que
    /// leer para saber si ya volvio.
    pub fn mas_vieja(&self) -> Option<usize> {
        if self.en_vuelo == 0 {
            None
        } else {
            Some(self.mas_vieja)
        }
    }

    /// **Recoge la mas vieja** si la tarjeta ya la solto (`lo_tiene` = su `OWN`).
    /// Se llama en bucle hasta `Nada`.
    pub fn recoger(&mut self, lo_tiene: bool, ahora: u64) -> Recogida {
        let Some(i) = self.mas_vieja() else { return Recogida::Nada };
        if lo_tiene {
            return Recogida::Nada;
        }
        let Casilla::EnVuelo { desde } = self.casillas[i] else { return Recogida::Nada };
        self.casillas[i] = Casilla::Libre;
        self.mas_vieja = (self.mas_vieja + 1) % ANILLO;
        self.en_vuelo -= 1;
        self.aterrizajes += 1;
        Recogida::Aterrizo { casilla: i, tardo: ahora.saturating_sub(desde) }
    }

    /// La mas vieja, si lleva fuera `plazo` ticks o mas: una tarjeta que no
    /// devuelve es la que el titular llama CADUCADO (R-DMA-8).
    pub fn caducada(&self, ahora: u64, plazo: u64) -> Option<(usize, u64)> {
        let i = self.mas_vieja()?;
        match self.casillas[i] {
            Casilla::EnVuelo { desde } if ahora.saturating_sub(desde) >= plazo => Some((i, ahora - desde)),
            _ => None,
        }
    }
}

// =====================================================================
//  EL GRIFO -- que puede salir al cable
// =====================================================================

/// El tiempo mas largo que se deja abierto el grifo de una vez: diez minutos a
/// 1 kHz. Abrirlo "para siempre" no existe: se vuelve a abrir.
pub const DURACION_MAX: u64 = 10 * 60 * 1000;
/// Las tramas mas que se conceden de una vez.
pub const CUPO_MAX: u32 = 10_000;
/// Tramas por segundo como mucho. Un ARP y un ping no necesitan mas; una
/// inundacion si, y esa es la que no sale.
pub const RITMO_MAX: u32 = 50;
/// Ticks en un segundo (el reloj del kernel late a 1 kHz).
pub const SEGUNDO: u64 = 1000;

/// **Por que una trama no sale.** Uno por motivo, y ninguno significa dos cosas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoSale {
    /// El grifo no se ha abierto.
    Cerrado,
    /// Se abrio y paso su plazo. Queda cerrado.
    Caducado,
    /// Se gastaron las tramas concedidas.
    SinCupo,
    /// Mas de `RITMO_MAX` en este segundo.
    Ritmo,
    /// Menos de 14 bytes: no es una trama Ethernet.
    Corta,
    /// Mas de `MAXIMA`.
    Larga,
    /// La MAC de origen no es la nuestra: suplantacion.
    OrigenAjeno,
    /// Destino todo a ceros.
    DestinoImposible,
    /// Ni ARP ni IPv4 (VLAN, IPv6, 802.3 con largo...).
    Tipo,
    /// El bufer de salida no da para la trama con su relleno.
    SinSitio,
}

impl NoSale {
    /// El numero que viaja al buzon (`ULTIMO_NO`). Empieza en 1: el cero es
    /// "la ultima salio".
    pub fn codigo(self) -> u32 {
        self as u32 + 1
    }

    pub fn texto(self) -> &'static str {
        match self {
            NoSale::Cerrado => "el grifo esta cerrado",
            NoSale::Caducado => "el grifo se abrio y caduco",
            NoSale::SinCupo => "se gasto el cupo de tramas",
            NoSale::Ritmo => "demasiadas tramas en este segundo",
            NoSale::Corta => "menos de 14 bytes: no es Ethernet",
            NoSale::Larga => "mas de 1514 bytes",
            NoSale::OrigenAjeno => "la MAC de origen no es la nuestra",
            NoSale::DestinoImposible => "destino todo a ceros",
            NoSale::Tipo => "ni ARP ni IPv4",
            NoSale::SinSitio => "no cabe en el bufer de salida",
        }
    }
}

/// **El grifo.** Cerrado al nacer; se abre a mano, caduca solo.
pub struct Grifo {
    hasta: u64,
    abierto: bool,
    cupo: u32,
    segundo: u64,
    en_este: u32,
    pub salieron: u64,
    pub negadas: u64,
    pub ultimo_no: Option<NoSale>,
}

impl Default for Grifo {
    fn default() -> Self {
        Self::cerrado()
    }
}

impl Grifo {
    pub const fn cerrado() -> Self {
        Self { hasta: 0, abierto: false, cupo: 0, segundo: 0, en_este: 0, salieron: 0, negadas: 0, ultimo_no: None }
    }

    /// **Abre** durante `duracion` ticks y `cupo` tramas, recortados a sus topes.
    /// Devuelve lo que de verdad se concedio, para que quien lo pidio lo vea.
    pub fn abrir(&mut self, ahora: u64, duracion: u64, cupo: u32) -> (u64, u32) {
        let d = duracion.min(DURACION_MAX);
        let c = cupo.min(CUPO_MAX);
        self.abierto = d > 0 && c > 0;
        self.hasta = ahora.saturating_add(d);
        self.cupo = c;
        (d, c)
    }

    pub fn cerrar(&mut self) {
        self.abierto = false;
        self.cupo = 0;
    }

    /// Las tramas que quedan por conceder. El radar revoca en el cero.
    pub fn cupo(&self) -> u32 {
        self.cupo
    }

    pub fn abierto(&self, ahora: u64) -> bool {
        self.abierto && ahora < self.hasta && self.cupo > 0
    }

    /// **Juzga una trama.** Si pasa, la copia a `salida` con relleno de ceros
    /// hasta `MINIMA` y devuelve el largo. Si no, dice por que -- y no consume
    /// ni cupo ni ritmo.
    ///
    /// El orden es el de las preguntas baratas primero y el ritmo al final: una
    /// trama mal formada no le gasta el segundo a una buena.
    pub fn juzgar(&mut self, trama: &[u8], mi_mac: Mac, ahora: u64, salida: &mut [u8]) -> Result<usize, NoSale> {
        let r = self.juzgar_sin_contar(trama, mi_mac, ahora, salida);
        match r {
            Ok(_) => {
                self.salieron += 1;
                self.ultimo_no = None;
            }
            Err(n) => {
                self.negadas += 1;
                self.ultimo_no = Some(n);
            }
        }
        r
    }

    fn juzgar_sin_contar(&mut self, trama: &[u8], mi_mac: Mac, ahora: u64, salida: &mut [u8]) -> Result<usize, NoSale> {
        if !self.abierto {
            return Err(NoSale::Cerrado);
        }
        if ahora >= self.hasta {
            self.cerrar();
            return Err(NoSale::Caducado);
        }
        if self.cupo == 0 {
            return Err(NoSale::SinCupo);
        }
        if trama.len() < 14 {
            return Err(NoSale::Corta);
        }
        if trama.len() > MAXIMA {
            return Err(NoSale::Larga);
        }
        if trama[6..12] != mi_mac {
            return Err(NoSale::OrigenAjeno);
        }
        if trama[0..6] == [0u8; 6] {
            return Err(NoSale::DestinoImposible);
        }
        let tipo = u16::from_be_bytes([trama[12], trama[13]]);
        if tipo != 0x0806 && tipo != 0x0800 {
            return Err(NoSale::Tipo);
        }
        let largo = trama.len().max(MINIMA);
        if salida.len() < largo {
            return Err(NoSale::SinSitio);
        }
        if ahora.saturating_sub(self.segundo) >= SEGUNDO {
            self.segundo = ahora;
            self.en_este = 0;
        }
        if self.en_este >= RITMO_MAX {
            return Err(NoSale::Ritmo);
        }
        salida[..trama.len()].copy_from_slice(trama);
        salida[trama.len()..largo].fill(0);
        self.en_este += 1;
        self.cupo -= 1;
        Ok(largo)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const YO: Mac = [0x02, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E];
    const ROUTER: Mac = [0x02, 0, 0, 0, 0, 0x01];

    fn plan() -> Plan {
        Plan::nuevo(0x1_0000_0000, bytes_necesarios()).unwrap()
    }

    fn trama(origen: Mac, tipo: u16, largo: usize) -> Vec<u8> {
        let mut t = vec![0u8; largo];
        t[0..6].copy_from_slice(&[0xFF; 6]);
        t[6..12].copy_from_slice(&origen);
        t[12..14].copy_from_slice(&tipo.to_be_bytes());
        t
    }

    // -- el plano ----------------------------------------------------------

    #[test]
    fn la_cuenta_y_las_arenas_que_no_valen() {
        assert_eq!(bytes_necesarios(), 16 * 16 + 2048 * 16);
        assert_eq!(Plan::nuevo(0x1000 + 1, bytes_necesarios()), Err(Falta::NoAlineada));
        assert_eq!(Plan::nuevo(0x1000, bytes_necesarios() - 1), Err(Falta::Pequena));
        assert_eq!(Plan::nuevo(u64::MAX - 255, bytes_necesarios()), Err(Falta::Desborda));
    }

    #[test]
    fn ningun_bufer_se_sale_ni_pisa_los_descriptores() {
        let p = plan();
        let fin_desc = p.descriptores() + DESC * ANILLO as u64;
        for i in 0..ANILLO {
            let b = p.bufer(i).unwrap();
            assert!(b >= fin_desc && p.contiene(b, BUFER), "bufer {i}");
        }
        assert_eq!(p.bufer(ANILLO), None);
    }

    /// *** EXACTAMENTE UN `EOR`, en el anillo quieto y en el que envia.
    #[test]
    fn un_solo_eor_y_en_el_ultimo() {
        let p = plan();
        let quietos = (0..ANILLO).filter(|&i| p.quieto(i).unwrap().opts1 & tx::EOR != 0).count();
        assert_eq!(quietos, 1);
        assert_ne!(p.quieto(ANILLO - 1).unwrap().opts1 & tx::EOR, 0);
        let enviando = (0..ANILLO).filter(|&i| p.para_enviar(i, 60).unwrap().opts1 & tx::EOR != 0).count();
        assert_eq!(enviando, 1);
    }

    #[test]
    fn el_quieto_no_envia_y_el_listo_lleva_todo() {
        let p = plan();
        let q = p.quieto(3).unwrap();
        assert!(!q.lo_tiene_la_tarjeta(), "un anillo armado no puede enviar solo");
        let d = p.para_enviar(3, 342).unwrap();
        assert!(d.lo_tiene_la_tarjeta());
        assert_eq!(d.opts1 & (tx::FS | tx::LS), tx::FS | tx::LS);
        assert_eq!(d.opts1 & tx::LEN_MASK, 342);
        assert_eq!(((d.addr_hi as u64) << 32) | d.addr_lo as u64, p.bufer(3).unwrap());
    }

    #[test]
    fn un_largo_que_no_es_trama_no_se_programa() {
        let p = plan();
        assert_eq!(p.para_enviar(0, MINIMA - 1), None);
        assert_eq!(p.para_enviar(0, MAXIMA + 1), None);
        assert!(p.para_enviar(0, MINIMA).is_some());
        assert!(p.para_enviar(0, MAXIMA).is_some());
        assert_eq!(p.para_enviar(ANILLO, 100), None);
    }

    // -- los vuelos --------------------------------------------------------

    #[test]
    fn despegan_en_orden_y_aterrizan_en_orden() {
        let mut v = Vuelos::nuevo();
        for t in 0..3u64 {
            let i = v.proxima().unwrap();
            v.despegar(i, 100 + t).unwrap();
        }
        assert_eq!(v.en_vuelo(), 3);
        assert_eq!(v.recoger(true, 200), Recogida::Nada, "la tarjeta aun la tiene");
        assert_eq!(v.recoger(false, 200), Recogida::Aterrizo { casilla: 0, tardo: 100 });
        assert_eq!(v.recoger(false, 200), Recogida::Aterrizo { casilla: 1, tardo: 99 });
        assert_eq!(v.recoger(false, 210), Recogida::Aterrizo { casilla: 2, tardo: 108 });
        assert_eq!(v.recoger(false, 210), Recogida::Nada);
        assert_eq!((v.despegues, v.aterrizajes, v.en_vuelo()), (3, 3, 0));
    }

    #[test]
    fn saltarse_una_casilla_o_repetirla_es_no() {
        let mut v = Vuelos::nuevo();
        assert_eq!(v.despegar(5, 0), Err(NoDespega::NoToca));
        v.despegar(0, 0).unwrap();
        assert_eq!(v.despegar(0, 1), Err(NoDespega::NoToca), "ya toca la 1");
    }

    #[test]
    fn lleno_a_16_y_da_la_vuelta_tres_veces() {
        let mut v = Vuelos::nuevo();
        for vuelta in 0..3u64 {
            for i in 0..ANILLO {
                assert_eq!(v.proxima(), Ok(i));
                v.despegar(i, vuelta * 100 + i as u64).unwrap();
            }
            assert_eq!(v.proxima(), Err(NoDespega::AnilloLleno));
            for i in 0..ANILLO {
                assert!(matches!(v.recoger(false, vuelta * 100 + 50), Recogida::Aterrizo { casilla, .. } if casilla == i));
            }
            assert_eq!(v.en_vuelo(), 0);
        }
    }

    #[test]
    fn la_caducada_se_ve_al_tick_exacto() {
        let mut v = Vuelos::nuevo();
        v.despegar(0, 1_000).unwrap();
        assert_eq!(v.caducada(1_999, 1_000), None);
        assert_eq!(v.caducada(2_000, 1_000), Some((0, 1_000)));
    }

    // -- el grifo ----------------------------------------------------------

    #[test]
    fn nace_cerrado() {
        let mut g = Grifo::cerrado();
        let mut s = [0u8; 1600];
        assert_eq!(g.juzgar(&trama(YO, 0x0806, 42), YO, 0, &mut s), Err(NoSale::Cerrado));
        assert_eq!((g.salieron, g.negadas, g.ultimo_no), (0, 1, Some(NoSale::Cerrado)));
    }

    #[test]
    fn abrir_recorta_a_los_topes_y_caduca_al_tick() {
        let mut g = Grifo::cerrado();
        assert_eq!(g.abrir(10, u64::MAX, u32::MAX), (DURACION_MAX, CUPO_MAX));
        assert_eq!(g.abrir(10, 500, 3), (500, 3));
        let mut s = [0u8; 1600];
        assert!(g.juzgar(&trama(YO, 0x0806, 42), YO, 509, &mut s).is_ok());
        assert_eq!(g.juzgar(&trama(YO, 0x0806, 42), YO, 510, &mut s), Err(NoSale::Caducado));
        assert_eq!(g.juzgar(&trama(YO, 0x0806, 42), YO, 511, &mut s), Err(NoSale::Cerrado), "caducado queda cerrado");
    }

    #[test]
    fn el_cupo_se_gasta_y_lo_malo_no_lo_gasta() {
        let mut g = Grifo::cerrado();
        g.abrir(0, 10_000, 2);
        let mut s = [0u8; 1600];
        assert_eq!(g.juzgar(&trama(ROUTER, 0x0806, 42), YO, 1, &mut s), Err(NoSale::OrigenAjeno));
        assert!(g.juzgar(&trama(YO, 0x0806, 42), YO, 2, &mut s).is_ok());
        assert!(g.juzgar(&trama(YO, 0x0800, 98), YO, 3, &mut s).is_ok());
        assert_eq!(g.juzgar(&trama(YO, 0x0806, 42), YO, 4, &mut s), Err(NoSale::SinCupo));
    }

    #[test]
    fn el_ritmo_por_segundo_y_se_renueva() {
        let mut g = Grifo::cerrado();
        g.abrir(0, DURACION_MAX, CUPO_MAX);
        let mut s = [0u8; 1600];
        for k in 0..RITMO_MAX {
            assert!(g.juzgar(&trama(YO, 0x0806, 42), YO, 5 + k as u64, &mut s).is_ok());
        }
        assert_eq!(g.juzgar(&trama(YO, 0x0806, 42), YO, 900, &mut s), Err(NoSale::Ritmo));
        assert!(g.juzgar(&trama(YO, 0x0806, 42), YO, 5 + SEGUNDO, &mut s).is_ok(), "al segundo siguiente, otra vez");
    }

    #[test]
    fn lo_que_no_es_arp_ni_ipv4_y_los_largos_imposibles() {
        let mut g = Grifo::cerrado();
        g.abrir(0, DURACION_MAX, CUPO_MAX);
        let mut s = [0u8; 1600];
        for tipo in [0x86DDu16, 0x8100, 0x0042] {
            assert_eq!(g.juzgar(&trama(YO, tipo, 60), YO, 1, &mut s), Err(NoSale::Tipo), "{tipo:#x}");
        }
        assert_eq!(g.juzgar(&trama(YO, 0x0800, 14)[..13], YO, 1, &mut s), Err(NoSale::Corta));
        assert_eq!(g.juzgar(&trama(YO, 0x0800, 1515), YO, 1, &mut s), Err(NoSale::Larga));
        let mut ceros = trama(YO, 0x0806, 42);
        ceros[0..6].fill(0);
        assert_eq!(g.juzgar(&ceros, YO, 1, &mut s), Err(NoSale::DestinoImposible));
        assert_eq!(g.juzgar(&trama(YO, 0x0806, 42), YO, 1, &mut [0u8; 59]), Err(NoSale::SinSitio));
    }

    /// ** El relleno es de CEROS: lo que hubiera en el bufer es la trama de antes.
    #[test]
    fn la_corta_sale_con_relleno_de_ceros() {
        let mut g = Grifo::cerrado();
        g.abrir(0, DURACION_MAX, CUPO_MAX);
        let mut s = [0xAAu8; 1600];
        let t = trama(YO, 0x0806, 42);
        assert_eq!(g.juzgar(&t, YO, 1, &mut s), Ok(MINIMA));
        assert_eq!(&s[..42], &t[..]);
        assert!(s[42..MINIMA].iter().all(|&b| b == 0));
    }

    #[test]
    fn veinte_mil_tramas_mutadas_no_revientan() {
        let mut g = Grifo::cerrado();
        g.abrir(0, DURACION_MAX, CUPO_MAX);
        let base = trama(YO, 0x0800, 200);
        let mut semilla = 0xC0FFEEu64;
        let mut azar = || {
            semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semilla >> 33) as usize
        };
        let mut s = [0u8; 1600];
        for k in 0..20_000u64 {
            let mut m = base.clone();
            for _ in 0..1 + azar() % 4 {
                let i = azar() % m.len();
                m[i] = azar() as u8;
            }
            m.truncate(azar() % (m.len() + 1));
            let _ = g.juzgar(&m, YO, k, &mut s);
        }
        assert_eq!(g.salieron + g.negadas, 20_000);
    }
}
