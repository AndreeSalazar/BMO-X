//! **A que velocidad va este nucleo AHORA MISMO.**
//!
//! Escalon 1 de la seccion 9 de `docs/maestro/AXION_MAESTRO.md` -- la terminal del CPU.
//!
//! # Por que esto no lo contesta la topologia
//!
//! `INFO_CPU_HILOS` y `INFO_TSC_HZ` dicen lo que la maquina ES: doce hilos, TSC
//! a 3,70 GHz. Ninguno de los dos dice a que va **ahora**, y en un Zen 3 esa
//! diferencia es de gigahercios enteros: un nucleo solo bajo carga sube a 4,6
//! GHz y doce a la vez se quedan cerca de la base. El mismo CPU, el mismo
//! segundo, y dos numeros distintos.
//!
//! Y esa es justo la pregunta que hace falta para AXION: *"esta al 100% de
//! ocupacion"* y *"esta al 100% Y ADEMAS a 4,6 GHz"* no son la misma frase, y
//! hasta hoy el sistema solo sabia decir la primera.
//!
//! # Como se mide, y por que son dos lecturas y no una
//!
//! ```text
//!    MPERF   sube al ritmo de REFERENCIA (el mismo que el TSC)
//!    APERF   sube al ritmo REAL del nucleo
//!
//!    efectiva = TSC_HZ * (APERF2 - APERF1) / (MPERF2 - MPERF1)
//! ```
//!
//! ** Los dos son CONTADORES, no medidas. Leerlos una vez no dice nada: dan el
//! total desde que arranco la maquina, o sea la media de toda su vida. La
//! velocidad de AHORA es una **diferencia entre dos instantes**, y por eso este
//! modulo guarda la lectura anterior en vez de contestar de una sola pasada.
//!
//! [!] Y los dos **solo cuentan mientras el nucleo esta despierto** (estado C0).
//! Eso no es un defecto: significa que lo que sale es *"a que va cuando
//! trabaja"*, que es justo lo que se quiere saber. Un nucleo dormido no tiene
//! frecuencia que reportar.
//!
//! # La trampa que hay que comprobar antes de leer
//!
//! Estos dos MSR **no existen en todo x86**. Leerlos donde no estan es un `#GP`,
//! o sea un fault de kernel -- y este codigo lo llama un panel que se repinta.
//! El bit esta en `CPUID.06H:ECX[0]` y se pregunta UNA vez al arrancar; despues
//! solo se mira una bandera.
//!
//! Es la misma regla que ya se aplico con XSAVE: preguntarle al CPU antes de
//! usar algo suyo, no suponerlo por el nombre del fabricante.

use core::arch::asm;

/// `MPERF`: contador al ritmo de referencia.
const MSR_MPERF: u32 = 0x0000_00E7;
/// `APERF`: contador al ritmo real.
const MSR_APERF: u32 = 0x0000_00E8;

/// El CPU soporta la pareja MPERF/APERF? Se resuelve una vez en [`init`].
static mut HAY: bool = false;
/// Lecturas anteriores, para poder restar. `0` = todavia no hay ninguna.
static mut PREV_MPERF: u64 = 0;
static mut PREV_APERF: u64 = 0;
/// La ultima frecuencia calculada, en Hz. `0` = aun no se ha podido calcular.
static mut ULTIMA_HZ: u64 = 0;

unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack));
    ((hi as u64) << 32) | lo as u64
}

/// **Pregunta si este CPU sabe contestar, y se apunta la respuesta.**
///
/// Se llama una vez, en el arranque. Despues [`medir`] solo mira una bandera --
/// un panel que se repinta no puede costar un `cpuid` por fotograma.
pub fn init() {
    // CPUID.06H:ECX bit 0 = "Hardware Coordination Feedback Capability", que es
    // el nombre largo de "existen MPERF y APERF".
    let ecx: u32;
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "pop rbx",
            inout("eax") 6u32 => _,
            out("ecx") ecx,
            out("edx") _,
            options(nostack)
        );
    }
    let hay = ecx & 1 != 0;
    unsafe { HAY = hay };
    if hay {
        // La primera lectura se toma YA, para que la segunda --la del primer
        // panel que pregunte-- tenga con que restar. Sin esto, el primer numero
        // que ve el dueno seria siempre 0 y pareceria que no funciona.
        unsafe {
            PREV_MPERF = rdmsr(MSR_MPERF);
            PREV_APERF = rdmsr(MSR_APERF);
        }
        crate::ring0::cabina::info("cpu", "MPERF/APERF disponibles: se puede medir la frecuencia real", 1);
    } else {
        // Se dice, y no se calla: sin esto el panel mostraria 0 y no habria
        // forma de distinguir "no lo soporta" de "esta roto".
        crate::ring0::cabina::warn("cpu", "sin MPERF/APERF: la frecuencia real no se puede medir", 0);
    }
}

/// Lo soporta esta maquina?
pub fn disponible() -> bool {
    unsafe { HAY }
}

/// **La frecuencia efectiva desde la ultima vez que se pregunto**, en Hz.
///
/// `0` si no se puede medir todavia: o el CPU no lo soporta, o es la primera
/// llamada, o no ha pasado tiempo suficiente entre dos lecturas.
///
/// * Devolver `0` y no la nominal es deliberado. Un `3.700.000.000` inventado
/// cuando no se sabe nada es peor que un cero: el cero se ve raro y se
/// investiga, y una nominal plausible se cree y se usa para decidir cosas. Es
/// la misma regla que `Identidad::megabits`, que no se inventa una velocidad
/// cuando el enlace esta caido.
///
/// # Safety interna
/// Toca MSR, asi que solo puede correr en Ring 0. No hay guarda de reentrancia:
/// dos llamadas solapadas darian dos deltas partidos, no memoria corrupta -- y
/// el unico llamante es el camino de `INFO`, que corre en el BSP.
pub fn medir() -> u64 {
    if !unsafe { HAY } {
        return 0;
    }
    let (m, a) = unsafe { (rdmsr(MSR_MPERF), rdmsr(MSR_APERF)) };
    let (pm, pa) = unsafe { (PREV_MPERF, PREV_APERF) };
    unsafe {
        PREV_MPERF = m;
        PREV_APERF = a;
    }

    // ** Los contadores se DESBORDAN y se pueden reiniciar. `wrapping_sub` da la
    // diferencia correcta al dar la vuelta; lo que no se puede es dividir por
    // cero ni fiarse de una ventana ridicula.
    let dm = m.wrapping_sub(pm);
    let da = a.wrapping_sub(pa);

    // Una ventana demasiado corta da un cociente sin sentido: con cien ciclos
    // entre lecturas, un solo evento del planificador mueve el resultado un
    // 50%. Se contesta lo ultimo bueno en vez de un numero nervioso.
    const VENTANA_MINIMA: u64 = 100_000;
    if dm < VENTANA_MINIMA {
        return unsafe { ULTIMA_HZ };
    }

    let base = crate::ring0::task::scheduler::tsc_freq();
    if base == 0 {
        return 0;
    }
    // El orden importa: multiplicar ANTES de dividir. Al reves, `da/dm` es una
    // fraccion menor que uno y en enteros vale 0 -- y el resultado seria 0 Hz
    // siempre, que ademas parece un fallo del CPU y no de la aritmetica.
    //
    // Y se divide `base` primero para no desbordar: `da` puede ser del orden de
    // 10^10 y `base` de 3,7x10^9; su producto no cabe comodo en 64 bits.
    let hz = (base / 1000).saturating_mul(da) / (dm / 1000).max(1);
    unsafe { ULTIMA_HZ = hz };
    hz
}
