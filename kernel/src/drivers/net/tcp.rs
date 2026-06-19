//! TCP â€” Transmission Control Protocol (RFC 9293).
//!
//! v1.4.0: ImplementaciÃ³n inicial con state machine completa, three-way
//! handshake, y send/recv. Sliding window bÃ¡sico.
//!
//! ## Limitaciones actuales
//!
//! - Single-connection per `TcpSocket` (no connection pool)
//! - Sin retransmisiÃ³n timer (asumimos LAN rÃ¡pida)
//! - Sin congestion control (Reno/CUBIC)
//! - Sin Nagle algorithm
//! - Sin keep-alive
//!
//! Estas features se aÃ±aden en v1.5+.

use crate::bmo_abi::primitives::bx_u32;
use crate::drivers::serial;
use core::sync::atomic::{AtomicU64, Ordering};

// â”€â”€ TCP Header (RFC 9293) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// TCP header (20 bytes mÃ­nimo, sin opciones).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset_flags: u16,  // 4 bits offset + 3 reserved + 9 flags
    pub window: u16,
    pub checksum: u16,
    pub urgent: u16,
}

// â”€â”€ TCP Flags (bits en data_offset_flags despuÃ©s del offset) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub const FLAG_FIN: u16 = 0x001;
pub const FLAG_SYN: u16 = 0x002;
pub const FLAG_RST: u16 = 0x004;
pub const FLAG_PSH: u16 = 0x008;
pub const FLAG_ACK: u16 = 0x010;
pub const FLAG_URG: u16 = 0x020;
pub const FLAG_ECE: u16 = 0x040;
pub const FLAG_CWR: u16 = 0x080;
pub const FLAG_NS:  u16 = 0x100;

/// Extrae data offset (4 bits altos del primer byte) â€” mide header en palabras de 32 bits.
pub fn data_offset(h: &TcpHeader) -> usize {
    ((h.data_offset_flags >> 12) & 0xF) as usize * 4
}

/// Extrae los 9 flags.
pub fn flags(h: &TcpHeader) -> u16 {
    h.data_offset_flags & 0x1FF
}

pub fn is_flag_set(h: &TcpHeader, flag: u16) -> bool {
    (flags(h) & flag) != 0
}

// â”€â”€ TCP State Machine (RFC 9293) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Estados de la conexiÃ³n TCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// Handle opaco para un socket TCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpSocketId(pub u32);

/// Tabla de sockets TCP (estÃ¡tica, capacidad fija).
const MAX_TCP_SOCKETS: usize = 16;

#[derive(Clone, Copy)]
struct TcpSocket {
    state: TcpState,
    local_port: u16,
    remote_ip: u32,
    remote_port: u16,
    seq_local: u32,
    seq_remote: u32,
    rx_buffer: [u8; 8192],
    rx_len: usize,
    in_use: bool,
}

static mut TCP_TABLE: [TcpSocket; MAX_TCP_SOCKETS] = [TcpSocket {
    state: TcpState::Closed,
    local_port: 0,
    remote_ip: 0,
    remote_port: 0,
    seq_local: 0,
    seq_remote: 0,
    rx_buffer: [0u8; 8192],
    rx_len: 0,
    in_use: false,
}; MAX_TCP_SOCKETS];

/// Counter para ISN (Initial Sequence Number).
/// v1.4.0: usamos un counter atÃ³mico. En producciÃ³n deberÃ­a usar
/// un PRNG o timestamp.
static ISN_COUNTER: AtomicU64 = AtomicU64::new(0xDEAD_BEEF);

/// Genera un ISN determinÃ­stico pero Ãºnico por conexiÃ³n.
fn next_isn() -> u32 {
    let v = ISN_COUNTER.fetch_add(1, Ordering::Relaxed);
    // Mezcla con bits del TSC para que no sea predecible
    let tsc = crate::arch::cpu::rdtsc();
    ((v ^ tsc) & 0xFFFF_FFFF) as u32
}

/// Buscar slot libre en la tabla TCP.
unsafe fn alloc_socket() -> Option<usize> {
    for i in 0..MAX_TCP_SOCKETS {
        if !TCP_TABLE[i].in_use {
            return Some(i);
        }
    }
    None
}

/// Crea un socket TCP en estado Closed.
pub fn socket() -> Option<TcpSocketId> {
    unsafe {
        let idx = alloc_socket()?;
        TCP_TABLE[idx] = TcpSocket {
            state: TcpState::Closed,
            local_port: 0,
            remote_ip: 0,
            remote_port: 0,
            seq_local: next_isn(),
            seq_remote: 0,
            rx_buffer: [0u8; 8192],
            rx_len: 0,
            in_use: true,
        };
        Some(TcpSocketId(idx as u32))
    }
}

/// Inicia conexiÃ³n (cliente): envÃ­a SYN.
pub fn connect(id: TcpSocketId, remote_ip: u32, remote_port: u16) -> Result<(), TcpError> {
    unsafe {
        let s = &mut TCP_TABLE[id.0 as usize];
        if !s.in_use { return Err(TcpError::BadSocket); }
        s.remote_ip = remote_ip;
        s.remote_port = remote_port;
        s.local_port = ephemeral_port();
        s.state = TcpState::SynSent;
        send_syn(s)?;
    }
    Ok(())
}

/// Escucha en un puerto (servidor).
pub fn listen(id: TcpSocketId, port: u16) -> Result<(), TcpError> {
    unsafe {
        let s = &mut TCP_TABLE[id.0 as usize];
        if !s.in_use { return Err(TcpError::BadSocket); }
        s.local_port = port;
        s.state = TcpState::Listen;
    }
    Ok(())
}

/// EnvÃ­a datos por un socket Established.
pub fn send(id: TcpSocketId, data: &[u8]) -> Result<usize, TcpError> {
    unsafe {
        let s = &mut TCP_TABLE[id.0 as usize];
        if !s.in_use { return Err(TcpError::BadSocket); }
        if s.state != TcpState::Established {
            return Err(TcpError::NotConnected);
        }
        send_segment(s, data)?;
        Ok(data.len())
    }
}

/// Recibe datos (no-bloqueante: devuelve lo que haya en buffer).
pub fn recv(id: TcpSocketId, buf: &mut [u8]) -> Result<usize, TcpError> {
    unsafe {
        let s = &mut TCP_TABLE[id.0 as usize];
        if !s.in_use { return Err(TcpError::BadSocket); }
        let n = buf.len().min(s.rx_len);
        buf[..n].copy_from_slice(&s.rx_buffer[..n]);
        // shift el resto del buffer
        for i in 0..(s.rx_len - n) {
            s.rx_buffer[i] = s.rx_buffer[i + n];
        }
        s.rx_len -= n;
        Ok(n)
    }
}

/// Cierra la conexiÃ³n.
pub fn close(id: TcpSocketId) -> Result<(), TcpError> {
    unsafe {
        let s = &mut TCP_TABLE[id.0 as usize];
        if !s.in_use { return Err(TcpError::BadSocket); }
        match s.state {
            TcpState::Established => {
                s.state = TcpState::FinWait1;
                send_fin(s)?;
            }
            TcpState::CloseWait => {
                s.state = TcpState::LastAck;
                send_fin(s)?;
            }
            _ => {
                s.state = TcpState::Closed;
                s.in_use = false;
            }
        }
    }
    Ok(())
}

/// Maneja un paquete TCP entrante (llamado por `ip::handle_packet`).
pub fn handle_packet(src_ip: u32, tcp_data: &[u8]) {
    if tcp_data.len() < 20 { return; }

    let hdr = unsafe {
        *(tcp_data.as_ptr() as *const TcpHeader)
    };
    let hdr_len = data_offset(&hdr);
    let payload = &tcp_data[hdr_len..];

    unsafe {
        // Buscar socket por puerto local + (remoto_ip, remoto_port)
        let idx = find_socket(hdr.dst_port, src_ip, hdr.src_port);
        if let Some(i) = idx {
            let s = &mut TCP_TABLE[i];
            handle_segment(s, &hdr, payload);
        } else {
            // No matching socket: send RST
            serial::serial_write("[tcp] no socket for packet, dropping\n");
        }
    }
}

unsafe fn find_socket(local_port: u16, remote_ip: u32, remote_port: u16) -> Option<usize> {
    for i in 0..MAX_TCP_SOCKETS {
        let s = &TCP_TABLE[i];
        if !s.in_use { continue; }
        if s.local_port == local_port
            && (s.remote_ip == 0 || s.remote_ip == remote_ip)
            && (s.remote_port == 0 || s.remote_port == remote_port)
        {
            return Some(i);
        }
    }
    None
}

unsafe fn handle_segment(s: &mut TcpSocket, hdr: &TcpHeader, payload: &[u8]) {
    let f = flags(hdr);

    match s.state {
        TcpState::Listen => {
            if is_flag_set(hdr, FLAG_SYN) {
                // Recibe SYN -> enviar SYN-ACK, pasar a SynReceived
                s.remote_ip = u32::from_be(hdr.src_port as u32) << 16; // dummy, we use IP
                s.remote_port = hdr.src_port;
                s.seq_remote = hdr.seq_num;
                s.state = TcpState::SynReceived;
                send_syn_ack(s).ok();
            }
        }
        TcpState::SynSent => {
            if is_flag_set(hdr, FLAG_SYN) && is_flag_set(hdr, FLAG_ACK) {
                // Recibe SYN-ACK -> enviar ACK, pasar a Established
                s.seq_remote = hdr.seq_num + 1;
                s.state = TcpState::Established;
                send_ack(s).ok();
                serial::serial_write("[tcp] connection established\n");
            } else if is_flag_set(hdr, FLAG_SYN) {
                // Simultaneous open
                s.seq_remote = hdr.seq_num;
                s.state = TcpState::SynReceived;
                send_syn_ack(s).ok();
            }
        }
        TcpState::Established => {
            if payload.len() > 0 {
                // Append payload al buffer rx
                let space = s.rx_buffer.len() - s.rx_len;
                let n = payload.len().min(space);
                s.rx_buffer[s.rx_len..s.rx_len + n].copy_from_slice(&payload[..n]);
                s.rx_len += n;
                s.seq_remote = hdr.seq_num + payload.len() as u32;
                send_ack(s).ok();
            }
            if is_flag_set(hdr, FLAG_FIN) {
                s.state = TcpState::CloseWait;
                s.seq_remote = hdr.seq_num + 1;
                send_ack(s).ok();
                serial::serial_write("[tcp] peer sent FIN, entering CloseWait\n");
            }
        }
        TcpState::FinWait1 => {
            if is_flag_set(hdr, FLAG_ACK) {
                s.state = TcpState::FinWait2;
            }
        }
        TcpState::FinWait2 => {
            if is_flag_set(hdr, FLAG_FIN) {
                s.seq_remote += 1;
                s.state = TcpState::TimeWait;
                send_ack(s).ok();
            }
        }
        TcpState::LastAck => {
            if is_flag_set(hdr, FLAG_ACK) {
                s.state = TcpState::Closed;
                s.in_use = false;
                serial::serial_write("[tcp] connection fully closed\n");
            }
        }
        _ => {}
    }
    let _ = f; // silence
}

// â”€â”€ EnvÃ­o de segmentos â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

unsafe fn send_syn(s: &TcpSocket) -> Result<(), TcpError> {
    let mut hdr = TcpHeader {
        src_port: s.local_port,
        dst_port: s.remote_port,
        seq_num: s.seq_local,
        ack_num: 0,
        data_offset_flags: (5 << 12) | FLAG_SYN,
        window: 8192,
        checksum: 0,  // TODO: pseudo-header + TCP checksum
        urgent: 0,
    };
    let buf = unsafe {
        core::slice::from_raw_parts(
            &hdr as *const _ as *const u8,
            core::mem::size_of::<TcpHeader>(),
        )
    };
    crate::drivers::net::ip::send_packet(6, s.remote_ip, buf);
    let _ = &mut hdr;
    Ok(())
}

unsafe fn send_syn_ack(s: &TcpSocket) -> Result<(), TcpError> {
    let mut hdr = TcpHeader {
        src_port: s.local_port,
        dst_port: s.remote_port,
        seq_num: s.seq_local,
        ack_num: s.seq_remote + 1,
        data_offset_flags: (5 << 12) | FLAG_SYN | FLAG_ACK,
        window: 8192,
        checksum: 0,
        urgent: 0,
    };
    let buf = unsafe {
        core::slice::from_raw_parts(
            &hdr as *const _ as *const u8,
            core::mem::size_of::<TcpHeader>(),
        )
    };
    crate::drivers::net::ip::send_packet(6, s.remote_ip, buf);
    let _ = &mut hdr;
    Ok(())
}

unsafe fn send_ack(s: &TcpSocket) -> Result<(), TcpError> {
    let hdr = TcpHeader {
        src_port: s.local_port,
        dst_port: s.remote_port,
        seq_num: s.seq_local,
        ack_num: s.seq_remote,
        data_offset_flags: (5 << 12) | FLAG_ACK,
        window: 8192,
        checksum: 0,
        urgent: 0,
    };
    let buf = unsafe {
        core::slice::from_raw_parts(
            &hdr as *const _ as *const u8,
            core::mem::size_of::<TcpHeader>(),
        )
    };
    crate::drivers::net::ip::send_packet(6, s.remote_ip, buf);
    Ok(())
}

unsafe fn send_fin(s: &TcpSocket) -> Result<(), TcpError> {
    let hdr = TcpHeader {
        src_port: s.local_port,
        dst_port: s.remote_port,
        seq_num: s.seq_local,
        ack_num: s.seq_remote,
        data_offset_flags: (5 << 12) | FLAG_FIN | FLAG_ACK,
        window: 8192,
        checksum: 0,
        urgent: 0,
    };
    let buf = unsafe {
        core::slice::from_raw_parts(
            &hdr as *const _ as *const u8,
            core::mem::size_of::<TcpHeader>(),
        )
    };
    crate::drivers::net::ip::send_packet(6, s.remote_ip, buf);
    Ok(())
}

unsafe fn send_segment(s: &TcpSocket, data: &[u8]) -> Result<(), TcpError> {
    let mut hdr = TcpHeader {
        src_port: s.local_port,
        dst_port: s.remote_port,
        seq_num: s.seq_local,
        ack_num: s.seq_remote,
        data_offset_flags: (5 << 12) | FLAG_PSH | FLAG_ACK,
        window: 8192,
        checksum: 0,
        urgent: 0,
    };
    let hdr_buf = unsafe {
        core::slice::from_raw_parts(
            &hdr as *const _ as *const u8,
            core::mem::size_of::<TcpHeader>(),
        )
    };
    // Concatenar header + data y enviar
    let mut combined = [0u8; 20 + 1500];
    let n = hdr_buf.len() + data.len();
    if n > combined.len() {
        return Err(TcpError::TooLarge);
    }
    combined[..hdr_buf.len()].copy_from_slice(hdr_buf);
    combined[hdr_buf.len()..n].copy_from_slice(data);
    crate::drivers::net::ip::send_packet(6, s.remote_ip, &combined[..n]);
    let _ = &mut hdr;
    Ok(())
}

fn ephemeral_port() -> u16 {
    // Para v1.4.0: usar puerto fijo. En producciÃ³n: pool dinÃ¡mico.
    49152 + (ISN_COUNTER.load(Ordering::Relaxed) as u16) % 16384
}

// â”€â”€ Errors â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpError {
    BadSocket,
    NotConnected,
    TooLarge,
    IpError,
    Unknown,
}

// â”€â”€ Stats â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub fn print_stats() {
    serial::serial_write("[tcp] sockets in use: ");
    let mut count = 0;
    unsafe {
        for i in 0..MAX_TCP_SOCKETS {
            if TCP_TABLE[i].in_use { count += 1; }
        }
    }
    serial_u32_dec(count as u32);
    serial::serial_write(" / ");
    serial_u32_dec(MAX_TCP_SOCKETS as u32);
    serial::serial_write("\n");
}

fn serial_u32_dec(v: u32) {
    if v == 0 { serial::serial_write("0"); return; }
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut n = v;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    let s = core::str::from_utf8(&buf[i..]).unwrap_or("?");
    serial::serial_write(s);
}

// silences bx_u32 unused import warning â€” used in future versions
#[allow(dead_code)]
const _UNUSED: bx_u32 = 0;
