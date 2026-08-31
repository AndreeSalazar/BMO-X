//! **`placa` -- lo que el firmware le cuenta a BMO-X.**
//!
//! [carril]  VERDE     cuenta lo que el firmware declara
//!
//! ## Por que es un fichero y no un trozo de `hardware.rs` (L6a, L6b)
//!
//! Por las dos razones, y las dos cuentan:
//!
//! ** La de L6b, que es la buena: contesta una pregunta distinta. `hardware.rs`
//! ensena APARATOS --el disco, la red, el sonido, los nucleos-- y esto ensena lo
//! que la PLACA dice de si misma. Son dos censos, y el segundo no mira ningun
//! aparato: mira una tabla que dejo el firmware en memoria.
//!
//! ** Y la de L6a, que es la que forzo el momento: al entrar `placa`,
//! `hardware.rs` cruzo las **1.000 lineas**, y desde el 2026-08-24 un fichero
//! de **Ring 0** que las cruza no puede sellarse en la linea base -- no hay
//! `--motivo` que valga. La regla se escribio esa manana y **lo primero que
//! caso fue el trabajo de esa misma manana**.
//!
//! Que una regla se aplique a quien la escribe el dia que la escribe es la
//! mejor prueba de que no es decorativa.

use super::super::phase::s_log;
use crate::ring0::plat::placa;

/// **`placa` -- que le cuenta la placa base a BMO-X.**
///
/// ## Por que este comando existe, y por que solo LEE
///
/// Es el paso 0 del firmware, y tiene la misma forma que el de la red: cero
/// escrituras, respuestas **predecibles**, y se compara contra lo que dice el
/// otro sistema en la misma maquina. Predecir, leer, comparar.
///
/// *** Y la fila que hay que mirar no es cuantas tablas hay: es **cuantas no
/// pasan su suma de comprobacion**. En una placa sana es cero. Si no lo es, lo
/// que falla no es la placa -- es el mapeo de esas direcciones fisicas, y eso es
/// un fallo del kernel disfrazado de firmware raro.
pub(crate) fn shell_placa() {
    use crate::ring0::plat::placa;

    let rsdp = crate::ring0::plat::madt::rsdp_guardado();
    let Some(c) = placa::censar(rsdp) else {
        s_log("[placa] sin XSDT que leer: el firmware no dio un RSDP de ACPI 2.0+");
        return;
    };

    fn txt(b: &mut [u8; 80], o: &mut usize, t: &str) {
        for &ch in t.as_bytes() { if *o < b.len() { b[*o] = ch; *o += 1; } }
    }
    fn dec(b: &mut [u8; 80], o: &mut usize, mut v: u64) {
        let mut tmp = [0u8; 20];
        let mut i = 0;
        if v == 0 { tmp[0] = b'0'; i = 1; }
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if *o < b.len() { b[*o] = tmp[i]; *o += 1; } }
    }
    /// ** Una direccion se escribe en HEX y no en decimal, y no es estilo: el
    /// mapa de memoria de una placa esta alineado a potencias de dos, asi que
    /// en hex los ceros del final DICEN el tamano de la ventana. En decimal
    /// `4026531840` no dice nada; `0xF0000000` se lee de un vistazo.
    fn hex(b: &mut [u8; 80], o: &mut usize, v: u64) {
        const D: &[u8; 16] = b"0123456789ABCDEF";
        let mut visto = false;
        let mut i = 60i32;
        while i >= 0 {
            let n = ((v >> i) & 0xF) as usize;
            if n != 0 || visto || i == 0 {
                visto = true;
                if *o < b.len() { b[*o] = D[n]; *o += 1; }
            }
            i -= 4;
        }
    }

    // Quien fabrico este firmware, dicho por el propio XSDT.
    {
        let (oem, modelo) = placa::oem_texto(&c);
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[placa] firmware de ");
        txt(&mut b, &mut o, oem);
        txt(&mut b, &mut o, ", tabla ");
        txt(&mut b, &mut o, modelo);
        if let Ok(t) = core::str::from_utf8(&b[..o]) { s_log(t); }
    }

    // Una linea por tabla: la firma, lo que mide, y QUE ES en castellano.
    for f in c.filas() {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "  ");
        // *** La marca va DELANTE del nombre, no detras: en una lista de
        // veinte lineas, lo que se escanea con la vista es la primera columna.
        txt(&mut b, &mut o, if !f.creible {
            "[!] "
        } else if f.programa {
            " AML "
        } else {
            "     "
        });
        if let Ok(t) = core::str::from_utf8(&f.firma) { txt(&mut b, &mut o, t); }
        txt(&mut b, &mut o, "  ");
        dec(&mut b, &mut o, f.largo as u64);
        txt(&mut b, &mut o, " B  ");
        txt(&mut b, &mut o, f.que_es);
        if let Ok(t) = core::str::from_utf8(&b[..o]) { s_log(t); }
    }


    // === LA VENTANA DE PCIe EN MEMORIA ==============================
    //
    // ** Es la fila que mas desbloquea de todo el censo. Hoy PCI se lee por los
    // puertos 0xCF8/0xCFC y eso alcanza 256 bytes por funcion; PCIe tiene 4096,
    // y los otros 3.840 son las capabilities extendidas. No se llega a ellas
    // "con mas cuidado": hace falta esta direccion.
    {
        let mut r = [placa::RangoEcam { base: 0, segmento: 0, bus_desde: 0, bus_hasta: 0 };
            placa::MAX_ECAM];
        let n = placa::ecam(rsdp, &mut r);
        if n == 0 {
            s_log("[placa] sin MCFG: PCI se queda en 256 B por funcion, sin caps extendidas");
        } else {
            for i in 0..n {
                let mut b = [0u8; 80];
                let mut o = 0usize;
                txt(&mut b, &mut o, "[placa] PCIe config en 0x");
                hex(&mut b, &mut o, r[i].base);
                txt(&mut b, &mut o, "  buses ");
                dec(&mut b, &mut o, r[i].bus_desde as u64);
                txt(&mut b, &mut o, "..");
                dec(&mut b, &mut o, r[i].bus_hasta as u64);
                txt(&mut b, &mut o, "  (4096 B por funcion)");
                if let Ok(t) = core::str::from_utf8(&b[..o]) { s_log(t); }
            }
        }
    }

    // === LA IOMMU ===================================================
    //
    // *** Lo que esta fila decide no es rendimiento: es si un aparato con DMA
    // puede escribir donde quiera. Una capability dice que puede hacer un
    // PROCESO y no dice NADA de lo que puede hacer un APARATO.
    {
        let mut v = [placa::Ivhd {
            tipo: 0, banderas: 0, largo: 0, id_dispositivo: 0, base_mmio: 0, segmento: 0,
        }; placa::MAX_IOMMU];
        let m = placa::iommu(rsdp, &mut v);
        if m == 0 {
            s_log("[placa] [!] sin IVRS: nada limita adonde escribe un aparato con DMA");
        } else {
            for i in 0..m {
                let mut b = [0u8; 80];
                let mut o = 0usize;
                txt(&mut b, &mut o, "[placa] IOMMU tipo 0x");
                hex(&mut b, &mut o, v[i].tipo as u64);
                txt(&mut b, &mut o, "  registros en 0x");
                hex(&mut b, &mut o, v[i].base_mmio);
                if let Ok(t) = core::str::from_utf8(&b[..o]) { s_log(t); }
            }
            s_log("[placa]     la hay y se sabe donde. ENCENDERLA es otro trabajo");
        }
    }


    // === LO QUE ECAM DESBLOQUEA, SOBRE UN APARATO DE VERDAD ==========
    //
    // ** Se elige la NIC y no un aparato cualquiera porque es del que ya se sabe
    // todo: bus, dispositivo, funcion, MAC y enlace, verificados en metal. Si la
    // lista de capabilities de ESE aparato sale creible, ECAM sirve.
    if !crate::ring0::dev::pci::hay_ecam() {
        s_log("[placa] sin ECAM careado: las caps extendidas son INALCANZABLES, no ilegibles");
    } else {
        let (_, _, bus, dev, fun, _) = crate::ring0::dev::net::donde();
        let mut caps = [crate::ring0::dev::pci::CapExt { id: 0, version: 0, offset: 0 }; 16];
        let n = crate::ring0::dev::pci::caps_extendidas(bus, dev, fun, &mut caps);
        if n == 0 {
            s_log("[placa] la NIC no trae capabilities extendidas");
        } else {
            let mut b = [0u8; 80];
            let mut o = 0usize;
            txt(&mut b, &mut o, "[placa] la NIC trae ");
            dec(&mut b, &mut o, n as u64);
            txt(&mut b, &mut o, " caps extendidas (offset >= 0x100, fuera de los puertos)");
            if let Ok(t) = core::str::from_utf8(&b[..o]) { s_log(t); }

            let mut hay_acs = false;
            for c in caps[..n].iter() {
                let mut b = [0u8; 80];
                let mut o = 0usize;
                txt(&mut b, &mut o, "        0x");
                hex(&mut b, &mut o, c.id as u64);
                txt(&mut b, &mut o, " @0x");
                hex(&mut b, &mut o, c.offset as u64);
                txt(&mut b, &mut o, "  ");
                txt(&mut b, &mut o, crate::ring0::dev::pci::nombre_cap_ext(c.id));
                if let Ok(t) = core::str::from_utf8(&b[..o]) { s_log(t); }
                if c.id == 0x000D {
                    hay_acs = true;
                }
            }

            // *** ACS, y por que se dice aunque hoy no se use.
            //
            // ** Sin ACS, dos funciones detras del mismo puente pueden hacer DMA
            // la una contra la otra **sin que la IOMMU se entere**. Encender la
            // IOMMU sin mirar esto es poner una puerta en una habitacion que
            // tiene otra puerta -- y ese dato se sabe HOY, gratis, antes de
            // escribir una linea del driver de IOMMU.
            if !hay_acs {
                s_log("[placa] [!] sin ACS: dos funciones del mismo puente podrian");
                s_log("[placa]     hablarse saltandose la IOMMU. Importa el dia que se encienda");
            }
        }
    }

    // El resumen, y la unica cifra que puede ser mala.
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[placa] ");
        dec(&mut b, &mut o, c.cuantas() as u64);
        txt(&mut b, &mut o, " tablas, ");
        dec(&mut b, &mut o, c.programas() as u64);
        txt(&mut b, &mut o, " son AML (no se ejecutan), ");
        dec(&mut b, &mut o, c.malas() as u64);
        txt(&mut b, &mut o, " sin suma valida");
        if let Ok(t) = core::str::from_utf8(&b[..o]) { s_log(t); }
    }
    if c.malas() > 0 {
        s_log("[placa] [!] una tabla sin suma valida NO es una placa rara:");
        s_log("[placa]     es memoria que no se leyo bien. Mira el mapeo.");
    }
    // ** Se dice SIEMPRE, no solo cuando hay AML: es la linea que explica en que
    // se diferencia este sistema de uno generalista, y una explicacion que solo
    // sale a veces no la lee nadie.
    s_log("[placa] el AML es un PROGRAMA de la placa. BMO-X se perfila: no lo ejecuta");
}
