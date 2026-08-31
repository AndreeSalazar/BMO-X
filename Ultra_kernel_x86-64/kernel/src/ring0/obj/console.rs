//! `KIND_CONSOLE` -- la salida de un programa, como capability.
//!
//! [carril]  AMARILLO  la salida de un programa
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! ## La asimetria que esto cierra
//!
//! La pantalla es `KIND_FRAMEBUFFER`. La entrada es `KIND_INPUT`. La consola
//! era **lo unico que no lo era**: `OP_CONSOLE_WRITE` escribia siempre en el
//! mismo sitio global, el panel del kernel. El propio comentario del syscall lo
//! confesaba -- "la consola de arranque, no la salida de nadie en serio".
//!
//! Eso tenia una consecuencia que solo se ve cuando intentas construir encima:
//! **un terminal en Ring 3 no puede leer lo que imprime su propio hijo.** Lanza
//! un programa, el programa escribe, y su salida cae en el panel de Ring 0 --
//! debajo del escritorio, donde nadie mira. Se compila a ciegas.
//!
//! Es exactamente el problema que resuelve un PTY en Unix, y aqui tiene la
//! forma que tiene todo lo demas: un objeto con dueno.
//!
//! ## El trato
//!
//! - Un proceso **crea** una consola y recibe un handle de LECTURA. Es el
//!   terminal: la consola es suya y la drena a su ritmo.
//! - Al **lanzar** un hijo puede pasarle esa consola. Desde ese momento el
//!   `OP_CONSOLE_WRITE` del hijo aterriza en ese anillo, no en el panel.
//! - Sin consola asignada, se escribe en la del kernel **exactamente como
//!   antes**. Los cinco demos embebidos siguen hablando por el panel sin
//!   cambiar una linea: lo nuevo no rompe lo viejo, lo rodea.
//!
//! ## Por que el anillo es del kernel y no memoria compartida
//!
//! Un estuario (`KIND_CHANNEL`) seria mas rapido y algun dia sera lo correcto.
//! Pero el escritor es un proceso que puede morir a mitad de linea, y el lector
//! otro que puede no existir todavia. Un anillo pequeno en el kernel hace que
//! ninguno de los dos pueda corromper al otro, y que la salida de un programa
//! sobreviva a que su terminal se cierre. Cuando el RPC de endpoints este
//! rodado, esto se muda; el contrato de fuera no cambia.

use crate::ring0::obj::cap;

/// Cuantas consolas pueden existir a la vez. Una por terminal abierto.
pub const MAX_CONSOLAS: usize = 4;
/// Bytes de salida que aguanta cada una antes de descartar lo mas viejo.
const RING: usize = 2048;

pub const NO_OWNER: u32 = u32::MAX;

/// No quedan consolas libres.
pub const ERROR_NO_FREE_SLOT: u32 = 24;

/// Leer hasta **7** bytes: `(n << 56) | bytes_LE`, con `n` = cuantos son
/// validos. `n == 0` = no hay nada.
///
/// * Siete y no ocho, y el contador ARRIBA. La primera version devolvia
/// `(n << 32) | ocho_bytes` -- y eso **pisa el byte 4**: ocho bytes ocupan el
/// u64 entero y no dejan sitio para decir cuantos valen. Un byte de cada ocho
/// habria salido corrupto, en una ruta que solo se nota leyendo texto raro.
/// Se paga un byte de ancho de banda por tener un contador honesto.
pub const CONSOLA_OP_LEER: u64 = 0x01;
/// Cuantos bytes se han descartado por anillo lleno. Un terminal que va lento
/// tiene derecho a saber que esta perdiendo salida en vez de creerse completo.
pub const CONSOLA_OP_PERDIDOS: u64 = 0x02;

/// El TERMINAL mete 8 bytes (LE, el cero corta) en el anillo de ENTRADA.
///
/// Es el segundo sentido del canal, y sin el no puede haber `ACCEPT`: un
/// programa lanzado desde la caja no puede reclamar `KIND_INPUT` --la tiene el
/// compositor-- asi que su unica via para recibir teclas es por el mismo objeto
/// que ya usa para hablar. Un canal de un solo sentido deja al hijo mudo de
/// oido.
pub const CONSOLA_OP_ESCRIBIR: u64 = 0x03;
/// Hay algun proceso escribiendo a esta consola ahora mismo?
///
/// Lo pregunta el terminal para saber a donde mandar lo que se teclea: si hay
/// hijo vivo, la linea es PARA EL; si no, es un comando. Sin esto habria que
/// inventar un prefijo o un modo, y las dos cosas se olvidan.
pub const CONSOLA_OP_HAY_HIJO: u64 = 0x04;

/// Anillo de ENTRADA, mucho mas pequeno que el de salida: aqui cabe lo que una
/// persona teclea, no lo que un programa escupe.
const ENTRADA: usize = 256;
static mut IN_BUF: [[u8; ENTRADA]; MAX_CONSOLAS] = [[0; ENTRADA]; MAX_CONSOLAS];
static mut IN_LEE: [usize; MAX_CONSOLAS] = [0; MAX_CONSOLAS];
static mut IN_ESCRIBE: [usize; MAX_CONSOLAS] = [0; MAX_CONSOLAS];

static mut BUF: [[u8; RING]; MAX_CONSOLAS] = [[0; RING]; MAX_CONSOLAS];
static mut LEE: [usize; MAX_CONSOLAS] = [0; MAX_CONSOLAS];
static mut WRITES: [usize; MAX_CONSOLAS] = [0; MAX_CONSOLAS];
static mut PERDIDOS: [u32; MAX_CONSOLAS] = [0; MAX_CONSOLAS];
/// Pid del LECTOR (el terminal). `NO_OWNER` = ranura libre.
static mut LECTOR: [u32; MAX_CONSOLAS] = [NO_OWNER; MAX_CONSOLAS];

/// A que consola escribe cada proceso. `(pid, indice)`; pid `NO_OWNER` = vacio.
///
/// Tabla aparte y no un campo del proceso a proposito: el planificador no tiene
/// por que saber de consolas, y esto se consulta solo en el borde del syscall.
static mut SALIDA: [(u32, usize); MAX_CONSOLAS * 4] = [(NO_OWNER, 0); MAX_CONSOLAS * 4];

/// Crea una consola y entrega su handle de lectura a `pid`.
pub fn create(pid: u32) -> Result<u64, u32> {
    unsafe {
        let libre = (0..MAX_CONSOLAS).find(|&i| LECTOR[i] == NO_OWNER);
        let i = match libre {
            Some(i) => i,
            None => return Err(ERROR_NO_FREE_SLOT),
        };
        LEE[i] = 0;
        WRITES[i] = 0;
        PERDIDOS[i] = 0;
        LECTOR[i] = pid;
        match cap::grant(pid, cap::KIND_CONSOLE, cap::RIGHT_READ, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("consola", "consola creada para Ring 3", pid as u64);
                Ok(h)
            }
            None => {
                LECTOR[i] = NO_OWNER;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

/// Manda la salida de `pid` a la consola `idx`. La llama el lanzador cuando un
/// terminal entrega su consola a un hijo.
pub fn assign_output(pid: u32, idx: usize) {
    if idx >= MAX_CONSOLAS {
        return;
    }
    unsafe {
        let tabla = &mut *core::ptr::addr_of_mut!(SALIDA);
        // Si ya tenia una asignada, se reemplaza en su sitio.
        for e in tabla.iter_mut() {
            if e.0 == pid {
                e.1 = idx;
                return;
            }
        }
        for e in tabla.iter_mut() {
            if e.0 == NO_OWNER {
                *e = (pid, idx);
                return;
            }
        }
        // Sin hueco en la tabla: el hijo escribe al panel del kernel. Se pierde
        // el encauzado, no la salida -- y eso es lo correcto: mejor verla en el
        // sitio de siempre que no verla.
        crate::ring0::cabina::warn("consola", "sin hueco para encauzar la salida", pid as u64);
    }
}

/// A que consola escribe `pid`, si es que escribe a alguna.
pub fn output_of(pid: u32) -> Option<usize> {
    unsafe {
        let tabla = &*core::ptr::addr_of!(SALIDA);
        for e in tabla.iter() {
            if e.0 == pid {
                // Una consola cuyo lector murio ya no encauza a nadie.
                if LECTOR[e.1] == NO_OWNER {
                    return None;
                }
                return Some(e.1);
            }
        }
        None
    }
}

/// Mete bytes en el anillo. Si esta lleno, se descarta lo MAS VIEJO y se
/// cuenta: en una consola, la linea que acabas de imprimir importa mas que la
/// de hace dos mil bytes.
pub fn write(idx: usize, datos: &[u8]) {
    if idx >= MAX_CONSOLAS {
        return;
    }
    unsafe {
        for &b in datos {
            let sig = (WRITES[idx] + 1) % RING;
            if sig == LEE[idx] {
                // Lleno: avanza el lector, o sea que se pierde el byte mas
                // antiguo. Se anota para que el terminal pueda decirlo.
                LEE[idx] = (LEE[idx] + 1) % RING;
                PERDIDOS[idx] = PERDIDOS[idx].saturating_add(1);
            }
            BUF[idx][WRITES[idx]] = b;
            WRITES[idx] = sig;
        }
    }
}

/// Saca hasta 7 bytes: `(n << 56) | empaquetado_LE`. Ver `CONSOLA_OP_LEER`.
pub fn read(idx: usize) -> u64 {
    if idx >= MAX_CONSOLAS {
        return 0;
    }
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        while n < 7 && LEE[idx] != WRITES[idx] {
            w[n] = BUF[idx][LEE[idx]];
            LEE[idx] = (LEE[idx] + 1) % RING;
            n += 1;
        }
        // w[7] queda a cero por construccion: el bucle para en 7.
        ((n as u64) << 56) | u64::from_le_bytes(w)
    }
}

pub fn dropped(idx: usize) -> u64 {
    if idx >= MAX_CONSOLAS {
        return 0;
    }
    unsafe { PERDIDOS[idx] as u64 }
}

/// Mete bytes en el anillo de ENTRADA. Si esta lleno se descartan los NUEVOS
/// --al reves que la salida-- porque aqui el orden es el que tecleo una persona:
/// tirar lo viejo dejaria media linea sin principio.
pub fn write_entry(idx: usize, datos: &[u8]) {
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
///
/// ** Y NUNCA CRUZA UN SALTO DE LINEA. Eso es parte del contrato, no una
/// optimizacion, y sin ello el que lee lineas no puede ser correcto.
///
/// === El fallo que lo obligo, que llevaba aqui desde siempre ===
///
/// `bmo-lower::console::read_line` --lo que emite un `ACCEPT` de COBOL-- pide
/// un paquete, lo desempaqueta byte a byte y al ver el `\n` da la linea por
/// cerrada. **Lo que quedaba del paquete se perdia**: la llamada siguiente pide
/// uno NUEVO, y esos bytes no vuelven.
///
/// Con la calculadora del escritorio eso era mortal. El compositor escribe sus
/// tres lineas de golpe --`12.50\n3\n4\n`-- y despues lanza el motor, asi que
/// los diez bytes ya estan en este anillo antes de la primera lectura:
///
/// ```text
///   sin esta regla   paquete 1 = "12.50\n3"   -> el ACCEPT se queda "12.50"
///                                                y TIRA el 3, que era la
///                                                operacion que se pedia
/// ```
///
/// El motor contestaba una cuenta que nadie habia pedido, sin dar error.
///
/// === Por que se arregla AQUI y no en el que lee ===
///
/// Un lector que no pierda el sobrante necesita **guardarlo entre llamadas**, y
/// el codigo que emite el compilador no tiene donde: cada `ACCEPT` es una
/// emision independiente, sin estado que sobreviva. Arreglarlo ahi obligaria a
/// dar memoria persistente a todos los lectores presentes y futuros.
///
/// Aqui es una comparacion. Y el que lee bytes en crudo no pierde nada: sigue
/// recibiendo todo, solo que en paquetes que acaban donde acaba una linea.
pub fn read_entry(idx: usize) -> u64 {
    if idx >= MAX_CONSOLAS {
        return 0;
    }
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        while n < 7 && IN_LEE[idx] != IN_ESCRIBE[idx] {
            let b = IN_BUF[idx][IN_LEE[idx]];
            IN_LEE[idx] = (IN_LEE[idx] + 1) % ENTRADA;
            w[n] = b;
            n += 1;
            // El salto SE ENTREGA --forma parte de la linea para quien la
            // cierra-- y es lo ultimo que va en este paquete.
            if b == b'\n' {
                break;
            }
        }
        ((n as u64) << 56) | u64::from_le_bytes(w)
    }
}

/// Hay algun proceso cuya salida vaya a esta consola?
pub fn has_child(idx: usize) -> bool {
    unsafe {
        let tabla = &*core::ptr::addr_of!(SALIDA);
        tabla.iter().any(|e| e.0 != NO_OWNER && e.1 == idx)
    }
}

/// Despacho de las operaciones sobre un handle de consola.
pub fn operation(idx: u64, operation: u64, arg0: u64) -> Option<u64> {
    let i = idx as usize;
    match operation {
        CONSOLA_OP_LEER => Some(read(i)),
        CONSOLA_OP_PERDIDOS => Some(dropped(i)),
        CONSOLA_OP_ESCRIBIR => {
            let w = arg0.to_le_bytes();
            let n = w.iter().position(|&b| b == 0).unwrap_or(8);
            write_entry(i, &w[..n]);
            Some(0)
        }
        CONSOLA_OP_HAY_HIJO => Some(has_child(i) as u64),
        _ => None,
    }
}

/// Lo llama `cap::revoke_all`. Si muere el LECTOR, su consola se libera y los
/// hijos que escribian ahi vuelven al panel del kernel -- su salida deja de
/// encauzarse, pero no desaparece. Si muere un escritor, solo se suelta su
/// entrada de la tabla.
pub fn process_died(pid: u32) {
    unsafe {
        for i in 0..MAX_CONSOLAS {
            if LECTOR[i] == pid {
                LECTOR[i] = NO_OWNER;
                LEE[i] = 0;
                WRITES[i] = 0;
                IN_LEE[i] = 0;
                IN_ESCRIBE[i] = 0;
            }
        }
        let tabla = &mut *core::ptr::addr_of_mut!(SALIDA);
        for e in tabla.iter_mut() {
            if e.0 == pid {
                *e = (NO_OWNER, 0);
            }
        }
    }
}
