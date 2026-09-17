//! **HOSTILE FRAMES** -- every entry point of the stack that reads what a
//! stranger put on the cable, fed garbage and mutations of good frames.
//!
//! Checked: nothing panics. Not checked: that the answer is right -- the unit
//! tests next to each parser do that. See `bmo-hostile` for why the difference
//! matters.
//!
//! ** Every mutated frame is tried TWICE: as it came, and with its IPv4 and
//! TCP/UDP checksums recomputed. Without the second pass the mutations die at
//! the checksum and the code behind it is never reached -- which is exactly the
//! code a real attacker reaches, because a real attacker computes checksums.

use bmo_hostile::{attack, DEFAULT_SEED};
use bmo_pila::ether::{self, Mac};
use bmo_pila::ipv4::{self, Ip};
use bmo_pila::tcp::segmento::{self, bandera};
use bmo_pila::{arp, dhcp, dns, icmp, nodo, suma, tcp, udp};

const CASES: u32 = 20_000;

const ME: Mac = [0x02, 0, 0, 0, 0, 1];
const ROUTER: Mac = [0x02, 0, 0, 0, 0, 2];
const MY_IP: Ip = [10, 0, 0, 2];
const PEER: Ip = [10, 0, 0, 1];
const XID: u32 = 0x1234_5678;
const DNS_ID: u16 = 0xB0B0;

fn be16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

/// Ethernet + IPv4 around `l4`.
fn frame(proto: u8, src: Ip, dst: Ip, l4: &[u8]) -> Vec<u8> {
    let mut t = vec![0u8; ether::CABECERA + ipv4::CABECERA + l4.len()];
    ether::escribir(&mut t, ME, ROUTER, ether::TIPO_IPV4).unwrap();
    ipv4::escribir(&mut t[ether::CABECERA..], src, dst, proto, l4.len(), 7).unwrap();
    t[ether::CABECERA + ipv4::CABECERA..].copy_from_slice(l4);
    t
}

fn udp_frame(src_port: u16, dst_port: u16, data: &[u8]) -> Vec<u8> {
    let mut l4 = vec![0u8; udp::CABECERA + data.len()];
    udp::escribir(&mut l4, PEER, MY_IP, src_port, dst_port, data).unwrap();
    frame(ipv4::UDP, PEER, MY_IP, &l4)
}

fn tcp_segment(flags: u8, seq: u32, ack: u32, mss: Option<u16>, data: &[u8]) -> Vec<u8> {
    let mut l4 = vec![0u8; segmento::CABECERA + 4 + data.len()];
    let n = segmento::escribir(&mut l4, PEER, MY_IP, 40000, 80, seq, ack, flags, 4096, mss, data).unwrap();
    l4.truncate(n);
    l4
}

fn icmp_frame() -> Vec<u8> {
    let mut l4 = vec![0u8; icmp::CABECERA + 16];
    let n = icmp::escribir(&mut l4, icmp::ECO_PETICION, 9, 1, &[0x55; 16]).unwrap();
    l4.truncate(n);
    frame(ipv4::ICMP, PEER, MY_IP, &l4)
}

fn arp_frame() -> Vec<u8> {
    let mut t = vec![0u8; ether::CABECERA + arp::LARGO];
    ether::escribir(&mut t, ether::DIFUSION, ROUTER, ether::TIPO_ARP).unwrap();
    let a = arp::Arp { op: arp::Op::Pregunta, mac_origen: ROUTER, ip_origen: PEER, mac_destino: [0; 6], ip_destino: MY_IP };
    arp::escribir(&mut t[ether::CABECERA..], &a).unwrap();
    t
}

fn dhcp_offer() -> Vec<u8> {
    // BOOTP fixed part is 236 bytes, then the magic cookie and the options.
    let mut b = vec![0u8; 240];
    b[0] = 2;
    b[1] = 1;
    b[2] = 6;
    b[4..8].copy_from_slice(&XID.to_be_bytes());
    b[16..20].copy_from_slice(&MY_IP);
    b[28..34].copy_from_slice(&ME);
    b[236..240].copy_from_slice(&0x6382_5363u32.to_be_bytes());
    b.extend_from_slice(&[53, 1, 2, 54, 4, 10, 0, 0, 1, 1, 4, 255, 255, 255, 0]);
    b.extend_from_slice(&[3, 4, 10, 0, 0, 1, 6, 4, 10, 0, 0, 1, 51, 4, 0, 0, 14, 16, 255]);
    let mut t = vec![0u8; ether::CABECERA + ipv4::CABECERA + udp::CABECERA + b.len()];
    ether::escribir(&mut t, ME, ROUTER, ether::TIPO_IPV4).unwrap();
    ipv4::escribir(&mut t[14..], PEER, [255; 4], ipv4::UDP, udp::CABECERA + b.len(), 7).unwrap();
    udp::escribir(&mut t[34..], PEER, [255; 4], dhcp::PUERTO_SERVIDOR, dhcp::PUERTO_CLIENTE, &b).unwrap();
    t
}

fn dns_answer() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&be16(DNS_ID));
    b.extend_from_slice(&be16(0x8180));
    b.extend_from_slice(&[0, 1, 0, 2, 0, 0, 0, 0]);
    for label in ["example", "com"] {
        b.push(label.len() as u8);
        b.extend_from_slice(label.as_bytes());
    }
    b.push(0);
    b.extend_from_slice(&[0, 1, 0, 1]);
    // A CNAME back to the question (a pointer loop candidate) and an A record.
    b.extend_from_slice(&[0xC0, 12, 0, 5, 0, 1, 0, 0, 1, 0, 0, 2, 0xC0, 12]);
    b.extend_from_slice(&[0xC0, 12, 0, 1, 0, 1, 0, 0, 1, 44, 0, 4, 93, 184, 216, 34]);
    b
}

/// Recomputes the IPv4 header checksum and the TCP/UDP checksum of an Ethernet
/// frame, as far as the (possibly mutated) lengths allow. Never panics: a
/// frame too broken to fix is left as it is.
fn fix_checksums(t: &mut [u8]) {
    if t.len() < ether::CABECERA + ipv4::CABECERA || t[12..14] != be16(ether::TIPO_IPV4) {
        return;
    }
    let ip = &mut t[ether::CABECERA..];
    let ihl = (ip[0] & 0x0F) as usize * 4;
    if ihl < ipv4::CABECERA || ihl > ip.len() {
        return;
    }
    ip[10] = 0;
    ip[11] = 0;
    let s = suma::de(&ip[..ihl]);
    ip[10..12].copy_from_slice(&be16(s));
    let total = (u16::from_be_bytes([ip[2], ip[3]]) as usize).clamp(ihl, ip.len());
    let (src, dst) = ([ip[12], ip[13], ip[14], ip[15]], [ip[16], ip[17], ip[18], ip[19]]);
    let proto = ip[9];
    fix_l4(proto, src, dst, &mut ip[ihl..total]);
}

fn fix_l4(proto: u8, src: Ip, dst: Ip, l4: &mut [u8]) {
    let at = match proto {
        ipv4::UDP if l4.len() >= udp::CABECERA => 6,
        ipv4::TCP if l4.len() >= segmento::CABECERA => 16,
        _ => return,
    };
    if l4.len() > u16::MAX as usize {
        return;
    }
    l4[at] = 0;
    l4[at + 1] = 0;
    let mut s = suma::pseudo(src, dst, proto, l4.len() as u16);
    s.bytes(l4);
    let mut v = s.cerrar();
    if proto == ipv4::UDP && v == 0 {
        v = 0xFFFF;
    }
    l4[at..at + 2].copy_from_slice(&be16(v));
}

/// Attacks `target` with the raw case and with its checksums fixed.
fn attack_frames<F: FnMut(&[u8])>(name: &str, seed: u64, samples: &[&[u8]], mut target: F) {
    attack(name, seed, CASES, samples, 1600, |b| {
        target(b);
        let mut fixed = b.to_vec();
        fix_checksums(&mut fixed);
        target(&fixed);
    });
}

#[test]
fn each_parser_survives_hostile_frames() {
    let (a, i, d, n, u) = (arp_frame(), icmp_frame(), dhcp_offer(), udp_frame(53, 5353, &dns_answer()), udp_frame(9, 9, b"hola"));
    let tcp_frame = frame(ipv4::TCP, PEER, MY_IP, &tcp_segment(bandera::SYN, 1000, 0, Some(1460), b""));
    let samples: &[&[u8]] = &[&a, &i, &d, &n, &u, &tcp_frame];

    attack_frames("ether::leer", DEFAULT_SEED, samples, |b| {
        let _ = ether::leer(b);
    });
    attack_frames("arp::leer (after ethernet)", DEFAULT_SEED ^ 1, samples, |b| {
        if b.len() > ether::CABECERA {
            let _ = arp::leer(&b[ether::CABECERA..]);
        }
    });
    attack_frames("ipv4 -> icmp/udp/tcp", DEFAULT_SEED ^ 2, samples, |b| {
        let Ok(t) = ether::leer(b) else { return };
        let Ok(p) = ipv4::leer(t.carga) else { return };
        let _ = icmp::leer(p.carga);
        let _ = udp::leer(p.carga, p.origen, p.destino);
        let _ = segmento::leer(p.carga, p.origen, p.destino);
    });
    attack_frames("dhcp::leer", DEFAULT_SEED ^ 3, samples, |b| {
        let _ = dhcp::leer(b, ME, XID);
    });
}

#[test]
fn dns_survives_hostile_answers() {
    let good = dns_answer();
    attack("dns::leer", DEFAULT_SEED ^ 4, CASES * 2, &[&good], 600, |b| {
        let _ = dns::leer(b, DNS_ID, b"example.com");
    });
}

#[test]
fn the_node_survives_hostile_frames() {
    let (a, i, u) = (arp_frame(), icmp_frame(), udp_frame(9, 9, b"hola"));
    let t = frame(ipv4::TCP, PEER, MY_IP, &tcp_segment(bandera::SYN, 1000, 0, Some(1460), b""));
    let mut node = nodo::Nodo::nuevo(ME, MY_IP);
    let mut out = [0u8; 1600];
    let mut now = 0u64;
    attack_frames("Nodo::atender", DEFAULT_SEED ^ 5, &[&a, &i, &u, &t], |b| {
        now += 7;
        let _ = node.atender(b, now, &mut out);
    });
}

#[test]
fn the_dhcp_client_survives_hostile_frames() {
    let offer = dhcp_offer();
    let mut client = dhcp::Cliente::nuevo(ME, XID);
    client.empezar();
    let mut out = [0u8; 1600];
    let mut now = 0u64;
    attack_frames("dhcp::Cliente::oir", DEFAULT_SEED ^ 6, &[&offer], |b| {
        now += 50;
        let _ = client.latir(now, &mut out);
        let _ = client.oir(b, now);
        if !client.en_marcha() {
            client.empezar();
        }
    });
}

/// Asks the stack for what it wants to send, with a bound: a machine that
/// always has something to say is a finding, not a hang.
fn drain(stack: &mut tcp::Tcp, now: u64, out: &mut [u8]) {
    for _ in 0..64 {
        if stack.salida(now, out).is_none() {
            return;
        }
    }
}

/// ** TCP is a state machine, so one segment is not enough: the hostile
/// segment lands on a connection that a good SYN already opened, and the
/// machine is then asked to talk.
#[test]
fn tcp_survives_hostile_segments_on_a_live_connection() {
    let syn = tcp_segment(bandera::SYN, 1000, 0, Some(1460), b"");
    let ack = tcp_segment(bandera::ACK | bandera::PSH, 1001, 0, None, b"GET / HTTP/1.0\r\n\r\n");
    let fin = tcp_segment(bandera::FIN | bandera::ACK, 1019, 0, None, b"");
    let rst = tcp_segment(bandera::RST, 1001, 0, None, b"");
    let mut out = [0u8; 1600];
    let mut rx = [0u8; 512];
    attack("Tcp::entrada", DEFAULT_SEED ^ 7, CASES, &[&syn, &ack, &fin, &rst], 1500, |b| {
        let mut stack = tcp::Tcp::nueva([7; 32]);
        let Ok(listener) = stack.escuchar(MY_IP, 80) else { return };
        let mut now = 1u64;
        let _ = stack.entrada(PEER, MY_IP, &syn, now);
        drain(&mut stack, now, &mut out);
        let conn = stack.aceptar(listener);
        for pass in 0..2 {
            let mut seg = b.to_vec();
            if pass == 1 {
                fix_l4(ipv4::TCP, PEER, MY_IP, &mut seg);
            }
            now += 250;
            let _ = stack.entrada(PEER, MY_IP, &seg, now);
            drain(&mut stack, now, &mut out);
            if let Some(c) = conn {
                let _ = stack.recibir(c, &mut rx);
                let _ = stack.enviar(c, b"ok");
            }
        }
        // Let every timer fire: retransmission and close paths run on hostile state.
        for _ in 0..8 {
            now += 40_000;
            drain(&mut stack, now, &mut out);
        }
    });
}

/// ** The guard of this file: if a sample is not a GOOD frame, every mutation
/// dies at the first check and the tests above pass without reaching anything.
/// A hostile test that attacks the front door only is a green light that lies.
#[test]
fn the_samples_reach_the_code_behind_the_checks() {
    let t = arp_frame();
    assert!(arp::leer(&ether::leer(&t).unwrap().carga).is_ok(), "arp");
    let t = icmp_frame();
    let p = ipv4::leer(ether::leer(&t).unwrap().carga).unwrap();
    assert!(icmp::leer(p.carga).is_ok(), "icmp");
    let t = frame(ipv4::TCP, PEER, MY_IP, &tcp_segment(bandera::SYN, 1000, 0, Some(1460), b""));
    let p = ipv4::leer(ether::leer(&t).unwrap().carga).unwrap();
    assert!(segmento::leer(p.carga, p.origen, p.destino).is_ok(), "tcp");
    assert!(dhcp::leer(&dhcp_offer(), ME, XID).is_ok(), "dhcp: {:?}", dhcp::leer(&dhcp_offer(), ME, XID));
    assert!(dns::leer(&dns_answer(), DNS_ID, b"example.com").is_ok(), "dns: {:?}", dns::leer(&dns_answer(), DNS_ID, b"example.com"));
}
