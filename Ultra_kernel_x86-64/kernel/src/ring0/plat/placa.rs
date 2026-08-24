//! **EL CENSO DE LA PLACA: que tablas ofrece el firmware, y cual se cree.**
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
