//! Las pruebas de TCP: dos pilas hablando por un cable de mentira.
//!
//! El cable es una funcion: cada segmento pasa o se pierde segun lo que diga
//! `perder`, y el tiempo lo pone la prueba. Por eso todas son reproducibles.

use super::*;
use crate::pon16;
use crate::suma;

const CLI: Ip = [10, 0, 0, 2];
const SRV: Ip = [10, 0, 0, 1];

/// Lleva segmentos de una pila a la otra hasta que las dos se callan.
fn bombear(a: &mut Tcp, b: &mut Tcp, ahora: u64, mut perder: impl FnMut(u32, &[u8]) -> bool) -> u32 {
    let mut buf = [0u8; 1600];
    let mut cuenta = 0u32;
    loop {
        let mut algo = false;
        while let Some((o, d, n)) = a.salida(ahora, &mut buf) {
            algo = true;
            cuenta += 1;
            if !perder(cuenta, &buf[..n]) {
                let _ = b.entrada(o, d, &buf[..n], ahora);
            }
        }
        while let Some((o, d, n)) = b.salida(ahora, &mut buf) {
            algo = true;
            cuenta += 1;
            if !perder(cuenta, &buf[..n]) {
                let _ = a.entrada(o, d, &buf[..n], ahora);
            }
        }
        if !algo {
            return cuenta;
        }
        assert!(cuenta < 200_000, "las pilas no se callan");
    }
}

/// Cliente y servidor ya conectados: `(cliente, servidor, asa_c, asa_s)`.
fn par() -> (Box<Tcp>, Box<Tcp>, usize, usize) {
    let mut s = Box::new(Tcp::nueva([7; 32]));
    let mut c = Box::new(Tcp::nueva([9; 32]));
    let e = s.escuchar(SRV, 80).unwrap();
    let k = c.conectar(CLI, SRV, 80, 0).unwrap();
    bombear(&mut c, &mut s, 0, |_, _| false);
    let a = s.aceptar(e).expect("el servidor acepta");
    (c, s, k, a)
}

/// Un segmento hecho a mano del servidor al cliente.
fn a_mano(c: &Tcp, k: usize, sec: u32, ack: u32, banderas: u8) -> Vec<u8> {
    let (_, cp, _, sp) = c.extremos(k).unwrap();
    let mut b = vec![0u8; 20];
    segmento::escribir(&mut b, SRV, CLI, sp, cp, sec, ack, banderas, 1000, None, &[]).unwrap();
    b
}

#[test]
fn tres_pasos() {
    let (c, s, k, a) = par();
    assert_eq!(c.estado(k), Estado::Establecida);
    assert_eq!(s.estado(a), Estado::Establecida);
}

#[test]
fn los_datos_llegan_enteros_y_en_orden() {
    let (mut c, mut s, k, a) = par();
    let datos: Vec<u8> = (0..50_000u32).map(|i| (i * 7 + i / 300) as u8).collect();
    let (mut puestos, mut llegados) = (0, Vec::new());
    let mut buf = [0u8; 3000];
    for vuelta in 0..10_000u64 {
        if puestos < datos.len() {
            puestos += c.enviar(k, &datos[puestos..]).unwrap();
        }
        bombear(&mut c, &mut s, vuelta, |_, _| false);
        let n = s.recibir(a, &mut buf).unwrap();
        llegados.extend_from_slice(&buf[..n]);
        if llegados.len() == datos.len() {
            break;
        }
    }
    assert!(llegados == datos, "llegaron {} de {}", llegados.len(), datos.len());
}

#[test]
fn cierre_ordenado_y_tiempo_de_espera() {
    let (mut c, mut s, k, a) = par();
    c.cerrar(k).unwrap();
    bombear(&mut c, &mut s, 10, |_, _| false);
    assert_eq!(s.estado(a), Estado::CierreEspera);
    assert_eq!(c.estado(k), Estado::FinEspera2);
    s.cerrar(a).unwrap();
    bombear(&mut c, &mut s, 20, |_, _| false);
    assert_eq!(s.estado(a), Estado::Cerrada(Cierre::Normal));
    assert_eq!(c.estado(k), Estado::TiempoEspera);
    bombear(&mut c, &mut s, 20 + TIEMPO_ESPERA_MS, |_, _| false);
    assert_eq!(c.estado(k), Estado::Cerrada(Cierre::Normal));
    c.soltar(k).unwrap();
    assert_eq!(c.estado(k), Estado::Libre);
}

#[test]
fn una_perdida_se_recupera_al_segundo_exacto() {
    let (mut c, mut s, k, a) = par();
    c.enviar(k, b"hola").unwrap();
    bombear(&mut c, &mut s, 100, |n, _| n == 1);
    let mut buf = [0u8; 16];
    assert_eq!(s.recibir(a, &mut buf).unwrap(), 0, "se perdio");
    bombear(&mut c, &mut s, 100 + RTO_INICIAL_MS - 1, |_, _| false);
    assert_eq!(s.recibir(a, &mut buf).unwrap(), 0, "un milisegundo antes, nada");
    bombear(&mut c, &mut s, 100 + RTO_INICIAL_MS, |_, _| false);
    let n = s.recibir(a, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"hola");
}

#[test]
fn el_syn_se_rinde_con_tiempos_fijos() {
    let mut c = Tcp::nueva([1; 32]);
    let k = c.conectar(CLI, SRV, 80, 0).unwrap();
    let mut buf = [0u8; 64];
    let mut envios = Vec::new();
    for t in (0..=100_000u64).step_by(500) {
        while c.salida(t, &mut buf).is_some() {
            envios.push(t);
        }
    }
    assert_eq!(envios, [0, 1_000, 3_000, 7_000, 15_000, 31_000, 63_000]);
    assert_eq!(c.estado(k), Estado::Cerrada(Cierre::Agotada));
}

#[test]
fn un_rst_que_no_es_exacto_no_corta() {
    let (mut c, _s, k, _a) = par();
    let (_, rcv) = c.secuencias(k).unwrap();
    let mut buf = [0u8; 64];
    let rst = a_mano(&c, k, rcv.wrapping_add(100), 0, RST);
    assert_eq!(c.entrada(SRV, CLI, &rst, 50), Ok(()));
    assert_eq!(c.estado(k), Estado::Establecida, "RFC 5961: no corta");
    let (_, _, n) = c.salida(50, &mut buf).expect("ACK de reto");
    assert_eq!(segmento::leer(&buf[..n], CLI, SRV).unwrap().banderas, ACK);
    let rst = a_mano(&c, k, rcv, 0, RST);
    c.entrada(SRV, CLI, &rst, 60).unwrap();
    assert_eq!(c.estado(k), Estado::Cerrada(Cierre::Reset));
}

#[test]
fn un_syn_en_una_conexion_viva_da_reto() {
    let (mut c, _s, k, _a) = par();
    let (_, rcv) = c.secuencias(k).unwrap();
    let syn = a_mano(&c, k, rcv.wrapping_add(5), 0, SYN);
    c.entrada(SRV, CLI, &syn, 50).unwrap();
    assert_eq!(c.estado(k), Estado::Establecida);
    let mut buf = [0u8; 64];
    assert!(c.salida(50, &mut buf).is_some(), "contesta con un ACK, no se reinicia");
}

#[test]
fn un_ack_del_futuro_no_se_cree() {
    let (mut c, _s, k, _a) = par();
    c.enviar(k, b"abc").unwrap();
    let mut buf = [0u8; 64];
    c.salida(10, &mut buf).unwrap();
    let (snd, rcv) = c.secuencias(k).unwrap();
    let ack = a_mano(&c, k, rcv, snd.wrapping_add(1000), ACK);
    assert_eq!(c.entrada(SRV, CLI, &ack, 20), Err(Rechazo::Secuencia));
    assert_eq!(c.pendientes(k).1, 3, "lo enviado sigue sin confirmar");
}

#[test]
fn un_puerto_que_no_escucha_calla() {
    let mut s = Tcp::nueva([7; 32]);
    let mut b = [0u8; 24];
    segmento::escribir(&mut b, CLI, SRV, 40000, 81, 1, 0, SYN, 1000, Some(1460), &[]).unwrap();
    assert_eq!(s.entrada(CLI, SRV, &b, 0), Err(Rechazo::NoEsParaMi));
    assert_eq!(s.salida(0, &mut [0u8; 64]), None, "ni RST");
}

#[test]
fn una_inundacion_de_syn_llena_y_la_viva_sigue() {
    let (mut c, mut s, k, a) = par();
    let mut llenos = 0;
    for p in 0..50u16 {
        let mut b = [0u8; 24];
        let falso: Ip = [10, 9, (p >> 8) as u8, p as u8];
        let n = segmento::escribir(&mut b, falso, SRV, 1000 + p, 80, p as u32, 0, SYN, 1000, None, &[]).unwrap();
        if s.entrada(falso, SRV, &b[..n], 5) == Err(Rechazo::Lleno) {
            llenos += 1;
        }
    }
    assert!(llenos > 0);
    c.enviar(k, b"sigo aqui").unwrap();
    bombear(&mut c, &mut s, 6, |_, _| false);
    let mut buf = [0u8; 32];
    let n = s.recibir(a, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"sigo aqui");
}

#[test]
fn el_mismo_secreto_da_el_mismo_syn_y_otro_no() {
    let syn = |secreto: [u8; 32]| {
        let mut t = Tcp::nueva(secreto);
        t.conectar(CLI, SRV, 443, 1234).unwrap();
        let mut b = [0u8; 64];
        let (_, _, n) = t.salida(1234, &mut b).unwrap();
        b[..n].to_vec()
    };
    assert_eq!(syn([3; 32]), syn([3; 32]));
    assert_ne!(syn([3; 32]), syn([4; 32]));
}

/// ** Ningun segmento hace panico ni rompe las cuentas: veinte mil mutaciones
/// de segmentos buenos, con la suma rehecha para que lleguen hasta el fondo.
#[test]
fn veinte_mil_mutaciones_no_revientan() {
    let (mut c, mut s, k, a) = par();
    c.enviar(k, b"datos para mutar").unwrap();
    let mut buf = [0u8; 1600];
    let (o, d, n) = c.salida(10, &mut buf).unwrap();
    let base = buf[..n].to_vec();
    let mut semilla = 0x5EED_u64;
    let mut azar = || {
        semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (semilla >> 33) as usize
    };
    for vuelta in 0..20_000u64 {
        let mut m = base.clone();
        for _ in 0..1 + azar() % 4 {
            let i = azar() % m.len();
            m[i] = azar() as u8;
        }
        m.truncate(1 + azar() % m.len());
        if m.len() >= 18 {
            pon16(&mut m, 16, 0);
            let mut sm = suma::pseudo(o, d, ipv4::TCP, m.len() as u16);
            sm.bytes(&m);
            let v = sm.cerrar();
            pon16(&mut m, 16, v);
        }
        let _ = s.entrada(o, d, &m, 10 + vuelta);
        let _ = s.salida(10 + vuelta, &mut buf);
        let (rx, tx) = s.pendientes(a);
        assert!(rx <= BUFER && tx <= BUFER);
    }
}

/// *** POR EL CABLE ENTERO (G5, 2026-09-18): dos NODOS con su pila TCP,
/// hablando por tramas Ethernet de verdad -- lo que `red hola` hace sobre el
/// buzon, sin el buzon. Cada segmento sale por `Tcp::salida`, se envuelve con
/// `Nodo::envolver` (Ethernet + IPv4) y entra por `Nodo::atender`, que lo
/// demultiplexa como `Hecho::Tcp` y lo entrega a `Tcp::entrada`. Si alguna
/// de las cuatro costuras se equivocara de byte, los tres pasos no cerrarian.
#[test]
fn el_saludo_entero_por_tramas_ethernet_y_cierre_limpio() {
    use crate::nodo::{Hecho, Nodo, CARGA};
    const MC: [u8; 6] = [0x02, 0, 0, 0, 0, 0x0C];
    const MS: [u8; 6] = [0x02, 0, 0, 0, 0, 0x05];
    let mut nc = Nodo::nuevo(MC, CLI);
    let mut ns = Nodo::nuevo(MS, SRV);
    let mut c = Box::new(Tcp::nueva([3; 32]));
    let mut s = Box::new(Tcp::nueva([5; 32]));
    let e = s.escuchar(SRV, 7117).unwrap();
    let k = c.conectar(CLI, SRV, 7117, 0).unwrap();

    // Un lado manda todo lo que su pila quiera, envuelto; el otro lo atiende.
    fn cruza(de: &mut Tcp, nodo_de: &mut Nodo, mac_a: [u8; 6], a: &mut Tcp, nodo_a: &mut Nodo, ahora: u64) -> bool {
        let mut algo = false;
        let mut buf = [0u8; 1514];
        let mut resp = [0u8; 1514];
        while let Some((_, su_ip, n)) = de.salida(ahora, &mut buf[CARGA..]) {
            algo = true;
            let total = nodo_de.envolver(&mut buf, mac_a, su_ip, crate::ipv4::TCP, n).unwrap();
            match nodo_a.atender(&buf[..total], ahora, &mut resp).unwrap() {
                Hecho::Tcp { origen, destino, segmento } => a.entrada(origen, destino, segmento, ahora).unwrap(),
                otro => panic!("el nodo no lo vio como TCP: {:?}", otro),
            }
        }
        algo
    }
    let mut ahora = 0u64;
    let mut vueltas = 0;
    loop {
        let a = cruza(&mut c, &mut nc, MS, &mut s, &mut ns, ahora);
        let b = cruza(&mut s, &mut ns, MC, &mut c, &mut nc, ahora);
        if !a && !b {
            break;
        }
        vueltas += 1;
        ahora += 16;
        assert!(vueltas < 100, "no se callan");
    }
    let a = s.aceptar(e).expect("el servidor acepta");
    assert_eq!(c.estado(k), Estado::Establecida);
    assert_eq!(s.estado(a), Estado::Establecida);
    // SYN y ACK al servidor, SYN+ACK al cliente: tres, y los tres por el nodo.
    assert_eq!(nc.cuentas.tcp + ns.cuentas.tcp, 3, "los tres pasos pasaron por los nodos");

    // HOLA, y la respuesta de vuelta, como la antena.
    c.enviar(k, b"HOLA ANTENA/1\n").unwrap();
    for _ in 0..10 {
        cruza(&mut c, &mut nc, MS, &mut s, &mut ns, ahora);
        cruza(&mut s, &mut ns, MC, &mut c, &mut nc, ahora);
        ahora += 16;
    }
    let mut buf = [0u8; 64];
    let n = s.recibir(a, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"HOLA ANTENA/1\n");
    s.enviar(a, b"HOLA ANTENA/1 honor\n").unwrap();
    for _ in 0..10 {
        cruza(&mut s, &mut ns, MC, &mut c, &mut nc, ahora);
        cruza(&mut c, &mut nc, MS, &mut s, &mut ns, ahora);
        ahora += 16;
    }
    let n = c.recibir(k, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"HOLA ANTENA/1 honor\n");

    // El cierre limpio, desde el cliente, como `red hola`.
    c.cerrar(k).unwrap();
    for _ in 0..10 {
        cruza(&mut c, &mut nc, MS, &mut s, &mut ns, ahora);
        cruza(&mut s, &mut ns, MC, &mut c, &mut nc, ahora);
        ahora += 16;
    }
    assert_eq!(s.estado(a), Estado::CierreEspera);
    s.cerrar(a).unwrap();
    for _ in 0..10 {
        cruza(&mut s, &mut ns, MC, &mut c, &mut nc, ahora);
        cruza(&mut c, &mut nc, MS, &mut s, &mut ns, ahora);
        ahora += 16;
    }
    assert_eq!(s.estado(a), Estado::Cerrada(Cierre::Normal));
    assert_eq!(c.estado(k), Estado::TiempoEspera, "los dos FIN cruzaron: TIME-WAIT");
}
