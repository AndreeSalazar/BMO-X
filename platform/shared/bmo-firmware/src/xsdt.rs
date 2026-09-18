//! **EL RECORRIDO DEL XSDT** -- que tablas ofrece el firmware, y cuantos bytes
//! de cada una se pueden leer sin creerse lo que no se debe.
//!
//! ## Por que existe (2026-09-17)
//!
//! Hasta este dia el recorrido vivia DENTRO DEL KERNEL (`ring0/plat/placa.rs`)
//! y estaba escrito DOS VECES, en `censar` y en `tabla_de`. Las dos copias
//! tenian el mismo hueco y una diferencia:
//!
//! ```text
//!    el hueco        el largo de cada tabla lo escribe el firmware -- o, si un
//!                    puntero del XSDT apunta a otra cosa, la basura -- y se
//!                    leian ESOS bytes, sin tope, para hacer la suma. Un largo
//!                    de 0xFFFF_FFFF pedia 4 GiB de memoria fisica en el arranque
//!    la diferencia   ante una entrada ilegible, `censar` seguia y `tabla_de`
//!                    ABANDONABA la busqueda: una sola entrada mala antes del
//!                    MCFG o del IVRS y `ecam()`/`iommu()` contestaban CERO
//! ```
//!
//! La diferencia hoy no se alcanza --`Cabecera::leer` solo falla con menos de
//! 36 bytes y los dos sitios le daban 36-- pero era una trampa armada: el dia
//! que `leer` comprobara cualquier cosa, el kernel diria "no hay IOMMU" en
//! silencio. Ahora hay UN recorrido, y ante una entrada ilegible SIGUE.
//!
//! ## Lo que este crate NO puede decir
//!
//! El tope acota el LARGO. Que la DIRECCION este mapeada no lo sabe nadie aqui:
//! eso es del mapa de memoria del kernel. Un puntero del XSDT fuera del mapa
//! sigue siendo un fallo de pagina en el arranque, y no se tapa con esto.

use crate::{revisar, Cabecera, CABECERA_LEN};

/// Cuantas entradas del XSDT se miran como mucho.
///
/// ** Un tope y no un `Vec`: esto corre en el arranque, sin monton. Un XSDT con
/// mas de 64 tablas no es una placa generosa, es un largo que miente.
pub const MAX_ENTRADAS: usize = 64;

/// **El largo maximo de una tabla que se LEE para sumarla.** 1 MiB.
///
/// [!] SUPOSICION DECLARADA, no un numero de esta placa: el perfil de la placa
/// todavia no registra el tamano de sus tablas (LEY 24: una estimacion generica
/// es una estimacion de OTRO proyecto). Se sostiene por dos motivos:
///
/// * las tablas que BMO-X USA --MCFG, IVRS, MADT-- miden bytes, no megas;
/// * las que no usa --DSDT y SSDT, que son AML y aqui no se ejecutan-- son las
///   grandes, y si una pasara del tope saldria en el censo como NO CREIBLE, que
///   se ve, en vez de pedirle al arranque que lea lo que diga un numero.
///
/// ** Como se sabe si esta bien: `confesar` ya imprime el largo de cada tabla en
/// CABINA al arrancar. El mayor de esos numeros va a `PERFIL/PLACA`, y este
/// tope se compara con el.
pub const MAX_TABLA: usize = 1 << 20;

/// Bytes del RSDP de ACPI 2.0+ que hacen falta para llegar al XSDT.
pub const RSDP_LEN: usize = 36;

fn u64_de(b: &[u8]) -> Option<u64> {
    let b = b.get(..8)?;
    Some(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

/// **Donde esta el XSDT**, segun el RSDP. `None` si el RSDP no lleva a uno.
///
/// Se exige revision 2 o mas: el RSDT de 32 bits es de ACPI 1.0, y una maquina
/// que arranca por UEFI trae XSDT.
pub fn xsdt_del_rsdp(rsdp: &[u8]) -> Option<u64> {
    if rsdp.get(0..8)? != b"RSD PTR " {
        return None;
    }
    if *rsdp.get(15)? < 2 {
        return None;
    }
    match u64_de(rsdp.get(24..)?)? {
        0 => None,
        x => Some(x),
    }
}

/// Por que no se pudo recorrer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoXsdt {
    /// El XSDT esta donde dice y su cabecera no se lee.
    CabeceraMala,
    /// Declara un largo MENOR que su propia cabecera. Se para antes de restar:
    /// esa resta en `usize` da la vuelta y saldria un recorrido enorme.
    LargoImposible,
}

/// Una tabla que ofrece el firmware.
#[derive(Clone, Copy, Debug)]
pub struct Tabla<'a> {
    pub fisica: u64,
    pub cabecera: Cabecera,
    /// **La tabla entera, solo si se puede creer**: largo posible (entre la
    /// cabecera y [`MAX_TABLA`]) y suma que cuadra. `None` es NO CREIBLE, y una
    /// tabla no creible no es una tabla: es memoria que se leyo.
    pub bytes: Option<&'a [u8]>,
}

/// **Recorre el XSDT que esta en `xsdt`.**
///
/// `leer(fisica, n)` da `n` bytes de memoria fisica --o menos, si no hay tantos:
/// eso se toma como "no se lee"--. `cada` recibe cada tabla y devuelve si
/// seguir; asi quien busca UNA puede parar en ella.
///
/// Devuelve la cabecera del propio XSDT, que es de donde sale el fabricante.
pub fn recorrer<'a>(
    xsdt: u64,
    leer: impl Fn(u64, usize) -> &'a [u8],
    mut cada: impl FnMut(Tabla<'a>) -> bool,
) -> Result<Cabecera, NoXsdt> {
    let cab = Cabecera::leer(leer(xsdt, CABECERA_LEN)).ok_or(NoXsdt::CabeceraMala)?;
    if (cab.largo as usize) < CABECERA_LEN {
        return Err(NoXsdt::LargoImposible);
    }
    let n = ((cab.largo as usize - CABECERA_LEN) / 8).min(MAX_ENTRADAS);
    for i in 0..n {
        let Some(donde) = xsdt.checked_add((CABECERA_LEN + i * 8) as u64) else {
            break;
        };
        let Some(t) = u64_de(leer(donde, 8)) else {
            continue;
        };
        if t == 0 {
            continue;
        }
        // *** ILEGIBLE = SE SIGUE. Era la diferencia entre las dos copias.
        let Some(c) = Cabecera::leer(leer(t, CABECERA_LEN)) else {
            continue;
        };
        let largo = c.largo as usize;
        // *** EL TOPE, antes de pedir un solo byte de mas.
        let bytes = if (CABECERA_LEN..=MAX_TABLA).contains(&largo) {
            let todo = leer(t, largo);
            revisar(todo).ok().map(|_| todo)
        } else {
            None
        };
        if !cada(Tabla { fisica: t, cabecera: c, bytes }) {
            break;
        }
    }
    Ok(cab)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;

    /// Memoria fisica de mentira: `bytes` a partir de `base`. Apunta el mayor
    /// `n` que se le haya pedido, que es lo que dice si se respeto el tope.
    struct Mem {
        base: u64,
        bytes: Vec<u8>,
        mayor: Cell<usize>,
    }

    impl Mem {
        fn nueva(base: u64, largo: usize) -> Mem {
            Mem { base, bytes: vec![0; largo], mayor: Cell::new(0) }
        }
        fn leer(&self, fis: u64, n: usize) -> &[u8] {
            if n > self.mayor.get() {
                self.mayor.set(n);
            }
            let Some(off) = fis.checked_sub(self.base) else { return &[] };
            let off = off as usize;
            if off >= self.bytes.len() {
                return &[];
            }
            let fin = off.saturating_add(n).min(self.bytes.len());
            &self.bytes[off..fin]
        }
        fn poner(&mut self, fis: u64, datos: &[u8]) {
            let off = (fis - self.base) as usize;
            self.bytes[off..off + datos.len()].copy_from_slice(datos);
        }
    }

    const BASE: u64 = 0x7F00_0000;

    /// Una tabla ACPI valida: cabecera con su largo y la suma cuadrada.
    fn tabla(firma: &[u8; 4], cuerpo: usize) -> Vec<u8> {
        let largo = CABECERA_LEN + cuerpo;
        let mut t = vec![0u8; largo];
        t[0..4].copy_from_slice(firma);
        t[4..8].copy_from_slice(&(largo as u32).to_le_bytes());
        t[8] = 1;
        t[10..16].copy_from_slice(b"ALASKA");
        let suma = t.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        t[9] = 0u8.wrapping_sub(suma);
        t
    }

    /// Un XSDT que apunta a `entradas`.
    fn xsdt(entradas: &[u64]) -> Vec<u8> {
        let mut t = tabla(b"XSDT", entradas.len() * 8);
        for (i, e) in entradas.iter().enumerate() {
            let o = CABECERA_LEN + i * 8;
            t[o..o + 8].copy_from_slice(&e.to_le_bytes());
        }
        t[9] = 0;
        let suma = t.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        t[9] = 0u8.wrapping_sub(suma);
        t
    }

    fn firmas(m: &Mem, x: u64) -> Vec<([u8; 4], bool)> {
        let mut v = Vec::new();
        recorrer(x, |f, n| m.leer(f, n), |t| {
            v.push((t.cabecera.firma, t.bytes.is_some()));
            true
        })
        .unwrap();
        v
    }

    #[test]
    fn un_xsdt_sano_da_sus_tablas_creibles() {
        let mut m = Mem::nueva(BASE, 0x1000);
        let (x, a, b) = (BASE, BASE + 0x100, BASE + 0x200);
        m.poner(x, &xsdt(&[a, b]));
        m.poner(a, &tabla(b"APIC", 20));
        m.poner(b, &tabla(b"MCFG", 44));
        assert_eq!(firmas(&m, x), vec![(*b"APIC", true), (*b"MCFG", true)]);
    }

    /// *** EL HUECO QUE TENIA EL KERNEL. Una entrada que dice medir 4 GiB --lo
    /// que da un puntero del XSDT que apunta a basura-- no se lee: sale en el
    /// recorrido como NO CREIBLE, y lo que se le pide a la memoria no pasa del
    /// tope. Quitar el tope pone esta fila roja.
    #[test]
    fn un_largo_de_tabla_absurdo_no_se_lee() {
        let mut m = Mem::nueva(BASE, 0x1000);
        let (x, a) = (BASE, BASE + 0x100);
        m.poner(x, &xsdt(&[a]));
        let mut mala = tabla(b"SSDT", 0);
        mala[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        m.poner(a, &mala);
        assert_eq!(firmas(&m, x), vec![(*b"SSDT", false)]);
        assert!(m.mayor.get() <= MAX_TABLA, "se pidieron {} bytes", m.mayor.get());
    }

    /// Y justo en el tope si se lee: el limite es inclusivo, no un off-by-one.
    #[test]
    fn una_tabla_justo_en_el_tope_si_se_lee() {
        let base = 0x1000_0000;
        let mut m = Mem::nueva(base, MAX_TABLA + 0x200);
        let (x, a) = (base, base + 0x100);
        m.poner(x, &xsdt(&[a]));
        m.poner(a, &tabla(b"DSDT", MAX_TABLA - CABECERA_LEN));
        assert_eq!(firmas(&m, x), vec![(*b"DSDT", true)]);
    }

    /// *** LA DIFERENCIA ENTRE LAS DOS COPIAS, con fila. Una entrada ilegible
    /// ANTES del MCFG no corta la busqueda: el MCFG se encuentra igual. Con el
    /// `?` de la copia vieja de `tabla_de`, esto devolvia "no hay MCFG".
    #[test]
    fn una_entrada_ilegible_no_corta_la_busqueda() {
        let mut m = Mem::nueva(BASE, 0x1000);
        let (x, fuera, mcfg) = (BASE, BASE + 0x10_0000, BASE + 0x200);
        m.poner(x, &xsdt(&[fuera, mcfg]));
        m.poner(mcfg, &tabla(b"MCFG", 44));
        let mut hallada = None;
        recorrer(x, |f, n| m.leer(f, n), |t| {
            if &t.cabecera.firma == b"MCFG" {
                hallada = t.bytes.map(|b| b.len());
                return false;
            }
            true
        })
        .unwrap();
        assert_eq!(hallada, Some(CABECERA_LEN + 44));
    }

    #[test]
    fn una_entrada_a_cero_se_salta() {
        let mut m = Mem::nueva(BASE, 0x1000);
        let (x, a) = (BASE, BASE + 0x100);
        m.poner(x, &xsdt(&[0, a]));
        m.poner(a, &tabla(b"FACP", 8));
        assert_eq!(firmas(&m, x), vec![(*b"FACP", true)]);
    }

    #[test]
    fn una_tabla_que_no_suma_cero_no_es_creible() {
        let mut m = Mem::nueva(BASE, 0x1000);
        let (x, a) = (BASE, BASE + 0x100);
        m.poner(x, &xsdt(&[a]));
        let mut t = tabla(b"IVRS", 40);
        t[40] ^= 0xFF;
        m.poner(a, &t);
        assert_eq!(firmas(&m, x), vec![(*b"IVRS", false)]);
    }

    /// Un XSDT con mas entradas VIVAS de las que caben se corta en 64.
    ///
    /// ** Reescrita el 17-09 porque la mutacion la pillo: la primera version
    /// declaraba un largo enorme con entradas que eran todas CERO, asi que el
    /// recorrido las saltaba con tope o sin el y la fila pasaba igual -- media
    /// algo que no ocurria. Aqui las 100 apuntan a una tabla de verdad, y sin el
    /// tope se verian las 100.
    #[test]
    fn un_xsdt_con_mas_entradas_de_las_que_caben_se_corta_en_64() {
        let mut m = Mem::nueva(BASE, 0x2000);
        let a = BASE + 0x1000;
        m.poner(a, &tabla(b"SSDT", 4));
        m.poner(BASE, &xsdt(&[a; 100]));
        let mut vistas = 0;
        let r = recorrer(BASE, |f, n| m.leer(f, n), |_| {
            vistas += 1;
            true
        });
        assert!(r.is_ok());
        assert_eq!(vistas, MAX_ENTRADAS);
    }

    /// Un XSDT mas corto que su propia cabecera para ANTES de restar.
    #[test]
    fn un_xsdt_mas_corto_que_su_cabecera_para() {
        let mut m = Mem::nueva(BASE, 0x100);
        let mut x = xsdt(&[]);
        x[4..8].copy_from_slice(&10u32.to_le_bytes());
        m.poner(BASE, &x);
        let r = recorrer(BASE, |f, n| m.leer(f, n), |_| true);
        assert_eq!(r.err(), Some(NoXsdt::LargoImposible));
    }

    #[test]
    fn un_xsdt_ilegible_se_dice() {
        let m = Mem::nueva(BASE, 0x10);
        let r = recorrer(BASE + 0x1000, |f, n| m.leer(f, n), |_| true);
        assert_eq!(r.err(), Some(NoXsdt::CabeceraMala));
    }

    #[test]
    fn el_rsdp_se_exige_de_acpi_2_y_con_puntero() {
        let mut r = vec![0u8; RSDP_LEN];
        r[0..8].copy_from_slice(b"RSD PTR ");
        r[15] = 2;
        r[24..32].copy_from_slice(&0x7F00_0000u64.to_le_bytes());
        assert_eq!(xsdt_del_rsdp(&r), Some(0x7F00_0000));

        let mut v1 = r.clone();
        v1[15] = 0;
        assert_eq!(xsdt_del_rsdp(&v1), None, "ACPI 1.0 no trae XSDT");

        let mut sin = r.clone();
        sin[24..32].copy_from_slice(&0u64.to_le_bytes());
        assert_eq!(xsdt_del_rsdp(&sin), None, "un puntero a cero no es un XSDT");

        let mut firma = r.clone();
        firma[0] = b'X';
        assert_eq!(xsdt_del_rsdp(&firma), None);

        assert_eq!(xsdt_del_rsdp(&r[..20]), None, "un RSDP cortado no se lee a medias");
    }
}
