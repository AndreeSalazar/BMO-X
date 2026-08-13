//! **Las dos puertas del sistema.** Y la lapida de la tercera.
//!
//! ```text
//!    INVOKE   haz esto AHORA
//!    WAIT     despiertame CUANDO
//! ```
//!
//! Y esa frontera no es estetica: `WAIT` **no se puede expresar con `INVOKE`**
//! porque lo unico que hace es no devolver el turno, y una llamada sincrona no
//! puede decir eso sin mentir. Por eso quedan dos y no una.
//!
//! El `1` esta reservado y no se reutiliza. Ver [`NR_CHANNEL_KICK`]: un binario
//! viejo que lo llame tiene que fallar DICIENDOLO -- si ese numero pasara a
//! significar otra cosa, haria algo que nadie pidio y no fallaria en ningun
//! sitio, que es la peor clase de rotura de ABI.

/// Synchronous, capability-scoped control operation.
pub const NR_INVOKE: u32 = 0x00;

/// ** RETIRADO el 2026-08-10. Reservado, y NO se reutiliza.
///
/// `CHANNEL_KICK(cap, secuencia)` resolvia un handle, comprobaba que era un
/// canal y llamaba al servicio del estuario: **una operacion sobre un handle**,
/// que es la definicion de [`NR_INVOKE`]. Tenia numero propio por como nacio, no
/// por lo que hace. Ahora es `CHANNEL_OP_KICK` sobre el canal.
///
/// La superficie queda en dos puertas, con la frontera dicha en una linea:
///
/// ```text
///   INVOKE   haz esto AHORA
///   WAIT     despiertame CUANDO
/// ```
///
/// Y no baja a una: `WAIT` no se puede expresar con `INVOKE` porque lo unico que
/// hace es **no devolver el turno**, y una llamada sincrona no puede decir eso
/// sin mentir.
///
/// El numero se reserva en vez de reciclarse: un binario viejo que llame al `1`
/// tiene que fallar **diciendolo**. Si el `1` pasara a significar otra cosa, ese
/// binario haria algo que nadie pidio y no fallaria en ningun sitio.
pub const NR_CHANNEL_KICK: u32 = 0x01;

/// Block until a sequence changes or an absolute deadline expires.
pub const NR_WAIT: u32 = 0x02;

/// **Dos.** Ver [`NR_CHANNEL_KICK`] para el tercero que hubo.
pub const CORE_SYSCALL_COUNT: usize = 2;

/// Process-local pseudo-handle that always resolves to the calling task.
/// It grants no authority over another task and must never be transferred.
pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
