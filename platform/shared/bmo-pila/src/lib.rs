//! **BMO PILA** -- TCP/IP propio: determinista y blindado (2026-09-13).
//!
//! generacion: hijo -- recibe bytes y la hora; no sabe de tarjetas, de anillos
//! ni de quien la usa. Debajo solo tiene a `bmo-cripto`.
//!
//! ## Por que existe
//!
//! Eddi: *"TCP/IP propio, ultra determinista por completo, no quiero sorpresa, y
//! que ESTE BLINDADO AGRESIVAMENTE"*. Es el primer escalon de la escalera que
//! acaba en un navegador, y el que decide `docs/maestro/RED_MAESTRO.md`: **el
//! kernel entrega tramas crudas y se aparta; los protocolos son de usuario**.
//! Por eso esto es un crate de Ring 3 que se prueba en el anfitrion, y no un
//! modulo del kernel.
//!
//! ## Las tres leyes de este crate
//!
//! ```text
//!    1. DETERMINISTA  sin reloj propio (la hora entra como argumento), sin
//!                     monton, tamanos fijos, desempates por indice. La misma
//!                     secuencia de llamadas da los mismos bytes
//!    2. LISTA BLANCA  lo que no esta escrito que se acepta, se rechaza: VLAN,
//!                     IPv6, opciones IP, fragmentos, ICMP que no es eco,
//!                     banderas de escaner, UDP sin suma
//!    3. EL NO SE DICE cada rechazo lleva su `Rechazo`, con nombre. "Se descarto"
//!                     no es un diagnostico (L6i/L6j)
//! ```
//!
//! ## La escalera, y donde esta cada peldano
//!
//! ```text
//!    Ethernet, ARP, IPv4, ICMP, UDP   este crate, probado aqui
//!    TCP                              este crate, probado aqui
//!    ---------------------------------------------------------------------
//!    recibir en el Ryzen              HECHO en metal (2026-09-13)
//!    transmitir (el GATE RED)         HECHO en metal (2026-09-14): el router
//!                                     contesto a `red prueba`
//!    DHCP                             `dhcp.rs`, probado aqui; `red ip` en metal
//!    DNS                              encima de `udp`, el siguiente aqui
//!    TLS 1.3                          bmo-cripto tiene X25519, AES-GCM,
//!                                     SHA-256 y HKDF; falta la maquina de
//!                                     estados y X.509
//!    el navegador de TEXTO / Gemini   una app en C o INTI sobre TCP + TLS
//!    NetSurf                          el unico motor real portable
//! ```
//!
//! [!] Lo de arriba de la raya esta probado contra pilas de mentira en el
//! anfitrion. Desde el 2026-09-14 el cable del Ryzen ya lleva tramas de BMO-X.

#![cfg_attr(not(test), no_std)]

pub mod arp;
/// DHCP, del lado del cliente: la IP propia (G2, 2026-09-14).
pub mod dhcp;
pub mod ether;
pub mod icmp;
pub mod ipv4;
pub mod nodo;
pub mod suma;
pub mod tcp;
pub mod udp;

/// **Por que no.** Cada funcion que lee bytes de fuera contesta con uno de
/// estos, y ninguno significa dos cosas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rechazo {
    /// Faltan bytes para lo que dice la cabecera.
    Corto,
    /// Mas grande de lo que se admite.
    Largo,
    /// No es IPv4.
    Version,
    /// Un campo de cabecera imposible.
    Cabecera,
    /// Opciones IP, o una opcion TCP mal formada.
    Opciones,
    /// Un fragmento IP: no se reensambla.
    Fragmento,
    /// Una combinacion de banderas que no existe honradamente.
    Banderas,
    /// La suma no cuadra (o falta).
    Suma,
    /// Una direccion que no puede ser (grupo como origen, MAC que miente...).
    Direccion,
    /// Un tipo de trama, ARP o ICMP que no esta en la lista.
    Tipo,
    /// Un protocolo IP que no se habla.
    Protocolo,
    /// Puerto cero.
    Puerto,
    /// Un numero de secuencia o de ACK fuera de lo posible.
    Secuencia,
    /// Una tabla fija esta llena, o se paso un tope por segundo.
    Lleno,
    /// La operacion no vale en el estado en que esta la conexion.
    Estado,
    /// Un asa que no es de ninguna conexion.
    Asa,
    /// Correcto, pero no es para esta maquina.
    NoEsParaMi,
}

impl Rechazo {
    pub fn texto(self) -> &'static str {
        match self {
            Rechazo::Corto => "faltan bytes",
            Rechazo::Largo => "demasiado grande",
            Rechazo::Version => "no es IPv4",
            Rechazo::Cabecera => "cabecera imposible",
            Rechazo::Opciones => "opciones no admitidas o mal formadas",
            Rechazo::Fragmento => "fragmento: no se reensambla",
            Rechazo::Banderas => "banderas de escaner",
            Rechazo::Suma => "la suma no cuadra",
            Rechazo::Direccion => "direccion imposible o suplantada",
            Rechazo::Tipo => "tipo fuera de la lista",
            Rechazo::Protocolo => "protocolo que no se habla",
            Rechazo::Puerto => "puerto cero",
            Rechazo::Secuencia => "secuencia fuera de lo posible",
            Rechazo::Lleno => "tabla llena o tope por segundo",
            Rechazo::Estado => "no vale en este estado",
            Rechazo::Asa => "asa que no existe",
            Rechazo::NoEsParaMi => "no es para esta maquina",
        }
    }
}

pub(crate) fn be16(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}

pub(crate) fn be32(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

pub(crate) fn pon16(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_be_bytes());
}

pub(crate) fn pon32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_be_bytes());
}
