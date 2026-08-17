//! **El formato de TRIM**: un rango de LBA convertido en los bytes que
//! `DATA SET MANAGEMENT` espera. Nada mas.
//!
//! # Que es TRIM, en una frase
//!
//! Es la unica forma que tiene el sistema de decirle a un SSD *"estos sectores
//! ya no le importan a nadie"*. Sin eso el disco los sigue creyendo vivos y los
//! **copia** cada vez que recoge basura por dentro: el recolector de la seccion
//! 9 de ESTRATOS liberaria bloques para el sistema de ficheros y no para el
//! aparato (R-DISCO10, `docs/componente/EL_DISCO_EXIGE.md`).
//!
//! # Por que esto es un crate y no dos funciones dentro del driver
//!
//! Porque **no toca hardware**. Aqui entran un LBA y unos sectores y salen
//! bytes; quien manda el comando es `bmo-ahci` y quien decide si se puede
//! recortar es el kernel. Esa frontera es la misma que ya separo `bmo-identify`
//! (lo que el disco contesta) de `bmo-disco-juicio` (lo que se concluye), y
//! compra lo mismo: **estas reglas se prueban con `cargo test` en el
//! anfitrion**, sin arrancar la maquina y sin arriesgar un sector.
//!
//! Y en este componente eso no es comodidad. Un descriptor mal empaquetado no
//! da un fallo: le dice al disco que **olvide sectores que si importaban**.
//!
//! # El formato, que es de ACS-3 y cabe aqui
//!
//! ```text
//!   descriptor = 8 bytes     bytes 0..5   LBA, little-endian de 48 bits
//!                            bytes 6..7   cuantos sectores, LE de 16 bits
//!
//!   64 descriptores          en un bloque de 512 B
//!   el contador del comando  va en BLOQUES de 512 B, no en sectores
//!   longitud 0               = descriptor vacio: el disco lo ignora
//! ```
//!
//! De ahi salen los dos techos que obligan a partir el trabajo en tandas:
//!
//! ```text
//!   65.535 sectores   lo mas que cubre UN descriptor  (32 MiB)
//!   la palabra 105    lo mas que admite UNA orden     (lo dice el disco)
//! ```
//!
//! Un bloque de payload cubre `64 x 65.535` = ~2 GiB. La cola libre de un
//! volumen de 414 GiB **no cabe en una sola orden**, asi que el bucle de tandas
//! no es teorico: es el caso normal.

#![no_std]

/// Bytes de un bloque de payload. Es la unidad en la que cuenta el comando.
pub const BLOQUE: usize = 512;

/// Bytes de un descriptor.
pub const DESCRIPTOR: usize = 8;

/// Descriptores que caben en un bloque.
pub const POR_BLOQUE: usize = BLOQUE / DESCRIPTOR;

/// Lo mas que cubre un solo descriptor, en sectores.
pub const MAX_POR_DESCRIPTOR: u64 = 0xFFFF;

/// El techo de LBA48. Un descriptor tiene **seis bytes** para la direccion.
pub const MAX_LBA: u64 = 1 << 48;

/// **Lo que queda por recortar.** Se consume a tandas.
///
/// No es una peticion inmutable a proposito: recortar 800 millones de sectores
/// son varias ordenes al disco, y el que las manda necesita saber por donde iba.
/// El estado del avance vive aqui, que es donde se puede probar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rango {
    /// El primer sector que todavia no se ha recortado.
    pub lba: u64,
    /// Cuantos quedan desde ahi.
    pub sectores: u64,
}

/// Lo que UNA orden va a llevar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tanda {
    /// Bloques de payload de 512 B. **Es el contador del comando**, no sectores.
    pub bloques: u16,
    /// Descriptores escritos de verdad. Los demas bytes del ultimo bloque van a
    /// cero, que para el disco es "aqui no hay rango".
    pub descriptores: u16,
    /// Sectores que cubre esta tanda. Para poder contar lo devuelto.
    pub sectores: u64,
}

impl Rango {
    /// Un rango, si es representable. `None` si no lo es, y eso es el punto.
    ///
    /// ** Un LBA que no cabe en 48 bits **no se recorta a 48 bits**: se rechaza.
    /// Truncarlo daria un descriptor perfectamente valido apuntando a **otro
    /// sitio del disco**, y el disco obedeceria sin quejarse -- que es la peor
    /// forma de fallo que puede tener este fichero.
    pub fn nuevo(lba: u64, sectores: u64) -> Option<Rango> {
        if sectores == 0 {
            return None;
        }
        let fin = lba.checked_add(sectores)?;
        if fin > MAX_LBA {
            return None;
        }
        Some(Rango { lba, sectores })
    }

    /// Queda algo?
    pub fn vacio(&self) -> bool {
        self.sectores == 0
    }

    /// **Llena `buf` con la siguiente tanda y avanza.** `None` cuando no queda.
    ///
    /// `bloques_max` es lo que el disco declara en la palabra 105 (ver
    /// `bmo_identify::Trim`): se respeta aunque el buffer de por mas, porque el
    /// techo es del aparato y no de la memoria que tengamos a mano.
    ///
    /// [!] **El payload se pone a cero primero, entero.** Los descriptores que
    /// sobran del ultimo bloque tienen que valer cero o el disco leeria basura
    /// de la vuelta anterior como si fueran rangos -- y esa basura son LBA
    /// validos de este mismo disco.
    pub fn siguiente(&mut self, buf: &mut [u8], bloques_max: u16) -> Option<Tanda> {
        if self.vacio() {
            return None;
        }
        let cabe = buf.len() / BLOQUE;
        if cabe == 0 || bloques_max == 0 {
            return None;
        }
        let bloques = cabe.min(bloques_max as usize).min(u16::MAX as usize);
        let sitio = bloques * BLOQUE;
        for b in buf[..sitio].iter_mut() {
            *b = 0;
        }

        let mut escritos = 0usize;
        let mut cubiertos = 0u64;
        while escritos < bloques * POR_BLOQUE && !self.vacio() {
            let trozo = self.sectores.min(MAX_POR_DESCRIPTOR);
            let off = escritos * DESCRIPTOR;
            let lba = self.lba.to_le_bytes();
            buf[off..off + 6].copy_from_slice(&lba[..6]);
            buf[off + 6..off + 8].copy_from_slice(&(trozo as u16).to_le_bytes());
            self.lba += trozo;
            self.sectores -= trozo;
            cubiertos += trozo;
            escritos += 1;
        }

        // ** Se mandan los bloques que se USARON, no los que se pusieron a cero.
        //
        // Un rango pequeno cabe en un descriptor: mandar los ocho bloques de la
        // pagina serian 511 descriptores vacios viajando por el cable en cada
        // orden. El disco los ignoraria, y aun asi es transferencia que nadie
        // pidio.
        let usados = escritos.div_ceil(POR_BLOQUE).max(1);
        Some(Tanda {
            bloques: usados as u16,
            descriptores: escritos as u16,
            sectores: cubiertos,
        })
    }

    /// **Cuantas ordenes hacen falta para este rango**, sin escribir nada.
    ///
    /// Existe para poder DECIRLO antes de mandar. La seccion 9 de ESTRATOS pide
    /// que el mando manual liste lo que va a hacer *antes* de hacerlo, y un
    /// numero de ordenes es la mitad de esa frase -- la otra mitad son los
    /// bytes, que sabe contar quien pregunta.
    pub fn ordenes(&self, bloques_por_orden: u16) -> u64 {
        if self.vacio() || bloques_por_orden == 0 {
            return 0;
        }
        let por_orden = bloques_por_orden as u64 * POR_BLOQUE as u64 * MAX_POR_DESCRIPTOR;
        self.sectores.div_ceil(por_orden)
    }
}

#[cfg(test)]
mod casillas {
    use super::*;

    fn descriptor(buf: &[u8], n: usize) -> (u64, u16) {
        let o = n * DESCRIPTOR;
        let mut lba = [0u8; 8];
        lba[..6].copy_from_slice(&buf[o..o + 6]);
        (
            u64::from_le_bytes(lba),
            u16::from_le_bytes([buf[o + 6], buf[o + 7]]),
        )
    }

    #[test]
    fn un_rango_pequeno_cabe_en_un_descriptor_y_en_un_bloque() {
        let mut r = Rango::nuevo(206_848, 8).unwrap();
        let mut buf = [0xAAu8; 4096];
        let t = r.siguiente(&mut buf, 8).unwrap();
        assert_eq!(t.bloques, 1, "un descriptor no necesita ocho bloques");
        assert_eq!(t.descriptores, 1);
        assert_eq!(t.sectores, 8);
        assert_eq!(descriptor(&buf, 0), (206_848, 8));
        assert!(r.vacio());
        assert!(r.siguiente(&mut buf, 8).is_none(), "no queda nada que mandar");
    }

    /// ** EL RELLENO TIENE QUE ESTAR A CERO, y esta es la casilla que lo prueba.
    ///
    /// El buffer llega sucio a proposito. Un descriptor sobrante con basura es
    /// un LBA valido de este disco con una longitud valida: el disco olvidaria
    /// sectores que nadie le mando olvidar.
    #[test]
    fn lo_que_sobra_del_bloque_queda_a_cero() {
        let mut r = Rango::nuevo(1000, 4).unwrap();
        let mut buf = [0xAAu8; 4096];
        let t = r.siguiente(&mut buf, 8).unwrap();
        for n in 1..POR_BLOQUE {
            assert_eq!(descriptor(&buf, n), (0, 0), "descriptor {} con basura", n);
        }
        assert_eq!(t.descriptores, 1);
    }

    /// Un rango mas largo que 65.535 sectores se PARTE, no se trunca.
    #[test]
    fn un_rango_largo_se_reparte_en_varios_descriptores() {
        let mut r = Rango::nuevo(0, 65_535 * 2 + 7).unwrap();
        let mut buf = [0u8; 512];
        let t = r.siguiente(&mut buf, 1).unwrap();
        assert_eq!(t.descriptores, 3);
        assert_eq!(t.sectores, 65_535 * 2 + 7);
        assert_eq!(descriptor(&buf, 0), (0, 65_535));
        assert_eq!(descriptor(&buf, 1), (65_535, 65_535));
        assert_eq!(descriptor(&buf, 2), (65_535 * 2, 7));
        assert!(r.vacio());
    }

    /// ** LA CASILLA DE LA COLA LIBRE DE VERDAD: 414 GiB no caben en una orden.
    ///
    /// Con un bloque por orden se cubren ~2 GiB, asi que el bucle de tandas
    /// tiene que dar muchas vueltas **y no perder ni saltarse un sector**. Se
    /// comprueba que la union de las tandas es exactamente el rango original.
    #[test]
    fn la_cola_libre_entera_se_manda_a_tandas_sin_huecos() {
        const SECTORES: u64 = 108_670_208 * 8; // 414 GiB en sectores de 512 B
        let mut r = Rango::nuevo(206_848, SECTORES).unwrap();
        let mut buf = [0u8; 512];
        let mut esperado = 206_848u64;
        let mut suma = 0u64;
        let mut ordenes = 0u64;
        while let Some(t) = r.siguiente(&mut buf, 1) {
            for n in 0..t.descriptores as usize {
                let (lba, largo) = descriptor(&buf, n);
                assert_eq!(lba, esperado, "hueco o solape en el descriptor {}", n);
                esperado += largo as u64;
                suma += largo as u64;
            }
            ordenes += 1;
        }
        assert_eq!(suma, SECTORES, "se perdieron sectores por el camino");
        assert_eq!(esperado, 206_848 + SECTORES);
        assert!(ordenes > 200, "solo {} ordenes: la cuenta no cuadra", ordenes);
    }

    /// El techo del disco manda sobre el del buffer. La palabra 105 es del
    /// aparato; el buffer es nuestro.
    #[test]
    fn la_palabra_105_acota_aunque_el_buffer_de_para_mas() {
        let mut r = Rango::nuevo(0, MAX_POR_DESCRIPTOR * 200).unwrap();
        let mut buf = [0u8; 4096]; // 8 bloques
        let t = r.siguiente(&mut buf, 1).unwrap();
        assert_eq!(t.bloques, 1, "el disco dijo 1 bloque por orden");
        assert_eq!(t.descriptores, POR_BLOQUE as u16);
    }

    /// Un buffer que no llega ni a un bloque no sirve, y se dice con `None` en
    /// vez de mandar media orden.
    #[test]
    fn un_buffer_corto_no_manda_nada() {
        let mut r = Rango::nuevo(0, 8).unwrap();
        let mut buf = [0u8; 100];
        assert!(r.siguiente(&mut buf, 8).is_none());
        assert!(!r.vacio(), "y el rango NO se consume");
    }

    /// ** UN LBA QUE NO CABE EN 48 BITS SE RECHAZA, no se recorta.
    #[test]
    fn fuera_de_lba48_no_hay_rango() {
        assert!(Rango::nuevo(MAX_LBA, 1).is_none());
        assert!(Rango::nuevo(MAX_LBA - 4, 8).is_none(), "el FINAL tambien cuenta");
        assert!(Rango::nuevo(u64::MAX, 8).is_none(), "y no da la vuelta la suma");
        assert!(Rango::nuevo(MAX_LBA - 8, 8).is_some());
    }

    #[test]
    fn cero_sectores_no_es_un_rango() {
        assert!(Rango::nuevo(1000, 0).is_none());
    }

    /// Contar ordenes sin escribir nada tiene que dar lo mismo que darlas.
    #[test]
    fn la_cuenta_previa_coincide_con_las_tandas_que_salen() {
        let r0 = Rango::nuevo(2048, MAX_POR_DESCRIPTOR * 130).unwrap();
        let previstas = r0.ordenes(1);
        let mut r = r0;
        let mut buf = [0u8; 512];
        let mut dadas = 0u64;
        while r.siguiente(&mut buf, 1).is_some() {
            dadas += 1;
        }
        assert_eq!(previstas, dadas);
    }
}
