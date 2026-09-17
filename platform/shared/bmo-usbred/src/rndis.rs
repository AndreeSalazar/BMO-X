//! **RNDIS**: los mensajes que el driver escribe y lee (Microsoft [MS-RNDIS]).
//!
//! [carril]  AMARILLO  lee lo que manda el movil
//! [cuesta]  DATO      un desplazamiento mal leido saca bytes de fuera de la trama
//! [riesgo]  AJENO     cada largo y cada desplazamiento los escribe el movil
//!
//! # Lo que hace falta, y nada mas
//!
//! ```text
//!    control (por el endpoint 0, SEND/GET_ENCAPSULATED)
//!       INITIALIZE      ->  INITIALIZE_CMPLT   status, tamanos
//!       QUERY MAC       ->  QUERY_CMPLT        la MAC que el movil nos da
//!       SET FILTRO      ->  SET_CMPLT          para empezar a recibir
//!    datos (por los endpoints BULK)
//!       PACKET_MSG = cabecera de 44 bytes + la trama Ethernet
//! ```
//!
//! # Lista blanca, como `bmo-pila`
//!
//! Todo largo y todo desplazamiento se comprueba contra el bufer ANTES de mirar
//! lo que apunta. `DataOffset` se cuenta desde el propio campo (byte 8), no desde
//! el principio: es la trampa clasica de RNDIS, y aqui se cuenta bien una vez.

/// Tipos de mensaje.
pub mod tipo {
    pub const PACKET: u32 = 0x0000_0001;
    pub const INITIALIZE: u32 = 0x0000_0002;
    pub const INITIALIZE_CMPLT: u32 = 0x8000_0002;
    pub const QUERY: u32 = 0x0000_0004;
    pub const QUERY_CMPLT: u32 = 0x8000_0004;
    pub const SET: u32 = 0x0000_0005;
    pub const SET_CMPLT: u32 = 0x8000_0005;
    pub const KEEPALIVE_CMPLT: u32 = 0x8000_0008;
}

/// Los OID que se usan.
pub mod oid {
    /// La MAC que el movil asigna a su lado USB.
    pub const MAC_PERMANENTE: u32 = 0x0101_0101;
    /// Que tramas se quieren recibir.
    pub const FILTRO: u32 = 0x0001_010E;
}

/// El filtro: dirigidas a nosotros, multidifusion y difusion.
pub const FILTRO_NORMAL: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0008;
/// `RNDIS_STATUS_SUCCESS`.
pub const EXITO: u32 = 0;
/// La cabecera de un `PACKET_MSG`.
pub const CABECERA_PAQUETE: usize = 44;
/// La trama Ethernet mas larga que se envuelve, sin FCS.
pub const TRAMA_MAX: usize = 1514;
/// Lo que se le dice al movil que cabe en una transferencia.
pub const TRANSFERENCIA_MAX: u32 = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rechazo {
    Corto,
    Largo,
    Tipo,
    Desplazamiento,
    Pedido,
    Estado(u32),
    Medio,
}

impl Rechazo {
    pub fn texto(self) -> &'static str {
        match self {
            Rechazo::Corto => "faltan bytes",
            Rechazo::Largo => "mas grande de lo admitido",
            Rechazo::Tipo => "un mensaje que no es el esperado",
            Rechazo::Desplazamiento => "un desplazamiento que apunta fuera del mensaje",
            Rechazo::Pedido => "la respuesta es de otro pedido",
            Rechazo::Estado(_) => "el movil contesto con error",
            Rechazo::Medio => "no es Ethernet (802.3)",
        }
    }
}

fn le32(b: &[u8], i: usize) -> Result<u32, Rechazo> {
    let s = b.get(i..i + 4).ok_or(Rechazo::Corto)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn pon32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}

/// Comprueba el tipo y que `MessageLength` quepa en lo recibido. Devuelve el largo.
fn cabecera(b: &[u8], esperado: u32) -> Result<usize, Rechazo> {
    if le32(b, 0)? != esperado {
        return Err(Rechazo::Tipo);
    }
    let largo = le32(b, 4)? as usize;
    if largo < 8 || largo > b.len() {
        return Err(Rechazo::Corto);
    }
    Ok(largo)
}

/// `INITIALIZE_MSG`: RNDIS 1.0 y el tamano de transferencia.
pub fn inicializar(dst: &mut [u8], pedido: u32) -> Result<usize, Rechazo> {
    if dst.len() < 24 {
        return Err(Rechazo::Corto);
    }
    for (i, v) in [tipo::INITIALIZE, 24, pedido, 1, 0, TRANSFERENCIA_MAX].iter().enumerate() {
        pon32(dst, i * 4, *v);
    }
    Ok(24)
}

/// Lo que importa de `INITIALIZE_CMPLT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inicio {
    pub paquetes_por_transferencia: u32,
    pub transferencia_max: u32,
}

pub fn leer_inicio(b: &[u8], pedido: u32) -> Result<Inicio, Rechazo> {
    let largo = cabecera(b, tipo::INITIALIZE_CMPLT)?;
    if largo < 52 {
        return Err(Rechazo::Corto);
    }
    if le32(b, 8)? != pedido {
        return Err(Rechazo::Pedido);
    }
    let estado = le32(b, 12)?;
    if estado != EXITO {
        return Err(Rechazo::Estado(estado));
    }
    // Medium 0 = 802.3. Cualquier otro no es una trama Ethernet.
    if le32(b, 28)? != 0 {
        return Err(Rechazo::Medio);
    }
    Ok(Inicio { paquetes_por_transferencia: le32(b, 32)?, transferencia_max: le32(b, 36)? })
}

/// `QUERY_MSG` de un OID, sin datos de entrada.
pub fn preguntar(dst: &mut [u8], pedido: u32, oid: u32) -> Result<usize, Rechazo> {
    if dst.len() < 28 {
        return Err(Rechazo::Corto);
    }
    for (i, v) in [tipo::QUERY, 28, pedido, oid, 0, 20, 0].iter().enumerate() {
        pon32(dst, i * 4, *v);
    }
    Ok(28)
}

/// La respuesta a una pregunta: los bytes de informacion, dentro del mensaje.
pub fn leer_respuesta(b: &[u8], pedido: u32) -> Result<&[u8], Rechazo> {
    let largo = cabecera(b, tipo::QUERY_CMPLT)?;
    if le32(b, 8)? != pedido {
        return Err(Rechazo::Pedido);
    }
    let estado = le32(b, 12)?;
    if estado != EXITO {
        return Err(Rechazo::Estado(estado));
    }
    let n = le32(b, 16)? as usize;
    // ** El desplazamiento se cuenta desde RequestId, que esta en el byte 8.
    let desde = (le32(b, 20)? as usize).checked_add(8).ok_or(Rechazo::Desplazamiento)?;
    let hasta = desde.checked_add(n).ok_or(Rechazo::Desplazamiento)?;
    if n > 0 && (desde < 24 || hasta > largo) {
        return Err(Rechazo::Desplazamiento);
    }
    Ok(&b[desde.min(largo)..hasta.min(largo)])
}

/// La MAC de una respuesta a `oid::MAC_PERMANENTE`.
pub fn mac(info: &[u8]) -> Result<[u8; 6], Rechazo> {
    let s = info.get(..6).ok_or(Rechazo::Corto)?;
    if info.len() != 6 || s[0] & 1 != 0 || s == [0; 6] {
        return Err(Rechazo::Desplazamiento);
    }
    Ok([s[0], s[1], s[2], s[3], s[4], s[5]])
}

/// `SET_MSG` de un OID con un valor de 32 bits (el filtro).
pub fn poner(dst: &mut [u8], pedido: u32, oid: u32, valor: u32) -> Result<usize, Rechazo> {
    if dst.len() < 32 {
        return Err(Rechazo::Corto);
    }
    for (i, v) in [tipo::SET, 32, pedido, oid, 4, 20, 0, valor].iter().enumerate() {
        pon32(dst, i * 4, *v);
    }
    Ok(32)
}

pub fn leer_puesto(b: &[u8], pedido: u32) -> Result<(), Rechazo> {
    cabecera(b, tipo::SET_CMPLT)?;
    if le32(b, 8)? != pedido {
        return Err(Rechazo::Pedido);
    }
    match le32(b, 12)? {
        EXITO => Ok(()),
        e => Err(Rechazo::Estado(e)),
    }
}

/// **Envuelve una trama Ethernet** en un `PACKET_MSG`.
pub fn envolver(dst: &mut [u8], trama: &[u8]) -> Result<usize, Rechazo> {
    if trama.len() > TRAMA_MAX {
        return Err(Rechazo::Largo);
    }
    let total = CABECERA_PAQUETE + trama.len();
    if dst.len() < total {
        return Err(Rechazo::Corto);
    }
    dst[..CABECERA_PAQUETE].fill(0);
    pon32(dst, 0, tipo::PACKET);
    pon32(dst, 4, total as u32);
    // DataOffset: desde el propio campo (byte 8) hasta los datos (byte 44).
    pon32(dst, 8, (CABECERA_PAQUETE - 8) as u32);
    pon32(dst, 12, trama.len() as u32);
    dst[CABECERA_PAQUETE..total].copy_from_slice(trama);
    Ok(total)
}

/// **Saca las tramas** de una transferencia BULK IN. El movil puede meter varias
/// seguidas: se llama en bucle con lo que quede. Devuelve `(trama, consumido)`.
pub fn desenvolver(b: &[u8]) -> Result<(&[u8], usize), Rechazo> {
    let largo = cabecera(b, tipo::PACKET)?;
    if largo < CABECERA_PAQUETE {
        return Err(Rechazo::Corto);
    }
    let desde = (le32(b, 8)? as usize).checked_add(8).ok_or(Rechazo::Desplazamiento)?;
    let n = le32(b, 12)? as usize;
    let hasta = desde.checked_add(n).ok_or(Rechazo::Desplazamiento)?;
    if desde < CABECERA_PAQUETE || hasta > largo {
        return Err(Rechazo::Desplazamiento);
    }
    if n > TRAMA_MAX {
        return Err(Rechazo::Largo);
    }
    Ok((&b[desde..hasta], largo))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn inicio_cmplt(pedido: u32, estado: u32, medio: u32) -> Vec<u8> {
        let mut b = vec![0u8; 52];
        for (i, v) in [tipo::INITIALIZE_CMPLT, 52, pedido, estado, 1, 0, 0, medio, 1, 16384, 0, 0, 0].iter().enumerate() {
            pon32(&mut b, i * 4, *v);
        }
        b
    }

    #[test]
    fn el_saludo() {
        let mut b = [0u8; 64];
        assert_eq!(inicializar(&mut b, 7), Ok(24));
        assert_eq!(le32(&b, 0), Ok(tipo::INITIALIZE));
        assert_eq!(le32(&b, 12), Ok(1), "RNDIS 1.0");
        assert_eq!(leer_inicio(&inicio_cmplt(7, 0, 0), 7), Ok(Inicio { paquetes_por_transferencia: 1, transferencia_max: 16384 }));
        assert_eq!(leer_inicio(&inicio_cmplt(8, 0, 0), 7), Err(Rechazo::Pedido));
        assert_eq!(leer_inicio(&inicio_cmplt(7, 0xC000_0001, 0), 7), Err(Rechazo::Estado(0xC000_0001)));
        assert_eq!(leer_inicio(&inicio_cmplt(7, 0, 5), 7), Err(Rechazo::Medio));
        assert_eq!(leer_inicio(&inicio_cmplt(7, 0, 0)[..40], 7), Err(Rechazo::Corto));
    }

    fn query_cmplt(pedido: u32, info: &[u8], desplazamiento: u32) -> Vec<u8> {
        let mut b = vec![0u8; 24];
        let total = 24 + info.len();
        for (i, v) in [tipo::QUERY_CMPLT, total as u32, pedido, 0, info.len() as u32, desplazamiento].iter().enumerate() {
            pon32(&mut b, i * 4, *v);
        }
        b.extend_from_slice(info);
        b
    }

    #[test]
    fn la_mac_del_movil() {
        let mut b = [0u8; 64];
        assert_eq!(preguntar(&mut b, 3, oid::MAC_PERMANENTE), Ok(28));
        let m = [0x02, 0x11, 0x22, 0x33, 0x44, 0x55];
        let r = query_cmplt(3, &m, 16);
        assert_eq!(leer_respuesta(&r, 3).and_then(mac), Ok(m));
        assert_eq!(leer_respuesta(&query_cmplt(3, &m, 200), 3), Err(Rechazo::Desplazamiento), "fuera");
        assert_eq!(leer_respuesta(&query_cmplt(3, &m, 0), 3), Err(Rechazo::Desplazamiento), "dentro de la cabecera");
        assert_eq!(mac(&[0x01, 0, 0, 0, 0, 1]), Err(Rechazo::Desplazamiento), "una MAC de grupo no es de nadie");
    }

    #[test]
    fn el_filtro() {
        let mut b = [0u8; 64];
        assert_eq!(poner(&mut b, 9, oid::FILTRO, FILTRO_NORMAL), Ok(32));
        assert_eq!(le32(&b, 28), Ok(FILTRO_NORMAL));
        let mut ok = vec![0u8; 16];
        for (i, v) in [tipo::SET_CMPLT, 16, 9, 0].iter().enumerate() {
            pon32(&mut ok, i * 4, *v);
        }
        assert_eq!(leer_puesto(&ok, 9), Ok(()));
        pon32(&mut ok, 12, 0xC000_00BB);
        assert_eq!(leer_puesto(&ok, 9), Err(Rechazo::Estado(0xC000_00BB)));
    }

    #[test]
    fn una_trama_va_y_vuelve() {
        let trama: Vec<u8> = (0..60u8).collect();
        let mut b = [0u8; 128];
        let n = envolver(&mut b, &trama).unwrap();
        assert_eq!(n, 104);
        assert_eq!(le32(&b, 8), Ok(36), "DataOffset desde el propio campo");
        assert_eq!(desenvolver(&b[..n]), Ok((&trama[..], 104)));
    }

    #[test]
    fn dos_tramas_en_una_transferencia() {
        let mut b = [0u8; 256];
        let n1 = envolver(&mut b, &[0xAA; 60]).unwrap();
        let n2 = envolver(&mut b[n1..], &[0xBB; 70]).unwrap();
        let (t1, c1) = desenvolver(&b[..n1 + n2]).unwrap();
        let (t2, c2) = desenvolver(&b[c1..n1 + n2]).unwrap();
        assert_eq!((t1.len(), t2.len(), c1 + c2), (60, 70, n1 + n2));
    }

    #[test]
    fn desplazamientos_que_mienten() {
        let mut b = [0u8; 128];
        let n = envolver(&mut b, &[1; 60]).unwrap();
        let mut malo = b;
        pon32(&mut malo, 12, 500);
        assert_eq!(desenvolver(&malo[..n]), Err(Rechazo::Desplazamiento), "largo mas alla del mensaje");
        let mut malo = b;
        pon32(&mut malo, 8, 0);
        assert_eq!(desenvolver(&malo[..n]), Err(Rechazo::Desplazamiento), "datos dentro de la cabecera");
        let mut malo = b;
        pon32(&mut malo, 8, u32::MAX);
        assert_eq!(desenvolver(&malo[..n]), Err(Rechazo::Desplazamiento));
        let mut malo = b;
        pon32(&mut malo, 4, 1000);
        assert_eq!(desenvolver(&malo[..n]), Err(Rechazo::Corto), "MessageLength mayor que lo recibido");
        assert_eq!(envolver(&mut [0u8; 2000], &[0; 1515]), Err(Rechazo::Largo));
    }

    #[test]
    fn veinte_mil_transferencias_mutadas_no_revientan() {
        let mut base = [0u8; 256];
        let n = envolver(&mut base, &[7; 100]).unwrap();
        let mut semilla = 0x4D15u64;
        let mut azar = || {
            semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semilla >> 33) as usize
        };
        for _ in 0..20_000 {
            let mut m = base[..n].to_vec();
            for _ in 0..1 + azar() % 6 {
                let i = azar() % m.len();
                m[i] = azar() as u8;
            }
            m.truncate(azar() % (m.len() + 1));
            let _ = desenvolver(&m);
            let _ = leer_respuesta(&m, 1);
            let _ = leer_inicio(&m, 1);
        }
    }

    /// ** HOSTILE PASS (2026-09-17): the phone on the other end of the cable
    /// writes every byte these read. Mutations of the good answers, with their
    /// length and offset fields broken on purpose. Checked: nothing panics.
    #[test]
    fn hostile_answers_never_panic() {
        let m = [0x02, 0x11, 0x22, 0x33, 0x44, 0x55];
        let (a, b, c) = (inicio_cmplt(7, 0, 0), query_cmplt(7, &m, 16), query_cmplt(7, &[0u8; 64], 20));
        bmo_hostile::attack("rndis", bmo_hostile::DEFAULT_SEED, 30_000, &[&a, &b, &c], 512, |x| {
            let _ = leer_inicio(x, 7);
            let _ = leer_puesto(x, 7);
            if let Ok(info) = leer_respuesta(x, 7) {
                let _ = mac(info);
            }
            let _ = mac(x);
        });
    }
}
