//! **La MADT: el censo de nucleos que da el firmware.**
//!
//! [carril]  ROJO      el censo de nucleos: el bring-up de SMP cuelga de esta tabla
//!
//! === Por que existe este modulo ===
//!
//! `plat::smp` despertaba suponiendo que los APIC IDs son `0..hilos-1`. Es
//! cierto en un Zen 3 de un solo CCD y **es una suposicion** en cualquier otra
//! cosa -- y lo peor de una suposicion asi es como falla: a un ID que no existe
//! la IPI se va al vacio, y un nucleo real con un ID fuera del rango no se
//! llama nunca. **Las dos cosas se ven igual desde fuera**: *"faltan nucleos"*.
//!
//! Y habia un sitio que *parecia* el censo y no lo era: `Topology::cpus` se
//! rellena con `[bsp; 64]` --doce copias del mismo nucleo-- y quedo marcado
//! `#[deprecated]` por eso mismo.
//!
//! El censo de verdad lo da el firmware en la **MADT** (`APIC`), en sus entries
//! de tipo 0 (*Processor Local APIC*) y tipo 9 (*Local x2APIC*). No se deduce de
//! CPUID: **es lo que la placa dice que hay**, que es justo lo que hace falta
//! para saber a quien llamar.
//!
//! === Donde encaja, y por que no toca el arranque ===
//!
//! `s2_mem` ya localiza la MADT y **ya recorre sus entries** -- pero solo mira
//! el tipo 1 (I/O APIC) y se salta el resto. Aqui no se cambia s2: el kernel
//! tiene el `rsdp` en el `BootContext` y puede llegar solo. Asi el camino de
//! arranque **no se toca en absoluto** y esto solo corre cuando alguien escribe
//! `smp`.
//!
//! === Todo se lee por el PHYSMAP ===
//!
//! Las tablas de ACPI viven en direcciones fisicas altas --tipicamente justo por
//! debajo de 4 GiB--. Se llega por `HIGH_MEM_BASE + fisica`, igual que hacen
//! `timer.rs` y `disk.rs`, y no por la direccion fisica a pelo: el identity map
//! documentado del kernel es `0..32 MiB` y fiarse de que hoy alcance mas lejos
//! es construir sobre algo que nadie prometio.
//!
//! Y **todas las lecturas son `read_unaligned`**: las entries del XSDT son de 8
//! bytes empezando en el offset 36, que no esta alineado a 8. Leerlas alineadas
//! es correcto en x86 por casualidad y es UB en Rust -- la clase de cosa que
//! funciona hasta que el optimizador decide otra cosa.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::ring0::mm::HIGH_MEM_BASE;

/// El `rsdp` que trajo el `BootContext`, guardado una vez.
///
/// * Esta aqui y no se pasa por parametro porque el que acabo necesitandolo es
/// **el manejador de syscall**, que no tiene el `BootContext` delante. La
/// alternativa era arrastrar el contexto por media docena de firmas para
/// entregar un numero que no cambia nunca.
static RSDP: AtomicU64 = AtomicU64::new(0);

/// Lo llama `phase::main` una vez. Solo guarda un numero: no lee la tabla, no
/// toca hardware, y por eso puede estar en el camino de arranque sin discusion.
/// El `rsdp` que se guardo al arrancar, para quien lo necesite y no tenga el
/// `BootContext` delante.
///
/// ** Se expone en vez de guardarlo por segunda vez en otro modulo: dos sitios
/// con el mismo puntero se separan el dia que alguien toca uno.
pub fn rsdp_guardado() -> u64 {
    RSDP.load(Ordering::SeqCst)
}

pub fn recordar(rsdp: u64) {
    RSDP.store(rsdp, Ordering::SeqCst);
}

/// El censo, con el `rsdp` que se guardo al arrancar.
pub fn censo() -> Option<Censo> {
    enumerar(RSDP.load(Ordering::SeqCst))
}

/// Hasta cuantos nucleos se apuntan. El 5600X trae 12; 64 cubre cualquier cosa
/// que quepa en la mascara de 32 bits de `smp` y bastante mas.
pub const MAX: usize = 64;

/// Lo que dice el firmware que hay.
#[derive(Clone, Copy)]
pub struct Censo {
    ids: [u32; MAX],
    n: usize,
    /// Los que la MADT lista pero marca como **no habilitados**. No se llaman,
    /// y se cuentan porque un hueco sin explicar es peor que un numero.
    apagados: usize,
}

impl Censo {
    /// Los APIC IDs **habilitados**, en el orden en que los da el firmware.
    pub fn ids(&self) -> &[u32] {
        &self.ids[..self.n]
    }
    /// Cuantos listaba la tabla y no se pueden usar.
    pub fn apagados(&self) -> usize {
        self.apagados
    }
}

/// Lee un `T` de una direccion **fisica**, por el physmap y sin alinear.
unsafe fn fis<T>(fisica: u64) -> T {
    unsafe { core::ptr::read_unaligned((HIGH_MEM_BASE + fisica) as *const T) }
}

/// Los cuatro bytes de firma de una tabla ACPI.
unsafe fn firma(tabla: u64) -> [u8; 4] {
    unsafe { fis::<[u8; 4]>(tabla) }
}

/// Longitud declarada en la cabecera SDT (offset 4).
unsafe fn largo(tabla: u64) -> u32 {
    unsafe { fis::<u32>(tabla + 4) }
}

/// El XSDT a partir del RSDP.
///
/// Se exige revision >= 2: el RSDT de 32 bits es de ACPI 1.0 y esta maquina
/// arranca por UEFI, que obliga a XSDT. Si algun dia hace falta el otro camino,
/// se anade **con su motivo**, no por si acaso.
unsafe fn xsdt(rsdp: u64) -> Option<u64> {
    unsafe {
        if fis::<[u8; 8]>(rsdp) != *b"RSD PTR " {
            return None;
        }
        let revision: u8 = fis(rsdp + 15);
        if revision < 2 {
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

/// Busca una tabla por su firma dentro del XSDT.
unsafe fn find_by(xsdt_addr: u64, sig: &[u8; 4]) -> Option<u64> {
    unsafe {
        let len = largo(xsdt_addr) as u64;
        if len < 36 {
            return None;
        }
        let n = (len - 36) / 8;
        for i in 0..n {
            // * `read_unaligned`: estas entries de 8 bytes empiezan en el
            // offset 36, que no esta alineado a 8.
            let t: u64 = fis(xsdt_addr + 36 + i * 8);
            if t != 0 && firma(t) == *sig {
                return Some(t);
            }
        }
        None
    }
}

/// **Enumera los APIC IDs que declara el firmware.**
///
/// `rsdp` sale del `BootContext`. Devuelve `None` si no hay tabla que leer --y
/// entonces quien llame decide que hacer, que es distinto de devolver una lista
/// vacia y dejar que parezca *"esta maquina tiene cero nucleos"*.
///
/// # *** EL PASEO POR LAS ENTRADAS YA NO ESTA AQUI (2026-08-25, C6)
///
/// Vivia entero en este fichero, y por eso **tenia cero pruebas**: `madt.rs`
/// esta dentro del kernel, que es `no_std` para una maquina sin sistema
/// operativo, y ahi no corre un test.
///
/// Y el paseo es justo la parte peligrosa: recorre entradas cuya longitud la
/// escribe **la placa**, en el arranque, y su respuesta decide a que APIC IDs se
/// les manda INIT-SIPI-SIPI -- la unica operacion de todo el sistema que cambia
/// el hardware de forma que no se deshace sin reiniciar.
///
/// ```text
///    lo que se queda AQUI    llegar a la tabla: RSDP -> XSDT -> APIC
///                            eso pide leer memoria fisica, y es del kernel
///    lo que se fue           interpretar sus bytes -> `bmo_firmware::leer_madt`
///                            eso es una funcion pura, y ahora tiene 7 pruebas
/// ```
///
/// ** Es el mismo reparto que ya tenian MCFG e IVRS en ese crate. La MADT era la
/// unica de las tres que se habia quedado dentro -- y la mas peligrosa.
///
/// [!] Y C6 decia que esta casilla era la mas cara porque *"un informe HID malo
/// hay que INYECTARLO"*. Para la MADT era falso: **lo que impedia probarla no era
/// el aparato, era el sitio.**
pub fn enumerar(rsdp: u64) -> Option<Censo> {
    if rsdp == 0 {
        return None;
    }
    let (madt, len) = unsafe {
        let x = xsdt(rsdp)?;
        let madt = find_by(x, b"APIC")?;
        let len = largo(madt) as usize;
        if len < 44 {
            return None;
        }
        (madt, len)
    };

    // ** LA TABLA COMO REBANADA, y de ahi en adelante no hay `unsafe`.
    //
    // Un `&[u8]` no pide alineacion, asi que esto no reintroduce el problema que
    // `read_unaligned` resolvia: lo que estaba desalineado eran las entradas de
    // 8 bytes del XSDT, y esas siguen leyendose arriba.
    let bytes = unsafe { core::slice::from_raw_parts((HIGH_MEM_BASE + madt) as *const u8, len) };

    let mut declarados = [bmo_firmware::NucleoDeclarado { apic: 0, habilitado: false }; MAX];
    let n = bmo_firmware::leer_madt(bytes, &mut declarados);

    let mut c = Censo { ids: [0; MAX], n: 0, apagados: 0 };
    for d in &declarados[..n] {
        if d.habilitado {
            c.ids[c.n] = d.apic;
            c.n += 1;
        } else {
            // Se cuentan y no se llaman. Un hueco sin explicar es peor que un
            // numero: `smp` lo dice por CABINA.
            c.apagados += 1;
        }
    }
    if c.n == 0 {
        None
    } else {
        Some(c)
    }
}
