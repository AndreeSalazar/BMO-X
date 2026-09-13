//! **ARP**: que MAC tiene una IP. Y la cache que no se deja envenenar.
//!
//! ## ** No se aprende lo que no se pregunto
//!
//! Una pila normal apunta la MAC de cualquier respuesta --o de cualquier
//! pregunta-- que le llegue. Eso es el envenenamiento de ARP: quien comparte el
//! cable dice "la puerta de enlace soy yo" y se lleva el trafico. Aqui:
//!
//! ```text
//!    respuesta a una pregunta NUESTRA reciente     se apunta
//!    la misma MAC que ya teniamos                  refresca
//!    otra MAC para una IP conocida, sin preguntar  RECHAZO (Direccion)
//!    una IP que nunca preguntamos                  se ignora (NoEsParaMi)
//!    las PREGUNTAS de otros                        se contestan, no se apuntan
//! ```
//!
//! ## Determinista
//!
//! Tamanos fijos, sin reloj propio: el tiempo lo pasa quien llama (`ahora`, en
//! milisegundos). Con la misma secuencia de llamadas, la misma cache. Llena, se
//! pisa la fila MAS VIEJA, y si hay empate la de indice menor.

use crate::ether::{self, Mac};
use crate::ipv4::{self, Ip};
use crate::{be16, pon16, Rechazo};

pub const LARGO: usize = 28;
pub const FILAS: usize = 16;
pub const PREGUNTAS: usize = 4;
/// Lo que vive una MAC apuntada.
pub const VIDA_MS: u64 = 60_000;
/// Cuanto se espera la respuesta a una pregunta.
pub const ESPERA_MS: u64 = 3_000;
/// Cada cuanto se puede repetir la misma pregunta.
pub const REINTENTO_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Pregunta,
    Respuesta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arp {
    pub op: Op,
    pub mac_origen: Mac,
    pub ip_origen: Ip,
    pub mac_destino: Mac,
    pub ip_destino: Ip,
}

pub fn leer(b: &[u8]) -> Result<Arp, Rechazo> {
    if b.len() < LARGO {
        return Err(Rechazo::Corto);
    }
    if be16(b, 0) != 1 || be16(b, 2) != ether::TIPO_IPV4 || b[4] != 6 || b[5] != 4 {
        return Err(Rechazo::Cabecera);
    }
    let op = match be16(b, 6) {
        1 => Op::Pregunta,
        2 => Op::Respuesta,
        _ => return Err(Rechazo::Tipo),
    };
    let a = Arp {
        op,
        mac_origen: [b[8], b[9], b[10], b[11], b[12], b[13]],
        ip_origen: [b[14], b[15], b[16], b[17]],
        mac_destino: [b[18], b[19], b[20], b[21], b[22], b[23]],
        ip_destino: [b[24], b[25], b[26], b[27]],
    };
    if ether::es_grupo(&a.mac_origen) || a.mac_origen == [0; 6] {
        return Err(Rechazo::Direccion);
    }
    if ipv4::es_grupo(&a.ip_origen) || ipv4::es_difusion(&a.ip_origen) {
        return Err(Rechazo::Direccion);
    }
    Ok(a)
}

pub fn escribir(dst: &mut [u8], a: &Arp) -> Result<usize, Rechazo> {
    if dst.len() < LARGO {
        return Err(Rechazo::Corto);
    }
    pon16(dst, 0, 1);
    pon16(dst, 2, ether::TIPO_IPV4);
    dst[4] = 6;
    dst[5] = 4;
    pon16(dst, 6, if a.op == Op::Pregunta { 1 } else { 2 });
    dst[8..14].copy_from_slice(&a.mac_origen);
    dst[14..18].copy_from_slice(&a.ip_origen);
    dst[18..24].copy_from_slice(&a.mac_destino);
    dst[24..28].copy_from_slice(&a.ip_destino);
    Ok(LARGO)
}

#[derive(Clone, Copy)]
struct Fila {
    ip: Ip,
    mac: Mac,
    visto: u64,
    usada: bool,
}

#[derive(Clone, Copy)]
struct Pregunta {
    ip: Ip,
    enviada: u64,
    usada: bool,
}

pub struct Cache {
    filas: [Fila; FILAS],
    preguntas: [Pregunta; PREGUNTAS],
}

impl Default for Cache {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Cache {
    pub const fn nueva() -> Self {
        Self {
            filas: [Fila { ip: [0; 4], mac: [0; 6], visto: 0, usada: false }; FILAS],
            preguntas: [Pregunta { ip: [0; 4], enviada: 0, usada: false }; PREGUNTAS],
        }
    }

    /// La MAC de `ip`, si esta apuntada y no ha caducado.
    pub fn buscar(&self, ip: Ip, ahora: u64) -> Option<Mac> {
        self.filas
            .iter()
            .find(|f| f.usada && f.ip == ip && ahora.saturating_sub(f.visto) < VIDA_MS)
            .map(|f| f.mac)
    }

    /// **Hay que mandar la pregunta por `ip` AHORA?** Apunta que se mando. Una
    /// misma IP no se pregunta mas de una vez por `REINTENTO_MS`: sin eso, un
    /// bucle de envios inunda el cable de difusiones.
    pub fn preguntar(&mut self, ip: Ip, ahora: u64) -> Result<bool, Rechazo> {
        if let Some(p) = self.preguntas.iter_mut().find(|p| p.usada && p.ip == ip) {
            if ahora.saturating_sub(p.enviada) >= REINTENTO_MS {
                p.enviada = ahora;
                return Ok(true);
            }
            return Ok(false);
        }
        let hueco = self
            .preguntas
            .iter_mut()
            .find(|p| !p.usada || ahora.saturating_sub(p.enviada) >= ESPERA_MS)
            .ok_or(Rechazo::Lleno)?;
        *hueco = Pregunta { ip, enviada: ahora, usada: true };
        Ok(true)
    }

    fn esperada(&self, ip: Ip, ahora: u64) -> bool {
        self.preguntas
            .iter()
            .any(|p| p.usada && p.ip == ip && ahora.saturating_sub(p.enviada) < ESPERA_MS)
    }

    fn contestada(&mut self, ip: Ip) {
        for p in self.preguntas.iter_mut().filter(|p| p.ip == ip) {
            p.usada = false;
        }
    }

    /// **Apunta que `ip` esta en `mac`**, si la regla de la cabecera lo deja.
    pub fn aprender(&mut self, ip: Ip, mac: Mac, ahora: u64) -> Result<(), Rechazo> {
        let esperada = self.esperada(ip, ahora);
        if let Some(f) = self.filas.iter_mut().find(|f| f.usada && f.ip == ip) {
            if f.mac != mac && !esperada {
                return Err(Rechazo::Direccion);
            }
            f.mac = mac;
            f.visto = ahora;
            self.contestada(ip);
            return Ok(());
        }
        if !esperada {
            return Err(Rechazo::NoEsParaMi);
        }
        let i = match self.filas.iter().position(|f| !f.usada) {
            Some(i) => i,
            None => {
                let mut viejo = 0;
                for (i, f) in self.filas.iter().enumerate() {
                    if f.visto < self.filas[viejo].visto {
                        viejo = i;
                    }
                }
                viejo
            }
        };
        self.filas[i] = Fila { ip, mac, visto: ahora, usada: true };
        self.contestada(ip);
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ROUTER: Ip = [192, 168, 1, 1];
    const MAC_R: Mac = [0x02, 0, 0, 0, 0, 0x01];
    const MAC_MALA: Mac = [0x02, 0, 0, 0, 0, 0x66];

    #[test]
    fn ida_y_vuelta() {
        let a = Arp { op: Op::Respuesta, mac_origen: MAC_R, ip_origen: ROUTER, mac_destino: [2; 6], ip_destino: [192, 168, 1, 10] };
        let mut b = [0u8; LARGO];
        escribir(&mut b, &a).unwrap();
        assert_eq!(leer(&b), Ok(a));
    }

    #[test]
    fn cabeceras_raras() {
        let a = Arp { op: Op::Pregunta, mac_origen: MAC_R, ip_origen: ROUTER, mac_destino: [0; 6], ip_destino: [1, 2, 3, 4] };
        let mut b = [0u8; LARGO];
        escribir(&mut b, &a).unwrap();
        let mut malo = b;
        malo[5] = 16;
        assert_eq!(leer(&malo), Err(Rechazo::Cabecera));
        let mut malo = b;
        malo[7] = 3;
        assert_eq!(leer(&malo), Err(Rechazo::Tipo));
        let mut malo = b;
        malo[8] = 0x01;
        assert_eq!(leer(&malo), Err(Rechazo::Direccion));
        assert_eq!(leer(&b[..27]), Err(Rechazo::Corto));
    }

    #[test]
    fn sin_preguntar_no_se_aprende() {
        let mut c = Cache::nueva();
        assert_eq!(c.aprender(ROUTER, MAC_R, 0), Err(Rechazo::NoEsParaMi));
        assert_eq!(c.buscar(ROUTER, 0), None);
    }

    #[test]
    fn preguntar_contestar_y_el_envenenador() {
        let mut c = Cache::nueva();
        assert_eq!(c.preguntar(ROUTER, 100), Ok(true));
        assert_eq!(c.preguntar(ROUTER, 500), Ok(false), "no se repite antes de un segundo");
        assert_eq!(c.aprender(ROUTER, MAC_R, 600), Ok(()));
        assert_eq!(c.buscar(ROUTER, 600), Some(MAC_R));
        // Otro dice ser el router, sin que nadie pregunte: fuera.
        assert_eq!(c.aprender(ROUTER, MAC_MALA, 700), Err(Rechazo::Direccion));
        assert_eq!(c.buscar(ROUTER, 700), Some(MAC_R));
        // La respuesta tardia a una pregunta vieja tampoco cuela.
        assert_eq!(c.preguntar(ROUTER, 5_000), Ok(true));
        assert_eq!(c.aprender(ROUTER, MAC_MALA, 9_000), Err(Rechazo::Direccion));
    }

    #[test]
    fn caduca_y_la_llena_pisa_la_mas_vieja() {
        let mut c = Cache::nueva();
        for i in 0..=FILAS as u8 {
            let ip = [10, 0, 0, i];
            c.preguntar(ip, i as u64 * 10).unwrap_or(false);
            // Libera la pregunta vieja para no llenar las cuatro.
            c.aprender(ip, [2, 0, 0, 0, 0, i], i as u64 * 10).unwrap();
        }
        assert_eq!(c.buscar([10, 0, 0, 0], 200), None, "la mas vieja se piso");
        assert_eq!(c.buscar([10, 0, 0, 16], 200), Some([2, 0, 0, 0, 0, 16]));
        assert_eq!(c.buscar([10, 0, 0, 16], 200 + VIDA_MS), None, "y caduca");
    }

    #[test]
    fn cuatro_preguntas_a_la_vez_y_la_quinta_espera() {
        let mut c = Cache::nueva();
        for i in 0..PREGUNTAS as u8 {
            assert_eq!(c.preguntar([10, 0, 0, i], 0), Ok(true));
        }
        assert_eq!(c.preguntar([10, 0, 0, 9], 10), Err(Rechazo::Lleno));
        assert_eq!(c.preguntar([10, 0, 0, 9], ESPERA_MS), Ok(true), "cuando caduca una, cabe");
    }
}
