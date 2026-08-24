//! **LO QUE LA PLACA BASE LE CUENTA A BMO-X** -- y lo que BMO-X se niega a
//! escuchar.
//!
//! # La linea que este crate existe para trazar
//!
//! ACPI son **dos cosas con el mismo nombre**, y casi ninguna discusion las
//! separa:
//!
//! ```text
//!    TABLAS ESTATICAS   MADT, FADT, MCFG, HPET, IVRS...
//!                       structs con cabecera y campos. Son HECHOS.
//!
//!    AML                DSDT, SSDT, PSDT
//!                       bytecode. Es un PROGRAMA que la placa quiere que
//!                       ejecutes DENTRO de tu kernel.
//! ```
//!
//! *** **BMO-X lee las primeras y NO EJECUTA las segundas.** Y no es una
//! carencia que se arreglara: es la ley 24 aplicada al firmware.
//!
//! ## Por que Windows si, y por que aqui no
//!
//! Windows y Linux traen un **interprete de AML** porque tienen que arrancar en
//! cualquier placa que exista, incluidas las que no han salido. No pueden saber
//! de antemano donde esta el registro que apaga el ventilador de ESA placa, asi
//! que dejan que la placa se lo diga **en su propio lenguaje**, y lo ejecutan.
//!
//! Eso es la respuesta correcta a *"soy generalista"*. Y trae su precio: el
//! interprete de AML de Linux son decenas de miles de lineas, corre en el
//! nucleo, y ha tenido su cuota de agujeros -- porque **es un interprete de
//! bytecode de terceros en Ring 0**.
//!
//! ** BMO-X no es generalista. Se perfila. Y en una placa perfilada, lo que el
//! AML te contaria **ya lo sabes**: esta escrito en el perfil, con su numero, y
//! un test puede exigir que no cambie. Un dato en una tabla del repositorio se
//! puede leer, discutir y versionar. Un dato que sale de ejecutar el programa
//! del fabricante, no.
//!
//! ```text
//!    generalista   ->  ejecuta el programa de la placa para saber donde esta todo
//!    perfilado     ->  ya sabe donde esta todo, y COMPRUEBA que la placa coincide
//! ```
//!
//! *** Y la diferencia se nota el dia que no coinciden: el generalista **no se
//! entera** --hace lo que le dijeron-- y el perfilado **se entera y lo dice**.
//!
//! # Lo que este crate SI hace
//!
//! Interpretar los bytes de una cabecera ACPI, que es la parte que se equivoca
//! en silencio y **la unica que se puede probar en el anfitrion**. Leer memoria
//! fisica es del kernel; decidir si esos 36 bytes son una tabla valida es de
//! aqui, y por eso hay pruebas.
//!
//! Es el mismo reparto que `bmo-net`: el kernel toca el aparato, y la
//! interpretacion de los bits se prueba donde se puede correr un test.

#![cfg_attr(not(test), no_std)]

/// Bytes de la cabecera que llevan **todas** las tablas ACPI, sin excepcion.
///
/// Es lo que hace posible censar el XSDT sin saber que tablas hay: se lee la
/// cabecera, se sabe la firma y el largo, y se salta a la siguiente.
pub const CABECERA_LEN: usize = 36;

/// La cabecera comun de una tabla ACPI, ya leida.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cabecera {
    /// Cuatro caracteres ASCII. `"APIC"`, `"FACP"`, `"MCFG"`...
    pub firma: [u8; 4],
    /// Bytes de la tabla ENTERA, cabecera incluida.
    pub largo: u32,
    pub revision: u8,
    /// El byte que hace que la suma de toda la tabla de cero.
    pub suma: u8,
    /// Seis caracteres: quien fabrico el firmware.
    pub oem: [u8; 6],
    /// Ocho mas: que modelo de placa, segun ese fabricante.
    pub oem_tabla: [u8; 8],
    pub oem_revision: u32,
}

/// Por que unos bytes no son una tabla.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Falta {
    /// No llegan ni a la cabecera comun.
    Corta,
    /// El largo que declara es menor que su propia cabecera: se contradice.
    LargoImposible,
    /// **La suma de todos sus bytes no da cero.**
    SumaMala,
    /// La firma tiene bytes que no son ASCII imprimible.
    FirmaRara,
}

impl Falta {
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::Corta => "no llega ni a la cabecera de 36 bytes",
            Falta::LargoImposible => "dice medir menos que su propia cabecera",
            Falta::SumaMala => "la suma de comprobacion no da cero",
            Falta::FirmaRara => "la firma no son cuatro letras",
        }
    }
}

fn u32_en(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

impl Cabecera {
    /// Lee la cabecera sin comprobar nada. Para eso esta [`revisar`].
    pub fn leer(bytes: &[u8]) -> Option<Cabecera> {
        if bytes.len() < CABECERA_LEN {
            return None;
        }
        let mut firma = [0u8; 4];
        let mut oem = [0u8; 6];
        let mut oem_tabla = [0u8; 8];
        firma.copy_from_slice(&bytes[0..4]);
        oem.copy_from_slice(&bytes[10..16]);
        oem_tabla.copy_from_slice(&bytes[16..24]);
        Some(Cabecera {
            firma,
            largo: u32_en(bytes, 4),
            revision: bytes[8],
            suma: bytes[9],
            oem,
            oem_tabla,
            oem_revision: u32_en(bytes, 24),
        })
    }

    /// La firma como texto, si son cuatro letras.
    pub fn firma_texto(&self) -> Option<&str> {
        core::str::from_utf8(&self.firma).ok()
    }

    /// **Que es esta tabla**, dicho en castellano.
    ///
    /// ** Una firma de cuatro letras es un dato para quien tenga la
    /// especificacion delante, y no es nada para todos los demas. Esta linea la
    /// lee una persona decidiendo si la placa le esta contando lo que espera.
    pub fn que_es(&self) -> &'static str {
        match &self.firma {
            b"APIC" => "el censo de nucleos (MADT)",
            b"FACP" => "energia, reset y el RTC (FADT)",
            b"MCFG" => "donde vive la config de PCIe en memoria",
            b"HPET" => "el temporizador de alta precision",
            b"SRAT" => "que memoria pertenece a que nucleo (NUMA)",
            b"SLIT" => "cuanto cuesta ir de un nodo NUMA a otro",
            b"IVRS" => "la IOMMU de AMD -- quien puede hacer DMA y adonde",
            b"DMAR" => "la IOMMU de Intel",
            b"BGRT" => "el logo que el firmware dejo en pantalla",
            b"WSMT" => "que mitigaciones dice el firmware que aplica",
            b"FPDT" => "cuanto tardo el firmware en arrancar",
            b"SSDT" => "[!] AML -- un PROGRAMA, y aqui no se ejecuta",
            b"DSDT" => "[!] AML -- un PROGRAMA, y aqui no se ejecuta",
            b"PSDT" => "[!] AML -- un PROGRAMA, y aqui no se ejecuta",
            _ => "",
        }
    }

    /// *** **Es esta tabla un PROGRAMA en vez de un HECHO?**
    ///
    /// ** DSDT, SSDT y PSDT no llevan campos: llevan **bytecode AML**, que un
    /// sistema generalista interpreta dentro de su nucleo para averiguar donde
    /// esta cada cosa en esa placa concreta.
    ///
    /// *** BMO-X no lo interpreta, y esa es una decision de arquitectura con dos
    /// motivos, no uno:
    ///
    /// 1. **Es un interprete de bytecode de terceros en Ring 0.** Un sistema
    ///    cuya frase de portada es *"la autoridad es funcional, nunca heredada"*
    ///    no ejecuta el programa del fabricante de la placa con sus permisos.
    ///
    /// 2. **Un sistema perfilado no lo necesita.** Lo que el AML contaria ya
    ///    esta en el perfil, escrito, con su numero y con un test que exige que
    ///    no cambie. El AML se ejecuta para *descubrir*; aqui se *comprueba*.
    ///
    /// [!] Y el precio, dicho: sin AML no hay control de ventiladores, ni
    /// estados de energia de aparatos, ni botones del portatil. Eso se paga y se
    /// dice, en vez de fingir que ACPI esta "soportado".
    pub fn es_un_programa(&self) -> bool {
        matches!(&self.firma, b"DSDT" | b"SSDT" | b"PSDT")
    }
}

/// **Que estos bytes sean una tabla ACPI y no basura que lo parece.**
///
/// [!] `bytes` tiene que ser la tabla ENTERA, no solo su cabecera: la suma de
/// comprobacion recorre todo el largo declarado, y ese es el punto -- una tabla
/// que se leyo a medias da suma mala, que es exactamente lo que se quiere saber.
pub fn revisar(bytes: &[u8]) -> Result<Cabecera, Falta> {
    let c = Cabecera::leer(bytes).ok_or(Falta::Corta)?;
    if (c.largo as usize) < CABECERA_LEN {
        return Err(Falta::LargoImposible);
    }
    // La firma tiene que ser imprimible. Una tabla con bytes de control en la
    // firma es un puntero que apunta a otra cosa, no una tabla desconocida.
    if !c.firma.iter().all(|&b| (0x20..0x7F).contains(&b)) {
        return Err(Falta::FirmaRara);
    }
    let hasta = (c.largo as usize).min(bytes.len());
    if hasta < c.largo as usize {
        // Faltan bytes para poder sumar lo que declara. No se aprueba a medias.
        return Err(Falta::SumaMala);
    }
    // ** La suma de TODOS los bytes de la tabla tiene que dar cero modulo 256.
    // Es la unica comprobacion que ACPI trae de serie, y es la que separa "esta
    // tabla existe" de "aqui hay memoria que se lee".
    let total = bytes[..hasta].iter().fold(0u8, |a, &b| a.wrapping_add(b));
    if total != 0 {
        return Err(Falta::SumaMala);
    }
    Ok(c)
}

/// Texto de un campo OEM, sin los espacios de relleno del final.
///
/// ** ACPI rellena estos campos con espacios, no con ceros. Imprimirlos crudos
/// mete tres espacios en medio de una linea y hace que dos placas distintas se
/// vean igual de mal.
pub fn recortar(campo: &[u8]) -> &str {
    let fin = campo
        .iter()
        .rposition(|&b| b != b' ' && b != 0)
        .map(|i| i + 1)
        .unwrap_or(0);
    core::str::from_utf8(&campo[..fin]).unwrap_or("")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Fabrica una tabla con firma y OEM dados, y la suma ya cuadrada.
    fn tabla(firma: &[u8; 4], oem: &[u8; 6], largo: usize) -> Vec<u8> {
        let mut t = vec![0u8; largo.max(CABECERA_LEN)];
        t[0..4].copy_from_slice(firma);
        let l = t.len() as u32;
        t[4..8].copy_from_slice(&l.to_le_bytes());
        t[8] = 2;
        t[10..16].copy_from_slice(oem);
        // El byte 9 es el que hace que todo sume cero.
        let suma: u8 = t.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        t[9] = (0u8).wrapping_sub(suma);
        t
    }

    #[test]
    fn una_tabla_bien_formada_se_lee_entera() {
        let t = tabla(b"APIC", b"ALASKA", 100);
        let c = revisar(&t).expect("tiene que pasar");
        assert_eq!(c.firma_texto(), Some("APIC"));
        assert_eq!(c.largo, 100);
        assert_eq!(recortar(&c.oem), "ALASKA");
        assert_eq!(c.que_es(), "el censo de nucleos (MADT)");
        assert!(!c.es_un_programa());
    }

    /// *** LA SUMA ES LA UNICA COMPROBACION QUE ACPI TRAE, y es la que separa
    /// "esta tabla existe" de "aqui hay memoria que se lee".
    ///
    /// ** Un puntero del XSDT que apunte a memoria que no es una tabla da una
    /// cabecera con campos plausibles: cuatro bytes cualesquiera parecen una
    /// firma, y un `u32` cualquiera parece un largo. **Sin la suma, el censo se
    /// creeria cualquier cosa.**
    #[test]
    fn un_byte_cambiado_tumba_la_suma() {
        let mut t = tabla(b"MCFG", b"ALASKA", 60);
        assert!(revisar(&t).is_ok());
        t[40] = t[40].wrapping_add(1);
        assert_eq!(revisar(&t), Err(Falta::SumaMala));
    }

    /// Una tabla leida a medias NO se aprueba: la suma no se puede hacer.
    #[test]
    fn una_tabla_cortada_no_pasa_por_estar_cortada() {
        let t = tabla(b"HPET", b"ALASKA", 200);
        assert_eq!(revisar(&t[..150]), Err(Falta::SumaMala));
        assert_eq!(revisar(&t[..10]), Err(Falta::Corta));
    }

    /// Una firma con bytes de control es un puntero a otra cosa, no una tabla
    /// desconocida. Distinguirlo importa: lo primero es un fallo del censo y lo
    /// segundo es una placa con una tabla que no conocemos.
    #[test]
    fn una_firma_que_no_son_letras_se_denuncia() {
        let mut t = tabla(b"APIC", b"ALASKA", 60);
        t[1] = 0x01;
        let suma: u8 = t[..t.len()].iter().fold(0u8, |a, &b| a.wrapping_add(b));
        t[9] = t[9].wrapping_sub(suma);
        assert_eq!(revisar(&t), Err(Falta::FirmaRara));
    }

    /// *** LAS TRES QUE SON UN PROGRAMA, y BMO-X no las ejecuta.
    ///
    /// ** DSDT, SSDT y PSDT llevan bytecode AML. Un sistema generalista lo
    /// interpreta dentro de su nucleo para descubrir donde esta cada cosa en esa
    /// placa. Un sistema perfilado ya lo sabe -- y no mete un interprete de
    /// bytecode de terceros en Ring 0.
    #[test]
    fn las_tablas_de_aml_se_marcan_como_programa() {
        for firma in [b"DSDT", b"SSDT", b"PSDT"] {
            let t = tabla(firma, b"ALASKA", 4000);
            let c = revisar(&t).unwrap();
            assert!(c.es_un_programa(), "{:?} es AML", firma);
            assert!(c.que_es().contains("no se ejecuta"));
        }
        // Y las de datos NO se marcan, o el aviso no diria nada.
        for firma in [b"APIC", b"FACP", b"MCFG", b"IVRS"] {
            let t = tabla(firma, b"ALASKA", 60);
            assert!(!revisar(&t).unwrap().es_un_programa());
        }
    }

    /// ACPI rellena los campos OEM con ESPACIOS, no con ceros.
    #[test]
    fn el_oem_se_recorta_por_espacios_y_por_ceros() {
        assert_eq!(recortar(b"ALASKA"), "ALASKA");
        assert_eq!(recortar(b"AMD   "), "AMD");
        assert_eq!(recortar(b"AMD\0\0\0"), "AMD");
        assert_eq!(recortar(b"      "), "");
    }

    /// Una tabla que dice medir menos que su propia cabecera se contradice sola,
    /// y eso es distinto de estar cortada: aqui el dato malo es el LARGO.
    #[test]
    fn un_largo_menor_que_la_cabecera_se_caza_aparte() {
        let mut t = tabla(b"APIC", b"ALASKA", 60);
        t[4..8].copy_from_slice(&10u32.to_le_bytes());
        assert_eq!(revisar(&t), Err(Falta::LargoImposible));
    }
}
