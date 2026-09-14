//! **DHCP**: pedir una IP sin tenerla (RFC 2131), del lado del cliente.
//!
//! [carril]  AMARILLO  lee bytes que manda el router, y de ellos sale la IP propia
//! [cuesta]  DATO      una concesion mal leida da una IP que es de otro
//! [riesgo]  AJENO     lo que llega lo escribe cualquiera del cable, no solo el router
//!
//! # G2 del camino a Gemini (`docs/plan/PLAN_RED_TX.md`, 2026-09-14)
//!
//! Cuatro tramas, y solo dos son nuestras:
//!
//! ```text
//!    DISCOVER  ->  difusion, origen 0.0.0.0    "hay alguien que reparta IPs?"
//!    OFFER     <-  el servidor                  "toma esta"
//!    REQUEST   ->  difusion, origen 0.0.0.0    "me quedo ESA, de ESE servidor"
//!    ACK       <-  el servidor                  "es tuya durante N segundos"
//! ```
//!
//! # Lo que se rechaza, por la ley 2 del crate (lista blanca)
//!
//! ```text
//!    otro `xid` u otra MAC en la respuesta    es de otra maquina: NoEsParaMi
//!    una IP ofrecida que no puede ser         0.0.0.0, 127/8, grupo: Direccion
//!    opciones que se salen del paquete        Corto
//!    un tipo de mensaje que falta o sobra     Cabecera / Opciones
//!    una concesion de menos de un minuto      Cabecera: un servidor roto
//!    UDP sin suma                             Suma, como todo UDP de este crate
//! ```
//!
//! [!] Determinista, ley 1: el `xid` y la hora los pone quien llama. Aqui no hay
//! reloj ni azar, y la misma secuencia de llamadas da los mismos bytes.

use crate::ether::{self, Mac};
use crate::ipv4::{self, Ip};
use crate::udp;
use crate::{be32, pon16, pon32, Rechazo};

pub const PUERTO_SERVIDOR: u16 = 67;
pub const PUERTO_CLIENTE: u16 = 68;
const MAGIA: u32 = 0x6382_5363;
/// op, htype, hlen, hops, xid, secs, flags, cuatro IP, chaddr, sname y file.
const BOOTP: usize = 236;
/// Donde empiezan las opciones: detras de la galleta magica.
const OPCIONES: usize = BOOTP + 4;
/// Cuanto se espera una respuesta antes de volver a preguntar.
pub const ESPERA_MS: u64 = 4_000;
/// Cuantas veces se pregunta en cada fase antes de rendirse.
pub const INTENTOS: u32 = 3;
/// La concesion mas corta que se cree.
pub const CONCESION_MIN_S: u32 = 60;

mod opcion {
    pub const RELLENO: u8 = 0;
    pub const MASCARA: u8 = 1;
    pub const ROUTER: u8 = 3;
    pub const DNS: u8 = 6;
    pub const PEDIDA: u8 = 50;
    pub const CONCESION: u8 = 51;
    pub const TIPO: u8 = 53;
    pub const SERVIDOR: u8 = 54;
    pub const LISTA: u8 = 55;
    pub const FIN: u8 = 255;
}

mod tipo {
    pub const DISCOVER: u8 = 1;
    pub const OFFER: u8 = 2;
    pub const REQUEST: u8 = 3;
    pub const ACK: u8 = 5;
    pub const NAK: u8 = 6;
}

/// **Lo que concede el servidor.** Lo que no dijo va a ceros.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Concesion {
    pub ip: Ip,
    pub servidor: Ip,
    pub mascara: Ip,
    pub router: Ip,
    pub dns: Ip,
    pub segundos: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mensaje {
    Oferta(Concesion),
    Confirmada(Concesion),
    Negada,
}

/// **Arma una peticion entera**: Ethernet, IPv4, UDP y BOOTP, lista para el buzon.
fn armar(dst: &mut [u8], mac: Mac, xid: u32, tipo_msg: u8, pedida: Option<(Ip, Ip)>) -> Result<usize, Rechazo> {
    let mut b = [0u8; OPCIONES + 32];
    b[0] = 1; // BOOTREQUEST
    b[1] = 1; // Ethernet
    b[2] = 6; // seis bytes de MAC
    pon32(&mut b, 4, xid);
    // ** BROADCAST: sin IP no se puede recibir una respuesta dirigida a una IP.
    pon16(&mut b, 10, 0x8000);
    b[28..34].copy_from_slice(&mac);
    pon32(&mut b, BOOTP, MAGIA);
    let mut i = OPCIONES;
    b[i..i + 3].copy_from_slice(&[opcion::TIPO, 1, tipo_msg]);
    i += 3;
    if let Some((ip, servidor)) = pedida {
        b[i] = opcion::PEDIDA;
        b[i + 1] = 4;
        b[i + 2..i + 6].copy_from_slice(&ip);
        i += 6;
        b[i] = opcion::SERVIDOR;
        b[i + 1] = 4;
        b[i + 2..i + 6].copy_from_slice(&servidor);
        i += 6;
    }
    b[i..i + 6].copy_from_slice(&[opcion::LISTA, 4, opcion::MASCARA, opcion::ROUTER, opcion::DNS, opcion::CONCESION]);
    i += 6;
    b[i] = opcion::FIN;
    i += 1;
    let bootp = &b[..i];

    let origen = [0u8; 4];
    let destino = [255u8; 4];
    let e = ether::escribir(dst, ether::DIFUSION, mac, ether::TIPO_IPV4)?;
    let largo_udp = udp::CABECERA + bootp.len();
    if dst.len() < e + ipv4::CABECERA + largo_udp {
        return Err(Rechazo::Corto);
    }
    ipv4::escribir(&mut dst[e..], origen, destino, ipv4::UDP, largo_udp, (xid & 0xFFFF) as u16)?;
    udp::escribir(&mut dst[e + ipv4::CABECERA..], origen, destino, PUERTO_CLIENTE, PUERTO_SERVIDOR, bootp)?;
    Ok(e + ipv4::CABECERA + largo_udp)
}

/// DISCOVER: quien reparte IPs?
pub fn descubrir(dst: &mut [u8], mac: Mac, xid: u32) -> Result<usize, Rechazo> {
    armar(dst, mac, xid, tipo::DISCOVER, None)
}

/// REQUEST: me quedo `ip`, de `servidor`.
pub fn pedir(dst: &mut [u8], mac: Mac, xid: u32, ip: Ip, servidor: Ip) -> Result<usize, Rechazo> {
    armar(dst, mac, xid, tipo::REQUEST, Some((ip, servidor)))
}

/// **Lee una trama entera** y dice si es una respuesta DHCP para ESTA peticion.
pub fn leer(trama: &[u8], mac: Mac, xid: u32) -> Result<Mensaje, Rechazo> {
    let t = ether::leer(trama)?;
    if t.tipo != ether::TIPO_IPV4 {
        return Err(Rechazo::Tipo);
    }
    let p = ipv4::leer(t.carga)?;
    if p.protocolo != ipv4::UDP {
        return Err(Rechazo::Protocolo);
    }
    let d = udp::leer(p.carga, p.origen, p.destino)?;
    if d.origen != PUERTO_SERVIDOR || d.destino != PUERTO_CLIENTE {
        return Err(Rechazo::NoEsParaMi);
    }
    let b = d.datos;
    if b.len() < OPCIONES {
        return Err(Rechazo::Corto);
    }
    if b[0] != 2 || b[1] != 1 || b[2] != 6 {
        return Err(Rechazo::Cabecera);
    }
    if be32(b, 4) != xid || b[28..34] != mac {
        return Err(Rechazo::NoEsParaMi);
    }
    if be32(b, BOOTP) != MAGIA {
        return Err(Rechazo::Cabecera);
    }
    let mut c = Concesion {
        ip: [b[16], b[17], b[18], b[19]],
        servidor: [0; 4],
        mascara: [0; 4],
        router: [0; 4],
        dns: [0; 4],
        segundos: 0,
    };
    let mut tipo_msg = 0u8;
    let mut i = OPCIONES;
    loop {
        // Sin FIN antes del final: el paquete miente sobre donde acaba.
        if i >= b.len() {
            return Err(Rechazo::Corto);
        }
        let codigo = b[i];
        if codigo == opcion::FIN {
            break;
        }
        if codigo == opcion::RELLENO {
            i += 1;
            continue;
        }
        if i + 1 >= b.len() {
            return Err(Rechazo::Corto);
        }
        let v0 = i + 2;
        let v1 = v0 + b[i + 1] as usize;
        if v1 > b.len() {
            return Err(Rechazo::Corto);
        }
        let v = &b[v0..v1];
        let ip4 = |v: &[u8]| -> Result<Ip, Rechazo> {
            if v.len() < 4 {
                return Err(Rechazo::Opciones);
            }
            Ok([v[0], v[1], v[2], v[3]])
        };
        match codigo {
            opcion::TIPO => {
                if v.len() != 1 {
                    return Err(Rechazo::Opciones);
                }
                tipo_msg = v[0];
            }
            opcion::MASCARA => c.mascara = ip4(v)?,
            opcion::ROUTER => c.router = ip4(v)?,
            opcion::DNS => c.dns = ip4(v)?,
            opcion::SERVIDOR => c.servidor = ip4(v)?,
            opcion::CONCESION => {
                if v.len() != 4 {
                    return Err(Rechazo::Opciones);
                }
                c.segundos = be32(v, 0);
            }
            // Las demas se saltan por su largo, sin creerse nada de ellas.
            _ => {}
        }
        i = v1;
    }
    match tipo_msg {
        tipo::NAK => Ok(Mensaje::Negada),
        tipo::OFFER | tipo::ACK => {
            if ipv4::origen_imposible(&c.ip) || ipv4::es_difusion(&c.ip) {
                return Err(Rechazo::Direccion);
            }
            if c.servidor == [0; 4] {
                return Err(Rechazo::Cabecera);
            }
            if tipo_msg == tipo::ACK && c.segundos < CONCESION_MIN_S {
                return Err(Rechazo::Cabecera);
            }
            Ok(if tipo_msg == tipo::OFFER { Mensaje::Oferta(c) } else { Mensaje::Confirmada(c) })
        }
        0 => Err(Rechazo::Cabecera),
        _ => Err(Rechazo::Tipo),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallo {
    /// Tres preguntas en cada fase y ni una respuesta valida.
    NadieContesta,
    /// El servidor dijo NAK.
    Negada,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    Quieto,
    Descubriendo,
    Pidiendo(Concesion),
    Concedida { concesion: Concesion, desde_ms: u64 },
    Fallo(Fallo),
}

/// **EL CLIENTE**: una peticion, con su `xid`. Avanza con la hora que le dan.
pub struct Cliente {
    mac: Mac,
    xid: u32,
    estado: Estado,
    enviado_ms: Option<u64>,
    intentos: u32,
}

impl Cliente {
    pub const fn nuevo(mac: Mac, xid: u32) -> Self {
        Self { mac, xid, estado: Estado::Quieto, enviado_ms: None, intentos: 0 }
    }

    pub fn estado(&self) -> Estado {
        self.estado
    }

    pub fn en_marcha(&self) -> bool {
        matches!(self.estado, Estado::Descubriendo | Estado::Pidiendo(_))
    }

    /// Empieza (o vuelve a empezar) desde DISCOVER.
    pub fn empezar(&mut self) {
        self.estado = Estado::Descubriendo;
        self.enviado_ms = None;
        self.intentos = 0;
    }

    /// **La trama que toca enviar AHORA**, si toca. Se llama en cada vuelta.
    pub fn latir(&mut self, ahora_ms: u64, dst: &mut [u8]) -> Result<Option<usize>, Rechazo> {
        if !self.en_marcha() {
            return Ok(None);
        }
        let toca = match self.enviado_ms {
            None => true,
            Some(t) => ahora_ms.saturating_sub(t) >= ESPERA_MS,
        };
        if !toca {
            return Ok(None);
        }
        if self.intentos >= INTENTOS {
            self.estado = Estado::Fallo(Fallo::NadieContesta);
            return Ok(None);
        }
        let n = match self.estado {
            Estado::Pidiendo(o) => pedir(dst, self.mac, self.xid, o.ip, o.servidor)?,
            _ => descubrir(dst, self.mac, self.xid)?,
        };
        self.intentos += 1;
        self.enviado_ms = Some(ahora_ms);
        Ok(Some(n))
    }

    /// **Una trama del cable.** `Ok(true)` si cambio el estado.
    pub fn oir(&mut self, trama: &[u8], ahora_ms: u64) -> Result<bool, Rechazo> {
        let m = leer(trama, self.mac, self.xid)?;
        match (self.estado, m) {
            (Estado::Descubriendo, Mensaje::Oferta(c)) => {
                self.estado = Estado::Pidiendo(c);
                self.enviado_ms = None;
                self.intentos = 0;
                Ok(true)
            }
            // ** El ACK tiene que ser DE LO QUE SE PIDIO: otra IP u otro servidor
            // es otra conversacion.
            (Estado::Pidiendo(o), Mensaje::Confirmada(c)) if c.ip == o.ip && c.servidor == o.servidor => {
                self.estado = Estado::Concedida { concesion: c, desde_ms: ahora_ms };
                Ok(true)
            }
            (Estado::Pidiendo(_), Mensaje::Negada) => {
                self.estado = Estado::Fallo(Fallo::Negada);
                Ok(true)
            }
            // Una segunda oferta, un ACK sin pedir: se ignoran.
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const YO: Mac = [0x02, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E];
    const OTRA: Mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x99];
    const ROUTER_MAC: Mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    const SRV: Ip = [10, 0, 0, 1];
    const LIBRE: Ip = [10, 0, 0, 57];
    const XID: u32 = 0x1234_5678;

    fn bootp(tipo_msg: u8, xid: u32, mac: Mac, ip: Ip, segundos: u32) -> Vec<u8> {
        let mut b = vec![0u8; OPCIONES];
        b[0] = 2;
        b[1] = 1;
        b[2] = 6;
        pon32(&mut b, 4, xid);
        b[16..20].copy_from_slice(&ip);
        b[28..34].copy_from_slice(&mac);
        pon32(&mut b, BOOTP, MAGIA);
        b.extend_from_slice(&[53, 1, tipo_msg, 54, 4, 10, 0, 0, 1, 1, 4, 255, 255, 255, 0]);
        b.extend_from_slice(&[3, 4, 10, 0, 0, 1, 6, 4, 10, 0, 0, 1, 51, 4]);
        b.extend_from_slice(&segundos.to_be_bytes());
        b.push(255);
        b
    }

    fn envolver(datos: &[u8]) -> Vec<u8> {
        let mut t = vec![0u8; ether::CABECERA + ipv4::CABECERA + udp::CABECERA + datos.len()];
        ether::escribir(&mut t, YO, ROUTER_MAC, ether::TIPO_IPV4).unwrap();
        ipv4::escribir(&mut t[14..], SRV, [255; 4], ipv4::UDP, udp::CABECERA + datos.len(), 7).unwrap();
        udp::escribir(&mut t[34..], SRV, [255; 4], PUERTO_SERVIDOR, PUERTO_CLIENTE, datos).unwrap();
        t
    }

    fn respuesta(tipo_msg: u8, ip: Ip) -> Vec<u8> {
        envolver(&bootp(tipo_msg, XID, YO, ip, 3600))
    }

    #[test]
    fn el_discover_es_el_del_rfc() {
        let mut t = [0u8; 400];
        let n = descubrir(&mut t, YO, XID).unwrap();
        assert!(n <= 1514);
        assert_eq!(&t[0..6], &[0xFF; 6], "difusion");
        assert_eq!(&t[6..12], &YO, "con nuestra MAC: el grifo del kernel exige eso");
        assert_eq!(t[23], ipv4::UDP);
        assert_eq!(&t[26..30], &[0, 0, 0, 0], "origen 0.0.0.0");
        assert_eq!(&t[30..34], &[255; 4]);
        assert_eq!(&t[34..38], &[0, 68, 0, 67]);
        let b = &t[42..n];
        assert_eq!((b[0], b[1], b[2]), (1, 1, 6));
        assert_eq!(be32(b, 4), XID);
        assert_eq!(&b[10..12], &[0x80, 0x00], "bandera BROADCAST");
        assert_eq!(&b[28..34], &YO);
        assert_eq!(be32(b, BOOTP), MAGIA);
        assert_eq!(&b[OPCIONES..OPCIONES + 3], &[53, 1, 1]);
        assert_eq!(b[n - 42 - 1], 255, "acaba en FIN");
    }

    #[test]
    fn el_request_lleva_la_ip_y_el_servidor() {
        let mut t = [0u8; 400];
        let n = pedir(&mut t, YO, XID, LIBRE, SRV).unwrap();
        let b = &t[42..n];
        assert_eq!(&b[OPCIONES..OPCIONES + 3], &[53, 1, 3]);
        assert_eq!(&b[OPCIONES + 3..OPCIONES + 9], &[50, 4, 10, 0, 0, 57]);
        assert_eq!(&b[OPCIONES + 9..OPCIONES + 15], &[54, 4, 10, 0, 0, 1]);
    }

    #[test]
    fn el_saludo_entero() {
        let mut c = Cliente::nuevo(YO, XID);
        let mut t = [0u8; 400];
        assert_eq!(c.latir(0, &mut t), Ok(None), "quieto no envia");
        c.empezar();
        assert!(c.latir(0, &mut t).unwrap().is_some(), "DISCOVER");
        assert_eq!(c.oir(&respuesta(tipo::OFFER, LIBRE), 100), Ok(true));
        assert!(matches!(c.estado(), Estado::Pidiendo(o) if o.ip == LIBRE));
        assert!(c.latir(150, &mut t).unwrap().is_some(), "el REQUEST sale sin esperar");
        assert_eq!(c.oir(&respuesta(tipo::ACK, LIBRE), 300), Ok(true));
        match c.estado() {
            Estado::Concedida { concesion, desde_ms } => {
                assert_eq!(concesion.ip, LIBRE);
                assert_eq!(concesion.servidor, SRV);
                assert_eq!(concesion.mascara, [255, 255, 255, 0]);
                assert_eq!(concesion.router, SRV);
                assert_eq!(concesion.dns, SRV);
                assert_eq!(concesion.segundos, 3600);
                assert_eq!(desde_ms, 300);
            }
            e => panic!("{e:?}"),
        }
        assert_eq!(c.latir(10_000, &mut t), Ok(None), "concedida no vuelve a preguntar");
    }

    #[test]
    fn otra_peticion_u_otra_maquina_no_son_para_mi() {
        let mut c = Cliente::nuevo(YO, XID);
        c.empezar();
        let otro_xid = envolver(&bootp(tipo::OFFER, XID + 1, YO, LIBRE, 3600));
        let otra_mac = envolver(&bootp(tipo::OFFER, XID, OTRA, LIBRE, 3600));
        assert_eq!(c.oir(&otro_xid, 0), Err(Rechazo::NoEsParaMi));
        assert_eq!(c.oir(&otra_mac, 0), Err(Rechazo::NoEsParaMi));
        assert_eq!(c.estado(), Estado::Descubriendo);
    }

    #[test]
    fn tres_preguntas_cada_cuatro_segundos_y_se_rinde() {
        let mut c = Cliente::nuevo(YO, XID);
        let mut t = [0u8; 400];
        c.empezar();
        assert!(c.latir(0, &mut t).unwrap().is_some());
        assert_eq!(c.latir(3_999, &mut t), Ok(None));
        assert!(c.latir(4_000, &mut t).unwrap().is_some());
        assert!(c.latir(8_000, &mut t).unwrap().is_some());
        assert_eq!(c.latir(12_000, &mut t), Ok(None));
        assert_eq!(c.estado(), Estado::Fallo(Fallo::NadieContesta));
    }

    #[test]
    fn un_nak_niega_y_un_ack_de_otra_ip_no_cuenta() {
        let mut c = Cliente::nuevo(YO, XID);
        c.empezar();
        c.oir(&respuesta(tipo::OFFER, LIBRE), 0).unwrap();
        assert_eq!(c.oir(&respuesta(tipo::ACK, [10, 0, 0, 58]), 1), Ok(false), "ACK de otra IP");
        assert_eq!(c.oir(&respuesta(tipo::NAK, [0; 4]), 2), Ok(true));
        assert_eq!(c.estado(), Estado::Fallo(Fallo::Negada));
    }

    #[test]
    fn ips_que_no_pueden_ser() {
        for ip in [[0, 0, 0, 0], [127, 0, 0, 1], [224, 0, 0, 1], [255, 255, 255, 255]] {
            assert_eq!(leer(&respuesta(tipo::OFFER, ip), YO, XID), Err(Rechazo::Direccion), "{ip:?}");
        }
    }

    #[test]
    fn opciones_que_mienten() {
        let mut sin_fin = bootp(tipo::OFFER, XID, YO, LIBRE, 3600);
        sin_fin.pop();
        assert_eq!(leer(&envolver(&sin_fin), YO, XID), Err(Rechazo::Corto));
        let mut largo = bootp(tipo::OFFER, XID, YO, LIBRE, 3600);
        largo[OPCIONES + 4] = 200;
        assert_eq!(leer(&envolver(&largo), YO, XID), Err(Rechazo::Corto));
        let mut tipo_raro = bootp(tipo::OFFER, XID, YO, LIBRE, 3600);
        tipo_raro[OPCIONES + 1] = 2;
        assert!(leer(&envolver(&tipo_raro), YO, XID).is_err());
        assert_eq!(leer(&envolver(&bootp(tipo::ACK, XID, YO, LIBRE, 10)), YO, XID), Err(Rechazo::Cabecera), "10 s");
    }

    #[test]
    fn veinte_mil_respuestas_mutadas_no_revientan() {
        let base = bootp(tipo::OFFER, XID, YO, LIBRE, 3600);
        let mut semilla = 0xD4C9_0001u64;
        let mut azar = || {
            semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semilla >> 33) as usize
        };
        for _ in 0..20_000 {
            let mut m = base.clone();
            for _ in 0..1 + azar() % 6 {
                let i = OPCIONES + azar() % (m.len() - OPCIONES);
                m[i] = azar() as u8;
            }
            m.truncate(OPCIONES + azar() % (m.len() - OPCIONES + 1));
            let _ = leer(&envolver(&m), YO, XID);
        }
    }
}
