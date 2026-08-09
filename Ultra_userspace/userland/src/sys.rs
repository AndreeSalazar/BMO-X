//! Hablar con el kernel: la puerta, quien soy, y lo que contesta.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

/// Lo que devuelve un syscall: un codigo y un valor.
///
/// `code == 0` es lo unico que significa exito. `flags` lleva pistas del
/// kernel -- por ejemplo `NEEDS_CAP`, que distingue "no tienes permiso" de
/// "ese handle no existe".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Status {
    pub code: u32,
    pub flags: u32,
    pub value: u64,
}

impl Status {
    #[inline(always)]
    pub fn ok(self) -> bool {
        self.code == 0
    }
    /// El valor si fue bien, o `None`. Para no comprobar el codigo a mano
    /// cada vez y acabar olvidandolo una.
    #[inline(always)]
    pub fn valor(self) -> Option<u64> {
        if self.code == 0 {
            Some(self.value)
        } else {
            None
        }
    }
}

#[inline(always)]
fn syscall(nr: u32, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> Status {
    let rax: u64;
    let rdx: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr as u64 => rax,
            in("rdi") a0,
            in("rsi") a1,
            inlateout("rdx") a2 => rdx,
            in("r10") a3,
            in("r8") a4,
            // El CPU los machaca: rcx = RIP de retorno, r11 = RFLAGS.
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    Status {
        code: rax as u32,
        flags: (rax >> 32) as u32,
        value: rdx,
    }
}

/// `INVOKE` -- la puerta sincrona.
#[inline(always)]
pub fn invoke(cap: u64, operation: u32, a0: u64, a1: u64, a2: u64) -> Status {
    syscall(NR_INVOKE, cap, operation as u64, a0, a1, a2)
}

/// `CHANNEL_KICK` -- avisar al consumidor de un estuario.
#[inline(always)]
pub fn channel_kick(cap: u64, secuencia: u64) -> Status {
    syscall(NR_CHANNEL_KICK, cap, secuencia, 0, 0, 0)
}

/// `WAIT` -- bloquearse hasta que la secuencia del esperable pase de `visto`,
/// o hasta que venza el plazo. `esperable = 0` es dormir a secas.
#[inline(always)]
pub fn wait(esperable: u64, visto: u64, timeout_ns: u64) -> Status {
    syscall(NR_WAIT, esperable, visto, timeout_ns, 0, 0)
}

// -- Lo que uno tiene por ser quien es -----------------------------------

#[inline]
pub fn pid() -> u64 {
    invoke(CURRENT_TASK, OP_GET_PID, 0, 0, 0).value
}

#[inline]
pub fn tid() -> u64 {
    invoke(CURRENT_TASK, OP_GET_TID, 0, 0, 0).value
}

/// Ceder el turno. Un bucle de espera en Ring 3 que no cede se come el quantum
/// entero sin avanzar nada.
#[inline]
pub fn yield_screen() {
    invoke(CURRENT_TASK, OP_YIELD, 0, 0, 0);
}

/// * El contador de ciclos del CPU. **No es privilegiado: Ring 3 puede.**
///
/// Vive aqui y no en una escena del compositor porque es una primitiva de la
/// maquina, no de una pantalla -- y porque ya habia una copia privada en
/// `escena::entrada` y una segunda copia habria sido la tercera.
///
/// # Para que sirve de verdad
///
/// Los tres syscalls congelados no traen reloj, asi que sin esto la unica forma
/// de esperar es **contar vueltas de bucle** -- y eso da una espera de dos
/// segundos en un Ryzen y de veinte en algo mas lento, que es como se hacian las
/// cosas cuando no habia forma de saber la hora. Con `rdtsc` y la frecuencia que
/// el kernel publica en [`crate::INFO_TSC_HZ`], una espera de 900 ms es de 900 ms
/// **en esta maquina y en la siguiente**.
#[inline]
pub fn ciclos() -> u64 {
    let (hi, lo): (u32, u32);
    unsafe {
        core::arch::asm!("rdtsc", out("edx") hi, out("eax") lo, options(nomem, nostack));
    }
    ((hi as u64) << 32) | lo as u64
}

/// Terminar. No vuelve: el kernel revoca las capabilities del proceso y
/// cambia de contexto en el propio borde del syscall.
pub fn salir() -> ! {
    invoke(CURRENT_TASK, OP_EXIT, 0, 0, 0);
    // Si el kernel nos devolviera el control, seguir ejecutando seria peor
    // que quedarse quieto.
    loop {
        yield_screen();
    }
}

/// Un dato numerico del sistema. `0` si el kernel no sabe contestar ese campo.
///
/// Cuanta RAM hay, cuantos hilos tiene el CPU, cuantas ranuras de tarea quedan.
/// Esto vivia **solo** en el shell de Ring 0 --`info`, `cpu`, `mem`-- y no porque
/// hiciera falta el privilegio: porque los datos estaban a su alcance. Leer un
/// contador no ejerce ningun poder.
#[inline]
pub fn info(campo: u64) -> u64 {
    invoke(CURRENT_TASK, OP_INFO, campo, 0, 0).value
}

// -- El log del kernel, leido desde aqui ---------------------------------
//
// * Esto NO es un salto a Ring 0, y la diferencia importa: no se ejecuta nada
// privilegiado, se piden bytes de texto. El kernel contesta y no cede nada,
// igual que con `info`. Ver `ring0/core/klog.rs`.

/// Cuantas lineas del log del kernel se pueden leer ahora mismo.
pub fn klog_lineas() -> u64 {
    invoke(CURRENT_TASK, OP_KLOG_INFO, 0, 0, 0).value
}

/// Cuantas ha escrito el kernel desde el arranque. La resta con
/// [`klog_lineas`] son las que se cayeron por el borde del anillo -- y decirlo
/// es lo que separa "no paso nada mas" de "no cabia".
pub fn klog_total() -> u64 {
    invoke(CURRENT_TASK, OP_KLOG_INFO, 1, 0, 0).value
}

// -- LA AUTOPSIA de un fallo de Ring 3 -----------------------------------
//
// El klog cuenta el relato de la maquina; esto es el INFORME de cada muerte:
// vector, codigo de error en palabras, la direccion que se toco, el `rip`, la
// pila, **que programa era** y lo ultimo que llego a escribir.
//
// Mismo trato que el klog: contesta texto y no concede nada. Ver
// `ring0/core/autopsia.rs` -- el kernel captura en RAM porque escribir a disco
// dentro de un fault es entrar en el driver que quiza acaba de caerse.

/// Cuantos fallos de Ring 3 van desde el arranque.
///
/// **Este es el numero que se mira en bucle.** Si cambio hay una autopsia
/// nueva, y eso se sabe sin leer un solo renglon: una comparacion de enteros
/// por fotograma en vez de un informe por fotograma.
pub fn autopsia_total() -> u64 {
    invoke(CURRENT_TASK, OP_AUTOPSIA_INFO, AUTOPSIA_TOTAL, 0, 0).value
}

/// Cuantos informes se pueden leer ahora mismo.
pub fn autopsia_disponibles() -> u64 {
    invoke(CURRENT_TASK, OP_AUTOPSIA_INFO, AUTOPSIA_DISPONIBLES, 0, 0).value
}

/// Cuantos renglones tiene el informe `n` (**0 = el mas reciente**).
pub fn autopsia_renglones(n: u64) -> u64 {
    invoke(CURRENT_TASK, OP_AUTOPSIA_INFO, AUTOPSIA_RENGLONES, n, 0).value
}

/// Copia el renglon `fila` del informe `n` en `dst`. Devuelve cuantos bytes.
///
/// Los dos indices viajan empaquetados en un solo argumento --informe arriba,
/// fila abajo-- porque la puerta tiene tres y dos los ocupan la operacion y el
/// trozo. Es la misma aritmetica que usa la entrada para el raton.
pub fn autopsia_linea(n: u64, fila: u64, dst: &mut [u8]) -> usize {
    let idx = (n << 32) | fila;
    let mut escritos = 0usize;
    let mut trozo = 0u64;
    // Tope de trozos: el renglon mide 72 bytes, o sea nueve palabras. Con
    // dieciseis sobra, y que sea finito es lo que impide que un kernel que
    // conteste raro cuelgue al escritorio.
    while trozo < 16 {
        let w = invoke(CURRENT_TASK, OP_AUTOPSIA_TEXTO, idx, trozo, 0).value;
        if w == 0 {
            break;
        }
        for b in w.to_le_bytes() {
            if b == 0 || escritos >= dst.len() {
                return escritos;
            }
            dst[escritos] = b;
            escritos += 1;
        }
        trozo += 1;
    }
    escritos
}

/// **Despierta los otros nucleos.** Devuelve `(alive, esperados)`, sin contar
/// el que ejecuta esto.
///
/// * Existe porque el comando `smp` vivia solo en el shell de Ring 0, y ese
/// shell **deja de leer el teclado** en cuanto el compositor reclama la
/// entrada. O sea: habia codigo que no se podia ejecutar desde donde se esta
/// sentado. Un mando al que no se llega es un mando que no existe.
///
/// [!] **Bloquea**, y bastante: hasta ~10 ms por nucleo, mas la espera final. Es
/// la unica llamada del userland que puede tardar un segundo entero, asi que
/// quien la use deberia pintar el aviso **antes** y no despues.
/// `cuantos`: **0 no despierta a nadie** y solo contesta el censo, `u32::MAX`
/// despierta a todos, y cualquier otro numero despierta exactamente esos.
///
/// * Que se pueda pedir un numero, y que el 0 sea inofensivo, es lo que separa
/// un boton de un mando. Mandar INIT+SIPI es la unica operacion del sistema que
/// cambia el hardware de forma que no se deshace sin reiniciar: se dispara **a
/// proposito**, no por escribir su nombre.
pub fn smp_despertar(cuantos: u32) -> (u32, u32) {
    let v = invoke(CURRENT_TASK, OP_SMP_DESPERTAR, cuantos as u64, 0, 0).value;
    ((v >> 32) as u32, v as u32)
}

/// **Ofrece un trozo de un bloque MIO a otra tarea.** `true` si quedo apuntado.
///
/// `bloque` es el handle de la memoria propia; `desde`/`bytes`, el trozo; `tid`,
/// a quien va -- el que devuelve `ejecutar_en`.
///
/// * Esto es lo que hace posible el LIENZO sin que el kernel sepa que es un
/// lienzo. El compositor ofrece la parte de abajo de su lienzo, la app la toma,
/// y pinta ahi directamente: **cero copias**. Y la misma operacion sirve para
/// audio, captura o cualquier bloque grande entre procesos.
///
/// Quien decide cuanto y a quien es **quien presta**, no el kernel. El kernel
/// solo comprueba que el bloque sea tuyo y que el trozo quepa dentro.
pub fn offer(bloque: u64, desde: u64, bytes: u64, tid: u32) -> bool {
    invoke(bloque, MEM_OP_OFRECER, desde, bytes, tid as u64).value != 0
}

/// **Toma lo que otro me haya ofrecido.** Devuelve `(base, bytes)`, o `None`.
///
/// El mapeo ocurre dentro de esta llamada, en el espacio de direcciones de
/// quien la hace. A partir de aqui se escribe con un `mov` normal: el kernel no
/// vuelve a enterarse, que es el punto entero de prestar memoria.
pub fn tomar_prestado() -> Option<(u64, u64)> {
    let h = invoke(CURRENT_TASK, OP_TOMAR, 0, 0, 0).value;
    if h == 0 {
        return None;
    }
    let base = invoke(h, PRESTADO_OP_BASE, 0, 0, 0).value;
    let bytes = invoke(h, PRESTADO_OP_BYTES, 0, 0, 0).value;
    if base == 0 || bytes == 0 {
        None
    } else {
        Some((base, bytes))
    }
}

/// **Desactiva los obreros**: vuelven a `hlt` y ahi se quedan.
///
/// La otra mitad del mando. Un obrero en espera **gira**, no duerme --sacarlo de
/// `hlt` pediria una IPI, y para atenderla haria falta GS por-CPU--, asi que con
/// los doce en pie hay once nucleos al 100 %. Esto es lo que lo apaga.
pub fn smp_parar() {
    let _ = invoke(CURRENT_TASK, OP_SMP_DESPERTAR, 0, 1, 0);
}

/// **La prueba de reparto.** Devuelve la aceleracion **x100**: `842` son 8,42x.
///
/// Corre la misma cuenta pura con un nucleo y con todos. Es el caso MAS
/// favorable que existe --sin memoria compartida ni bloqueos--, asi que el numero
/// que salga es **el techo** y no lo que dara un programa de verdad.
pub fn smp_prueba() -> u64 {
    invoke(CURRENT_TASK, OP_SMP_DESPERTAR, 0, 2, 0).value
}

/// **Cierra una transaccion vacia en ESTRATOS.** Devuelve la generacion nueva,
/// o **0** si no se pudo.
///
/// * Es la primera llamada de todo el userland que **ESCRIBE EN EL DISCO**, y
/// lo hace de la forma mas pequena que existe: sin datos, apuntando al mismo
/// estrato, y sobre la copia del superbloque que no manda. Si sale mal, el
/// volumen es exactamente el de antes.
///
/// El motivo del fallo no vuelve por aqui -- vuelve por CABINA y se lee con
/// **F11**. Es a proposito: caben mas motivos en una linea de log que en un
/// codigo de retorno, y el que la llama ya tiene la ventana para leerlos.
pub fn estratos_sellar() -> u64 {
    invoke(CURRENT_TASK, OP_ESTRATOS_SELLAR, 0, 0, 0).value
}


// -- CABINA: lo que el kernel ve, CON severidad --------------------------
//
// El klog ya se leia y es util, pero es la transcripcion en texto plano: no
// lleva severidad ni capa. Con esto una linea del SMP se puede pintar en su
// color y separar de las veinte lineas verdes que la rodean, que es justo lo
// que hace falta para leer un arranque de un vistazo.
//
// **No concede nada.** Ni una de estas llamadas escribe: ver y poder son cosas
// separadas, y esta es la mitad de mirar.

/// Cuantos eventos se pueden leer AHORA (el anillo son 48).
pub fn cabina_disponibles() -> u64 {
    invoke(CURRENT_TASK, OP_CABINA_INFO, CABINA_DISPONIBLES, 0, 0).valor().unwrap_or(0)
}

/// Cuantos ha habido desde el arranque, y cuantos se cayeron del anillo.
///
/// Los perdidos valen tanto como los que quedan: un anillo que dio la vuelta y
/// no lo dice hace creer que el arranque empezo donde empieza el primero que
/// sobrevive.
pub fn cabina_total() -> u64 {
    invoke(CURRENT_TASK, OP_CABINA_INFO, CABINA_TOTAL, 0, 0).valor().unwrap_or(0)
}

pub fn cabina_perdidos() -> u64 {
    invoke(CURRENT_TASK, OP_CABINA_INFO, CABINA_PERDIDOS, 0, 0).valor().unwrap_or(0)
}

/// Un campo del evento `n` (0 = el mas reciente). `None` si ese evento no
/// existe -- que NO es lo mismo que un campo a cero.
pub fn cabina_campo(campo: u64, n: u64) -> Option<u64> {
    invoke(CURRENT_TASK, OP_CABINA_INFO, campo, n, 0).valor()
}

/// La severidad del evento `n`: `SEV_INFO`..`SEV_PANIC`. Es lo que el klog no
/// podia dar.
pub fn cabina_severidad(n: u64) -> u64 {
    cabina_campo(CABINA_SEVERIDAD, n).unwrap_or(SEV_INFO)
}

/// El modulo o el mensaje del evento `n`, copiado en `dst`. Devuelve cuantos
/// bytes se escribieron.
///
/// Llega de 8 en 8 porque la superficie congelada no acepta punteros: el texto
/// viaja por valor, igual que en el klog y en la autopsia.
pub fn cabina_texto(n: u64, cual: u64, dst: &mut [u8]) -> usize {
    let mut escritos = 0usize;
    let mut trozo = 0u64;
    while escritos < dst.len() {
        let arg0 = (n << 32) | cual;
        let w = match invoke(CURRENT_TASK, OP_CABINA_TEXTO, arg0, trozo, 0).valor() {
            Some(v) => v,
            None => break,
        };
        if w == 0 {
            break;
        }
        let bytes = w.to_le_bytes();
        for b in bytes {
            if b == 0 || escritos >= dst.len() {
                return escritos;
            }
            dst[escritos] = b;
            escritos += 1;
        }
        trozo += 1;
    }
    escritos
}
