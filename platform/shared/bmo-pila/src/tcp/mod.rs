//! **TCP**: la maquina de estados, con tamanos fijos y sin reloj propio.
//!
//! ## Determinista: la misma entrada, la misma salida
//!
//! ```text
//!    el tiempo          lo pasa quien llama (`ahora`, en ms). No hay reloj
//!    los reintentos     1, 2, 4, 8, 16, 32 s y a la septima se rinde. SIN
//!                       estimar el RTT: un temporizador que aprende es un
//!                       temporizador que hoy dice una cosa y manana otra
//!    el orden           los segmentos FUERA DE ORDEN se descartan y se pide
//!                       el que falta. Sin cola de reensamblado: sin memoria
//!                       que un atacante pueda llenar, y el otro lado reenvia
//!    la memoria         ocho conexiones, 8 KiB de ida y 8 de vuelta cada una.
//!                       Llena es `Rechazo::Lleno`, nunca un monton que crece
//!    el ISN             HMAC-SHA256(secreto, las cuatro direcciones) + reloj
//!                       de 4 ms (RFC 6528). Con el mismo secreto, el mismo
//!                       numero; sin el secreto, imposible de adivinar
//! ```
//!
//! ## Blindado: RFC 5961 y lo que la experiencia ensena
//!
//! ```text
//!    RST                 corta SOLO si su secuencia es EXACTA; dentro de la
//!                        ventana pero no exacta -> ACK de reto, sin cortar
//!    RST en TIME-WAIT    se ignora (RFC 1337)
//!    SYN en una viva     ACK de reto, nunca un reinicio
//!    ACK del futuro      (mas alla de lo enviado) se descarta con reto
//!    retos               como mucho diez por segundo, para no ser un eco
//!    puerto sin escucha  SILENCIO: ni RST. Un puerto cerrado no contesta
//!    huerfanas           una conexion aceptada por la pila que la aplicacion
//!                        no recoge en 60 s se aborta: no ocupa un hueco
//! ```
//!
//! Lo que NO hace, y es a proposito: escala de ventana, SACK, marcas de tiempo,
//! apertura simultanea, datos en el SYN y urgente. Cada uno es un camino mas.

use bmo_cripto::hmac_sha256;

use crate::ipv4::{self, Ip};
use crate::Rechazo;

pub mod segmento;
#[cfg(test)]
mod pruebas;

use segmento::bandera::*;
use segmento::Segmento;

pub const CONEXIONES: usize = 8;
pub const BUFER: usize = 8192;
pub const MSS: u16 = 1460;
pub const MSS_MIN: u16 = 536;
pub const RTO_INICIAL_MS: u64 = 1_000;
pub const RTO_MAX_MS: u64 = 32_000;
pub const REINTENTOS: u8 = 6;
pub const TIEMPO_ESPERA_MS: u64 = 60_000;
pub const RETOS_POR_SEGUNDO: u32 = 10;
const PUERTO_BAJO: u16 = 49_152;
const PUERTOS: u32 = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    Libre,
    Escucha,
    SynEnviado,
    SynRecibido,
    Establecida,
    FinEspera1,
    FinEspera2,
    CierreEspera,
    Cerrando,
    UltimoAck,
    TiempoEspera,
    Cerrada(Cierre),
}

/// Por que se cerro: una conexion que acaba tiene que decirlo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cierre {
    Normal,
    /// El otro lado mando un RST valido.
    Reset,
    /// Seis reintentos sin respuesta.
    Agotada,
    /// La aplicacion la aborto.
    Abortada,
}

#[derive(Clone, Copy)]
struct Conexion {
    estado: Estado,
    mi_ip: Ip,
    su_ip: Ip,
    mi_puerto: u16,
    su_puerto: u16,
    isn: u32,
    snd_una: u32,
    snd_nxt: u32,
    snd_wnd: u32,
    su_mss: u16,
    tx: [u8; BUFER],
    tx_len: usize,
    fin_pedido: bool,
    fin_confirmado: bool,
    rcv_nxt: u32,
    rx: [u8; BUFER],
    rx_len: usize,
    rto: u64,
    reintentos: u8,
    vence: Option<u64>,
    sondear: bool,
    cwnd: u32,
    ssthresh: u32,
    ack_debido: bool,
    desde: u64,
    escucha: Option<u8>,
    huerfana: bool,
}

const LIBRE: Conexion = Conexion {
    estado: Estado::Libre,
    mi_ip: [0; 4],
    su_ip: [0; 4],
    mi_puerto: 0,
    su_puerto: 0,
    isn: 0,
    snd_una: 0,
    snd_nxt: 0,
    snd_wnd: 0,
    su_mss: MSS_MIN,
    tx: [0; BUFER],
    tx_len: 0,
    fin_pedido: false,
    fin_confirmado: false,
    rcv_nxt: 0,
    rx: [0; BUFER],
    rx_len: 0,
    rto: RTO_INICIAL_MS,
    reintentos: 0,
    vence: None,
    sondear: false,
    cwnd: 2 * MSS as u32,
    ssthresh: u32::MAX,
    ack_debido: false,
    desde: 0,
    escucha: None,
    huerfana: false,
};

/// `a` va antes que `b` en el espacio de secuencias (mitad del circulo).
fn antes(a: u32, b: u32) -> bool {
    let d = b.wrapping_sub(a);
    d != 0 && d < 0x8000_0000
}

fn dentro(x: u32, desde: u32, largo: u32) -> bool {
    x.wrapping_sub(desde) < largo
}

fn mss_de(s: &Segmento) -> u16 {
    s.mss.unwrap_or(MSS_MIN).clamp(MSS_MIN, MSS)
}

impl Conexion {
    fn viva(&self) -> bool {
        !matches!(self.estado, Estado::Libre | Estado::Escucha | Estado::Cerrada(_))
    }

    fn ventana_rx(&self) -> u16 {
        (BUFER - self.rx_len).min(u16::MAX as usize) as u16
    }

    fn en_vuelo(&self) -> u32 {
        self.snd_nxt.wrapping_sub(self.snd_una)
    }

    fn mss(&self) -> u32 {
        self.su_mss.min(MSS) as u32
    }

    fn armar(&mut self, ahora: u64) {
        if self.vence.is_none() {
            self.vence = Some(ahora + self.rto);
        }
    }

    fn cerrar_con(&mut self, c: Cierre) {
        self.vence = None;
        self.estado = if self.huerfana { Estado::Libre } else { Estado::Cerrada(c) };
    }
}

#[derive(Clone, Copy)]
struct Rst {
    mi_ip: Ip,
    su_ip: Ip,
    mi_puerto: u16,
    su_puerto: u16,
    sec: u32,
    ack: u32,
}

pub struct Tcp {
    c: [Conexion; CONEXIONES],
    secreto: [u8; 32],
    contador_puertos: u32,
    retos: (u64, u32),
    rst: Option<Rst>,
    turno: usize,
}

impl Tcp {
    /// `secreto` sale de `bmo_cripto::azar::clave()` en la maquina, y fijo en
    /// las pruebas: es lo que hace el ISN imprevisible Y reproducible.
    pub fn nueva(secreto: [u8; 32]) -> Self {
        Self { c: [LIBRE; CONEXIONES], secreto, contador_puertos: 0, retos: (0, 0), rst: None, turno: 0 }
    }

    fn con(&mut self, asa: usize) -> Result<&mut Conexion, Rechazo> {
        match self.c.get_mut(asa) {
            Some(c) if c.estado != Estado::Libre => Ok(c),
            _ => Err(Rechazo::Asa),
        }
    }

    fn libre(&self) -> Result<usize, Rechazo> {
        self.c.iter().position(|c| c.estado == Estado::Libre).ok_or(Rechazo::Lleno)
    }

    fn isn(&self, mi_ip: Ip, su_ip: Ip, mi_puerto: u16, su_puerto: u16, ahora: u64) -> u32 {
        let mut m = [0u8; 12];
        m[..4].copy_from_slice(&mi_ip);
        m[4..8].copy_from_slice(&su_ip);
        m[8..10].copy_from_slice(&mi_puerto.to_be_bytes());
        m[10..].copy_from_slice(&su_puerto.to_be_bytes());
        let h = hmac_sha256(&self.secreto, &m);
        u32::from_be_bytes([h[0], h[1], h[2], h[3]]).wrapping_add((ahora / 4) as u32)
    }

    pub fn estado(&self, asa: usize) -> Estado {
        self.c.get(asa).map_or(Estado::Libre, |c| c.estado)
    }

    /// `(mi ip, mi puerto, su ip, su puerto)`, para la cabina y las pruebas.
    pub fn extremos(&self, asa: usize) -> Option<(Ip, u16, Ip, u16)> {
        self.c.get(asa).filter(|c| c.estado != Estado::Libre).map(|c| (c.mi_ip, c.mi_puerto, c.su_ip, c.su_puerto))
    }

    /// `(siguiente a enviar, siguiente esperado)`.
    pub fn secuencias(&self, asa: usize) -> Option<(u32, u32)> {
        self.c.get(asa).filter(|c| c.estado != Estado::Libre).map(|c| (c.snd_nxt, c.rcv_nxt))
    }

    /// Bytes esperando a la aplicacion y bytes sin confirmar.
    pub fn pendientes(&self, asa: usize) -> (usize, usize) {
        self.c.get(asa).map_or((0, 0), |c| (c.rx_len, c.tx_len))
    }

    /// **Abre una conexion.** El puerto propio sale de RFC 6056 (hash + contador).
    pub fn conectar(&mut self, mi_ip: Ip, su_ip: Ip, su_puerto: u16, ahora: u64) -> Result<usize, Rechazo> {
        if su_puerto == 0 {
            return Err(Rechazo::Puerto);
        }
        if ipv4::origen_imposible(&su_ip) {
            return Err(Rechazo::Direccion);
        }
        let i = self.libre()?;
        let mut m = [0u8; 6];
        m[..4].copy_from_slice(&su_ip);
        m[4..].copy_from_slice(&su_puerto.to_be_bytes());
        let h = hmac_sha256(&self.secreto, &m);
        let base = u32::from_be_bytes([h[0], h[1], h[2], h[3]]).wrapping_add(self.contador_puertos);
        self.contador_puertos = self.contador_puertos.wrapping_add(1);
        let puerto = (0..PUERTOS)
            .map(|k| PUERTO_BAJO + (base.wrapping_add(k) % PUERTOS) as u16)
            .find(|&p| {
                !self.c.iter().any(|c| {
                    c.estado != Estado::Libre && c.mi_ip == mi_ip && c.su_ip == su_ip && c.mi_puerto == p && c.su_puerto == su_puerto
                })
            })
            .ok_or(Rechazo::Lleno)?;
        let isn = self.isn(mi_ip, su_ip, puerto, su_puerto, ahora);
        self.c[i] = Conexion {
            estado: Estado::SynEnviado,
            mi_ip,
            su_ip,
            mi_puerto: puerto,
            su_puerto,
            isn,
            snd_una: isn,
            snd_nxt: isn,
            desde: ahora,
            ..LIBRE
        };
        Ok(i)
    }

    pub fn escuchar(&mut self, mi_ip: Ip, puerto: u16) -> Result<usize, Rechazo> {
        if puerto == 0 {
            return Err(Rechazo::Puerto);
        }
        if self.c.iter().any(|c| c.estado == Estado::Escucha && c.mi_puerto == puerto && c.mi_ip == mi_ip) {
            return Err(Rechazo::Estado);
        }
        let i = self.libre()?;
        self.c[i] = Conexion { estado: Estado::Escucha, mi_ip, mi_puerto: puerto, ..LIBRE };
        Ok(i)
    }

    /// Una conexion ya establecida en la escucha `asa`, si la hay.
    pub fn aceptar(&mut self, asa: usize) -> Option<usize> {
        let i = self.c.iter().position(|c| {
            c.huerfana && c.escucha == Some(asa as u8) && matches!(c.estado, Estado::Establecida | Estado::CierreEspera)
        })?;
        self.c[i].huerfana = false;
        self.c[i].escucha = None;
        Some(i)
    }

    /// Mete datos en la cola de salida. Devuelve cuantos cupieron.
    pub fn enviar(&mut self, asa: usize, datos: &[u8]) -> Result<usize, Rechazo> {
        let c = self.con(asa)?;
        let admite = matches!(c.estado, Estado::SynEnviado | Estado::Establecida | Estado::CierreEspera);
        if c.fin_pedido || !admite {
            return Err(Rechazo::Estado);
        }
        let n = datos.len().min(BUFER - c.tx_len);
        c.tx[c.tx_len..c.tx_len + n].copy_from_slice(&datos[..n]);
        c.tx_len += n;
        Ok(n)
    }

    /// Saca lo recibido. `Ok(0)` con estado `CierreEspera` es "el otro acabo".
    pub fn recibir(&mut self, asa: usize, dst: &mut [u8]) -> Result<usize, Rechazo> {
        let c = self.con(asa)?;
        let estaba_llena = c.ventana_rx() == 0;
        let n = c.rx_len.min(dst.len());
        dst[..n].copy_from_slice(&c.rx[..n]);
        c.rx.copy_within(n..c.rx_len, 0);
        c.rx_len -= n;
        // Se abre la ventana: se dice, o el otro se queda esperando.
        if estaba_llena && n > 0 {
            c.ack_debido = true;
        }
        Ok(n)
    }

    /// Cierre ordenado: FIN cuando salga lo pendiente.
    pub fn cerrar(&mut self, asa: usize) -> Result<(), Rechazo> {
        let c = self.con(asa)?;
        match c.estado {
            Estado::Escucha => c.estado = Estado::Libre,
            Estado::SynEnviado => c.cerrar_con(Cierre::Normal),
            Estado::SynRecibido | Estado::Establecida | Estado::CierreEspera => c.fin_pedido = true,
            _ => {}
        }
        Ok(())
    }

    /// Corta ya, con RST.
    pub fn abortar(&mut self, asa: usize) -> Result<(), Rechazo> {
        let c = self.con(asa)?;
        if c.viva() && c.estado != Estado::SynEnviado && c.estado != Estado::TiempoEspera {
            let r = Rst { mi_ip: c.mi_ip, su_ip: c.su_ip, mi_puerto: c.mi_puerto, su_puerto: c.su_puerto, sec: c.snd_nxt, ack: c.rcv_nxt };
            c.cerrar_con(Cierre::Abortada);
            self.rst = Some(r);
        } else {
            c.cerrar_con(Cierre::Abortada);
        }
        Ok(())
    }

    /// La aplicacion ya no quiere el asa. Solo de una conexion que no esta viva;
    /// en TIME-WAIT, el hueco se libera al acabar la espera.
    pub fn soltar(&mut self, asa: usize) -> Result<(), Rechazo> {
        let c = self.con(asa)?;
        match c.estado {
            Estado::Cerrada(_) | Estado::Escucha => c.estado = Estado::Libre,
            Estado::TiempoEspera => c.huerfana = true,
            _ => return Err(Rechazo::Estado),
        }
        Ok(())
    }

    fn reto(&mut self, i: usize, ahora: u64) {
        if ahora.saturating_sub(self.retos.0) >= 1_000 {
            self.retos = (ahora, 0);
        }
        if self.retos.1 < RETOS_POR_SEGUNDO {
            self.retos.1 += 1;
            self.c[i].ack_debido = true;
        }
    }

    /// **Un segmento que llego**, ya fuera de su paquete IPv4.
    pub fn entrada(&mut self, ip_origen: Ip, ip_destino: Ip, bytes: &[u8], ahora: u64) -> Result<(), Rechazo> {
        let s = segmento::leer(bytes, ip_origen, ip_destino)?;
        let exacta = self.c.iter().position(|c| {
            c.viva() && c.mi_ip == ip_destino && c.su_ip == ip_origen && c.mi_puerto == s.destino && c.su_puerto == s.origen
        });
        if let Some(i) = exacta {
            return self.en_conexion(i, &s, ahora);
        }
        let escucha = self
            .c
            .iter()
            .position(|c| c.estado == Estado::Escucha && c.mi_puerto == s.destino && c.mi_ip == ip_destino);
        match escucha {
            Some(e) => self.en_escucha(e, ip_origen, ip_destino, &s, ahora),
            None => Err(Rechazo::NoEsParaMi),
        }
    }

    fn en_escucha(&mut self, e: usize, ip_origen: Ip, ip_destino: Ip, s: &Segmento, ahora: u64) -> Result<(), Rechazo> {
        if s.banderas & RST != 0 {
            return Ok(());
        }
        if s.banderas & (SYN | ACK) != SYN {
            return Err(Rechazo::Banderas);
        }
        let i = self.libre()?;
        let isn = self.isn(ip_destino, ip_origen, s.destino, s.origen, ahora);
        self.c[i] = Conexion {
            estado: Estado::SynRecibido,
            mi_ip: ip_destino,
            su_ip: ip_origen,
            mi_puerto: s.destino,
            su_puerto: s.origen,
            isn,
            snd_una: isn,
            snd_nxt: isn,
            snd_wnd: s.ventana as u32,
            su_mss: mss_de(s),
            rcv_nxt: s.sec.wrapping_add(1),
            escucha: Some(e as u8),
            huerfana: true,
            desde: ahora,
            ..LIBRE
        };
        Ok(())
    }

    fn en_conexion(&mut self, i: usize, s: &Segmento, ahora: u64) -> Result<(), Rechazo> {
        let fl = s.banderas;
        if self.c[i].estado == Estado::SynEnviado {
            let c = &mut self.c[i];
            let ack_bueno = fl & ACK != 0 && s.ack == c.isn.wrapping_add(1);
            if fl & ACK != 0 && !ack_bueno {
                return Err(Rechazo::Secuencia);
            }
            if fl & RST != 0 {
                if ack_bueno {
                    c.cerrar_con(Cierre::Reset);
                }
                return Ok(());
            }
            // Sin ACK seria apertura simultanea, que no se hace.
            if fl & SYN == 0 || !ack_bueno {
                return Err(Rechazo::Banderas);
            }
            c.rcv_nxt = s.sec.wrapping_add(1);
            c.snd_una = s.ack;
            c.snd_wnd = s.ventana as u32;
            c.su_mss = mss_de(s);
            c.estado = Estado::Establecida;
            c.vence = None;
            c.reintentos = 0;
            c.rto = RTO_INICIAL_MS;
            c.ack_debido = true;
            c.desde = ahora;
            return Ok(());
        }

        let (rcv_nxt, ventana) = (self.c[i].rcv_nxt, self.c[i].ventana_rx() as u32);
        if fl & RST != 0 {
            if self.c[i].estado == Estado::TiempoEspera {
                return Ok(());
            }
            if s.sec == rcv_nxt {
                self.c[i].cerrar_con(Cierre::Reset);
            } else if dentro(s.sec, rcv_nxt, ventana.max(1)) {
                self.reto(i, ahora);
            }
            return Ok(());
        }
        if fl & SYN != 0 {
            let c = &mut self.c[i];
            if c.estado == Estado::SynRecibido && s.sec.wrapping_add(1) == c.rcv_nxt {
                // El SYN repetido: se vuelve a mandar el SYN-ACK.
                c.snd_nxt = c.isn;
                return Ok(());
            }
            self.reto(i, ahora);
            return Ok(());
        }
        if fl & ACK == 0 {
            return Err(Rechazo::Banderas);
        }
        if antes(self.c[i].snd_nxt, s.ack) {
            self.reto(i, ahora);
            return Err(Rechazo::Secuencia);
        }

        let c = &mut self.c[i];
        if c.estado == Estado::SynRecibido {
            if c.snd_nxt == c.isn || s.ack != c.snd_nxt {
                return Err(Rechazo::Secuencia);
            }
            c.snd_una = s.ack;
            c.snd_wnd = s.ventana as u32;
            c.estado = Estado::Establecida;
            c.vence = None;
            c.reintentos = 0;
            c.rto = RTO_INICIAL_MS;
            c.desde = ahora;
        } else if !antes(s.ack, c.snd_una) {
            let confirmados = s.ack.wrapping_sub(c.snd_una) as usize;
            if confirmados > 0 {
                let de_datos = confirmados.min(c.tx_len);
                if confirmados > c.tx_len {
                    c.fin_confirmado = true;
                }
                c.tx.copy_within(de_datos..c.tx_len, 0);
                c.tx_len -= de_datos;
                c.snd_una = s.ack;
                c.reintentos = 0;
                c.rto = RTO_INICIAL_MS;
                c.vence = if c.en_vuelo() > 0 { Some(ahora + c.rto) } else { None };
                let mss = c.mss();
                c.cwnd = if c.cwnd < c.ssthresh {
                    c.cwnd.saturating_add((de_datos as u32).min(mss))
                } else {
                    c.cwnd.saturating_add((mss * mss / c.cwnd).max(1))
                };
                if c.fin_confirmado {
                    match c.estado {
                        Estado::FinEspera1 => c.estado = Estado::FinEspera2,
                        Estado::Cerrando => {
                            c.estado = Estado::TiempoEspera;
                            c.desde = ahora;
                        }
                        Estado::UltimoAck => {
                            c.cerrar_con(Cierre::Normal);
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
            c.snd_wnd = s.ventana as u32;
        }

        let fin = fl & FIN != 0;
        if antes(c.rcv_nxt, s.sec) {
            // Fuera de orden: se descarta y se pide lo que falta.
            if !s.datos.is_empty() || fin {
                c.ack_debido = true;
            }
            return Ok(());
        }
        let atras = c.rcv_nxt.wrapping_sub(s.sec) as usize;
        if atras > s.datos.len() {
            if !s.datos.is_empty() || fin {
                c.ack_debido = true;
            }
            return Ok(());
        }
        let datos = &s.datos[atras..];
        let recibe = matches!(c.estado, Estado::Establecida | Estado::FinEspera1 | Estado::FinEspera2);
        let mut tomados = 0;
        if !datos.is_empty() {
            if recibe {
                tomados = datos.len().min(BUFER - c.rx_len);
                c.rx[c.rx_len..c.rx_len + tomados].copy_from_slice(&datos[..tomados]);
                c.rx_len += tomados;
                c.rcv_nxt = c.rcv_nxt.wrapping_add(tomados as u32);
            }
            c.ack_debido = true;
        }
        if fin && recibe && tomados == datos.len() {
            c.rcv_nxt = c.rcv_nxt.wrapping_add(1);
            c.ack_debido = true;
            match c.estado {
                Estado::Establecida => c.estado = Estado::CierreEspera,
                Estado::FinEspera1 if c.fin_confirmado => {
                    c.estado = Estado::TiempoEspera;
                    c.desde = ahora;
                }
                Estado::FinEspera1 => c.estado = Estado::Cerrando,
                Estado::FinEspera2 => {
                    c.estado = Estado::TiempoEspera;
                    c.desde = ahora;
                    c.vence = None;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// **El siguiente segmento que hay que mandar**, si alguno: `(mi ip, su ip,
    /// largo)` con el segmento escrito en `dst`. Quien llama lo envuelve en
    /// IPv4 y repite hasta que conteste `None`.
    pub fn salida(&mut self, ahora: u64, dst: &mut [u8]) -> Option<(Ip, Ip, usize)> {
        if let Some(r) = self.rst.take() {
            let n = segmento::escribir(dst, r.mi_ip, r.su_ip, r.mi_puerto, r.su_puerto, r.sec, r.ack, RST | ACK, 0, None, &[]).ok()?;
            return Some((r.mi_ip, r.su_ip, n));
        }
        for k in 0..CONEXIONES {
            let i = (self.turno + k) % CONEXIONES;
            if let Some(n) = self.emitir(i, ahora, dst) {
                self.turno = (i + 1) % CONEXIONES;
                return Some((self.c[i].mi_ip, self.c[i].su_ip, n));
            }
        }
        None
    }

    fn emitir(&mut self, i: usize, ahora: u64, dst: &mut [u8]) -> Option<usize> {
        let c = &mut self.c[i];
        if !c.viva() {
            return None;
        }
        if c.estado == Estado::TiempoEspera && ahora.saturating_sub(c.desde) >= TIEMPO_ESPERA_MS {
            c.cerrar_con(Cierre::Normal);
            return None;
        }
        // La huerfana que nadie recoge: no se queda con el hueco.
        if c.huerfana && c.escucha.is_some() && c.estado == Estado::Establecida
            && ahora.saturating_sub(c.desde) >= TIEMPO_ESPERA_MS
        {
            let r = Rst { mi_ip: c.mi_ip, su_ip: c.su_ip, mi_puerto: c.mi_puerto, su_puerto: c.su_puerto, sec: c.snd_nxt, ack: c.rcv_nxt };
            c.cerrar_con(Cierre::Abortada);
            self.rst = Some(r);
            return None;
        }
        if let Some(v) = c.vence {
            if ahora >= v {
                if c.reintentos >= REINTENTOS {
                    c.cerrar_con(Cierre::Agotada);
                    return None;
                }
                c.reintentos += 1;
                c.rto = (c.rto * 2).min(RTO_MAX_MS);
                let vuelo = c.en_vuelo();
                c.ssthresh = (vuelo / 2).max(2 * c.mss());
                c.cwnd = c.mss();
                c.vence = None;
                c.sondear = vuelo == 0;
                // Vuelta atras: se reenvia desde lo primero sin confirmar.
                c.snd_nxt = c.snd_una;
            }
        }

        let ventana = c.ventana_rx();
        let (mi_ip, su_ip, mp, sp) = (c.mi_ip, c.su_ip, c.mi_puerto, c.su_puerto);
        match c.estado {
            Estado::SynEnviado => {
                if c.snd_nxt != c.isn {
                    return None;
                }
                let n = segmento::escribir(dst, mi_ip, su_ip, mp, sp, c.isn, 0, SYN, ventana, Some(MSS), &[]).ok()?;
                c.snd_nxt = c.isn.wrapping_add(1);
                c.armar(ahora);
                return Some(n);
            }
            Estado::SynRecibido if c.snd_nxt == c.isn => {
                let n = segmento::escribir(dst, mi_ip, su_ip, mp, sp, c.isn, c.rcv_nxt, SYN | ACK, ventana, Some(MSS), &[]).ok()?;
                c.snd_nxt = c.isn.wrapping_add(1);
                c.ack_debido = false;
                c.armar(ahora);
                return Some(n);
            }
            _ => {}
        }

        let manda = matches!(
            c.estado,
            Estado::Establecida | Estado::CierreEspera | Estado::FinEspera1 | Estado::Cerrando | Estado::UltimoAck
        );
        let vuelo = c.en_vuelo() as usize;
        let enviados = vuelo.min(c.tx_len);
        let pendientes = c.tx_len - enviados;
        if manda && pendientes > 0 {
            let ventana_su = c.snd_wnd.min(c.cwnd) as usize;
            let mut cabe = ventana_su.saturating_sub(vuelo);
            if c.sondear {
                cabe = cabe.max(1);
            }
            let n = pendientes
                .min(cabe)
                .min(c.mss() as usize)
                .min(dst.len().saturating_sub(segmento::CABECERA));
            if n > 0 {
                let k = segmento::escribir(
                    dst, mi_ip, su_ip, mp, sp, c.snd_nxt, c.rcv_nxt, ACK | PSH, ventana, None,
                    &c.tx[enviados..enviados + n],
                )
                .ok()?;
                c.snd_nxt = c.snd_nxt.wrapping_add(n as u32);
                c.sondear = false;
                c.ack_debido = false;
                c.armar(ahora);
                return Some(k);
            }
            // Ventana cerrada y nada en vuelo: el temporizador hace de sonda.
            if vuelo == 0 {
                c.armar(ahora);
            }
        }

        let fin_listo = c.fin_pedido && !c.fin_confirmado && c.en_vuelo() as usize == c.tx_len;
        if manda && fin_listo {
            let k = segmento::escribir(dst, mi_ip, su_ip, mp, sp, c.snd_nxt, c.rcv_nxt, FIN | ACK, ventana, None, &[]).ok()?;
            c.snd_nxt = c.snd_nxt.wrapping_add(1);
            c.ack_debido = false;
            c.armar(ahora);
            c.estado = match c.estado {
                Estado::Establecida => Estado::FinEspera1,
                Estado::CierreEspera => Estado::UltimoAck,
                e => e,
            };
            return Some(k);
        }

        if c.ack_debido {
            let k = segmento::escribir(dst, mi_ip, su_ip, mp, sp, c.snd_nxt, c.rcv_nxt, ACK, ventana, None, &[]).ok()?;
            c.ack_debido = false;
            return Some(k);
        }
        None
    }
}
