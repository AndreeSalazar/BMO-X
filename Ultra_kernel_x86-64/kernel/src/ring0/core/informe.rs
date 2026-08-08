//! El informe del sistema, servido a Ring 3.
//!
//! ## Por que esto baja de anillo
//!
//! El shell de Ring 0 tenia `info`, `cpu`, `mem` y `tasks` desde siempre, y no
//! porque hiciera falta el privilegio: **porque los datos estaban a su
//! alcance**. Contar cuanta RAM hay no ejerce ningun poder -- leer un contador
//! no es tocar un puerto, ni mapear una pagina, ni reiniciar la maquina.
//!
//! Con este modulo el reparto queda donde debe: en Ring 0 se queda lo que **de
//! verdad** necesita el privilegio (E/S de puertos, tablas de paginas,
//! reinicio, la admision de un `.bex`), y la informacion baja a Ring 3, que es
//! quien tiene la pantalla y sabe pintarla.
//!
//! ## Una tabla, no una operacion por dato
//!
//! Dos operaciones (`TASK_OP_INFO` y `TASK_OP_INFO_TEXTO`) y una tabla de
//! campos. Anadir "cuantos programas se han lanzado" es **una fila**, no un
//! numero de syscall nuevo -- que es la misma forma que tienen las tablas de
//! `sem-asm` y la razon de que la superficie no crezca.
//!
//! ## Lo que este modulo NO hace
//!
//! No formatea. Devuelve enteros y bytes crudos: los KiB, los porcentajes, las
//! barras y el color son de Ring 3. Un kernel que decide como se ve un numero
//! es un kernel que tiene opiniones sobre la interfaz.

// Los codigos de campo, copiados de `bmo_abi::syscalls::surface`. Se
// redeclaran aqui por la misma razon que `syscall.rs` redeclara los suyos: Ring
// 0 no enlaza el ABI completo, que usa `alloc`. **La fuente de verdad es
// `surface.rs`** -- si estos numeros se separan, el que esta mal es este
// archivo.
const INFO_RAM_TOTAL: u64 = 0x01;
const INFO_RAM_LIBRE: u64 = 0x02;
const INFO_RAM_MARCOS: u64 = 0x03;
const INFO_RAM_MARCOS_LIBRES: u64 = 0x04;
const INFO_TSC_HZ: u64 = 0x05;
const INFO_CPU_HILOS: u64 = 0x06;
const INFO_CPU_NUCLEOS: u64 = 0x07;
/// * Quien tiene la pantalla: su `pid`, o `0` si no la tiene nadie.
///
/// Existe para que el escritorio pueda PRESTARLA y esperar. Hacia falta
/// preguntar, no tomar: intentar reclamarla para saber si esta libre te la deja
/// puesta, y entonces se la robas al programa que ibas a prestarsela.
///
/// `0` como "de nadie" y no `u32::MAX`: el pid 0 no se concede a un proceso de
/// Ring 3, asi que no hay ambiguedad, y desde Ring 3 un `0` se lee sin tener
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
// -- ESTRATOS --
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
/// Cero y no un error: un campo que este kernel todavia no sabe contestar tiene
/// que poder pedirse sin que el programa se caiga. Ring 3 pinta "--" y sigue,
/// que es lo que hace un panel cuando un dato no esta.
pub fn campo(n: u64) -> u64 {
    use crate::ring0::mm::phys;
    match n {
        INFO_RAM_TOTAL => phys::stats().0 * PAGE,
        INFO_RAM_LIBRE => phys::stats().1 * PAGE,
        INFO_RAM_MARCOS => phys::stats().0,
        INFO_RAM_MARCOS_LIBRES => phys::stats().1,
        INFO_TSC_HZ => crate::ring0::task::scheduler::tsc_freq(),
        // La topologia esta cacheada desde `init_bmo_cpu`: aqui no se vuelve a
        // preguntar al CPUID. Un panel que se repinta no debe costar CPUID.
        INFO_CPU_HILOS => cpu_topo().map(|t| t.hilos as u64).unwrap_or(0),
        INFO_CPU_NUCLEOS => cpu_topo().map(|t| t.nucleos as u64).unwrap_or(0),
        INFO_TAREAS_TOTAL => crate::ring0::task::scheduler::counts().0 as u64,
        INFO_TAREAS_LISTAS => crate::ring0::task::scheduler::counts().1 as u64,
        INFO_PANTALLA_DUENO => crate::ring0::obj::fb::owner().unwrap_or(0) as u64,
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
        // * Hoy SIEMPRE cero, y a proposito. La maquina de estados de la
        // transaccion existe y esta probada, pero **nadie la ha cableado al
        // dispositivo**: no hay `write` ni `FLUSH CACHE`. Contestar 1 aqui
        // seria prometer una escritura que no ocurre -- y en un almacen, una
        // promesa de escritura que no ocurre es como se pierde el trabajo.
        INFO_ES_ESCRIBIBLE => 0,
        // Lo que Ring 3 ha PEDIDO. Cero hasta que un programa llame a
        // `KIND_MEMORIA` -- y por eso vale: es la unica fila del informe que
        // solo se mueve si alguien ejercio la capability.
        INFO_MEM_ENTREGADA => crate::ring0::obj::memoria::total_handed_over(),
        _ => 0,
    }
}

/// La topologia, **por el perfil y no por el nombre del fabricante**.
///
/// Esta funcion nombraba `cpu_vendor::ryzen_5_5600x::bmo_cpu::topology()`
/// directamente, y la cabecera de `profile.rs` dice literalmente que el resto de
/// Ring 0 *"nunca nombra un modulo de fabricante directamente"*. La regla estaba
/// escrita en el propio fichero que define la abstraccion, y rota aqui.
fn cpu_topo() -> Option<crate::ring0::cpu_vendor::profile::Nucleos> {
    (crate::ring0::cpu_vendor::profile::active().nucleos)()
}

/// Ocho bytes del campo de texto, empaquetados en little-endian.
///
/// `trozo` cuenta de 8 en 8. Fuera del texto devuelve 0, y el cero es el final:
/// el llamante lee trozos hasta que llega uno con un cero dentro, igual que en
/// `console::write_const`. Es feo y es seguro, y lo segundo importa mas -- pasar
/// un puntero de Ring 3 obligaria al kernel a validar el rango entero contra el
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
