//! **EL NODO**: una MAC y una IP que contestan ARP y `ping`, y reparten el
//! resto. Es el paso 4 de `docs/maestro/RED_MAESTRO.md` escrito entero sin
//! tarjeta: lo que falta para verlo en el Ryzen es transmitir.
//!
//! ```text
//!    trama -> Ethernet -> ARP  -> pregunta por MI ip       contesta
//!                              -> respuesta a MI pregunta  se apunta
//!                      -> IPv4 -> ICMP eco a MI ip          contesta (10/s)
//!                              -> UDP                       se entrega
//!                              -> TCP                       se entrega a `Tcp`
//! ```
//!
//! Solo contesta a lo que va a SU IP y en una trama a SU MAC: un `ping` a la
//! difusion no tiene respuesta (el amplificador "smurf" no existe aqui), y una
//! IP propia metida en una trama de difusion se rechaza como sospechosa.

use crate::arp::{self, Arp, Op};
use crate::ether::{self, Mac};
use crate::icmp;
use crate::ipv4::{self, Ip};
use crate::udp::{self, Datagrama};
use crate::Rechazo;

/// Donde empieza la carga de un paquete IPv4 dentro de una trama: ahi escribe
/// `Tcp::salida` antes de llamar a [`Nodo::envolver`].
pub const CARGA: usize = ether::CABECERA + ipv4::CABECERA;
pub const ECOS_POR_SEGUNDO: u32 = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cuentas {
    pub tramas: u64,
    pub rechazadas: u64,
    pub arp_contestadas: u64,
    pub ecos_contestados: u64,
    pub udp: u64,
    pub tcp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hecho<'a> {
    Nada,
    /// Hay una trama lista en la salida, de este largo.
    Contestar(usize),
    Eco { origen: Ip, id: u16, secuencia: u16 },
    Udp { origen: Ip, datagrama: Datagrama<'a> },
    Tcp { origen: Ip, destino: Ip, segmento: &'a [u8] },
}

pub struct Nodo {
    pub mac: Mac,
    pub ip: Ip,
    pub arp: arp::Cache,
    pub cuentas: Cuentas,
    id: u16,
    ecos: (u64, u32),
}

impl Nodo {
    pub const fn nuevo(mac: Mac, ip: Ip) -> Self {
        Self {
            mac,
            ip,
            arp: arp::Cache::nueva(),
            cuentas: Cuentas { tramas: 0, rechazadas: 0, arp_contestadas: 0, ecos_contestados: 0, udp: 0, tcp: 0 },
            id: 0,
            ecos: (0, 0),
        }
    }

    /// **Una trama que llego.** Si hay que contestar, la respuesta queda en
    /// `salida` y el `Hecho` dice su largo.
    pub fn atender<'a>(&mut self, trama: &'a [u8], ahora: u64, salida: &mut [u8]) -> Result<Hecho<'a>, Rechazo> {
        self.cuentas.tramas += 1;
        let r = self.atender_sin_contar(trama, ahora, salida);
        if r.is_err() {
            self.cuentas.rechazadas += 1;
        }
        r
    }

    fn atender_sin_contar<'a>(&mut self, trama: &'a [u8], ahora: u64, salida: &mut [u8]) -> Result<Hecho<'a>, Rechazo> {
        let t = ether::leer(trama)?;
        if t.destino != self.mac && t.destino != ether::DIFUSION {
            return Err(Rechazo::NoEsParaMi);
        }
        match t.tipo {
            ether::TIPO_ARP => {
                let a = arp::leer(t.carga)?;
                // La MAC de dentro tiene que ser la de la trama: si no, alguien
                // habla en nombre de otro.
                if a.mac_origen != t.origen {
                    return Err(Rechazo::Direccion);
                }
                if a.ip_destino != self.ip {
                    return Err(Rechazo::NoEsParaMi);
                }
                match a.op {
                    Op::Respuesta => {
                        self.arp.aprender(a.ip_origen, a.mac_origen, ahora)?;
                        Ok(Hecho::Nada)
                    }
                    Op::Pregunta => {
                        let r = Arp {
                            op: Op::Respuesta,
                            mac_origen: self.mac,
                            ip_origen: self.ip,
                            mac_destino: a.mac_origen,
                            ip_destino: a.ip_origen,
                        };
                        let n = ether::escribir(salida, a.mac_origen, self.mac, ether::TIPO_ARP)?;
                        let m = arp::escribir(&mut salida[n..], &r)?;
                        self.cuentas.arp_contestadas += 1;
                        Ok(Hecho::Contestar(ether::con_relleno(salida, n + m)))
                    }
                }
            }
            ether::TIPO_IPV4 => {
                let p = ipv4::leer(t.carga)?;
                if p.destino != self.ip {
                    return Err(Rechazo::NoEsParaMi);
                }
                if t.destino != self.mac {
                    return Err(Rechazo::Direccion);
                }
                match p.protocolo {
                    ipv4::ICMP => {
                        let e = icmp::leer(p.carga)?;
                        if e.tipo == icmp::ECO_RESPUESTA {
                            return Ok(Hecho::Eco { origen: p.origen, id: e.id, secuencia: e.secuencia });
                        }
                        if ahora.saturating_sub(self.ecos.0) >= 1_000 {
                            self.ecos = (ahora, 0);
                        }
                        if self.ecos.1 >= ECOS_POR_SEGUNDO {
                            return Err(Rechazo::Lleno);
                        }
                        if salida.len() < CARGA {
                            return Err(Rechazo::Corto);
                        }
                        let m = icmp::escribir(&mut salida[CARGA..], icmp::ECO_RESPUESTA, e.id, e.secuencia, e.datos)?;
                        let n = self.envolver(salida, t.origen, p.origen, ipv4::ICMP, m)?;
                        self.ecos.1 += 1;
                        self.cuentas.ecos_contestados += 1;
                        Ok(Hecho::Contestar(n))
                    }
                    ipv4::UDP => {
                        let d = udp::leer(p.carga, p.origen, p.destino)?;
                        self.cuentas.udp += 1;
                        Ok(Hecho::Udp { origen: p.origen, datagrama: d })
                    }
                    ipv4::TCP => {
                        self.cuentas.tcp += 1;
                        Ok(Hecho::Tcp { origen: p.origen, destino: p.destino, segmento: p.carga })
                    }
                    _ => Err(Rechazo::Protocolo),
                }
            }
            _ => Err(Rechazo::Tipo),
        }
    }

    /// Pone Ethernet e IPv4 delante de `largo` bytes ya escritos en
    /// `salida[CARGA..]`. Devuelve el largo de la trama.
    pub fn envolver(&mut self, salida: &mut [u8], mac_destino: Mac, ip_destino: Ip, protocolo: u8, largo: usize) -> Result<usize, Rechazo> {
        if salida.len() < CARGA + largo {
            return Err(Rechazo::Corto);
        }
        ether::escribir(salida, mac_destino, self.mac, ether::TIPO_IPV4)?;
        self.id = self.id.wrapping_add(1);
        ipv4::escribir(&mut salida[ether::CABECERA..], self.ip, ip_destino, protocolo, largo, self.id)?;
        Ok(ether::con_relleno(salida, CARGA + largo))
    }

    /// La pregunta ARP por `ip`, si toca mandarla ahora.
    pub fn preguntar(&mut self, ip: Ip, ahora: u64, salida: &mut [u8]) -> Result<Option<usize>, Rechazo> {
        if !self.arp.preguntar(ip, ahora)? {
            return Ok(None);
        }
        let a = Arp { op: Op::Pregunta, mac_origen: self.mac, ip_origen: self.ip, mac_destino: [0; 6], ip_destino: ip };
        let n = ether::escribir(salida, ether::DIFUSION, self.mac, ether::TIPO_ARP)?;
        let m = arp::escribir(&mut salida[n..], &a)?;
        Ok(Some(ether::con_relleno(salida, n + m)))
    }

    /// Un datagrama UDP entero, listo para el cable.
    pub fn udp(&mut self, salida: &mut [u8], mac_destino: Mac, ip_destino: Ip, origen: u16, destino: u16, datos: &[u8]) -> Result<usize, Rechazo> {
        if salida.len() < CARGA {
            return Err(Rechazo::Corto);
        }
        let m = udp::escribir(&mut salida[CARGA..], self.ip, ip_destino, origen, destino, datos)?;
        self.envolver(salida, mac_destino, ip_destino, ipv4::UDP, m)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const YO_MAC: Mac = [0x02, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E];
    const YO: Ip = [192, 168, 1, 50];
    const R_MAC: Mac = [0x02, 0, 0, 0, 0, 0x01];
    const R: Ip = [192, 168, 1, 1];

    fn trama_arp(op: Op, destino: Mac, origen_trama: Mac, a: Arp) -> Vec<u8> {
        let mut b = vec![0u8; 60];
        let _ = (op, ());
        ether::escribir(&mut b, destino, origen_trama, ether::TIPO_ARP).unwrap();
        arp::escribir(&mut b[14..], &a).unwrap();
        b
    }

    fn ping(destino_ip: Ip, destino_mac: Mac, datos: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; CARGA + 8 + datos.len()];
        ether::escribir(&mut b, destino_mac, R_MAC, ether::TIPO_IPV4).unwrap();
        let m = icmp::escribir(&mut b[CARGA..], icmp::ECO_PETICION, 0x1234, 1, datos).unwrap();
        ipv4::escribir(&mut b[14..], R, destino_ip, ipv4::ICMP, m, 9).unwrap();
        b
    }

    #[test]
    fn contesta_la_pregunta_por_su_ip() {
        let mut n = Nodo::nuevo(YO_MAC, YO);
        let q = Arp { op: Op::Pregunta, mac_origen: R_MAC, ip_origen: R, mac_destino: [0; 6], ip_destino: YO };
        let t = trama_arp(Op::Pregunta, ether::DIFUSION, R_MAC, q);
        let mut sal = [0u8; 128];
        let Hecho::Contestar(largo) = n.atender(&t, 0, &mut sal).unwrap() else { panic!() };
        let e = ether::leer(&sal[..largo]).unwrap();
        let a = arp::leer(e.carga).unwrap();
        assert_eq!((e.destino, a.op, a.mac_origen, a.ip_origen, a.ip_destino), (R_MAC, Op::Respuesta, YO_MAC, YO, R));
    }

    #[test]
    fn la_pregunta_por_otro_y_la_mac_que_miente() {
        let mut n = Nodo::nuevo(YO_MAC, YO);
        let mut sal = [0u8; 128];
        let q = Arp { op: Op::Pregunta, mac_origen: R_MAC, ip_origen: R, mac_destino: [0; 6], ip_destino: [192, 168, 1, 77] };
        assert_eq!(n.atender(&trama_arp(Op::Pregunta, ether::DIFUSION, R_MAC, q), 0, &mut sal), Err(Rechazo::NoEsParaMi));
        let q = Arp { ip_destino: YO, ..q };
        assert_eq!(n.atender(&trama_arp(Op::Pregunta, ether::DIFUSION, [2, 9, 9, 9, 9, 9], q), 0, &mut sal), Err(Rechazo::Direccion));
        assert_eq!(n.cuentas.rechazadas, 2);
    }

    #[test]
    fn solo_aprende_lo_que_pregunto() {
        let mut n = Nodo::nuevo(YO_MAC, YO);
        let mut sal = [0u8; 128];
        let r = Arp { op: Op::Respuesta, mac_origen: R_MAC, ip_origen: R, mac_destino: YO_MAC, ip_destino: YO };
        let t = trama_arp(Op::Respuesta, YO_MAC, R_MAC, r);
        assert_eq!(n.atender(&t, 0, &mut sal), Err(Rechazo::NoEsParaMi));
        assert!(n.preguntar(R, 10, &mut sal).unwrap().is_some());
        assert_eq!(n.atender(&t, 20, &mut sal), Ok(Hecho::Nada));
        assert_eq!(n.arp.buscar(R, 20), Some(R_MAC));
    }

    #[test]
    fn contesta_el_ping_con_los_mismos_datos() {
        let mut n = Nodo::nuevo(YO_MAC, YO);
        let mut sal = [0u8; 256];
        let Hecho::Contestar(largo) = n.atender(&ping(YO, YO_MAC, b"abcdefgh"), 0, &mut sal).unwrap() else { panic!() };
        let e = ether::leer(&sal[..largo]).unwrap();
        let p = ipv4::leer(e.carga).unwrap();
        let eco = icmp::leer(p.carga).unwrap();
        assert_eq!((e.destino, p.destino, p.origen, eco.tipo, eco.id, eco.datos), (R_MAC, R, YO, icmp::ECO_RESPUESTA, 0x1234, &b"abcdefgh"[..]));
    }

    #[test]
    fn el_ping_a_la_difusion_no_se_contesta_y_hay_tope() {
        let mut n = Nodo::nuevo(YO_MAC, YO);
        let mut sal = [0u8; 256];
        assert_eq!(n.atender(&ping([192, 168, 1, 255], ether::DIFUSION, b""), 0, &mut sal), Err(Rechazo::NoEsParaMi));
        assert_eq!(n.atender(&ping(YO, ether::DIFUSION, b""), 0, &mut sal), Err(Rechazo::Direccion));
        for _ in 0..ECOS_POR_SEGUNDO {
            assert!(matches!(n.atender(&ping(YO, YO_MAC, b""), 500, &mut sal), Ok(Hecho::Contestar(_))));
        }
        assert_eq!(n.atender(&ping(YO, YO_MAC, b""), 900, &mut sal), Err(Rechazo::Lleno));
        assert!(n.atender(&ping(YO, YO_MAC, b""), 1_500, &mut sal).is_ok(), "al segundo siguiente, otra vez");
    }

    #[test]
    fn udp_se_entrega() {
        let mut n = Nodo::nuevo(YO_MAC, YO);
        let mut otro = Nodo::nuevo(R_MAC, R);
        let mut t = [0u8; 128];
        let largo = otro.udp(&mut t, YO_MAC, YO, 53, 40000, b"respuesta").unwrap();
        let mut sal = [0u8; 128];
        let Hecho::Udp { origen, datagrama } = n.atender(&t[..largo], 0, &mut sal).unwrap() else { panic!() };
        assert_eq!((origen, datagrama.origen, datagrama.destino, datagrama.datos), (R, 53, 40000, &b"respuesta"[..]));
    }

    /// ** Ninguna trama hace panico: veinte mil mutaciones de un ping y de una
    /// pregunta ARP buenos.
    #[test]
    fn veinte_mil_tramas_mutadas_no_revientan() {
        let mut n = Nodo::nuevo(YO_MAC, YO);
        let q = Arp { op: Op::Pregunta, mac_origen: R_MAC, ip_origen: R, mac_destino: [0; 6], ip_destino: YO };
        let bases = [ping(YO, YO_MAC, b"0123456789"), trama_arp(Op::Pregunta, ether::DIFUSION, R_MAC, q)];
        let mut semilla = 42u64;
        let mut azar = || {
            semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semilla >> 33) as usize
        };
        let mut sal = [0u8; 1600];
        for vuelta in 0..20_000u64 {
            let mut m = bases[azar() % 2].clone();
            for _ in 0..1 + azar() % 3 {
                let i = azar() % m.len();
                m[i] = azar() as u8;
            }
            m.truncate(azar() % (m.len() + 1));
            let _ = n.atender(&m, vuelta, &mut sal);
        }
        assert_eq!(n.cuentas.tramas, 20_000);
    }
}
