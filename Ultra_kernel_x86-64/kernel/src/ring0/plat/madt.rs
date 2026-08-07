//! **La MADT: el censo de núcleos que da el firmware.**
//!
//! ═══ Por qué existe este módulo ═══
//!
//! `plat::smp` despertaba suponiendo que los APIC IDs son `0..hilos-1`. Es
//! cierto en un Zen 3 de un solo CCD y **es una suposición** en cualquier otra
//! cosa — y lo peor de una suposición así es cómo falla: a un ID que no existe
//! la IPI se va al vacío, y un núcleo real con un ID fuera del rango no se
//! llama nunca. **Las dos cosas se ven igual desde fuera**: *"faltan núcleos"*.
//!
//! Y había un sitio que *parecía* el censo y no lo era: `Topology::cpus` se
//! rellena con `[bsp; 64]` —doce copias del mismo núcleo— y quedó marcado
//! `#[deprecated]` por eso mismo.
//!
//! El censo de verdad lo da el firmware en la **MADT** (`APIC`), en sus entradas
//! de tipo 0 (*Processor Local APIC*) y tipo 9 (*Local x2APIC*). No se deduce de
//! CPUID: **es lo que la placa dice que hay**, que es justo lo que hace falta
//! para saber a quién llamar.
//!
//! ═══ Dónde encaja, y por qué no toca el arranque ═══
//!
//! `s2_mem` ya localiza la MADT y **ya recorre sus entradas** — pero sólo mira
//! el tipo 1 (I/O APIC) y se salta el resto. Aquí no se cambia s2: el kernel
//! tiene el `rsdp` en el `BootContext` y puede llegar solo. Así el camino de
//! arranque **no se toca en absoluto** y esto sólo corre cuando alguien escribe
//! `smp`.
//!
//! ═══ Todo se lee por el PHYSMAP ═══
//!
//! Las tablas de ACPI viven en direcciones físicas altas —típicamente justo por
//! debajo de 4 GiB—. Se llega por `HIGH_MEM_BASE + física`, igual que hacen
//! `timer.rs` y `disk.rs`, y no por la dirección física a pelo: el identity map
//! documentado del kernel es `0..32 MiB` y fiarse de que hoy alcance más lejos
//! es construir sobre algo que nadie prometió.
//!
//! Y **todas las lecturas son `read_unaligned`**: las entradas del XSDT son de 8
//! bytes empezando en el offset 36, que no está alineado a 8. Leerlas alineadas
//! es correcto en x86 por casualidad y es UB en Rust — la clase de cosa que
//! funciona hasta que el optimizador decide otra cosa.

use crate::ring0::mm::HIGH_MEM_BASE;

/// Hasta cuántos núcleos se apuntan. El 5600X trae 12; 64 cubre cualquier cosa
/// que quepa en la máscara de 32 bits de `smp` y bastante más.
pub const MAX: usize = 64;

/// Lo que dice el firmware que hay.
#[derive(Clone, Copy)]
pub struct Censo {
    ids: [u32; MAX],
    n: usize,
    /// Los que la MADT lista pero marca como **no habilitados**. No se llaman,
    /// y se cuentan porque un hueco sin explicar es peor que un número.
    apagados: usize,
}

impl Censo {
    /// Los APIC IDs **habilitados**, en el orden en que los da el firmware.
    pub fn ids(&self) -> &[u32] {
        &self.ids[..self.n]
    }
    /// Cuántos listaba la tabla y no se pueden usar.
    pub fn apagados(&self) -> usize {
        self.apagados
    }
}

/// Lee un `T` de una dirección **física**, por el physmap y sin alinear.
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
/// Se exige revisión ≥ 2: el RSDT de 32 bits es de ACPI 1.0 y esta máquina
/// arranca por UEFI, que obliga a XSDT. Si algún día hace falta el otro camino,
/// se añade **con su motivo**, no por si acaso.
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
unsafe fn buscar(xsdt_addr: u64, sig: &[u8; 4]) -> Option<u64> {
    unsafe {
        let len = largo(xsdt_addr) as u64;
        if len < 36 {
            return None;
        }
        let n = (len - 36) / 8;
        for i in 0..n {
            // ★ `read_unaligned`: estas entradas de 8 bytes empiezan en el
            // offset 36, que no está alineado a 8.
            let t: u64 = fis(xsdt_addr + 36 + i * 8);
            if t != 0 && firma(t) == *sig {
                return Some(t);
            }
        }
        None
    }
}

// Tipos de entrada de la MADT que nos importan.
const ENTRADA_LAPIC: u8 = 0;
const ENTRADA_X2APIC: u8 = 9;
/// Bit 0 de las banderas: el núcleo se puede usar. Bit 1 es *online capable*
/// (se podría enchufar en caliente), y a ése **no se le manda un SIPI**.
const HABILITADO: u32 = 1;

/// **Enumera los APIC IDs que declara el firmware.**
///
/// `rsdp` sale del `BootContext`. Devuelve `None` si no hay tabla que leer —y
/// entonces quien llame decide qué hacer, que es distinto de devolver una lista
/// vacía y dejar que parezca *"esta máquina tiene cero núcleos"*.
pub fn enumerar(rsdp: u64) -> Option<Censo> {
    if rsdp == 0 {
        return None;
    }
    unsafe {
        let x = xsdt(rsdp)?;
        let madt = buscar(x, b"APIC")?;
        let len = largo(madt) as u64;
        if len < 44 {
            return None;
        }

        let mut c = Censo { ids: [0; MAX], n: 0, apagados: 0 };
        // La cabecera son 36 bytes de SDT + 4 de la dirección del LAPIC + 4 de
        // banderas. Las entradas empiezan en 44.
        let mut off = 44u64;
        while off + 2 <= len {
            let tipo: u8 = fis(madt + off);
            let elen: u8 = fis(madt + off + 1);
            // Una entrada de longitud 0 haría un bucle infinito sobre una tabla
            // corrupta. Se corta y se dice por el que llama, no aquí.
            if elen < 2 {
                break;
            }
            let (id, banderas) = match tipo {
                ENTRADA_LAPIC if elen >= 8 => {
                    let id: u8 = fis(madt + off + 3);
                    let f: u32 = fis(madt + off + 4);
                    (Some(id as u32), f)
                }
                ENTRADA_X2APIC if elen >= 16 => {
                    let id: u32 = fis(madt + off + 4);
                    let f: u32 = fis(madt + off + 8);
                    (Some(id), f)
                }
                _ => (None, 0),
            };
            if let Some(id) = id {
                if banderas & HABILITADO != 0 {
                    if c.n < MAX {
                        c.ids[c.n] = id;
                        c.n += 1;
                    }
                } else {
                    c.apagados += 1;
                }
            }
            off += elen as u64;
        }
        if c.n == 0 { None } else { Some(c) }
    }
}
