//! BMO ABI core syscall surface (the frozen 3-call surface).
//!
//! Services such as files, network, audio, graphics and input are capability
//! operations transported through BMO Channel. They are not kernel syscalls.

use super::{syscall2, syscall3, syscall6, SyscallResult};

/// Synchronous, capability-scoped control operation.
pub const NR_INVOKE: u32 = 0x00;
/// Notify a channel consumer after publishing submissions.
pub const NR_CHANNEL_KICK: u32 = 0x01;
/// Block until a sequence changes or an absolute deadline expires.
pub const NR_WAIT: u32 = 0x02;
pub const CORE_SYSCALL_COUNT: usize = 3;

/// Process-local pseudo-handle that always resolves to the calling task.
/// It grants no authority over another task and must never be transferred.
pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
pub const TASK_OP_GET_PID: u64 = 0x01;
pub const TASK_OP_GET_TID: u64 = 0x02;
pub const TASK_OP_YIELD: u64 = 0x03;
pub const TASK_OP_EXIT: u64 = 0x04;
/// `INVOKE(CURRENT_TASK, CHANNEL_OPEN, index)` → the caller's estuary
/// capability handle for BMO Channel `index`. Fails with NEEDS_CAP when
/// the process was not granted that estuary.
pub const TASK_OP_CHANNEL_OPEN: u64 = 0x05;
/// `INVOKE(CURRENT_TASK, CONSOLE_WRITE, packed)` → emit up to 8 bytes of
/// text (packed little-endian in `packed`, NUL-terminated within the word)
/// to the kernel bootstrap console. This is the debug door that lets the
/// very first Ring 3 program prove the CPL3→CPL0 path visually before a
/// real console capability/estuary service exists; it will migrate to a
/// console handle once the display server lands.
pub const TASK_OP_CONSOLE_WRITE: u64 = 0x06;

/// Crea un endpoint atendido por este proceso: `arg0` es el estuario por el
/// que se le entregaran las llamadas, y devuelve el handle del endpoint.
///
/// Es lo unico que Endpoint RPC anade a la superficie. Llamar, atender y
/// responder NO son operaciones nuevas: son lo que `INVOKE` y `WAIT` ya
/// significan cuando el handle resuelve a un endpoint o a un reply. La
/// superficie sigue siendo de tres puertas.
pub const TASK_OP_ENDPOINT_CREATE: u64 = 0x07;

/// LEE de la consola asignada al proceso. La PAREJA de `CONSOLE_WRITE`.
///
/// Sin esto un programa lanzado desde un terminal no puede recibir nada: la
/// capability del teclado la tiene el compositor, y darsela a cada hijo seria
/// romper la exclusividad que hace que la entrada tenga un solo dueno. El
/// terminal que lo lanzo le pasa lo que se teclea, por el mismo objeto que ya
/// usa para hablar.
pub const TASK_OP_CONSOLE_READ: u64 = 0x0F;

/// Acumula hasta 8 bytes de una RUTA en el renglon del proceso.
///
/// La superficie congelada no acepta punteros, asi que una ruta viaja de 8 en
/// 8 y la consume la siguiente operacion que necesite una. **Un solo renglon**
/// para `EJECUTAR`, `DIR_ABRIR` y los dos de archivo: inventar un mecanismo
/// por cada consumidor seria tener cuatro sitios donde se pierde un byte.
pub const TASK_OP_RUTA: u64 = 0x0B;
/// Lanza lo acumulado con [`TASK_OP_RUTA`] y vacía el renglón. Devuelve el tid.
///
/// ★ Estas tres (`0x0C`–`0x0E`) vivían **sólo dentro del kernel**: se añadieron
/// a `ring0/syscall.rs` y nunca subieron aquí, así que el guardián de deriva de
/// `build.ps1` no las miraba — no puede comparar lo que en un lado no existe.
/// La superficie es el contrato; el kernel es una implementación suya.
pub const TASK_OP_EJECUTAR: u64 = 0x0C;
/// Crea una consola y devuelve su handle de LECTURA. Quien la crea es el
/// terminal: la consola es suya y la drena a su ritmo.
pub const TASK_OP_CONSOLA_CREAR: u64 = 0x0D;
/// Abre un directorio del volumen de datos. La ruta se acumula antes con
/// [`TASK_OP_RUTA`] — el mismo renglón que usa `EJECUTAR`.
pub const TASK_OP_DIR_ABRIR: u64 = 0x0E;

/// Abre un archivo del volumen de datos para LEER. La ruta viene del renglon.
pub const TASK_OP_ARCHIVO_ABRIR: u64 = 0x10;

/// Abre un archivo del volumen de datos para ESCRIBIR (lo crea).
///
/// Son dos operaciones y no un argumento de modo porque crear puede fallar por
/// motivos que abrir no tiene —volumen de solo lectura, nombre que no es 8.3—
/// y mezclarlas obligaria a devolver errores que no aplican a la mitad de las
/// llamadas.
pub const TASK_OP_ARCHIVO_CREAR: u64 = 0x11;

// ── Operaciones sobre un handle de archivo (`KIND_ARCHIVO`) ──────────────
//
// Viven aqui y no en el kernel porque las emite `bmo-lower` y las ejecuta el
// emulador: tres sitios que tienen que decir el mismo numero. Ver
// `Ultra_kernel_x86-64/kernel/src/ring0/archivo.rs`.

/// Saca hasta 7 bytes: `(n << 56) | bytes_LE`. `n == 0` = se acabo.
///
/// La cuenta va en el byte alto y NO se corta en el primer cero, al reves que
/// la consola: un archivo no es texto y un `\0` en medio es un dato.
/// Reinicia la máquina. No vuelve.
///
/// Reiniciar es tocar puertos de E/S (`0xCF9`, el 8042), que Ring 3 no puede
/// —ni debe— hacer; por eso es una operación y no un permiso ambiental.
/// **Hoy no está atada a una capability**, igual que `EJECUTAR`: las dos
/// quieren la misma el día que exista.
pub const TASK_OP_REINICIAR: u64 = 0x12;

// ── INFORME DEL SISTEMA ─────────────────────────────────────────────────
//
// Leer cuánta RAM hay no es un privilegio: es una PREGUNTA. El shell de Ring 0
// tenía `info`, `cpu` y `mem` sólo porque los datos estaban a su alcance, no
// porque hiciera falta estar en Ring 0 para contarlos. Con estas dos
// operaciones el privilegio se queda con lo que de verdad lo necesita —tocar
// puertos, reiniciar, mapear páginas— y la información baja a Ring 3, que es
// donde se pinta.
//
// Dos operaciones y una TABLA de campos, en vez de una operación por dato: así
// añadir "cuántos programas se han lanzado" es una fila, no un número de
// syscall nuevo. Es la misma forma que tienen las tablas de `sem-asm`.

/// Un dato numérico del sistema. `arg0` = campo (`INFO_*`). Devuelve el valor.
pub const TASK_OP_INFO: u64 = 0x13;
/// Un dato de TEXTO. `arg0` = campo (`INFO_TXT_*`), `arg1` = qué trozo.
///
/// Devuelve 8 bytes empaquetados en little-endian, el cero corta — el mismo
/// formato que `TASK_OP_RUTA` y `TASK_OP_CONSOLE_WRITE`, y por la misma razón:
/// aquí no hay `copy_to_user`, así que el texto viaja por valor.
pub const TASK_OP_INFO_TEXTO: u64 = 0x14;

/// Bytes de RAM que el asignador de marcos gobierna.
pub const INFO_RAM_TOTAL: u64 = 0x01;
/// Bytes libres AHORA.
pub const INFO_RAM_LIBRE: u64 = 0x02;
/// Marcos totales de 4 KiB.
pub const INFO_RAM_MARCOS: u64 = 0x03;
/// Marcos libres.
pub const INFO_RAM_MARCOS_LIBRES: u64 = 0x04;
/// Frecuencia del TSC en Hz. Es la que mide el tiempo de verdad en esta
/// máquina, no un número nominal de la etiqueta.
pub const INFO_TSC_HZ: u64 = 0x05;
/// Hilos lógicos y núcleos físicos que el CPU declara.
pub const INFO_CPU_HILOS: u64 = 0x06;
pub const INFO_CPU_NUCLEOS: u64 = 0x07;
/// Tareas: ranuras ocupadas, listas para correr, y libres.
pub const INFO_TAREAS_TOTAL: u64 = 0x08;
pub const INFO_TAREAS_LISTAS: u64 = 0x09;
pub const INFO_TAREAS_LIBRES: u64 = 0x0A;
/// Ticks del temporizador desde el arranque.
pub const INFO_TICKS: u64 = 0x0B;
/// Bytes que ocupa el kernel en RAM, medidos (hasta el final de su `.bss`,
/// pila incluida). No es el tamaño del archivo.
pub const INFO_KERNEL_BYTES: u64 = 0x0C;
/// Programas que se han intentado admitir, y los que ya no caben en la
/// bitácora. La suma es el total de verdad.
pub const INFO_PROGRAMAS: u64 = 0x0D;
pub const INFO_PROGRAMAS_OLVIDADOS: u64 = 0x0E;
/// ¿Hay disco listo? ¿Está montado el volumen de datos para escribir?
pub const INFO_DISCO_LISTO: u64 = 0x0F;
pub const INFO_DATOS_MONTADO: u64 = 0x10;
/// ── ESTRATOS ──────────────────────────────────────────────────────
///
/// El volumen de datos grande. Ring 3 los necesita para poder ENSENAR el estado
/// del almacen sin cruzar a Ring 0 por cada dato: son una fila mas de la tabla
/// de `OP_INFO`, que es como crece esta superficie sin tocar el ABI.
pub const INFO_ES_MONTADO: u64 = 0x11;
/// Generacion del superbloque: cuantas transacciones lleva el volumen.
pub const INFO_ES_GENERACION: u64 = 0x12;
pub const INFO_ES_BLOQUES: u64 = 0x13;
pub const INFO_ES_USADOS: u64 = 0x14;
pub const INFO_ES_BLOQUE_TAM: u64 = 0x15;
/// 0 holgado, 1 ambar, 2 rojo, 3 solo lectura. Ver `bmo_estratos::espacio`.
pub const INFO_ES_NIVEL: u64 = 0x16;
/// El gate del §5: 1 si el volumen nacio en ESTE disco.
pub const INFO_ES_IDENTIDAD: u64 = 0x17;
/// 1 si hoy se puede escribir. Hoy siempre 0: falta cablear la E/S.
pub const INFO_ES_ESCRIBIBLE: u64 = 0x18;

/// Fabricante ("AMD"), nombre comercial, microarquitectura y familia/modelo.
pub const INFO_TXT_CPU_VENDOR: u64 = 0x01;
pub const INFO_TXT_CPU_NOMBRE: u64 = 0x02;
pub const INFO_TXT_UARCH: u64 = 0x03;
pub const INFO_TXT_FAMILIA: u64 = 0x04;

pub const ARCH_OP_LEER: u64 = 0x01;
/// Saca hasta 7 bytes **sin pasar del salto de linea**:
/// `(fin << 63) | (n << 56) | bytes_LE`.
///
/// - `fin = 1` — se llego al salto, que se CONSUME. El registro esta completo.
/// - `n = 0` y `fin = 0` — se acabo el archivo.
///
/// Existe porque `ARCH_OP_LEER` no sirve para leer registros: devuelve siete
/// bytes y avanza el cursor siete, asi que si el salto cae en medio del
/// paquete, lo que venia detras **se pierde**. Un fichero de movimientos
/// leido asi da bien el primer registro y basura los demas.
///
/// El corte lo hace el kernel y no el llamante porque el cursor es del kernel:
/// nadie de fuera puede devolverle los bytes que ya le dio.
pub const ARCH_OP_LEER_LINEA: u64 = 0x05;
/// Mete hasta 7 bytes: `arg0 = (n << 56) | bytes_LE`. Devuelve los aceptados.
pub const ARCH_OP_ESCRIBIR: u64 = 0x02;
/// Bytes que quedan por leer, o los acumulados si es de escritura.
pub const ARCH_OP_TAMANO: u64 = 0x03;
/// Cierra. En uno de escritura **es donde el contenido llega al disco**.
pub const ARCH_OP_CERRAR: u64 = 0x04;

// ── La entrada: ratón y teclado ─────────────────────────────────────────
//
// ★ Estas constantes vivían en DOS sitios —`ring0/obj/input.rs` y el userland
// de Rust— y en ninguno de los dos que fuera el contrato. Mientras el único
// cliente fue un compositor escrito en Rust eso se notaba poco; en cuanto un
// segundo lenguaje quiso leer la rueda, se vio lo que era: un contrato que no
// estaba publicado no lo puede cumplir nadie más. Aquí no hay lógica nueva,
// hay un sitio del que copiar en vez de dos de los que adivinar.

/// Reclama ratón + teclado. **Exclusivo**: mientras un proceso lo tenga, el
/// shell de Ring 0 deja de leer el teclado físico. No es un reparto — dos
/// lectores de la misma cola se robarían las letras.
pub const TASK_OP_INPUT_CLAIM: u64 = 0x0A;
/// Reclama la pantalla. También exclusivo.
pub const TASK_OP_FRAMEBUFFER_CLAIM: u64 = 0x09;

/// Dónde está el puntero y qué botones tiene: `(x << 32) | (y << 16) | botones`.
/// Ya viene recortado al panel: el kernel es quien sabe de qué tamaño es.
pub const INPUT_OP_PUNTERO: u64 = 0x01;
/// Cuántos informes HID se han visto desde el arranque. Distingue "el ratón no
/// se mueve" de "el ratón no llega": si esto no sube, el problema está en el USB.
pub const INPUT_OP_EVENTOS: u64 = 0x02;
/// La siguiente tecla: `0x100 | byte`, o `0` si no hay ninguna esperando.
/// **No bloquea.** El byte es Latin-1 ya resuelto (la `ñ` es `0xF1`).
pub const INPUT_OP_TECLA: u64 = 0x03;
/// Máscara de modificadores pulsados AHORA. Es estado, no consume nada.
pub const INPUT_OP_MODIFICADORES: u64 = 0x04;
/// Las muescas de rueda **desde la última vez**, como `i32` en complemento a
/// dos dentro del `u64`. Positivo = hacia arriba.
///
/// ★ **Consume**: dos lecturas seguidas sin girar dan cero la segunda. Devolver
/// un acumulado desde el arranque obligaría a cada llamante a guardar el
/// anterior y restar, y el primero que lo olvide tiene un scroll que se mueve
/// solo.
pub const INPUT_OP_RUEDA: u64 = 0x05;

/// Bits de la máscara de [`INPUT_OP_MODIFICADORES`].
pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_ALTGR: u8 = 1 << 3;
pub const MOD_CAPS: u8 = 1 << 4;

/// Las teclas sin glifo, en el rango C1 (0x80..0x9F) que eligió el driver.
///
/// No son ASCII y no lo pretenden: son bytes que ninguna distribución produce
/// como carácter, así que un programa puede distinguirlas de lo que se escribe
/// sin un segundo canal.
/// Son los mismos bytes que `ring0::dev::keyboard::KEY_*`, y esa igualdad es
/// el contrato: si divergen, un programa lee flechas donde hay páginas.
pub const TECLA_ARRIBA: u8 = 0x80;
pub const TECLA_ABAJO: u8 = 0x81;
pub const TECLA_IZQUIERDA: u8 = 0x82;
pub const TECLA_DERECHA: u8 = 0x83;
pub const TECLA_INICIO: u8 = 0x84;
pub const TECLA_FIN: u8 = 0x85;
pub const TECLA_SUPR: u8 = 0x86;
pub const TECLA_REPAG: u8 = 0x87;
pub const TECLA_AVPAG: u8 = 0x88;

/// Las teclas de función, detrás de la navegación en el mismo rango C1.
///
/// ★ Son el sitio correcto para un atajo del sistema porque **no producen
/// carácter en ninguna distribución**: no pueden chocar con escribir. Una
/// combinación con `Ctrl+Alt` sí puede — en español `Ctrl+Alt` *es* AltGr.
pub const TECLA_F1: u8 = 0x89;
pub const TECLA_F2: u8 = 0x8A;
pub const TECLA_F3: u8 = 0x8B;
pub const TECLA_F4: u8 = 0x8C;
pub const TECLA_F5: u8 = 0x8D;
pub const TECLA_F6: u8 = 0x8E;
pub const TECLA_F7: u8 = 0x8F;
pub const TECLA_F8: u8 = 0x90;
pub const TECLA_F9: u8 = 0x91;
pub const TECLA_F10: u8 = 0x92;
pub const TECLA_F11: u8 = 0x93;
pub const TECLA_F12: u8 = 0x94;

/// Operations accepted by `CURRENT_TASK`.
pub mod task_op {
    pub const GET_PID: u64 = super::TASK_OP_GET_PID;
    pub const ENDPOINT_CREATE: u64 = super::TASK_OP_ENDPOINT_CREATE;
    pub const GET_TID: u64 = super::TASK_OP_GET_TID;
    pub const YIELD: u64 = super::TASK_OP_YIELD;
    pub const EXIT: u64 = super::TASK_OP_EXIT;
    pub const CHANNEL_OPEN: u64 = super::TASK_OP_CHANNEL_OPEN;
    pub const CONSOLE_WRITE: u64 = super::TASK_OP_CONSOLE_WRITE;
    pub const CONSOLE_READ: u64 = super::TASK_OP_CONSOLE_READ;
}

/// `INVOKE` operations accepted by a channel (estuary) capability.
pub const CHANNEL_OP_GET_SEQ: u64 = 0x01;
pub const CHANNEL_OP_GET_INDEX: u64 = 0x02;

pub mod channel_op {
    /// Completion-side sequence — the value `WAIT` compares against.
    pub const GET_SEQ: u64 = super::CHANNEL_OP_GET_SEQ;
    /// Estuary index backing this capability.
    pub const GET_INDEX: u64 = super::CHANNEL_OP_GET_INDEX;
}

/// Translate the temporary v1 task surface into its v2 capability operation.
///
/// This belongs at the ABI boundary so compilers and runtimes do not each
/// duplicate a legacy-number mapping. It can be removed with the v1 table.
pub const fn task_operation_for_legacy_syscall(number: u32) -> Option<u64> {
    match number {
        super::NR_PROC_GET_PID => Some(TASK_OP_GET_PID),
        super::NR_PROC_GET_TID | super::NR_THREAD_SELF => Some(TASK_OP_GET_TID),
        super::NR_PROC_YIELD => Some(TASK_OP_YIELD),
        super::NR_PROC_EXIT | super::NR_THREAD_EXIT => Some(TASK_OP_EXIT),
        _ => None,
    }
}

/// `INVOKE(capability, operation, a0, a1, a2, a3)`.
#[inline(always)]
pub unsafe fn invoke(
    capability: u64,
    operation: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
) -> SyscallResult {
    syscall6(NR_INVOKE, capability, operation, a0, a1, a2, a3)
}

/// `CHANNEL_KICK(channel, published_sequence)`.
#[inline(always)]
pub unsafe fn channel_kick(channel: u64, published_sequence: u64) -> SyscallResult {
    syscall2(NR_CHANNEL_KICK, channel, published_sequence)
}

/// `WAIT(waitable, observed_sequence, timeout_ns)`.
///
/// Blocks until the waitable's sequence moves past `observed_sequence`
/// or `timeout_ns` elapses (0 = no timeout). `waitable = 0` is a pure
/// timed sleep. The kernel compares the sequence under its scheduler
/// lock, so a kick can never be lost between the caller's read and the
/// block. On resume, re-read the shared sequence — the returned value
/// is advisory.
#[inline(always)]
pub unsafe fn wait(
    waitable: u64,
    observed_sequence: u64,
    timeout_ns: u64,
) -> SyscallResult {
    syscall3(NR_WAIT, waitable, observed_sequence, timeout_ns)
}

pub const fn name(number: u32) -> Option<&'static str> {
    match number {
        NR_INVOKE => Some("bmo_invoke"),
        NR_CHANNEL_KICK => Some("bmo_channel_kick"),
        NR_WAIT => Some("bmo_wait"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_surface_is_frozen_to_three_calls() {
        assert_eq!(CORE_SYSCALL_COUNT, 3);
        assert_eq!(name(0), Some("bmo_invoke"));
        assert_eq!(name(1), Some("bmo_channel_kick"));
        assert_eq!(name(2), Some("bmo_wait"));
        assert_eq!(name(3), None);
    }

    #[test]
    fn legacy_task_translation_has_one_canonical_mapping() {
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_EXIT), Some(TASK_OP_EXIT));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_GET_PID), Some(TASK_OP_GET_PID));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_GET_TID), Some(TASK_OP_GET_TID));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_YIELD), Some(TASK_OP_YIELD));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_FS_OPEN), None);
    }
}
