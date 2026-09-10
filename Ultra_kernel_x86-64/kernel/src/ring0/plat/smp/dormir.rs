//! **DORMIR EN VEZ DE GIRAR: lo que le faltaba a AXION para poder ENCENDER.**
//!
//! [carril]  ROJO      duerme un nucleo. Si no despierta, ese nucleo no vuelve
//!
//! [cuesta]  MAQUINA -- un obrero que se duerme y no despierta deja a
//!           `crew::reparte` esperando en su barrera **para siempre**, y eso no
//!           es un nucleo perdido: es la maquina colgada, porque el BSP espera
//!           a todos (L6e)
//!
//! [riesgo]  SILENCIO -- un despertar perdido no da fault ni excepcion. El
//!           nucleo simplemente no vuelve, y lo unico que se ve es que el
//!           reparto no termina. Por eso este fichero **solo duerme cuando
//!           puede garantizar el despertar** -- ver el plazo (L6f)
//!
//! # *** EL PROBLEMA, DICHO POR `crew.rs` ANTES DE QUE EXISTIERA LA SOLUCION
//!
//! ```text
//!    "Un obrero en espera GIRA (`pause`), no duerme. Sacarlo de `hlt` pediria
//!     una IPI, y para atender una IPI un AP necesita GS por-CPU y su propia
//!     TSS -- que es justo el trabajo que este modulo evita. Consecuencia real
//!     y medible: con los doce en pie, once nucleos giran al 100 % y la maquina
//!     consume como si estuviera trabajando."
//! ```
//!
//! ** Y ahi esta la trampa que MONITOR/MWAIT deshace: **el obrero no espera una
//! interrupcion. Espera UNA ESCRITURA EN MEMORIA** -- que el BSP incremente
//! `RONDA`. `hlt` solo sabe despertar por interrupcion; `MWAIT` sabe despertar
//! **por escritura**, que es justo lo que hay.
//!
//! ```text
//!    hlt     duerme -> despierta con una IPI  -> pide GS por-CPU y TSS
//!    MWAIT   duerme -> despierta con un STORE -> no pide NADA
//! ```
//!
//! *** No es que MWAIT sea "la version buena de hlt". Es que **espera lo que
//! aqui de verdad se espera**. `hlt` era la herramienta equivocada, y por eso
//! su precio era tan alto.
//!
//! # ** EL PLAZO, Y POR QUE SIN EL NO SE DUERME
//!
//! `MWAIT` a secas no tiene despertador: si el `MONITOR` se rompe por lo que
//! sea --otra escritura en la misma linea, una transicion que lo desarma-- el
//! nucleo se queda ahi. Y un obrero que no vuelve **cuelga el reparto entero**,
//! porque `crew::reparte` espera a que esten todas las partes.
//!
//! AMD tiene la respuesta en el silicio: **`MWAITX`**, la variante con TIMEOUT
//! en `EBX`. El nucleo duerme, y si nadie escribe, **se despierta solo**.
//!
//! ```text
//!    hay MONITORX   -> MWAITX con plazo. Duerme, y despierta pase lo que pase
//!    solo MONITOR   -> NO SE DUERME. Se gira, como hasta hoy
//!    ninguno        -> NO SE DUERME
//! ```
//!
//! [!] La segunda fila es la decision de este fichero, y es deliberada: sin
//! plazo, dormir cambia *"once nucleos gastan de mas"* por *"la maquina puede
//! colgarse"*. **Se cambia un coste por un riesgo, y el coste era el que se
//! podia ver.**
//!
//! > Un ahorro que puede colgar la maquina no es un ahorro. Es una apuesta con
//! > la factura de la luz de premio.
//!
//! Este Ryzen 5 5600X es Zen 3 y trae `MONITORX` (CPUID 0x8000_0001, ECX bit
//! 29), asi que en la maquina del dueno se duerme de verdad. En una que no lo
//! traiga, esto no hace nada -- y lo dice.
//!
//! # El patron, y el orden NO es de gusto
//!
//! ```text
//!    1. MONITOR sobre la direccion      arma la vigilancia
//!    2. VOLVER A MIRAR el valor         *** y aqui esta todo
//!    3. MWAITX                          duerme
//! ```
//!
//! *** El paso 2 es el que hace correcto el patron. Entre que se decide dormir
//! y que se arma la vigilancia, el BSP puede haber publicado ya la ronda; sin
//! ese segundo vistazo, el obrero se duerme **con el trabajo delante** y no lo
//! ve hasta la siguiente. Con `MWAITX` eso costaria un plazo; sin el, seria
//! para siempre.
//!
//! Ver el apartado 5 de `docs/maestro/AXION_MAESTRO.md`, y `girando()` en
//! `smp/mod.rs`, que es el numero con el que se mide si esto sirvio.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// El plazo del `MWAITX`, en ticks del TSC. A 3,7 GHz son ~27 ms.
///
/// == *** POR QUE SUBIO x100 EL 2026-09-10 ============================
///
/// La primera version puso 1.000.000 (~0,27 ms), o sea **3.700 despertares
/// por segundo y nucleo**, con este argumento: *"vale mas despertar de mas
/// que arriesgarse a que `PARAR` tarde en verse"*.
///
/// ** Ese argumento era una tirita sobre otro fallo. `parar()` ahora TOCA
/// `RONDA`, asi que el obrero se entera **por escritura y al instante** -- que
/// es justo el mecanismo que este modulo existe para usar. Con eso, el plazo
/// deja de ser el despertador y vuelve a ser lo que tenia que ser: **una red
/// de seguridad por si el `MONITOR` se rompe**.
///
/// *** Y la diferencia importa de verdad, porque despertar de un C-state
/// PROFUNDO no es gratis: se vuelve a encender lo que se apago. Despertar
/// 3.700 veces por segundo para mirar un `u32` que no cambio es exactamente
/// *"comer electricidad sin sentido"* -- con mas pasos.
///
///   > Un plazo que despierta mas de lo que hace falta no es prudencia: es
///   > el bucle de espera de antes, disfrazado de siesta.
const PLAZO_TICKS: u32 = 100_000_000;

/// Veces que un obrero se ha dormido de verdad. `0` = no hay `MONITORX`, o
/// nadie ha esperado todavia.
static DORMIDAS: AtomicU64 = AtomicU64::new(0);
/// Ticks del TSC pasados DENTRO del `mwaitx`, sumando todos los obreros.
///
/// ** Es la mitad util del par: `DORMIDAS` dice cuantas veces, esto dice
/// cuanto. Mil siestas de un microsegundo no ahorran nada y con solo la
/// primera cuenta se verian igual de bien.
static TICKS_DORMIDOS: AtomicU64 = AtomicU64::new(0);
/// El `EAX` que se le pasa a `MWAITX`: **que tan profundo se duerme**.
/// `u32::MAX` = todavia no se ha mirado.
static PROFUNDIDAD: AtomicU32 = AtomicU32::new(u32::MAX);
/// Siestas que acabaron MUCHO antes del plazo, o sea que **algo desperto al
/// obrero**.
///
/// == *** EL NUMERO QUE CONVIERTE UNA SOSPECHA EN UN DATO ================
///
/// El arranque del 10-09 dio 12.502 siestas y 147 s dormidos: **11,7 ms de
/// media contra un plazo de 27**. O sea que mas de la mitad de las siestas se
/// cortaron, y con solo esas dos cuentas **no hay forma de saber por que**.
///
/// ** Un despertar temprano solo puede venir de tres sitios: trabajo de
/// verdad, una escritura en la MISMA LINEA de cache que `RONDA` (el falso
/// compartimiento que se arreglo el mismo dia), o una interrupcion. Esta
/// cuenta no dice cual -- dice **cuantas**, que es lo que hacia falta para
/// saber si el arreglo sirvio.
///
///   > Sin este contador, arreglar el falso compartimiento habria sido un
///   > cambio del que solo se puede decir que no rompio nada.
static SIESTAS_CORTAS: AtomicU64 = AtomicU64::new(0);

/// Si el silicio trae `MONITORX`. Se resuelve UNA vez, en el arranque.
static SE_PUEDE: AtomicU64 = AtomicU64::new(SIN_MIRAR);
const SIN_MIRAR: u64 = 0;
const NO: u64 = 1;
const SI: u64 = 2;

/// `CPUID` de una hoja, con `rbx` salvado a mano.
///
/// [!] `cpuid` ESCRIBE en `ebx`, y `ebx` es de los que LLVM no presta. Mismo
/// baile que en `esperar`, y por el mismo motivo.
fn cpuid(hoja: u32) -> (u32, u32, u32, u32) {
    let (a, b, c, d): (u32, u32, u32, u32);
    unsafe {
        core::arch::asm!(
            "mov {salvo:r}, rbx",
            "cpuid",
            "mov {sale:e}, ebx",
            "mov rbx, {salvo:r}",
            salvo = out(reg) _,
            sale = out(reg) b,
            inout("eax") hoja => a,
            inout("ecx") 0u32 => c,
            out("edx") d,
            options(nostack, preserves_flags),
        );
    }
    (a, b, c, d)
}

/// **QUE TAN PROFUNDO PUEDE DORMIR ESTE SILICIO**, preguntado y no supuesto.
///
/// # *** El fallo que corrige, y era mio
///
/// La primera version paso `EAX = 0` con esta nota: *"pide el estado mas
/// ligero, que es el que despierta antes -- aqui interesa reaccionar"*.
///
/// ** `EAX = 0` es **C1**, y C1 apenas apaga nada: el nucleo sigue con sus
/// relojes vivos. O sea que la version anterior dormia **de mentira**, y el
/// dueno lo olio: *"que duerma de verdad, que no este comiendo electricidad
/// sin sentido"*.
///
/// # Como se pregunta
///
/// `CPUID` hoja 5 enumera los sub-estados de `MWAIT`, cuatro bits por
/// C-state:
///
/// ```text
///    EDX bits  3:0   C0     ECX bit 0 = se pueden pedir extensiones
///        bits  7:4   C1               si es 0, EAX TIENE que ser 0
///        bits 11:8   C2
///        bits 15:12  C3
///        bits 19:16  C4
/// ```
///
/// Un grupo distinto de cero significa que ese C-state EXISTE aqui. Se elige
/// **el mas profundo que el silicio enumere**, y el `EAX` se arma como
/// `(cstate - 1) << 4` -- la convencion es que 0 pide C1, 1 pide C2, y asi.
///
/// [!] Y no se inventa ninguno. Pedir un C6 en un CPU que solo enumera hasta
/// C2 es pedirle al silicio algo que no dijo tener, que es LEY 24 al reves:
/// el hardware se PERFILA, y perfilarlo es preguntarle.
fn elegir_profundidad() -> u32 {
    let (max_basic, _, _, _) = cpuid(0);
    if max_basic < 5 {
        return 0;
    }
    let (_, _, ecx, edx) = cpuid(5);
    // ECX bit 0: si el silicio no admite extensiones, `EAX` tiene que ser 0.
    // Pedir profundidad sin ese bit es comportamiento no definido.
    if ecx & 1 == 0 {
        return 0;
    }
    // Del mas profundo al mas ligero: el primero que enumere sub-estados gana.
    let mut mejor = 0u32;
    for cstate in (1..=4u32).rev() {
        let grupo = (edx >> (cstate * 4)) & 0xF;
        if grupo != 0 {
            mejor = (cstate - 1) << 4;
            break;
        }
    }
    mejor
}

/// El `EAX` elegido, resuelto una vez.
pub fn profundidad() -> u32 {
    let v = PROFUNDIDAD.load(Ordering::Relaxed);
    if v != u32::MAX {
        return v;
    }
    let e = elegir_profundidad();
    PROFUNDIDAD.store(e, Ordering::Relaxed);
    e
}

/// **Se puede dormir en esta maquina?** Lo contesta el silicio, no una opcion.
///
/// [!] Se exige `MONITORX` y no solo `MONITOR`: ver el apartado del plazo en la
/// cabecera. Sin despertador no se duerme.
pub fn se_puede() -> bool {
    match SE_PUEDE.load(Ordering::Relaxed) {
        SI => true,
        NO => false,
        _ => {
            // ** `leer()` hace CPUID, asi que se hace UNA vez y se guarda:
            // esto lo pregunta un bucle que gira miles de veces por segundo.
            use crate::ring0::cpu_vendor::features::{silicon, Feat};
            let hay = silicon::has(Feat::Monitorx, &silicon::leer());
            SE_PUEDE.store(if hay { SI } else { NO }, Ordering::Relaxed);
            hay
        }
    }
}

/// **Espera a que `celda` deje de valer `visto`, sin girar.**
///
/// Vuelve cuando el valor cambio, cuando venci el plazo, o cuando al silicio le
/// dio la gana -- las tres son respuestas validas, y por eso quien llama tiene
/// que estar dentro de un bucle que vuelva a mirar. **Esto no promete que algo
/// cambio: promete no haber quemado un nucleo mientras tanto.**
///
/// Si la maquina no puede dormir, gira una vez y vuelve. Asi quien llama no
/// tiene que preguntar.
///
/// # Seguridad
///
/// `celda` tiene que apuntar a memoria valida y viva mientras dure la espera.
/// En su unico llamador es un `static`, o sea que vive lo que vive el kernel.
pub fn esperar(celda: &AtomicU32, visto: u32) {
    if !se_puede() {
        core::hint::spin_loop();
        return;
    }
    // Se resuelve ANTES de armar la vigilancia: entre el `monitor` y el
    // `mwaitx` no puede haber nada que toque memoria, o la vigilancia se cae.
    let hondo = profundidad();
    let dir = celda as *const _ as usize;
    unsafe {
        // 1. Armar la vigilancia sobre la linea de esa direccion.
        //    ECX = extensiones (0), EDX = pistas (0).
        core::arch::asm!(
            "monitor",
            in("rax") dir,
            in("ecx") 0,
            in("edx") 0,
            options(nostack, preserves_flags),
        );
        // 2. *** VOLVER A MIRAR. Si el BSP publico entre la decision y el
        //    `monitor`, el trabajo ya esta ahi y dormirse seria perderlo.
        if celda.load(Ordering::SeqCst) != visto {
            return;
        }
        // 3. Dormir. `ECX` bit 1 = usar `EBX` como plazo; `EAX` dice CUANTO
        //    se apaga, y sale de preguntarle al silicio -- ver
        //    `elegir_profundidad`.
        //
        // [!] `rbx` NO SE PUEDE PEDIR: LLVM lo usa por dentro y el compilador
        // lo rechaza de plano. Se salva a mano alrededor -- es el patron de
        // siempre para `cpuid` y companyia, y aqui hace falta porque `mwaitx`
        // lee el plazo de `ebx`.
        let t0 = super::ficha::ciclos();
        core::arch::asm!(
            "mov {salvo}, rbx",
            "mov ebx, {plazo:e}",
            "mwaitx",
            "mov rbx, {salvo}",
            salvo = out(reg) _,
            plazo = in(reg) PLAZO_TICKS,
            in("eax") hondo,
            in("ecx") 2,
            options(nostack, preserves_flags),
        );
        let duro = super::ficha::ciclos().wrapping_sub(t0);
        TICKS_DORMIDOS.fetch_add(duro, Ordering::Relaxed);
        // ** Menos de la mitad del plazo = algo la corto. El umbral es la
        // mitad y no un 99% porque el plazo no es exacto: lo que se busca no
        // es precision, es distinguir *durmio lo suyo* de *lo despertaron*.
        if duro < (PLAZO_TICKS as u64) / 2 {
            SIESTAS_CORTAS.fetch_add(1, Ordering::Relaxed);
        }
    }
    DORMIDAS.fetch_add(1, Ordering::Relaxed);
}

/// **Cuantas veces se durmio un obrero de verdad.**
///
/// Es el numero que convierte *"ahora deberia gastar menos"* en un dato. Si
/// sale CERO con los doce en pie, o no hay `MONITORX` o nadie llego a esperar
/// -- y las dos cosas se arreglan en sitios distintos.
pub fn dormidas() -> u64 {
    DORMIDAS.load(Ordering::Relaxed)
}

/// **Ticks del TSC pasados durmiendo**, sumando todos los obreros.
///
/// *** Este es el numero del ahorro, y el otro solo lo acompanya: mil siestas
/// de un microsegundo se ven igual de bien en `dormidas` y no apagan nada.
pub fn ticks_dormidos() -> u64 {
    TICKS_DORMIDOS.load(Ordering::Relaxed)
}

/// **Siestas que algo corto antes de tiempo.**
///
/// Comparada con `dormidas`, dice que fraccion de las siestas NO llego a su
/// plazo. Si es alta con la maquina en reposo, alguien esta escribiendo en la
/// linea de `RONDA` o llegando una interrupcion -- y las dos se arreglan en
/// sitios distintos.
pub fn siestas_cortas() -> u64 {
    SIESTAS_CORTAS.load(Ordering::Relaxed)
}
