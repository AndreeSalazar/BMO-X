//! El informe del sistema, servido a Ring 3.
//!
//! ## Por qué esto baja de anillo
//!
//! El shell de Ring 0 tenía `info`, `cpu`, `mem` y `tasks` desde siempre, y no
//! porque hiciera falta el privilegio: **porque los datos estaban a su
//! alcance**. Contar cuánta RAM hay no ejerce ningún poder — leer un contador
//! no es tocar un puerto, ni mapear una página, ni reiniciar la máquina.
//!
//! Con este módulo el reparto queda donde debe: en Ring 0 se queda lo que **de
//! verdad** necesita el privilegio (E/S de puertos, tablas de páginas,
//! reinicio, la admisión de un `.bex`), y la información baja a Ring 3, que es
//! quien tiene la pantalla y sabe pintarla.
//!
//! ## Una tabla, no una operación por dato
//!
//! Dos operaciones (`TASK_OP_INFO` y `TASK_OP_INFO_TEXTO`) y una tabla de
//! campos. Añadir "cuántos programas se han lanzado" es **una fila**, no un
//! número de syscall nuevo — que es la misma forma que tienen las tablas de
//! `sem-asm` y la razón de que la superficie no crezca.
//!
//! ## Lo que este módulo NO hace
//!
//! No formatea. Devuelve enteros y bytes crudos: los KiB, los porcentajes, las
//! barras y el color son de Ring 3. Un kernel que decide cómo se ve un número
//! es un kernel que tiene opiniones sobre la interfaz.

// Los códigos de campo, copiados de `bmo_abi::syscalls::surface`. Se
// redeclaran aquí por la misma razón que `syscall.rs` redeclara los suyos: Ring
// 0 no enlaza el ABI completo, que usa `alloc`. **La fuente de verdad es
// `surface.rs`** — si estos números se separan, el que está mal es este
// archivo.
const INFO_RAM_TOTAL: u64 = 0x01;
const INFO_RAM_LIBRE: u64 = 0x02;
const INFO_RAM_MARCOS: u64 = 0x03;
const INFO_RAM_MARCOS_LIBRES: u64 = 0x04;
const INFO_TSC_HZ: u64 = 0x05;
const INFO_CPU_HILOS: u64 = 0x06;
const INFO_CPU_NUCLEOS: u64 = 0x07;
const INFO_TAREAS_TOTAL: u64 = 0x08;
const INFO_TAREAS_LISTAS: u64 = 0x09;
const INFO_TAREAS_LIBRES: u64 = 0x0A;
const INFO_TICKS: u64 = 0x0B;
const INFO_KERNEL_BYTES: u64 = 0x0C;
const INFO_PROGRAMAS: u64 = 0x0D;
const INFO_PROGRAMAS_OLVIDADOS: u64 = 0x0E;
const INFO_DISCO_LISTO: u64 = 0x0F;
const INFO_DATOS_MONTADO: u64 = 0x10;

const INFO_TXT_CPU_VENDOR: u64 = 0x01;
const INFO_TXT_CPU_NOMBRE: u64 = 0x02;
const INFO_TXT_UARCH: u64 = 0x03;
const INFO_TXT_FAMILIA: u64 = 0x04;

const PAGE: u64 = 4096;

/// El valor del campo, o 0 si no existe.
///
/// Cero y no un error: un campo que este kernel todavía no sabe contestar tiene
/// que poder pedirse sin que el programa se caiga. Ring 3 pinta "—" y sigue,
/// que es lo que hace un panel cuando un dato no está.
pub fn campo(n: u64) -> u64 {
    use crate::ring0::mm::phys;
    match n {
        INFO_RAM_TOTAL => phys::stats().0 * PAGE,
        INFO_RAM_LIBRE => phys::stats().1 * PAGE,
        INFO_RAM_MARCOS => phys::stats().0,
        INFO_RAM_MARCOS_LIBRES => phys::stats().1,
        INFO_TSC_HZ => crate::ring0::task::scheduler::tsc_freq(),
        // La topología está cacheada desde `init_bmo_cpu`: aquí no se vuelve a
        // preguntar al CPUID. Un panel que se repinta no debe costar CPUID.
        INFO_CPU_HILOS => cpu_topo().map(|t| t.total_threads as u64).unwrap_or(0),
        INFO_CPU_NUCLEOS => cpu_topo().map(|t| t.total_cores as u64).unwrap_or(0),
        INFO_TAREAS_TOTAL => crate::ring0::task::scheduler::counts().0 as u64,
        INFO_TAREAS_LISTAS => crate::ring0::task::scheduler::counts().1 as u64,
        INFO_TAREAS_LIBRES => crate::ring0::task::scheduler::huecos_libres() as u64,
        INFO_TICKS => crate::ring0::plat::timer::ticks(),
        // Medido, no declarado: desde donde lo enlaza el guion hasta el final
        // de su `.bss`, que incluye la pila de 64 KiB.
        INFO_KERNEL_BYTES => {
            extern "C" {
                static __bss_end: u8;
            }
            let fin = unsafe { &__bss_end as *const u8 as u64 };
            fin.saturating_sub(0x400000)
        }
        INFO_PROGRAMAS => crate::ring0::task::proc::programs().len() as u64,
        INFO_PROGRAMAS_OLVIDADOS => crate::ring0::task::proc::programas_olvidados() as u64,
        INFO_DISCO_LISTO => crate::ring0::dev::disk::is_ready() as u64,
        INFO_DATOS_MONTADO => crate::ring0::fsys::fs::data_mounted() as u64,
        _ => 0,
    }
}

fn cpu_topo() -> Option<&'static crate::ring0::cpu_vendor::ryzen_5_5600x::topology::Topology> {
    crate::ring0::cpu_vendor::ryzen_5_5600x::bmo_cpu::topology()
}

/// Ocho bytes del campo de texto, empaquetados en little-endian.
///
/// `trozo` cuenta de 8 en 8. Fuera del texto devuelve 0, y el cero es el final:
/// el llamante lee trozos hasta que llega uno con un cero dentro, igual que en
/// `console::write_const`. Es feo y es seguro, y lo segundo importa más — pasar
/// un puntero de Ring 3 obligaría al kernel a validar el rango entero contra el
/// espacio del llamante, y esa infraestructura no existe.
pub fn texto(n: u64, trozo: u64) -> u64 {
    let p = crate::ring0::cpu_vendor::profile::active();
    let s: &str = match n {
        INFO_TXT_CPU_VENDOR => p.vendor,
        INFO_TXT_CPU_NOMBRE => p.name,
        INFO_TXT_UARCH => p.microarch,
        INFO_TXT_FAMILIA => p.family_model,
        _ => "",
    };
    let b = s.as_bytes();
    let base = (trozo as usize).saturating_mul(8);
    let mut w = [0u8; 8];
    for i in 0..8 {
        match b.get(base + i) {
            Some(&c) => w[i] = c,
            None => break,
        }
    }
    u64::from_le_bytes(w)
}
