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


// ===================================================================
//  MCFG -- donde vive la configuracion de PCIe EN MEMORIA
// ===================================================================
//
//  ## Por que esta tabla vale mas que las otras
//
//  PCI clasico se lee por dos puertos de E/S (`0xCF8` / `0xCFC`), y ese camino
//  **solo alcanza los primeros 256 bytes** del espacio de configuracion de cada
//  funcion. Es lo que BMO-X usa hoy.
//
//  PCIe tiene **4096**. Los otros 3.840 bytes son las *capabilities extendidas*
//  -- y ahi viven cosas que no son opcionales para lo que este sistema quiere
//  hacer:
//
//  ```text
//     AER          errores del enlace, con detalle
//     ATS / PASID  lo que hace falta para que un aparato use direcciones
//                  virtuales -- o sea, para una IOMMU util
//     SR-IOV       funciones virtuales
//     el enlace    ancho y velocidad negociados de verdad
//  ```
//
//  *** Y no se llega a ellos "con mas cuidado": **no hay forma** por los
//  puertos. Hace falta la direccion base que declara MCFG, y esa es toda la
//  razon de que esta tabla exista.

/// Un rango de buses y donde vive su configuracion, del MCFG.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RangoEcam {
    /// Direccion FISICA donde empieza la ventana de configuracion.
    pub base: u64,
    /// Grupo de segmento PCI. En una maquina de escritorio es 0.
    pub segmento: u16,
    pub bus_desde: u8,
    pub bus_hasta: u8,
}

/// Bytes de la cabecera del MCFG antes de la primera entrada: la cabecera ACPI
/// comun mas ocho reservados.
pub const MCFG_CABECERA: usize = CABECERA_LEN + 8;
/// Bytes de cada entrada del MCFG.
pub const MCFG_ENTRADA: usize = 16;

impl RangoEcam {
    /// **La direccion fisica de un registro de configuracion.**
    ///
    /// ```text
    ///    base + ((bus - bus_desde) << 20) + (dev << 15) + (fun << 12) + offset
    /// ```
    ///
    /// ** El `bus - bus_desde` es la parte que se olvida y no avisa: la ventana
    /// empieza en `bus_desde`, no en el bus 0. En una placa donde `bus_desde`
    /// es 0 --que son casi todas-- restar o no restar da lo mismo, **y por eso
    /// el fallo no aparece hasta la placa que no lo es.**
    ///
    /// `None` si el bus no cae en este rango, o si el offset se sale de los
    /// 4096 bytes que tiene una funcion.
    pub fn direccion(&self, bus: u8, dispositivo: u8, funcion: u8, offset: u16) -> Option<u64> {
        if bus < self.bus_desde || bus > self.bus_hasta {
            return None;
        }
        if dispositivo > 31 || funcion > 7 || offset >= 4096 {
            return None;
        }
        Some(
            self.base
                + (((bus - self.bus_desde) as u64) << 20)
                + ((dispositivo as u64) << 15)
                + ((funcion as u64) << 12)
                + offset as u64,
        )
    }

    /// Cuantos bytes ocupa la ventana entera de este rango. Un bus son 1 MiB.
    pub fn mide(&self) -> u64 {
        ((self.bus_hasta as u64) - (self.bus_desde as u64) + 1) << 20
    }
}

/// **Lee las entradas del MCFG.** `bytes` es la tabla entera.
///
/// Devuelve cuantas escribio en `salida`. Un array y no un `Vec` porque esto se
/// llama desde el arranque, sin monton.
pub fn leer_mcfg(bytes: &[u8], salida: &mut [RangoEcam]) -> usize {
    if bytes.len() < MCFG_CABECERA || salida.is_empty() {
        return 0;
    }
    let largo = u32_en(bytes, 4) as usize;
    let hasta = largo.min(bytes.len());
    if hasta < MCFG_CABECERA {
        return 0;
    }
    let cuantas = ((hasta - MCFG_CABECERA) / MCFG_ENTRADA).min(salida.len());
    for i in 0..cuantas {
        let o = MCFG_CABECERA + i * MCFG_ENTRADA;
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[o..o + 8]);
        salida[i] = RangoEcam {
            base: u64::from_le_bytes(b),
            segmento: u16::from_le_bytes([bytes[o + 8], bytes[o + 9]]),
            bus_desde: bytes[o + 10],
            bus_hasta: bytes[o + 11],
        };
    }
    cuantas
}

// ===================================================================
//  IVRS -- la IOMMU de AMD
// ===================================================================
//
//  ## Por que esta tabla le importa a un sistema de capabilities
//
//  Una capability dice que puede hacer un PROCESO. **No dice nada de lo que
//  puede hacer un APARATO**, y un aparato con DMA escribe donde le den la
//  direccion -- sin pasar por el kernel, sin pasar por las tablas de pagina, y
//  sin que nadie se entere.
//
//  *** O sea: **hoy el modelo de seguridad de BMO-X tiene un agujero del tamano
//  de cualquier aparato con bus-master.** Un anillo de descriptores mal armado
//  no da un fallo: da la tarjeta escribiendo en memoria de otro, y el sintoma
//  tres arranques despues. Ya se piso esa mina con el PRDT de AHCI.
//
//  La IOMMU es lo unico que cierra eso: pone tablas de pagina **para los
//  aparatos**, y un aparato que se salga de las suyas recibe un fallo en vez de
//  escribir.
//
//  ** Esto solo LEE la tabla: dice si la hay y donde esta. Encenderla es otro
//  trabajo, y grande. Pero saber que existe es lo que permite escribir el plan
//  con un numero en vez de una intencion.

/// Un bloque IVHD: **un** IOMMU, con donde vive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ivhd {
    /// `0x10`, `0x11` o `0x40`. Los tres describen un IOMMU; cambian los campos
    /// de detalle que traen detras.
    pub tipo: u8,
    pub banderas: u8,
    /// Bytes de este bloque, incluidas sus entradas de dispositivo.
    pub largo: u16,
    /// El BDF del propio IOMMU dentro del bus PCI.
    pub id_dispositivo: u16,
    /// **Donde viven sus registros.** Direccion fisica.
    pub base_mmio: u64,
    pub segmento: u16,
}

/// Bytes de la cabecera del IVRS antes del primer IVHD: la cabecera ACPI comun,
/// `IVinfo` (4) y ocho reservados.
pub const IVRS_CABECERA: usize = CABECERA_LEN + 4 + 8;

/// **Lee los bloques IVHD de un IVRS.** Devuelve cuantos escribio.
///
/// [!] Los campos de `IVinfo` **no se decodifican aqui**, y es deliberado: se
/// devuelve crudo. Es la misma regla que `Identidad::phy` en el driver de red --
/// *el byte entero es la prueba y las funciones son la opinion*. Decodificar
/// bits de una especificacion que no se tiene delante es como se inventan
/// campos que luego nadie puede refutar.
pub fn leer_ivrs(bytes: &[u8], salida: &mut [Ivhd]) -> usize {
    if bytes.len() < IVRS_CABECERA || salida.is_empty() {
        return 0;
    }
    let largo = (u32_en(bytes, 4) as usize).min(bytes.len());
    let mut o = IVRS_CABECERA;
    let mut n = 0usize;
    // ** El bucle avanza por el `largo` de cada bloque, asi que un largo de cero
    // lo dejaria girando para siempre. Se corta, y ademas se limita por el tope
    // de la salida: un bucle sobre datos de firmware tiene que terminar aunque
    // el firmware mienta.
    while o + 24 <= largo && n < salida.len() {
        let tipo = bytes[o];
        let banderas = bytes[o + 1];
        let l = u16::from_le_bytes([bytes[o + 2], bytes[o + 3]]);
        if l < 24 {
            break;
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[o + 8..o + 16]);
        salida[n] = Ivhd {
            tipo,
            banderas,
            largo: l,
            id_dispositivo: u16::from_le_bytes([bytes[o + 4], bytes[o + 5]]),
            base_mmio: u64::from_le_bytes(b),
            segmento: u16::from_le_bytes([bytes[o + 16], bytes[o + 17]]),
        };
        n += 1;
        o += l as usize;
    }
    n
}

/// El `IVinfo` crudo del IVRS. Cuatro bytes, sin interpretar.
pub fn ivinfo(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < CABECERA_LEN + 4 {
        return None;
    }
    Some(u32_en(bytes, CABECERA_LEN))
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

    // ===============================================================
    //  MCFG
    // ===============================================================

    fn mcfg(rangos: &[(u64, u16, u8, u8)]) -> Vec<u8> {
        let largo = MCFG_CABECERA + rangos.len() * MCFG_ENTRADA;
        let mut t = tabla(b"MCFG", b"ALASKA", largo);
        for (i, (base, seg, d, h)) in rangos.iter().enumerate() {
            let o = MCFG_CABECERA + i * MCFG_ENTRADA;
            t[o..o + 8].copy_from_slice(&base.to_le_bytes());
            t[o + 8..o + 10].copy_from_slice(&seg.to_le_bytes());
            t[o + 10] = *d;
            t[o + 11] = *h;
        }
        // Recuadrar la suma despues de escribir las entradas.
        t[9] = 0;
        let suma: u8 = t.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        t[9] = (0u8).wrapping_sub(suma);
        t
    }

    #[test]
    fn el_mcfg_da_la_ventana_de_configuracion() {
        let t = mcfg(&[(0xE000_0000, 0, 0, 255)]);
        assert!(revisar(&t).is_ok(), "la tabla tiene que seguir cuadrando");

        let mut r = [RangoEcam { base: 0, segmento: 0, bus_desde: 0, bus_hasta: 0 }; 4];
        assert_eq!(leer_mcfg(&t, &mut r), 1);
        assert_eq!(r[0].base, 0xE000_0000);
        assert_eq!(r[0].bus_hasta, 255);
        // 256 buses de 1 MiB cada uno.
        assert_eq!(r[0].mide(), 256 << 20);
    }

    /// *** LA CUENTA DEL ECAM, y el termino que se olvida sin avisar.
    ///
    /// ** `bus - bus_desde`: la ventana empieza en `bus_desde`, no en el bus 0.
    /// En una placa donde `bus_desde` es 0 --que son casi todas-- restar o no
    /// restar da lo mismo, **y por eso el fallo no aparece hasta la placa que no
    /// lo es**. Esta prueba usa una ventana que NO empieza en cero justamente
    /// para que el termino no se pueda quitar sin que algo se ponga rojo.
    #[test]
    fn la_direccion_resta_el_bus_de_inicio() {
        let r = RangoEcam { base: 0xF000_0000, segmento: 0, bus_desde: 16, bus_hasta: 31 };

        // El primer bus de la ventana cae en la base, no un desplazamiento mas alla.
        assert_eq!(r.direccion(16, 0, 0, 0), Some(0xF000_0000));
        // Un bus mas arriba es 1 MiB mas arriba.
        assert_eq!(r.direccion(17, 0, 0, 0), Some(0xF010_0000));
        // Dispositivo, funcion y offset se apilan dentro del megabyte del bus.
        assert_eq!(r.direccion(16, 1, 0, 0), Some(0xF000_8000));
        assert_eq!(r.direccion(16, 0, 1, 0), Some(0xF000_1000));
        assert_eq!(r.direccion(16, 0, 0, 0x100), Some(0xF000_0100));

        // Fuera del rango de buses: no es de este rango.
        assert_eq!(r.direccion(15, 0, 0, 0), None);
        assert_eq!(r.direccion(32, 0, 0, 0), None);
    }

    /// **Y los 4096 bytes son el limite**, que es el punto entero de ECAM: por
    /// los puertos `0xCF8`/`0xCFC` solo se alcanzan 256.
    #[test]
    fn el_offset_llega_a_cuatro_mil_noventa_y_seis_y_no_mas() {
        let r = RangoEcam { base: 0, segmento: 0, bus_desde: 0, bus_hasta: 0 };
        assert!(r.direccion(0, 0, 0, 4095).is_some(), "el ultimo byte SI");
        assert_eq!(r.direccion(0, 0, 0, 4096), None, "y uno mas ya no");
        // 256 es donde se acaba el PCI clasico, y aqui esta dentro de sobra.
        assert!(r.direccion(0, 0, 0, 256).is_some());
        // Un dispositivo o funcion imposibles se dicen, no se envuelven.
        assert_eq!(r.direccion(0, 32, 0, 0), None);
        assert_eq!(r.direccion(0, 0, 8, 0), None);
    }

    #[test]
    fn varios_rangos_se_leen_todos_y_el_tope_se_respeta() {
        let t = mcfg(&[(0xE000_0000, 0, 0, 63), (0xE400_0000, 0, 64, 127)]);
        let mut r = [RangoEcam { base: 0, segmento: 0, bus_desde: 0, bus_hasta: 0 }; 4];
        assert_eq!(leer_mcfg(&t, &mut r), 2);
        assert_eq!(r[1].bus_desde, 64);

        // Con sitio para uno, se lee uno. Nada de escribir fuera.
        let mut corto = [RangoEcam { base: 0, segmento: 0, bus_desde: 0, bus_hasta: 0 }; 1];
        assert_eq!(leer_mcfg(&t, &mut corto), 1);
    }

    // ===============================================================
    //  IVRS
    // ===============================================================

    fn ivrs(bloques: &[(u8, u16, u64)]) -> Vec<u8> {
        let largo = IVRS_CABECERA + bloques.iter().map(|b| b.1 as usize).sum::<usize>();
        let mut t = tabla(b"IVRS", b"AMD   ", largo);
        let mut o = IVRS_CABECERA;
        for (tipo, l, base) in bloques {
            t[o] = *tipo;
            t[o + 2..o + 4].copy_from_slice(&l.to_le_bytes());
            t[o + 8..o + 16].copy_from_slice(&base.to_le_bytes());
            o += *l as usize;
        }
        t[9] = 0;
        let suma: u8 = t.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        t[9] = (0u8).wrapping_sub(suma);
        t
    }

    /// **Si hay IVRS, hay IOMMU -- y esto dice DONDE.**
    ///
    /// ** Saber que existe no la enciende. Pero es lo que permite escribir el
    /// plan con un numero en vez de una intencion, y es lo que hoy falta para
    /// cerrar el agujero: una capability dice que puede hacer un PROCESO, y no
    /// dice nada de lo que puede hacer un APARATO con DMA.
    #[test]
    fn el_ivrs_dice_donde_vive_la_iommu() {
        let t = ivrs(&[(0x10, 40, 0xFEB8_0000)]);
        assert!(revisar(&t).is_ok());

        let mut v = [Ivhd { tipo: 0, banderas: 0, largo: 0, id_dispositivo: 0, base_mmio: 0, segmento: 0 }; 4];
        assert_eq!(leer_ivrs(&t, &mut v), 1);
        assert_eq!(v[0].tipo, 0x10);
        assert_eq!(v[0].base_mmio, 0xFEB8_0000, "los registros del IOMMU");
    }

    /// *** UN BUCLE SOBRE DATOS DE FIRMWARE TIENE QUE TERMINAR AUNQUE EL
    /// FIRMWARE MIENTA.
    ///
    /// ** El recorrido avanza por el `largo` de cada bloque. Un largo de CERO
    /// dejaria el bucle girando para siempre, dentro del kernel, en el arranque
    /// -- y el sintoma seria una maquina que no enciende, sin una linea que
    /// diga por que.
    #[test]
    fn un_bloque_de_largo_cero_no_cuelga_el_recorrido() {
        let mut t = ivrs(&[(0x10, 40, 0xFEB8_0000)]);
        // Se pisa el largo del bloque con cero, a mano.
        t[IVRS_CABECERA + 2] = 0;
        t[IVRS_CABECERA + 3] = 0;

        let mut v = [Ivhd { tipo: 0, banderas: 0, largo: 0, id_dispositivo: 0, base_mmio: 0, segmento: 0 }; 4];
        // Lo que importa de esta prueba es que TERMINE.
        assert_eq!(leer_ivrs(&t, &mut v), 0, "un largo imposible corta el recorrido");
    }

    #[test]
    fn varios_iommu_se_leen_en_orden() {
        let t = ivrs(&[(0x10, 32, 0xFEB8_0000), (0x11, 40, 0xFEC0_0000)]);
        let mut v = [Ivhd { tipo: 0, banderas: 0, largo: 0, id_dispositivo: 0, base_mmio: 0, segmento: 0 }; 4];
        assert_eq!(leer_ivrs(&t, &mut v), 2);
        assert_eq!(v[1].base_mmio, 0xFEC0_0000);
    }

    /// El `IVinfo` se devuelve **crudo**, sin decodificar.
    ///
    /// ** Es la misma regla que `Identidad::phy` en el driver de red: *el byte
    /// entero es la prueba y las funciones son la opinion*. Decodificar bits de
    /// una especificacion que no se tiene delante es como se inventan campos
    /// que luego nadie puede refutar.
    #[test]
    fn el_ivinfo_sale_crudo() {
        let mut t = ivrs(&[(0x10, 32, 0)]);
        t[CABECERA_LEN..CABECERA_LEN + 4].copy_from_slice(&0x0023_0F5Au32.to_le_bytes());
        assert_eq!(ivinfo(&t), Some(0x0023_0F5A));
        assert_eq!(ivinfo(&[0u8; 10]), None);
    }

}
