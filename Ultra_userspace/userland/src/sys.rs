//! Hablar con el kernel: la puerta, quien soy, y lo que contesta.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

/// Lo que devuelve un syscall: un código y un valor.
///
/// `code == 0` es lo único que significa éxito. `flags` lleva pistas del
/// kernel — por ejemplo `NEEDS_CAP`, que distingue "no tienes permiso" de
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
    /// El valor si fue bien, o `None`. Para no comprobar el código a mano
    /// cada vez y acabar olvidándolo una.
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

/// `INVOKE` — la puerta síncrona.
#[inline(always)]
pub fn invoke(cap: u64, operacion: u32, a0: u64, a1: u64, a2: u64) -> Status {
    syscall(NR_INVOKE, cap, operacion as u64, a0, a1, a2)
}

/// `CHANNEL_KICK` — avisar al consumidor de un estuario.
#[inline(always)]
pub fn channel_kick(cap: u64, secuencia: u64) -> Status {
    syscall(NR_CHANNEL_KICK, cap, secuencia, 0, 0, 0)
}

/// `WAIT` — bloquearse hasta que la secuencia del esperable pase de `visto`,
/// o hasta que venza el plazo. `esperable = 0` es dormir a secas.
#[inline(always)]
pub fn wait(esperable: u64, visto: u64, timeout_ns: u64) -> Status {
    syscall(NR_WAIT, esperable, visto, timeout_ns, 0, 0)
}

// ── Lo que uno tiene por ser quien es ───────────────────────────────────

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
pub fn ceder() {
    invoke(CURRENT_TASK, OP_YIELD, 0, 0, 0);
}

/// Terminar. No vuelve: el kernel revoca las capabilities del proceso y
/// cambia de contexto en el propio borde del syscall.
pub fn salir() -> ! {
    invoke(CURRENT_TASK, OP_EXIT, 0, 0, 0);
    // Si el kernel nos devolviera el control, seguir ejecutando sería peor
    // que quedarse quieto.
    loop {
        ceder();
    }
}

/// Un dato numérico del sistema. `0` si el kernel no sabe contestar ese campo.
///
/// Cuánta RAM hay, cuántos hilos tiene el CPU, cuántas ranuras de tarea quedan.
/// Esto vivía **sólo** en el shell de Ring 0 —`info`, `cpu`, `mem`— y no porque
/// hiciera falta el privilegio: porque los datos estaban a su alcance. Leer un
/// contador no ejerce ningún poder.
#[inline]
pub fn info(campo: u64) -> u64 {
    invoke(CURRENT_TASK, OP_INFO, campo, 0, 0).value
}

// ── El log del kernel, leído desde aquí ─────────────────────────────────
//
// ★ Esto NO es un salto a Ring 0, y la diferencia importa: no se ejecuta nada
// privilegiado, se piden bytes de texto. El kernel contesta y no cede nada,
// igual que con `info`. Ver `ring0/core/klog.rs`.

/// Cuántas líneas del log del kernel se pueden leer ahora mismo.
pub fn klog_lineas() -> u64 {
    invoke(CURRENT_TASK, OP_KLOG_INFO, 0, 0, 0).value
}

/// Cuántas ha escrito el kernel desde el arranque. La resta con
/// [`klog_lineas`] son las que se cayeron por el borde del anillo — y decirlo
/// es lo que separa "no pasó nada más" de "no cabía".
pub fn klog_total() -> u64 {
    invoke(CURRENT_TASK, OP_KLOG_INFO, 1, 0, 0).value
}

/// **Cierra una transacción vacía en ESTRATOS.** Devuelve la generación nueva,
/// o **0** si no se pudo.
///
/// ★ Es la primera llamada de todo el userland que **ESCRIBE EN EL DISCO**, y
/// lo hace de la forma más pequeña que existe: sin datos, apuntando al mismo
/// estrato, y sobre la copia del superbloque que no manda. Si sale mal, el
/// volumen es exactamente el de antes.
///
/// El motivo del fallo no vuelve por aquí — vuelve por CABINA y se lee con
/// **F11**. Es a propósito: caben más motivos en una línea de log que en un
/// código de retorno, y el que la llama ya tiene la ventana para leerlos.
pub fn estratos_sellar() -> u64 {
    invoke(CURRENT_TASK, OP_ESTRATOS_SELLAR, 0, 0, 0).value
}

