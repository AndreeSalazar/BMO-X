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
/// ★ Quién tiene la pantalla: su `pid`, o `0` si no la tiene nadie.
///
/// Existe para que el escritorio pueda PRESTARLA y esperar. Hacía falta
/// preguntar, no tomar: intentar reclamarla para saber si está libre te la deja
/// puesta, y entonces se la robas al programa que ibas a prestársela.
///
/// `0` como "de nadie" y no `u32::MAX`: el pid 0 no se concede a un proceso de
/// Ring 3, así que no hay ambigüedad, y desde Ring 3 un `0` se lee sin tener
/// que conocer el centinela del kernel.
const INFO_PANTALLA_DUENO: u64 = 0x1A;
const INFO_TAREAS_TOTAL: u64 = 0x08;
const INFO_TAREAS_LISTAS: u64 = 0x09;
const INFO_TAREAS_LIBRES: u64 = 0x0A;
const INFO_TICKS: u64 = 0x0B;
const INFO_KERNEL_BYTES: u64 = 0x0C;
const INFO_PROGRAMAS: u64 = 0x0D;
const INFO_PROGRAMAS_OLVIDADOS: u64 = 0x0E;
const INFO_DISCO_LISTO: u64 = 0x0F;
const INFO_DATOS_MONTADO: u64 = 0x10;
// ── ESTRATOS ──
//
// El volumen grande. Ring 3 los necesita para poder ENSENAR el estado del
// almacen; anadirlos es una fila cada uno, no una operacion nueva.
const INFO_ES_MONTADO: u64 = 0x11;
const INFO_ES_GENERACION: u64 = 0x12;
const INFO_ES_BLOQUES: u64 = 0x13;
const INFO_ES_USADOS: u64 = 0x14;
const INFO_ES_BLOQUE_TAM: u64 = 0x15;
const INFO_ES_NIVEL: u64 = 0x16;
const INFO_ES_IDENTIDAD: u64 = 0x17;
const INFO_ES_ESCRIBIBLE: u64 = 0x18;
/// Lo que Ring 3 ha pedido con `KIND_MEMORIA`. Ver `surface.rs`.
const INFO_MEM_ENTREGADA: u64 = 0x19;

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
        INFO_CPU_HILOS => cpu_topo().map(|t| t.hilos as u64).unwrap_or(0),
        INFO_CPU_NUCLEOS => cpu_topo().map(|t| t.nucleos as u64).unwrap_or(0),
        INFO_TAREAS_TOTAL => crate::ring0::task::scheduler::counts().0 as u64,
        INFO_TAREAS_LISTAS => crate::ring0::task::scheduler::counts().1 as u64,
        INFO_PANTALLA_DUENO => crate::ring0::obj::fb::dueno().unwrap_or(0) as u64,
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
        INFO_ES_MONTADO => crate::ring0::fsys::estratos::is_mounted() as u64,
        INFO_ES_IDENTIDAD => crate::ring0::fsys::estratos::identidad_ok() as u64,
        INFO_ES_GENERACION => {
            crate::ring0::fsys::estratos::superbloque().map_or(0, |sb| sb.generation)
        }
        INFO_ES_BLOQUES => {
            crate::ring0::fsys::estratos::ocupacion().map_or(0, |o| o.totales)
        }
        INFO_ES_USADOS => {
            crate::ring0::fsys::estratos::ocupacion().map_or(0, |o| o.usados)
        }
        INFO_ES_BLOQUE_TAM => {
            crate::ring0::fsys::estratos::superbloque().map_or(0, |sb| sb.block_size as u64)
        }
        INFO_ES_NIVEL => crate::ring0::fsys::estratos::ocupacion().map_or(0, |o| {
            match o.nivel() {
                bmo_estratos::Nivel::Holgado => 0,
                bmo_estratos::Nivel::Ambar => 1,
                bmo_estratos::Nivel::Rojo => 2,
                bmo_estratos::Nivel::SoloLectura => 3,
            }
        }),
        // ★ Hoy SIEMPRE cero, y a proposito. La maquina de estados de la
        // transaccion existe y esta probada, pero **nadie la ha cableado al
        // dispositivo**: no hay `write` ni `FLUSH CACHE`. Contestar 1 aqui
        // seria prometer una escritura que no ocurre — y en un almacen, una
        // promesa de escritura que no ocurre es como se pierde el trabajo.
        INFO_ES_ESCRIBIBLE => 0,
        // Lo que Ring 3 ha PEDIDO. Cero hasta que un programa llame a
        // `KIND_MEMORIA` — y por eso vale: es la única fila del informe que
        // sólo se mueve si alguien ejerció la capability.
        INFO_MEM_ENTREGADA => crate::ring0::obj::memoria::total_entregado(),
        _ => 0,
    }
}

/// La topología, **por el perfil y no por el nombre del fabricante**.
///
/// Esta función nombraba `cpu_vendor::ryzen_5_5600x::bmo_cpu::topology()`
/// directamente, y la cabecera de `profile.rs` dice literalmente que el resto de
/// Ring 0 *"nunca nombra un módulo de fabricante directamente"*. La regla estaba
/// escrita en el propio fichero que define la abstracción, y rota aquí.
fn cpu_topo() -> Option<crate::ring0::cpu_vendor::profile::Nucleos> {
    (crate::ring0::cpu_vendor::profile::active().nucleos)()
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
