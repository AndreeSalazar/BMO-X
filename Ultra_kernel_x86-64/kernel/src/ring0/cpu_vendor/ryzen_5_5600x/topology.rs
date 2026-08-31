//! CPU topology for the Ryzen 5 5600X (1 CCD, 1 CCX, 6C/12T).
//!
//! [carril]  VERDE     1 CCD, 1 CCX, 6C/12T: una tabla
//!
//! Recovers the legacy `topology.rs` from the deleted
//! `crates_Personal/ring0/cpu_vendor_profile/.../topology.rs`,
//! adapted for in-kernel use.
//!
//! Uses CPUID leaves 0x0B (extended topology) and 0x8000001E
//! (extended APIC ID) to determine SMT, cores, CCX, CCD, APIC IDs.

use super::cpuid::cpuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuId {
    pub apic_id: u8,
    pub thread: u8,    // 0 or 1 on Zen 3 (SMT)
    pub core: u8,      // 0..=5 on the 5600X
    pub ccd: u8,       // 0 on the 5600X (single CCD)
    pub ccx: u8,       // 0 on the 5600X (single CCX)
}

impl CpuId {
    /// Linear index 0..=11 (BSP=0). Useful for per-CPU tables.
    pub fn linear(&self) -> u8 { self.ccd * 12 + self.core * 2 + self.thread }
}

#[derive(Debug, Clone, Copy)]
pub struct Topology {
    pub bsp: CpuId,
    pub cpus: [CpuId; 64],
    pub cpu_count: u32,
    pub total_threads: u32,
    pub total_cores: u32,
    pub total_ccxs: u32,
    pub total_ccds: u32,

    // -- ** LOS TRES CAMPOS QUE EXISTEN PORQUE UN 6/12 DIJO 27/54 (2026-08-25)
    //
    // El 2026-08-25 el panel del escritorio contesto `27 fisicos / 54 logicos`
    // en un Ryzen 5 5600X, que es 6/12. El dia anterior, con el MISMO codigo,
    // habia contestado 12. O sea que el numero **no era reproducible** y nadie
    // podia saberlo, porque solo lo miraba un testigo.
    //
    // *** Y LA COMPROBACION QUE HABIA NO PODIA FALLAR NUNCA. `plat::smp`
    // validaba la topologia con `hilos != nucleos * 2`, y eso pasa siempre --
    // con 54 y 27 tambien-- porque `total_cores` estaba DEFINIDO como
    // `total_threads / 2` unas lineas mas abajo:
    //
    // ```text
    //    nucleos := hilos / 2     y luego se comprueba que  hilos == nucleos * 2
    // ```
    //
    // Un testigo que solo puede confirmarse a si mismo no es un testigo.
    /// **Hilos por nucleo, MEDIDOS** (`CPUID.0B.0:ECX[15:8]`). Antes esto era
    /// la constante 2 escondida en una division. Si el silicio dice 1 --SMT
    /// apagado en la BIOS-- ahora se entera todo el mundo.
    pub hilos_por_nucleo: u32,
    /// Lo que contesta el testigo VIEJO (`CPUID.1:EBX[23:16]`), guardado a
    /// proposito aunque ya no mande. Sin el no hay careo: una sola fuente no
    /// puede discrepar consigo misma.
    pub hilos_heredado: u32,
    /// **Los dos testigos no dicen lo mismo.** Quien lo lea tiene que
    /// desconfiar del numero, no elegir uno.
    pub discrepan: bool,
}

impl Topology {
    /// [!] **ESTE ARRAY NO ESTA ENUMERADO. Son `cpu_count` copias del BSP.**
    ///
    /// Se rellena con `[bsp; 64]` y nunca se toca despues, asi que `cpus()`
    /// devuelve doce veces el mismo nucleo con el mismo `apic_id`. Tiene forma
    /// de censo de CPUs y no lo es -- la clase de campo que engana a quien lo
    /// lee, porque *parece* un dato y es un relleno.
    ///
    /// **Donde esta el censo de verdad**: en la tabla **MADT** de ACPI, en sus
    /// entries de tipo 0 (*Processor Local APIC*), que traen el APIC ID de cada
    /// hilo. `s2_mem` ya localiza la MADT --`find_table(xsdt, b"APIC")`-- pero
    /// **solo le lee el campo de la direccion base del LAPIC** (offset 36) y no
    /// recorre sus entries. Enumerarlas es el trabajo pendiente.
    ///
    /// Mientras tanto, `plat::smp` despierta suponiendo APIC IDs `0..hilos-1`,
    /// que es lo correcto en un Zen 3 de un solo CCD y **una suposicion** en
    /// cualquier otra cosa. Dicho en `smp/mod.rs`.
    #[deprecated(note = "no enumerado: son copias del BSP. El censo real esta en la MADT")]
    pub fn cpus(&self) -> &[CpuId] { &self.cpus[..self.cpu_count as usize] }

    /// Buscar aqui solo puede encontrar al BSP. Ver [`Topology::cpus`].
    #[deprecated(note = "el array no esta enumerado; esto solo puede encontrar al BSP")]
    pub fn find_by_apic(&self, apic: u32) -> Option<&CpuId> {
        #[allow(deprecated)]
        self.cpus().iter().find(|c| c.apic_id as u32 == apic)
    }
}

/// **Preguntarle al silicio cuantos hilos y cuantos nucleos hay.**
///
/// Rellena solo el BSP --el CPU en el que corremos-- porque enumerar los demas
/// pide el bring-up de SMP, y el censo de verdad esta en la MADT. Ver
/// [`Topology::cpus`].
///
/// # *** LO QUE ESTA FUNCION HACIA MAL, Y COSTO UN 27/54 (2026-08-25)
///
/// Leia la hoja 0x0B **dos veces** y tiraba las dos respuestas al suelo:
///
/// ```text
///    let _smt_count  = ((smt_ecx  >> 8) & 0xFF) as u8;   // descartado
///    let _core_count = ((core_ecx >> 8) & 0xFF) as u8;   // descartado
/// ```
///
/// Y luego cogia el conteo de la hoja 1, que es la heredada, y **dividia entre
/// dos**. O sea que el segundo testigo ya estaba dentro de la funcion, ya
/// pagado, y se descartaba en la linea siguiente.
///
/// [!] Y las dos lineas descartadas ADEMAS estaban mal: en la hoja 0x0B el
/// conteo vive en **`EBX[15:0]`**, y `ECX[15:8]` es el **TIPO DE NIVEL**
/// (1 = SMT, 2 = Core). Leian el tipo creyendo que leian una cuenta. Que
/// estuvieran descartadas es lo unico que impidio que eso se notara -- **un
/// dato que no se usa no se comprueba nunca**.
///
/// ## Los tres numeros, y de donde sale cada uno
///
/// ```text
///    hilos por nucleo   CPUID.0B.0:EBX[15:0]     2 en Zen 3
///    hilos del paquete  CPUID.0B.1:EBX[15:0]     12 en el 5600X
///    (el testigo viejo) CPUID.1:EBX[23:16]       ya no manda: CAREA
/// ```
///
/// **`total_cores` ya no se divide entre dos: se divide entre lo que el silicio
/// diga que hay por nucleo.** Con SMT apagado en la BIOS eso es 1, y antes esa
/// maquina habria informado la mitad de sus nucleos sin que nada chistara.
pub fn detect_bsp() -> Topology {
    // Nivel SMT (sub-hoja 0). EBX[15:0] = procesadores logicos en este nivel,
    // o sea hilos por nucleo. ECX[15:8] = tipo de nivel, y tiene que ser 1.
    let (_, smt_ebx, smt_ecx, smt_edx) = cpuid(0x0B, 0);
    let tipo_smt = (smt_ecx >> 8) & 0xFF;
    let hilos_por_nucleo_medido = smt_ebx & 0xFFFF;

    // Nivel NUCLEO (sub-hoja 1). EBX[15:0] = logicos del paquete entero.
    let (_, core_ebx, core_ecx, _) = cpuid(0x0B, 1);
    let tipo_core = (core_ecx >> 8) & 0xFF;
    let hilos_paquete = core_ebx & 0xFFFF;

    // El x2APIC ID vive en EDX, no en EAX -- EAX[4:0] es el desplazamiento del
    // nivel. Esto tambien estaba mal y no se notaba porque no lo lee nadie.
    let apic_id = smt_edx as u8;

    // El testigo heredado. Ya no manda; existe para poder DISCREPAR.
    let hilos_heredado = hilos_hoja1();

    // * La hoja 0x0B manda SI contesta algo con sentido. Si no --CPU viejo, o
    // un firmware que la deja en blanco-- se cae al testigo heredado, pero
    // diciendolo: `hilos_por_nucleo = 0` es la senal de "no se ha medido", y
    // nunca se convierte en un 2 supuesto por el camino.
    let hoja_b_vale = tipo_smt == 1 && tipo_core == 2 && hilos_por_nucleo_medido > 0 && hilos_paquete > 0;

    let (total_threads, hilos_por_nucleo) = if hoja_b_vale {
        (hilos_paquete, hilos_por_nucleo_medido)
    } else {
        (hilos_heredado, 0)
    };

    // ** La division protegida, y el 0 NO es un caso raro: es lo que se guarda
    // cuando la hoja 0x0B no valio. Antes aqui habia un `/ 2` literal, y por eso
    // `total_cores` no era una medida sino una opinion.
    let total_cores = if hilos_por_nucleo > 0 {
        total_threads / hilos_por_nucleo
    } else {
        total_threads
    };

    // 1 CCD y 1 CCX es lo unico que sigue siendo una declaracion del perfil y
    // no una medida. Es cierto en el 5600X y esta escrito en la cabecera.
    let thread = if hilos_por_nucleo > 1 { apic_id & 1 } else { 0 };
    let core = if hilos_por_nucleo > 1 { (apic_id >> 1) & 0x07 } else { apic_id & 0x07 };

    let bsp = CpuId { apic_id, thread, core, ccd: 0, ccx: 0 };
    let mut cpus = [bsp; 64];
    cpus[0] = bsp;

    Topology {
        bsp,
        cpus,
        cpu_count: total_threads.min(64),
        total_threads,
        total_cores,
        total_ccxs: 1,
        total_ccds: 1,
        hilos_por_nucleo,
        hilos_heredado,
        // *** EL CAREO. Los dos testigos miden lo mismo por caminos distintos,
        // asi que discrepar es informacion: o el firmware apago algo, o uno de
        // los dos se leyo mal. Lo que no puede pasar es que nadie lo mire.
        discrepan: hoja_b_vale && hilos_paquete != hilos_heredado,
    }
}

/// El testigo VIEJO: `CPUID.1:EBX[23:16]`, el conteo de logicos heredado.
///
/// [!] Solo es valido si `CPUID.1:EDX[28]` (HTT) esta puesto, y **eso no se
/// comprobaba**. Se conserva porque para carear vale igual: si contesta una
/// barbaridad, la barbaridad es justo lo que hay que ver.
fn hilos_hoja1() -> u32 {
    let (_, ebx, _, _) = cpuid(1, 0);
    (ebx >> 16) & 0xFF
}
