//! **EL CENSO DE LA PLACA: que tablas ofrece el firmware, y cual se cree.**
//!
//! [carril]  AMARILLO  que tablas ofrece el firmware y cual se cree
//!
//! ## Por que esto existe, y por que es LEER y nada mas
//!
//! Hasta hoy BMO-X le hacia al firmware **una sola pregunta**: *"cuantos nucleos
//! hay"* (la MADT). Todo lo demas que la placa cuenta de si misma --donde vive
//! la config de PCIe, si hay IOMMU, quien fabrico el firmware-- estaba ahi y
//! nadie lo miraba.
//!
//! Este modulo no usa nada de eso todavia. **Lo cuenta**, que es el paso 0 --
//! el mismo de la red: cero escrituras, respuestas predecibles, y se compara
//! contra lo que dice el otro sistema en la misma maquina.
//!
//! ```text
//!    lo que se hace   recorrer el XSDT y decir que tablas hay
//!    lo que NO        interpretar ninguna de ellas todavia
//!    lo que NUNCA     ejecutar AML
//! ```
//!
//! ## *** LA LINEA QUE SEPARA A BMO-X DE UN SISTEMA GENERALISTA
//!
//! ACPI son dos cosas con el mismo nombre: **tablas estaticas** (structs: son
//! hechos) y **AML** (bytecode: es un programa). Windows y Linux traen un
//! interprete de AML porque tienen que arrancar en placas que no han salido
//! todavia, y no pueden saber donde esta cada registro de ESA placa.
//!
//! **BMO-X se perfila.** En una placa perfilada, lo que el AML contaria ya esta
//! escrito, con su numero y con un test que exige que no cambie.
//!
//! ```text
//!    generalista   ejecuta el programa de la placa para DESCUBRIR
//!    perfilado     ya lo sabe, y COMPRUEBA que la placa coincide
//! ```
//!
//! Y la diferencia se nota el dia que no coinciden: el generalista **no se
//! entera** y el perfilado **lo dice**.
//!
//! El razonamiento entero, con lo que cuesta, esta en `bmo_firmware`.
//!
//! ## El reparto con `bmo_firmware`
//!
//! Aqui se lee memoria fisica --que es de Ring 0 y no se puede probar en un
//! anfitrion-- y **la interpretacion de los bytes es del crate**, que si tiene
//! pruebas. Es el mismo reparto que `bmo-net`, y por el mismo motivo: la parte
//! que se equivoca en silencio es la de interpretar.

use bmo_firmware::{recortar, Cabecera, CABECERA_LEN};

// ** Se REEXPORTAN en vez de obligar a quien llame a enlazar `bmo_firmware`
// tambien. El shell pide el censo a este modulo; que ademas tuviera que saber
// de que crate salen los tipos seria contarle una costura que no le importa.
pub use bmo_firmware::{Ivhd, RangoEcam};

/// Cuantas tablas se censan como mucho.
///
/// ** Un tope y no un `Vec`: esto corre en el arranque, sin monton. Y un XSDT
/// con mas de 64 tablas no es una placa generosa, es un puntero que apunta a
/// basura -- que es exactamente lo que la suma de comprobacion existe para
/// cazar, pero el bucle tiene que terminar igual.
const MAX_TABLAS: usize = 64;

/// Lee un tipo de una direccion fisica ya mapeada en el rango de identidad.
///
/// [!] `read_unaligned` siempre: las entries del XSDT son de 8 bytes y empiezan
/// en el offset 36, que no esta alineado a 8.
unsafe fn fis<T: Copy>(addr: u64) -> T {
    unsafe { core::ptr::read_unaligned(addr as *const T) }
}

/// El XSDT a partir del RSDP.
///
/// ** Se exige revision >= 2 por lo mismo que en `madt`: el RSDT de 32 bits es
/// de ACPI 1.0, y esta maquina arranca por UEFI, que obliga a XSDT.
unsafe fn xsdt(rsdp: u64) -> Option<u64> {
    unsafe {
        if fis::<[u8; 8]>(rsdp) != *b"RSD PTR " {
            return None;
        }
        if fis::<u8>(rsdp + 15) < 2 {
            return None;
        }
        let x: u64 = fis(rsdp + 24);
        if x == 0 {
            None
        } else {
            Some(x)
        }
    }
}

/// Lo que se supo de una tabla.
#[derive(Clone, Copy)]
pub struct Fila {
    pub firma: [u8; 4],
    pub largo: u32,
    /// **Paso la suma de comprobacion?** Ver [`Censo::malas`].
    pub creible: bool,
    /// Es AML, o sea un programa que aqui no se ejecuta.
    pub programa: bool,
    pub que_es: &'static str,
}

/// El censo entero.
pub struct Censo {
    filas: [Option<Fila>; MAX_TABLAS],
    cuantas: usize,
    /// Quien fabrico el firmware, del XSDT.
    pub oem: [u8; 6],
    pub oem_tabla: [u8; 8],
}

impl Censo {
    pub fn filas(&self) -> impl Iterator<Item = &Fila> {
        self.filas[..self.cuantas].iter().filter_map(|f| f.as_ref())
    }

    pub fn cuantas(&self) -> usize {
        self.cuantas
    }

    /// **Cuantas no pasaron la suma de comprobacion.**
    ///
    /// *** Este numero es el que dice si el censo se puede creer. Un puntero del
    /// XSDT que apunte a memoria que no es una tabla produce una cabecera con
    /// campos **plausibles** --cuatro bytes cualesquiera parecen una firma-- y
    /// sin la suma el censo se creeria cualquier cosa.
    ///
    /// En una placa sana esto es **cero**. Si no lo es, lo que falla no es la
    /// placa: es el mapeo de esas direcciones fisicas.
    pub fn malas(&self) -> usize {
        self.filas().filter(|f| !f.creible).count()
    }

    /// Cuantas son AML.
    pub fn programas(&self) -> usize {
        self.filas().filter(|f| f.programa).count()
    }
}

/// **Censa las tablas que ofrece el firmware.** Cero escrituras.
///
/// `None` si no hay XSDT que leer -- que es distinto de un censo vacio, igual
/// que en `madt::enumerar`.
pub fn censar(rsdp: u64) -> Option<Censo> {
    if rsdp == 0 {
        return None;
    }
    unsafe {
        let x = xsdt(rsdp)?;

        // La cabecera del propio XSDT: de ahi salen el OEM y el largo que dice
        // cuantas entries trae.
        let cab_bytes = core::slice::from_raw_parts(x as *const u8, CABECERA_LEN);
        let cab = Cabecera::leer(cab_bytes)?;
        if (cab.largo as usize) < CABECERA_LEN {
            return None;
        }

        let mut censo = Censo {
            filas: [None; MAX_TABLAS],
            cuantas: 0,
            oem: cab.oem,
            oem_tabla: cab.oem_tabla,
        };

        let n = (((cab.largo as usize) - CABECERA_LEN) / 8).min(MAX_TABLAS);
        for i in 0..n {
            let t: u64 = fis(x + CABECERA_LEN as u64 + (i * 8) as u64);
            if t == 0 {
                continue;
            }
            let bytes = core::slice::from_raw_parts(t as *const u8, CABECERA_LEN);
            let Some(c) = Cabecera::leer(bytes) else {
                continue;
            };
            // ** La suma se hace sobre la tabla ENTERA, no sobre su cabecera. Es
            // la unica comprobacion que ACPI trae, y es lo que separa "aqui hay
            // una tabla" de "aqui hay memoria que se lee".
            let creible = if (c.largo as usize) >= CABECERA_LEN {
                let todo = core::slice::from_raw_parts(t as *const u8, c.largo as usize);
                bmo_firmware::revisar(todo).is_ok()
            } else {
                false
            };
            censo.filas[censo.cuantas] = Some(Fila {
                firma: c.firma,
                largo: c.largo,
                creible,
                programa: c.es_un_programa(),
                que_es: c.que_es(),
            });
            censo.cuantas += 1;
        }
        Some(censo)
    }
}


// ===================================================================
//  Las dos tablas que se leen de verdad
// ===================================================================

/// Cuantos rangos ECAM se guardan. En una maquina de escritorio hay UNO.
pub const MAX_ECAM: usize = 4;
/// Cuantos IOMMU. En un Ryzen de escritorio hay uno.
pub const MAX_IOMMU: usize = 4;

/// **Busca una tabla por su firma y devuelve sus bytes**, ya comprobada.
///
/// [!] Devuelve `None` si la suma no cuadra, y eso es deliberado: una tabla que
/// no pasa su suma **no es una tabla**, es memoria que se leyo. Devolverla
/// igual dejaria que un puntero malo se convirtiera en una direccion base con
/// pinta de buen dato -- que es el `unwrap_or(0)` de la ley 15, con otra ropa.
unsafe fn tabla_de(rsdp: u64, sig: &[u8; 4]) -> Option<&'static [u8]> {
    unsafe {
        let x = xsdt(rsdp)?;
        let cab = Cabecera::leer(core::slice::from_raw_parts(x as *const u8, CABECERA_LEN))?;
        if (cab.largo as usize) < CABECERA_LEN {
            return None;
        }
        let n = ((cab.largo as usize) - CABECERA_LEN) / 8;
        for i in 0..n.min(MAX_TABLAS) {
            let t: u64 = fis(x + CABECERA_LEN as u64 + (i * 8) as u64);
            if t == 0 {
                continue;
            }
            let c = Cabecera::leer(core::slice::from_raw_parts(t as *const u8, CABECERA_LEN))?;
            if &c.firma != sig {
                continue;
            }
            if (c.largo as usize) < CABECERA_LEN {
                return None;
            }
            let todo = core::slice::from_raw_parts(t as *const u8, c.largo as usize);
            return bmo_firmware::revisar(todo).ok().map(|_| todo);
        }
        None
    }
}

/// **La ventana de configuracion de PCIe en memoria**, del MCFG.
///
/// ** Hoy BMO-X lee PCI por los puertos `0xCF8`/`0xCFC`, y ese camino alcanza
/// **256 bytes** por funcion. PCIe tiene 4096, y los otros 3.840 son las
/// capabilities extendidas -- AER, ATS/PASID, SR-IOV, el estado real del
/// enlace. No se llega a ellas "con mas cuidado": hace falta esta base.
pub fn ecam(rsdp: u64, salida: &mut [RangoEcam]) -> usize {
    if rsdp == 0 {
        return 0;
    }
    unsafe {
        match tabla_de(rsdp, b"MCFG") {
            Some(t) => bmo_firmware::leer_mcfg(t, salida),
            None => 0,
        }
    }
}

/// **Los IOMMU que declara el firmware**, del IVRS.
///
/// *** Lo que esto abre no es rendimiento: es el agujero que hoy tiene el
/// modelo de seguridad. Una capability dice que puede hacer un PROCESO, y **no
/// dice nada de lo que puede hacer un APARATO**: uno con bus-master escribe
/// donde le den la direccion, sin pasar por el kernel ni por las tablas de
/// pagina. Es la mina del PRDT de AHCI, y la IOMMU es lo unico que la desactiva.
///
/// ** Esto solo LEE. Encenderla es otro trabajo y grande -- pero saber que
/// existe y donde vive es lo que permite escribir el plan con un numero.
pub fn iommu(rsdp: u64, salida: &mut [Ivhd]) -> usize {
    if rsdp == 0 {
        return 0;
    }
    unsafe {
        match tabla_de(rsdp, b"IVRS") {
            Some(t) => bmo_firmware::leer_ivrs(t, salida),
            None => 0,
        }
    }
}

/// El `IVinfo` crudo, si hay IVRS.
pub fn ivinfo(rsdp: u64) -> Option<u32> {
    if rsdp == 0 {
        return None;
    }
    unsafe { tabla_de(rsdp, b"IVRS").and_then(bmo_firmware::ivinfo) }
}

/// **Cuenta a CABINA lo que dijo la placa.** Se llama una vez, al arrancar.
pub fn confesar(rsdp: u64) {
    let Some(c) = censar(rsdp) else {
        crate::ring0::cabina::warn("placa", "sin XSDT que leer -- el firmware no lo dio", rsdp);
        return;
    };

    crate::ring0::cabina::count("placa", "tablas que ofrece el firmware", c.cuantas() as u64);

    // ** El OEM va como cuenta de bytes y no como texto porque CABINA no tiene
    // linea de solo texto. El nombre se imprime en el comando `placa`, que si
    // escribe en la consola.
    crate::ring0::cabina::count("placa", "tablas AML que NO se ejecutan", c.programas() as u64);

    // *** LA VENTANA DE PCIe EN MEMORIA. Sin esto, la config de cada funcion se
    // queda en 256 bytes de 4096 -- o sea, sin capabilities extendidas.
    let mut rangos = [RangoEcam { base: 0, segmento: 0, bus_desde: 0, bus_hasta: 0 }; MAX_ECAM];
    let n = ecam(rsdp, &mut rangos);
    if n > 0 {
        crate::ring0::cabina::addr("placa", "config de PCIe en memoria (ECAM)", rangos[0].base);
        crate::ring0::cabina::bytes("placa", "  ...y la ventana mide", rangos[0].mide());
    } else {
        // ** No es un fallo: es una respuesta. Sin MCFG, PCI se lee por puertos
        // y no hay capabilities extendidas -- y saberlo vale mas que suponerlo.
        crate::ring0::cabina::count("placa", "sin MCFG: PCI se queda en 256 B por funcion", 0);
    }

    // *** LA IOMMU. Lo que decide si un aparato con DMA puede escribir donde
    // quiera. Ver `iommu()`.
    let mut ius = [Ivhd { tipo: 0, banderas: 0, largo: 0, id_dispositivo: 0, base_mmio: 0, segmento: 0 }; MAX_IOMMU];
    let m = iommu(rsdp, &mut ius);
    if m > 0 {
        crate::ring0::cabina::count("placa", "IOMMU que declara el firmware", m as u64);
        crate::ring0::cabina::addr("placa", "  ...sus registros en", ius[0].base_mmio);
        if let Some(iv) = ivinfo(rsdp) {
            // Crudo a proposito: los bits se decodifican con la spec delante.
            crate::ring0::cabina::bits("placa", "  ...IVinfo, sin interpretar", iv as u64);
        }
    } else {
        crate::ring0::cabina::warn(
            "placa",
            "[!] sin IVRS: un aparato con DMA no tiene quien lo limite",
            0,
        );
    }

    // *** Y esta es la que hay que mirar. En una placa sana es CERO.
    if c.malas() > 0 {
        crate::ring0::cabina::warn(
            "placa",
            "[!] tablas que NO pasan su suma de comprobacion",
            c.malas() as u64,
        );
    } else {
        crate::ring0::cabina::count("placa", "tablas que no pasan la suma", 0);
    }
}

/// El OEM como texto, para quien pueda imprimirlo.
pub fn oem_texto(c: &Censo) -> (&str, &str) {
    (recortar(&c.oem), recortar(&c.oem_tabla))
}
