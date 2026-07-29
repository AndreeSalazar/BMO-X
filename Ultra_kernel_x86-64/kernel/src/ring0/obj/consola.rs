//! `KIND_CONSOLE` — la salida de un programa, como capability.
//!
//! ## La asimetría que esto cierra
//!
//! La pantalla es `KIND_FRAMEBUFFER`. La entrada es `KIND_INPUT`. La consola
//! era **lo único que no lo era**: `OP_CONSOLE_WRITE` escribía siempre en el
//! mismo sitio global, el panel del kernel. El propio comentario del syscall lo
//! confesaba — "la consola de arranque, no la salida de nadie en serio".
//!
//! Eso tenía una consecuencia que sólo se ve cuando intentas construir encima:
//! **un terminal en Ring 3 no puede leer lo que imprime su propio hijo.** Lanza
//! un programa, el programa escribe, y su salida cae en el panel de Ring 0 —
//! debajo del escritorio, donde nadie mira. Se compila a ciegas.
//!
//! Es exactamente el problema que resuelve un PTY en Unix, y aquí tiene la
//! forma que tiene todo lo demás: un objeto con dueño.
//!
//! ## El trato
//!
//! - Un proceso **crea** una consola y recibe un handle de LECTURA. Es el
//!   terminal: la consola es suya y la drena a su ritmo.
//! - Al **lanzar** un hijo puede pasarle esa consola. Desde ese momento el
//!   `OP_CONSOLE_WRITE` del hijo aterriza en ese anillo, no en el panel.
//! - Sin consola asignada, se escribe en la del kernel **exactamente como
//!   antes**. Los cinco demos embebidos siguen hablando por el panel sin
//!   cambiar una línea: lo nuevo no rompe lo viejo, lo rodea.
//!
//! ## Por qué el anillo es del kernel y no memoria compartida
//!
//! Un estuario (`KIND_CHANNEL`) sería más rápido y algún día será lo correcto.
//! Pero el escritor es un proceso que puede morir a mitad de línea, y el lector
//! otro que puede no existir todavía. Un anillo pequeño en el kernel hace que
//! ninguno de los dos pueda corromper al otro, y que la salida de un programa
//! sobreviva a que su terminal se cierre. Cuando el RPC de endpoints esté
//! rodado, esto se muda; el contrato de fuera no cambia.

use crate::ring0::obj::cap;

/// Cuántas consolas pueden existir a la vez. Una por terminal abierto.
pub const MAX_CONSOLAS: usize = 4;
/// Bytes de salida que aguanta cada una antes de descartar lo más viejo.
const ANILLO: usize = 2048;

pub const SIN_DUENO: u32 = u32::MAX;

/// No quedan consolas libres.
pub const ERROR_SIN_HUECO: u32 = 24;

/// Leer hasta **7** bytes: `(n << 56) | bytes_LE`, con `n` = cuántos son
/// válidos. `n == 0` = no hay nada.
///
/// ★ Siete y no ocho, y el contador ARRIBA. La primera versión devolvía
/// `(n << 32) | ocho_bytes` — y eso **pisa el byte 4**: ocho bytes ocupan el
/// u64 entero y no dejan sitio para decir cuántos valen. Un byte de cada ocho
/// habría salido corrupto, en una ruta que sólo se nota leyendo texto raro.
/// Se paga un byte de ancho de banda por tener un contador honesto.
pub const CONSOLA_OP_LEER: u64 = 0x01;
/// Cuántos bytes se han descartado por anillo lleno. Un terminal que va lento
/// tiene derecho a saber que está perdiendo salida en vez de creerse completo.
pub const CONSOLA_OP_PERDIDOS: u64 = 0x02;

/// El TERMINAL mete 8 bytes (LE, el cero corta) en el anillo de ENTRADA.
///
/// Es el segundo sentido del canal, y sin el no puede haber `ACCEPT`: un
/// programa lanzado desde la caja no puede reclamar `KIND_INPUT` —la tiene el
/// compositor— asi que su unica via para recibir teclas es por el mismo objeto
/// que ya usa para hablar. Un canal de un solo sentido deja al hijo mudo de
/// oido.
pub const CONSOLA_OP_ESCRIBIR: u64 = 0x03;
/// ¿Hay algun proceso escribiendo a esta consola ahora mismo?
///
/// Lo pregunta el terminal para saber a donde mandar lo que se teclea: si hay
/// hijo vivo, la linea es PARA EL; si no, es un comando. Sin esto habria que
/// inventar un prefijo o un modo, y las dos cosas se olvidan.
pub const CONSOLA_OP_HAY_HIJO: u64 = 0x04;

/// Anillo de ENTRADA, mucho mas pequeño que el de salida: aqui cabe lo que una
/// persona teclea, no lo que un programa escupe.
const ENTRADA: usize = 256;
static mut IN_BUF: [[u8; ENTRADA]; MAX_CONSOLAS] = [[0; ENTRADA]; MAX_CONSOLAS];
static mut IN_LEE: [usize; MAX_CONSOLAS] = [0; MAX_CONSOLAS];
static mut IN_ESCRIBE: [usize; MAX_CONSOLAS] = [0; MAX_CONSOLAS];

static mut BUF: [[u8; ANILLO]; MAX_CONSOLAS] = [[0; ANILLO]; MAX_CONSOLAS];
static mut LEE: [usize; MAX_CONSOLAS] = [0; MAX_CONSOLAS];
static mut ESCRIBE: [usize; MAX_CONSOLAS] = [0; MAX_CONSOLAS];
static mut PERDIDOS: [u32; MAX_CONSOLAS] = [0; MAX_CONSOLAS];
/// Pid del LECTOR (el terminal). `SIN_DUENO` = ranura libre.
static mut LECTOR: [u32; MAX_CONSOLAS] = [SIN_DUENO; MAX_CONSOLAS];

/// A qué consola escribe cada proceso. `(pid, indice)`; pid `SIN_DUENO` = vacío.
///
/// Tabla aparte y no un campo del proceso a propósito: el planificador no tiene
/// por qué saber de consolas, y esto se consulta sólo en el borde del syscall.
static mut SALIDA: [(u32, usize); MAX_CONSOLAS * 4] = [(SIN_DUENO, 0); MAX_CONSOLAS * 4];

/// Crea una consola y entrega su handle de lectura a `pid`.
pub fn crear(pid: u32) -> Result<u64, u32> {
    unsafe {
        let libre = (0..MAX_CONSOLAS).find(|&i| LECTOR[i] == SIN_DUENO);
        let i = match libre {
            Some(i) => i,
            None => return Err(ERROR_SIN_HUECO),
        };
        LEE[i] = 0;
        ESCRIBE[i] = 0;
        PERDIDOS[i] = 0;
        LECTOR[i] = pid;
        match cap::grant(pid, cap::KIND_CONSOLE, cap::RIGHT_READ, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("consola", "consola creada para Ring 3", pid as u64);
                Ok(h)
            }
            None => {
                LECTOR[i] = SIN_DUENO;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

/// Manda la salida de `pid` a la consola `idx`. La llama el lanzador cuando un
/// terminal entrega su consola a un hijo.
pub fn asignar_salida(pid: u32, idx: usize) {
    if idx >= MAX_CONSOLAS {
        return;
    }
    unsafe {
        let tabla = &mut *core::ptr::addr_of_mut!(SALIDA);
        // Si ya tenía una asignada, se reemplaza en su sitio.
        for e in tabla.iter_mut() {
            if e.0 == pid {
                e.1 = idx;
                return;
            }
        }
        for e in tabla.iter_mut() {
            if e.0 == SIN_DUENO {
                *e = (pid, idx);
                return;
            }
        }
        // Sin hueco en la tabla: el hijo escribe al panel del kernel. Se pierde
        // el encauzado, no la salida — y eso es lo correcto: mejor verla en el
        // sitio de siempre que no verla.
        crate::ring0::cabina::warn("consola", "sin hueco para encauzar la salida", pid as u64);
    }
}

/// A qué consola escribe `pid`, si es que escribe a alguna.
pub fn salida_de(pid: u32) -> Option<usize> {
    unsafe {
        let tabla = &*core::ptr::addr_of!(SALIDA);
        for e in tabla.iter() {
            if e.0 == pid {
                // Una consola cuyo lector murió ya no encauza a nadie.
                if LECTOR[e.1] == SIN_DUENO {
                    return None;
                }
                return Some(e.1);
            }
        }
        None
    }
}

/// Mete bytes en el anillo. Si está lleno, se descarta lo MÁS VIEJO y se
/// cuenta: en una consola, la línea que acabas de imprimir importa más que la
/// de hace dos mil bytes.
pub fn escribir(idx: usize, datos: &[u8]) {
    if idx >= MAX_CONSOLAS {
        return;
    }
    unsafe {
        for &b in datos {
            let sig = (ESCRIBE[idx] + 1) % ANILLO;
            if sig == LEE[idx] {
                // Lleno: avanza el lector, o sea que se pierde el byte más
                // antiguo. Se anota para que el terminal pueda decirlo.
                LEE[idx] = (LEE[idx] + 1) % ANILLO;
                PERDIDOS[idx] = PERDIDOS[idx].saturating_add(1);
            }
            BUF[idx][ESCRIBE[idx]] = b;
            ESCRIBE[idx] = sig;
        }
    }
}

/// Saca hasta 7 bytes: `(n << 56) | empaquetado_LE`. Ver `CONSOLA_OP_LEER`.
pub fn leer(idx: usize) -> u64 {
    if idx >= MAX_CONSOLAS {
        return 0;
    }
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        while n < 7 && LEE[idx] != ESCRIBE[idx] {
            w[n] = BUF[idx][LEE[idx]];
            LEE[idx] = (LEE[idx] + 1) % ANILLO;
            n += 1;
        }
        // w[7] queda a cero por construcción: el bucle para en 7.
        ((n as u64) << 56) | u64::from_le_bytes(w)
    }
}

pub fn perdidos(idx: usize) -> u64 {
    if idx >= MAX_CONSOLAS {
        return 0;
    }
    unsafe { PERDIDOS[idx] as u64 }
}

/// Mete bytes en el anillo de ENTRADA. Si esta lleno se descartan los NUEVOS
/// —al reves que la salida— porque aqui el orden es el que tecleo una persona:
/// tirar lo viejo dejaria media linea sin principio.
pub fn escribir_entrada(idx: usize, datos: &[u8]) {
    if idx >= MAX_CONSOLAS {
        return;
    }
    unsafe {
        for &b in datos {
            let sig = (IN_ESCRIBE[idx] + 1) % ENTRADA;
            if sig == IN_LEE[idx] {
                return;
            }
            IN_BUF[idx][IN_ESCRIBE[idx]] = b;
            IN_ESCRIBE[idx] = sig;
        }
    }
}

/// Saca hasta 7 bytes de la ENTRADA: `(n << 56) | empaquetado_LE`.
pub fn leer_entrada(idx: usize) -> u64 {
    if idx >= MAX_CONSOLAS {
        return 0;
    }
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        while n < 7 && IN_LEE[idx] != IN_ESCRIBE[idx] {
            w[n] = IN_BUF[idx][IN_LEE[idx]];
            IN_LEE[idx] = (IN_LEE[idx] + 1) % ENTRADA;
            n += 1;
        }
        ((n as u64) << 56) | u64::from_le_bytes(w)
    }
}

/// ¿Hay algun proceso cuya salida vaya a esta consola?
pub fn hay_hijo(idx: usize) -> bool {
    unsafe {
        let tabla = &*core::ptr::addr_of!(SALIDA);
        tabla.iter().any(|e| e.0 != SIN_DUENO && e.1 == idx)
    }
}

/// Despacho de las operaciones sobre un handle de consola.
pub fn operacion(idx: u64, operacion: u64, arg0: u64) -> Option<u64> {
    let i = idx as usize;
    match operacion {
        CONSOLA_OP_LEER => Some(leer(i)),
        CONSOLA_OP_PERDIDOS => Some(perdidos(i)),
        CONSOLA_OP_ESCRIBIR => {
            let w = arg0.to_le_bytes();
            let n = w.iter().position(|&b| b == 0).unwrap_or(8);
            escribir_entrada(i, &w[..n]);
            Some(0)
        }
        CONSOLA_OP_HAY_HIJO => Some(hay_hijo(i) as u64),
        _ => None,
    }
}

/// Lo llama `cap::revoke_all`. Si muere el LECTOR, su consola se libera y los
/// hijos que escribían ahí vuelven al panel del kernel — su salida deja de
/// encauzarse, pero no desaparece. Si muere un escritor, sólo se suelta su
/// entrada de la tabla.
pub fn proceso_muerto(pid: u32) {
    unsafe {
        for i in 0..MAX_CONSOLAS {
            if LECTOR[i] == pid {
                LECTOR[i] = SIN_DUENO;
                LEE[i] = 0;
                ESCRIBE[i] = 0;
                IN_LEE[i] = 0;
                IN_ESCRIBE[i] = 0;
            }
        }
        let tabla = &mut *core::ptr::addr_of_mut!(SALIDA);
        for e in tabla.iter_mut() {
            if e.0 == pid {
                *e = (SIN_DUENO, 0);
            }
        }
    }
}
