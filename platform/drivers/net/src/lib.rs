//! **RED -- de momento, solo RECONOCER.**
//!
//! ## Que habia aqui antes, y por que se borro
//!
//! 287 lineas de driver de **Intel e1000** que no llamaba nadie. La NIC de esta
//! maquina es `PCI\VEN_10EC&DEV_8168` --una Realtek RTL8111/8168-- y el e1000 es
//! la NIC por defecto de **QEMU**: aquel codigo no habria encendido un LED en el
//! Ryzen. Sigue en el historial (`git log -- platform/drivers/net`).
//!
//! ## El contrato que va a ocupar este sitio
//!
//! Se escribe antes que el driver, que es la regla de la casa:
//!
//! ```text
//!    Ring 0                          Ring 3
//!    ------                          ------
//!    KIND_RED                        ARP, IP, TCP, DNS, TLS
//!    tramas Ethernet crudas          todo lo que tiene versiones
//!    la MAC, el enlace, el DMA       y por tanto se equivoca
//! ```
//!
//! **El kernel no sabe lo que es una IP.** Una pila TCP es la superficie de
//! ataque mas grande de un sistema conectado, y aqui se puede morir sin llevarse
//! la maquina. Windows y Linux la tienen dentro del nucleo porque en 1990 no
//! habia otra forma.
//!
//! ## ** Y por que HOY esto solo mira ==
//!
//! Un anillo de descriptores mal armado no da un fallo: da **el disco de red
//! escribiendo en memoria de otro**, y el sintoma tres arranques despues. Antes
//! de programar un solo DMA hay que contestar tres preguntas que no cuestan
//! nada y que ninguna teoria puede contestar:
//!
//! 1. La tarjeta que encuentra el barrido, **es la que dice Windows**?
//! 2. El BAR que se eligio, **es el que lleva a los registros**?
//! 3. El cable, **esta enchufado**?
//!
//! Las tres se responden con lecturas, sin escribir un byte en el aparato. Y
//! hay algo mejor: **se pueden predecir antes de mirar**. El Windows de esta
//! misma maquina dice `2C-F0-5D-D9-3C-E3`, enlace arriba a 100 Mbps. Si el
//! kernel imprime eso, las tres estan contestadas de golpe y el driver se
//! empieza sobre suelo firme. Si imprime otra cosa, **el numero dice cual de las
//! tres fallo**, que es justo lo que un `no funciona` no dice.
//!
//! Es el metodo de las cinco sondas del `#GP` de julio: predecir, leer, comparar.

// `no_std` en el kernel, `std` en el banco: la interpretacion de los bits SI se
// puede probar en el anfitrion, y es justo la parte que se equivoca en silencio.
#![cfg_attr(not(test), no_std)]

/// **El plano del anillo de recepcion y el corral del DMA.** Aparte porque es
/// la aritmetica que decide donde puede escribir un aparato, y esa es la unica
/// parte de un driver que se equivoca SIN dar un fallo.
pub mod anillo;

/// Seis bytes. El nombre existe para que una firma no diga `[u8; 6]` y deje al
/// que lee adivinando si son bytes de MAC o de otra cosa.
pub type Mac = [u8; 6];

/// **Lo que un RTL8111/8168 cuenta de si mismo sin que se le configure nada.**
#[derive(Clone, Copy)]
pub struct Identidad {
    /// La direccion fisica, tal como el chip la cargo de su EEPROM al arrancar.
    pub mac: Mac,
    /// El registro `PHYstatus` crudo. Se guarda **sin interpretar** ademas de
    /// interpretado: el dia que un bit no cuadre, el byte entero es la prueba y
    /// las funciones de abajo son la opinion.
    pub phy: u8,
}

/// Registros del RTL8169/8168 que hacen falta para mirar. Del mapa de la
/// familia, el mismo que usa el `r8169` de Linux.
mod reg {
    /// `IDR0..IDR5`: la MAC, cargada de la EEPROM en el reset. Es de solo
    /// lectura hasta que se desbloquea con `9346CR`, y aqui **no se desbloquea
    /// nada**: se lee y se sale.
    pub const IDR0: usize = 0x00;
    /// `PHYstatus`. Un byte, y cuenta el enlace entero.
    pub const PHY_STATUS: usize = 0x6C;
}

/// Bits de `PHYstatus`.
mod phy {
    pub const LINK_UP: u8 = 0x02;
    pub const FULL_DUPLEX: u8 = 0x01;
    pub const M10: u8 = 0x04;
    pub const M100: u8 = 0x08;
    pub const M1000: u8 = 0x10;
}

/// **Preguntale al chip quien es.** No escribe **nada**.
///
/// `mmio` es la base del BAR de memoria, ya en direccion virtual alcanzable.
///
/// # Safety
/// `mmio` tiene que ser el MMIO de una Realtek de la familia 8169/8168 y estar
/// mapeado. Leer estos offsets en otro aparato devuelve lo que haya ahi -- que
/// es exactamente por que quien llama comprueba el vendor **antes**.
pub unsafe fn identificar(mmio: *mut u8) -> Identidad {
    // Dos lecturas de 32 bits en vez de seis de 8.
    //
    // Los `IDR` admiten acceso por byte, pero varios registros de esta familia
    // NO -- y un MMIO al que se le lee un byte donde pide una palabra puede
    // devolver basura sin que nada falle. Dos dwords cubren los seis bytes y
    // dejan la costumbre puesta para los registros que si son quisquillosos.
    let base = mmio as *const u32;
    let lo = core::ptr::read_volatile(base.add(reg::IDR0 / 4));
    let hi = core::ptr::read_volatile(base.add(reg::IDR0 / 4 + 1));
    let mac: Mac = [
        (lo & 0xFF) as u8,
        ((lo >> 8) & 0xFF) as u8,
        ((lo >> 16) & 0xFF) as u8,
        ((lo >> 24) & 0xFF) as u8,
        (hi & 0xFF) as u8,
        ((hi >> 8) & 0xFF) as u8,
    ];
    let phy = core::ptr::read_volatile(mmio.add(reg::PHY_STATUS));
    Identidad { mac, phy }
}

impl Identidad {
    /// Hay cable, y del otro lado contesta alguien?
    pub fn enlace_arriba(&self) -> bool {
        self.phy & phy::LINK_UP != 0
    }

    /// A cuantos megabits negocio. `0` = sin enlace, o el chip no lo dice.
    ///
    /// ** No se inventa un valor por defecto. Un `1000` supuesto en un enlace
    /// que negocio a 100 no da un error: da un sistema que cree ir diez veces
    /// mas rapido de lo que va, y eso sale como paquetes perdidos mucho despues.
    pub fn megabits(&self) -> u16 {
        if !self.enlace_arriba() {
            return 0;
        }
        match self.phy {
            p if p & phy::M1000 != 0 => 1000,
            p if p & phy::M100 != 0 => 100,
            p if p & phy::M10 != 0 => 10,
            _ => 0,
        }
    }

    /// Duplex completo? En un enlace moderno siempre, y por eso mismo un `false`
    /// aqui es una pista de que se esta leyendo el registro equivocado.
    pub fn duplex_completo(&self) -> bool {
        self.phy & phy::FULL_DUPLEX != 0
    }

    /// **La MAC como un solo numero, para poder pintarla.**
    ///
    /// El byte 0 arriba del todo, asi que en hexadecimal sale en el mismo orden
    /// en que se escribe: `2C:F0:5D:D9:3C:E3` -> `2CF05DD93CE3`. Es lo que
    /// permite comparar de un vistazo con lo que dice cualquier otro sistema, y
    /// esa comparacion es toda la prueba de este paso.
    pub fn mac_u64(&self) -> u64 {
        let m = &self.mac;
        ((m[0] as u64) << 40)
            | ((m[1] as u64) << 32)
            | ((m[2] as u64) << 24)
            | ((m[3] as u64) << 16)
            | ((m[4] as u64) << 8)
            | (m[5] as u64)
    }

    /// **Es una MAC creible?** Ni todo ceros ni todo unos.
    ///
    /// Los dos son lo que devuelve un MMIO que no lleva a ningun sitio: ceros si
    /// la lectura cae en un agujero, `FF` si el aparato no contesta al ciclo.
    /// O sea que esto no comprueba la tarjeta -- **comprueba el BAR**, y es la
    /// diferencia entre "la NIC esta rota" y "le estoy leyendo el sitio que no".
    ///
    /// Tampoco puede ser multicast: el bit 0 del primer byte puesto significa
    /// "grupo", y ninguna tarjeta tiene de fabrica una direccion de grupo.
    pub fn creible(&self) -> bool {
        let ceros = self.mac.iter().all(|&b| b == 0x00);
        let unos = self.mac.iter().all(|&b| b == 0xFF);
        !ceros && !unos && (self.mac[0] & 0x01) == 0
    }
}

// == STEP 1: RECEIVE. NOTHING IS TRANSMITTED. =================================
//
// # Why receiving comes before transmitting, and it is not caution for its own
// # sake
//
// A plugged cable **already carries traffic**: ARP, mDNS, DHCP, router
// broadcasts. So an RX-only ring turns "is there a network?" into a question the
// machine answers by printing bytes **that another computer sent**. No IP, no
// ARP, no stack -- six destination bytes, six source bytes and an ethertype.
//
// And since nothing is transmitted, a mistake here cannot disturb anyone else on
// the network. The worst case stays inside this machine.
//
// # What is dangerous here, said plainly
//
// A badly built descriptor ring does not produce a fault: it produces **the card
// writing into somebody else's memory**, with the symptom three boots later. The
// same mine was already stepped on with the AHCI PRDT. That is why everything in
// this module that can be decided without hardware -- bit layout, sizes,
// ownership, frame length -- lives here as pure functions with tests on the host,
// and the kernel side is left as thin as it can be.

/// One RX descriptor of the RTL8169/8168 family. **16 bytes, and the layout is a
/// hardware contract**: the card reads these fields by offset.
///
/// `repr(C)` is not decoration here. Rust is free to reorder the fields of a
/// plain struct, and a reordered descriptor is not a compile error -- it is a DMA
/// engine reading a buffer address out of the status word.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RxDesc {
    /// Ownership, flags and length. See [`rx`].
    pub opts1: u32,
    /// VLAN. Not used, kept because the descriptor is 16 bytes whether we use it
    /// or not.
    pub opts2: u32,
    /// Physical address of the buffer, low 32 bits.
    pub addr_lo: u32,
    /// ...and high 32 bits. The card is a 64-bit DMA master.
    pub addr_hi: u32,
}

/// Bits of `opts1` in an RX descriptor.
pub mod rx {
    /// Set by the host = the CARD owns the descriptor. Cleared by the card when
    /// it has written a frame into it.
    ///
    /// * The direction is the opposite of what the name suggests the first time
    /// you read it, and getting it backwards means polling forever on a ring that
    /// is working perfectly.
    pub const OWN: u32 = 1 << 31;
    /// End Of Ring. Goes on the LAST descriptor and it is what tells the card to
    /// wrap around. Without it the card walks off the end of the ring and keeps
    /// writing -- which is the memory-corruption case, not a hang.
    pub const EOR: u32 = 1 << 30;
    /// First segment of a frame.
    pub const FS: u32 = 1 << 29;
    /// Last segment of a frame.
    pub const LS: u32 = 1 << 28;
    /// The card marks a receive error here.
    pub const RES: u32 = 1 << 21;
    /// Buffer size on the way in, frame length on the way out. 14 bits.
    pub const LEN_MASK: u32 = 0x3FFF;
}

/// How many bytes of Ethernet FCS the card leaves at the end of the frame.
///
/// The reported length **includes the CRC**, so a minimum-size Ethernet frame
/// comes back as 64 and not 60. Forgetting this does not break anything visibly:
/// it just makes every length four too big, which reads as plausible.
pub const FCS_LEN: u16 = 4;

impl RxDesc {
    /// The descriptor as it must be handed to the card: buffer address, size, and
    /// ownership given away.
    ///
    /// `last` marks the end of the ring. **Exactly one descriptor of a ring must
    /// have it.**
    pub fn to_card(buf_phys: u64, buf_len: u16, last: bool) -> Self {
        let mut opts1 = rx::OWN | (buf_len as u32 & rx::LEN_MASK);
        if last {
            opts1 |= rx::EOR;
        }
        RxDesc {
            opts1,
            opts2: 0,
            addr_lo: (buf_phys & 0xFFFF_FFFF) as u32,
            addr_hi: (buf_phys >> 32) as u32,
        }
    }

    /// Does the card still own it? `true` = nothing has arrived here yet.
    pub fn owned_by_card(&self) -> bool {
        self.opts1 & rx::OWN != 0
    }

    /// Length of the received frame **without the FCS**, or `None` if the
    /// descriptor is not a complete, error-free frame.
    ///
    /// Three things are checked and not one: ownership returned, first AND last
    /// segment present (a frame split across descriptors is not a frame we can
    /// read yet), and no error bit.
    pub fn frame_len(&self) -> Option<u16> {
        if self.owned_by_card() {
            return None;
        }
        if self.opts1 & rx::RES != 0 {
            return None;
        }
        if self.opts1 & (rx::FS | rx::LS) != (rx::FS | rx::LS) {
            return None;
        }
        let with_fcs = (self.opts1 & rx::LEN_MASK) as u16;
        with_fcs.checked_sub(FCS_LEN)
    }
}

/// **The head of an Ethernet frame.** Fourteen bytes, and it is the whole of what
/// Ring 0 needs to understand: everything above this is Ring 3's business.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EthHeader {
    pub dst: Mac,
    pub src: Mac,
    /// `0x0806` = ARP, `0x0800` = IPv4, `0x86DD` = IPv6. Below `0x0600` it is a
    /// length and not a type, which is 802.3 and does not appear on a modern LAN.
    pub ethertype: u16,
}

/// Bytes of an Ethernet header.
pub const ETH_HEADER_LEN: usize = 14;

impl EthHeader {
    /// Reads the header off the front of a frame. `None` if there are not even
    /// fourteen bytes -- which is a runt, not a frame.
    pub fn parse(frame: &[u8]) -> Option<Self> {
        if frame.len() < ETH_HEADER_LEN {
            return None;
        }
        let mut dst: Mac = [0; 6];
        let mut src: Mac = [0; 6];
        dst.copy_from_slice(&frame[0..6]);
        src.copy_from_slice(&frame[6..12]);
        // Ethertype travels BIG-endian on the wire and this machine is
        // little-endian. Reading it the native way turns 0x0806 (ARP) into
        // 0x0608, which matches nothing and looks like an unknown protocol
        // instead of a byte-order bug.
        let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
        Some(EthHeader { dst, src, ethertype })
    }

    /// Was it sent to everybody? An ARP query is, and it is the traffic most
    /// likely to be the first thing this machine ever receives.
    pub fn is_broadcast(&self) -> bool {
        self.dst.iter().all(|&b| b == 0xFF)
    }

    /// The source address as one number, same convention as [`Identidad::mac_u64`]:
    /// printable and comparable at a glance against any other system.
    pub fn src_u64(&self) -> u64 {
        mac_u64(&self.src)
    }

    /// **The DESTINATION as one number**, and it is not a nicety.
    ///
    /// ** Showing only the source cannot tell "the receive filter works" apart
    /// from "the filter is wide open". A card in promiscuous mode and one
    /// filtering correctly produce the *same* source addresses; what differs is
    /// **who the frames were addressed to**. Step 1 is supposed to answer three
    /// questions, and without this it can only answer two.
    pub fn dst_u64(&self) -> u64 {
        mac_u64(&self.dst)
    }

    /// **What the ethertype is CALLED**, or an empty string if we do not know it.
    ///
    /// ** A number is not a reading. `0x0806` means ARP to somebody who has the
    /// table memorised and means nothing to everybody else -- and this line is
    /// meant to be read on a screen, once, by a person deciding whether the
    /// driver works.
    ///
    /// [!] The unknown case returns `"tipo"` bare: the raw number is printed
    /// next to it anyway, and a word like "unknown" would push the number that
    /// says everything off an 80-column line.
    ///
    /// ** It returns the whole LABEL and not just the name because the line it
    /// feeds takes `(mensaje, numero)`: there is no text-only line in CABINA,
    /// and inventing one to print four letters would be a worse trade than
    /// putting the four letters where the message already goes.
    pub fn nombre_del_tipo(&self) -> &'static str {
        match self.ethertype {
            0x0806 => "     tipo ARP",
            0x0800 => "     tipo IPv4",
            0x86DD => "     tipo IPv6",
            0x8100 => "     tipo VLAN",
            // Below 0x0600 the field is a LENGTH, not a type. That is 802.3, and
            // seeing it on a modern LAN is itself the finding.
            n if n < 0x0600 => "     [!] es un LARGO, no un tipo (802.3)",
            _ => "     tipo",
        }
    }
}

/// Six bytes to one number, most significant first -- the order they are written
/// in when a person says a MAC out loud.
fn mac_u64(m: &Mac) -> u64 {
    ((m[0] as u64) << 40)
        | ((m[1] as u64) << 32)
        | ((m[2] as u64) << 24)
        | ((m[3] as u64) << 16)
        | ((m[4] as u64) << 8)
        | (m[5] as u64)
}

/// Registers that step 1 writes. Separate from [`reg`] on purpose: that module is
/// the read-only set, and the difference between "this module only looks" and
/// "this module configures the card" should be visible in the imports.
pub mod reg_rx {
    /// `ChipCmd`, 8-bit. Reset and the receive enable live here.
    pub const CR: usize = 0x37;
    /// `RxConfig`, 32-bit. Which frames are accepted, and the DMA burst.
    pub const RCR: usize = 0x44;
    /// `Cfg9346`, 8-bit. The lock that guards the config registers.
    pub const CFG9346: usize = 0x50;
    /// `IntrMask`, 16-bit. Left at zero: this driver POLLS.
    pub const IMR: usize = 0x3C;
    /// `IntrStatus`, 16-bit. Write-1-to-clear.
    pub const ISR: usize = 0x3E;
    /// `RxMaxSize`, 16-bit. The largest frame the card will accept.
    pub const RMS: usize = 0xDA;
    /// `RxDescStartAddr`, 64-bit, written as two dwords. **The ring must be
    /// 256-byte aligned**; a page-aligned frame satisfies that with room to
    /// spare.
    pub const RDSAR_LO: usize = 0xE4;
    pub const RDSAR_HI: usize = 0xE8;
    /// `CPlusCmd`, 16-bit. The C+ mode of the 8169/8168 family.
    pub const CPCR: usize = 0xE0;
    /// **`MPC` -- Missed Packet Count**, 32 bits. Tramas que la tarjeta recibio
    /// y **tuvo que tirar** porque no habia descriptor libre donde ponerlas.
    ///
    /// *** ES EL NUMERO HONESTO DE UN DRIVER DE RED, y no lo lleva el driver:
    /// lo lleva el silicio. Un contador propio solo puede contar lo que se cogio
    /// --nunca sabe lo que se perdio-- asi que "he recibido 40 tramas" es una
    /// frase que no dice nada sin esto al lado. Si esto sube, el anillo es
    /// pequeno o nadie llama a `rx_poll` bastante a menudo.
    ///
    /// ** Se pone a cero escribiendo, asi que **leerlo es destructivo si se
    /// limpia**: aqui solo se lee.
    pub const MPC: usize = 0x4C;
}

/// Bits of `CR` (`ChipCmd`).
pub mod cr {
    /// Soft reset. The card clears it by itself when it is done -- **it is not a
    /// delay, it is a handshake**, and waiting a fixed time instead of watching
    /// the bit is how a driver works on one machine and not on the next.
    pub const RST: u8 = 0x10;
    /// Receiver enable.
    pub const RE: u8 = 0x08;
    /// Transmitter enable. **Deliberately not set in step 1.**
    pub const TE: u8 = 0x04;
}

/// Bits of `RCR` (`RxConfig`).
pub mod rcr {
    /// Accept All Physical: promiscuous. **Off**, see [`rx_config`].
    pub const AAP: u32 = 1 << 0;
    /// Accept Physical Match: frames addressed to our own MAC.
    pub const APM: u32 = 1 << 1;
    /// Accept Multicast.
    pub const AM: u32 = 1 << 2;
    /// Accept Broadcast. **This is the one that makes step 1 work**: ARP queries
    /// and mDNS are broadcast, so a plugged cable produces traffic without
    /// anybody doing anything.
    pub const AB: u32 = 1 << 3;
    /// Unlimited DMA burst (bits 8..10 all set).
    pub const MXDMA_UNLIMITED: u32 = 0x7 << 8;
    /// No FIFO threshold: hand over the whole frame (bits 13..15 all set).
    pub const RXFTH_NONE: u32 = 0x7 << 13;
}

/// The value written to `RxConfig` for step 1.
///
/// **Promiscuous (`AAP`) is deliberately left out.** It would show more traffic
/// and it is tempting for a first test, but it also means this machine listens to
/// everything that crosses its port -- and turning that on by default is a
/// decision about the product, not about the driver. Broadcast alone already
/// guarantees ARP, mDNS and DHCP, which is more than enough for the question this
/// step is asking.
pub const fn rx_config() -> u32 {
    rcr::AB | rcr::AM | rcr::APM | rcr::MXDMA_UNLIMITED | rcr::RXFTH_NONE
}

/// Size of each receive buffer.
///
/// 2048 and not 1536: a jumbo-less Ethernet frame tops out at 1518 with FCS, but
/// the ring is walked by index and a power of two keeps every buffer inside its
/// own page half. The wasted memory is 8 KiB total for the whole ring.
pub const RX_BUF_LEN: u16 = 2048;

/// How many descriptors the ring has.
///
/// 16 buffers = 32 KiB. Enough that a burst of broadcast traffic does not lap the
/// poller between two turns, small enough to fit in one contiguous allocation.
pub const RX_RING_LEN: usize = 16;

#[cfg(test)]
mod tests {
    use super::*;

    /// Un bloque de registros de mentira, **alineado**. Sin la alineacion, leer
    /// un `u32` de un `[u8]` cualquiera es comportamiento indefinido en la
    /// prueba y correcto en el kernel -- que es la peor combinacion posible.
    #[repr(C, align(4))]
    struct Registros([u8; 256]);

    /// La tarjeta de esta maquina, segun su otro sistema operativo. La prueba
    /// lleva el numero real a proposito: **es la prediccion contra la que se va
    /// a comparar la foto del arranque**.
    const MAC_DEL_RYZEN: [u8; 6] = [0x2C, 0xF0, 0x5D, 0xD9, 0x3C, 0xE3];

    fn con(mac: [u8; 6], phy: u8) -> Identidad {
        let mut r = Registros([0u8; 256]);
        r.0[0..6].copy_from_slice(&mac);
        r.0[0x6C] = phy;
        unsafe { identificar(r.0.as_mut_ptr()) }
    }

    /// ** EL ORDEN DE LOS BYTES, que es lo unico que aqui se puede torcer sin
    /// que nada falle.
    ///
    /// La MAC sale de **dos lecturas de 32 bits**, y en little-endian el byte 0
    /// del registro es el byte BAJO de la palabra. Equivocarse invierte la
    /// direccion, y una MAC invertida es perfectamente creible: seis bytes que
    /// no son ceros ni unos. Se imprimiria, se compararia con Windows, no
    /// cuadraria, y el sospechoso seria el BAR -- que estaria bien.
    #[test]
    fn la_mac_sale_en_el_orden_en_que_se_escribe() {
        let id = con(MAC_DEL_RYZEN, 0);
        assert_eq!(id.mac, MAC_DEL_RYZEN, "los seis bytes no salen en orden");
        assert_eq!(
            id.mac_u64(),
            0x2CF0_5DD9_3CE3,
            "en hexadecimal tiene que leerse igual que se escribe: 2C:F0:5D:D9:3C:E3"
        );
    }

    /// ** CREIBLE COMPRUEBA EL BAR, NO LA TARJETA.
    ///
    /// Ceros y unos son lo que devuelve un MMIO que no lleva a ningun sitio: los
    /// primeros si la lectura cae en un agujero, los segundos si nadie contesta
    /// al ciclo de bus. Confundir eso con "la NIC esta rota" manda a cambiar de
    /// tarjeta cuando lo que hay que cambiar es un indice de BAR.
    #[test]
    fn una_mac_imposible_delata_el_bar_y_no_la_tarjeta() {
        assert!(!con([0x00; 6], 0).creible(), "todo ceros: el MMIO no lleva a los registros");
        assert!(!con([0xFF; 6], 0).creible(), "todo unos: el aparato no contesta");
        // Bit 0 del primer byte = direccion de GRUPO. Ninguna tarjeta trae de
        // fabrica una MAC multicast, asi que si sale una, se leyo otra cosa.
        assert!(!con([0x01, 0x00, 0x5E, 0x11, 0x22, 0x33], 0).creible(), "una MAC multicast no es de fabrica");
        assert!(con(MAC_DEL_RYZEN, 0).creible(), "y la de verdad si es creible");
    }

    /// `PHYstatus` traducido. El valor que se espera del Ryzen es `0x0B`
    /// --enlace + 100 Mbps + duplex completo-- porque su Windows dice
    /// exactamente eso: `Up`, `100 Mbps`.
    #[test]
    fn el_enlace_se_lee_del_phystatus() {
        let cien = con(MAC_DEL_RYZEN, 0x0B);
        assert!(cien.enlace_arriba());
        assert_eq!(cien.megabits(), 100, "0x0B es enlace + 100M + full");
        assert!(cien.duplex_completo());

        assert_eq!(con(MAC_DEL_RYZEN, 0x13).megabits(), 1000, "0x13 lleva el bit de giga");
        assert_eq!(con(MAC_DEL_RYZEN, 0x07).megabits(), 10, "0x07 es 10M");
    }

    /// ** SIN CABLE, CERO MEGABITS -- y no el ultimo valor que hubiera.
    ///
    /// Un `1000` supuesto en un enlace caido no da un error: da un sistema que
    /// cree ir diez veces mas rapido de lo que va, y eso sale como paquetes
    /// perdidos mucho despues y muy lejos de aqui. Es el patron del cero
    /// silencioso, en el sitio donde parece inofensivo.
    #[test]
    fn sin_enlace_no_se_inventa_velocidad() {
        let sin = con(MAC_DEL_RYZEN, 0x00);
        assert!(!sin.enlace_arriba());
        assert_eq!(sin.megabits(), 0, "sin enlace la velocidad es 0, no la que hubiera");
        // Y aunque el chip deje puesto el bit de 1000 con el enlace caido --que
        // pasa al desenchufar-- sigue siendo 0: manda el enlace.
        assert_eq!(con(MAC_DEL_RYZEN, 0x10).megabits(), 0, "el bit de velocidad sin enlace no vale");
    }

    // == STEP 1: the RX ring, in the only place it can be checked without a card

    /// ** THE DESCRIPTOR IS SIXTEEN BYTES, AND THAT IS A HARDWARE CONTRACT.
    ///
    /// The card walks the ring by adding a fixed stride. If Rust ever laid this
    /// struct out differently, the ring would still compile, still be allocated,
    /// still be handed to the card -- and the card would read buffer addresses out
    /// of status words and write frames wherever those happened to point. That is
    /// the memory-corruption case, and it has no error path.
    #[test]
    fn the_descriptor_is_exactly_sixteen_bytes() {
        assert_eq!(core::mem::size_of::<RxDesc>(), 16);
        assert_eq!(core::mem::align_of::<RxDesc>(), 4);
        assert_eq!(core::mem::size_of::<[RxDesc; RX_RING_LEN]>(), 256);
    }

    /// ** OWN MEANS THE CARD HAS IT, NOT THAT WE DO.
    ///
    /// The name reads backwards the first time. Getting it inverted gives a
    /// driver that polls forever over a ring the card is filling perfectly, and
    /// the conclusion would be "the NIC receives nothing" -- which is false, and
    /// sends the search to the cable and the switch.
    #[test]
    fn own_set_means_the_card_owns_it() {
        let d = RxDesc::to_card(0x1234_5000, RX_BUF_LEN, false);
        assert!(d.owned_by_card(), "handed over: the card owns it");
        assert_eq!(d.frame_len(), None, "and there is nothing to read yet");

        let mut back = d;
        back.opts1 = rx::FS | rx::LS | 64; // returned by the card, 64 with FCS
        assert!(!back.owned_by_card());
        assert_eq!(back.frame_len(), Some(60), "64 on the wire is 60 of payload");
    }

    /// ** EOR ON THE LAST ONE, AND ONLY ON THE LAST ONE.
    ///
    /// Without End Of Ring the card does not wrap: it keeps walking past the end
    /// of the ring, writing descriptors over whatever memory follows. It is the
    /// single bit in this file whose absence corrupts instead of failing.
    #[test]
    fn end_of_ring_marks_the_last_descriptor_only() {
        let middle = RxDesc::to_card(0x1000, RX_BUF_LEN, false);
        let last = RxDesc::to_card(0x2000, RX_BUF_LEN, true);
        assert_eq!(middle.opts1 & rx::EOR, 0);
        assert_ne!(last.opts1 & rx::EOR, 0);
        // And the length survives next to the flags: a mask that ate into the
        // size would give the card a buffer smaller than the one that exists.
        assert_eq!(last.opts1 & rx::LEN_MASK, RX_BUF_LEN as u32);
    }

    /// A 64-bit address goes in as two halves, and the high half is not optional
    /// just because today's allocations happen to be low.
    #[test]
    fn the_buffer_address_is_split_in_two_halves() {
        let d = RxDesc::to_card(0x0000_0007_DEAD_B000, RX_BUF_LEN, false);
        assert_eq!(d.addr_lo, 0xDEAD_B000);
        assert_eq!(d.addr_hi, 0x0000_0007);
    }

    /// ** A HALF-WRITTEN OR BROKEN FRAME IS NOT A FRAME.
    ///
    /// Three separate things have to be true, and checking only ownership is the
    /// easy mistake: it would report error frames and half frames as good data,
    /// and the length would still look reasonable.
    #[test]
    fn only_a_whole_clean_frame_reports_a_length() {
        let mut d = RxDesc::default();
        d.opts1 = rx::FS | rx::LS | rx::RES | 64;
        assert_eq!(d.frame_len(), None, "the error bit disqualifies it");

        d.opts1 = rx::FS | 64; // first segment but not last
        assert_eq!(d.frame_len(), None, "a frame split across descriptors is not readable yet");

        d.opts1 = rx::FS | rx::LS | 2; // shorter than its own FCS
        assert_eq!(d.frame_len(), None, "under four bytes there is not even a CRC");
    }

    /// ** THE ETHERTYPE IS BIG-ENDIAN AND THIS MACHINE IS NOT.
    ///
    /// Read the native way, ARP (`0x0806`) comes out as `0x0608`: a number that
    /// matches no protocol, so the frame is filed as "unknown" instead of as a
    /// byte-order bug. It is the same class of mistake as the MAC order above,
    /// and just as invisible.
    #[test]
    fn the_ethertype_is_read_big_endian() {
        // A real ARP query: broadcast destination, ethertype 0x0806.
        let mut frame = [0u8; 60];
        frame[0..6].copy_from_slice(&[0xFF; 6]);
        frame[6..12].copy_from_slice(&MAC_DEL_RYZEN);
        frame[12] = 0x08;
        frame[13] = 0x06;

        let h = EthHeader::parse(&frame).expect("fourteen bytes are enough");
        assert_eq!(h.ethertype, 0x0806, "ARP, not 0x0608");
        assert!(h.is_broadcast(), "an ARP query goes to everybody");
        assert_eq!(h.src_u64(), 0x2CF0_5DD9_3CE3, "and the source reads like it is written");
    }

    /// Under fourteen bytes there is no header to read, and inventing one would
    /// mean printing six bytes of somebody else's buffer as a MAC address.
    #[test]
    fn a_runt_has_no_header() {
        assert!(EthHeader::parse(&[0u8; 13]).is_none());
        assert!(EthHeader::parse(&[]).is_none());
    }

    /// ** PROMISCUOUS STAYS OFF, AND IT IS A DECISION AND NOT AN OVERSIGHT.
    ///
    /// This test exists so that turning it on has to be deliberate: whoever adds
    /// `AAP` has to come here and delete an assertion that says why it was off.
    #[test]
    fn the_receiver_is_not_promiscuous() {
        let c = rx_config();
        assert_eq!(c & rcr::AAP, 0, "listening to everything is a product decision");
        assert_ne!(c & rcr::AB, 0, "broadcast is what makes a plugged cable produce traffic");
        assert_ne!(c & rcr::APM, 0, "and our own address, obviously");
    }

    /// *** EL DESTINO ES LO QUE PRUEBA EL FILTRO, y por eso se mira.
    ///
    /// ** Una tarjeta en modo promiscuo y una filtrando bien dan **los mismos
    /// origenes**. Lo que cambia es a quien iban dirigidas las tramas. Sin el
    /// destino, la foto del paso 1 no distingue las dos cosas -- y una de las
    /// tres preguntas que ese paso existe para contestar se quedaria sin
    /// contestar mientras la casilla se pone verde.
    #[test]
    fn la_cabecera_da_origen_Y_destino() {
        // Un ARP de broadcast: lo mas probable que reciba esta maquina primero.
        let mut trama = [0u8; 60];
        trama[0..6].copy_from_slice(&[0xFF; 6]);
        trama[6..12].copy_from_slice(&[0x2C, 0xF0, 0x5D, 0xD9, 0x3C, 0xE3]);
        trama[12] = 0x08;
        trama[13] = 0x06;

        let h = EthHeader::parse(&trama).expect("catorce bytes hay");
        assert_eq!(h.src_u64(), 0x2CF05DD93CE3, "el origen, como se dice en voz alta");
        assert_eq!(h.dst_u64(), 0xFFFFFFFFFFFF, "y el destino, que es el que faltaba");
        assert!(h.is_broadcast());
    }

    /// El ethertype viaja BIG-endian y esta maquina es little-endian. Leerlo del
    /// modo nativo convierte `0x0806` en `0x0608`, que no coincide con nada y
    /// parece un protocolo desconocido en vez de un fallo de orden de bytes.
    #[test]
    fn el_tipo_se_lee_del_cable_y_trae_su_nombre() {
        let mut t = [0u8; 14];
        t[12] = 0x08;
        t[13] = 0x06;
        let h = EthHeader::parse(&t).unwrap();
        assert_eq!(h.ethertype, 0x0806, "ARP, no 0x0608");
        assert!(h.nombre_del_tipo().contains("ARP"));

        t[12] = 0x08;
        t[13] = 0x00;
        assert!(EthHeader::parse(&t).unwrap().nombre_del_tipo().contains("IPv4"));

        // ** Uno que no conocemos NO dice "desconocido": el numero se imprime al
        // lado igual, y una palabra que no informa empujaria fuera de la linea
        // al numero que si.
        t[12] = 0x99;
        t[13] = 0x99;
        let x = EthHeader::parse(&t).unwrap();
        assert_eq!(x.nombre_del_tipo().trim(), "tipo");
    }

    /// [!] Por debajo de `0x0600` el campo es un LARGO, no un tipo. Eso es
    /// 802.3, y verlo en una LAN moderna **es el hallazgo**, asi que la etiqueta
    /// lo grita en vez de callarlo.
    #[test]
    fn un_largo_disfrazado_de_tipo_se_denuncia() {
        let mut t = [0u8; 14];
        t[12] = 0x00;
        t[13] = 0x2E; // 46: un largo de 802.3
        let h = EthHeader::parse(&t).unwrap();
        assert!(h.nombre_del_tipo().contains("LARGO"), "{}", h.nombre_del_tipo());
    }

}
