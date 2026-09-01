//! **LA PURGA: devolver Ring 3 a como estaba al arrancar, y DEMOSTRARLO.**
//!
//! [carril]  ROJO      cierra todo Ring 3 y fuerza la recogida; si se equivoca, no queda a quien volver
//!
//! [cuesta]  MAQUINA -- marca muertas a todas las tareas de usuario y cede el
//!           CPU para que se recojan. Equivocarse en quien entra en la lista
//!           mata el hilo que lee el teclado, y entonces no hay tecla que
//!           salve nada.
//!
//! [riesgo]  AJENO SILENCIO
//!           AJENO    -- lo que recoge son procesos que ya estaban rotos: sus
//!                       ranuras, sus capabilities y sus tablas son justo lo
//!                       que mas motivos tiene para estar pisado.
//!           SILENCIO -- una purga que no vuelve a cero **no falla**: deja la
//!                       maquina en un tercer estado y lo llama "limpio". Por
//!                       eso esto no limpia y se calla: limpia y CUENTA.
//!
//! # *** POR QUE ESTE FICHERO EXISTE
//!
//! El 2026-08-31 el dueno lo pidio con la maquina en la mano y con la razon
//! entera dentro de la frase:
//!
//! > *"que haga limpieza total en la RAM en Ring 3 como si estuviera
//! > reiniciando... porque ya llevo asi repitiendo constantemente"*
//!
//! Y la primera version fue superficial, tambien dicho por el. Marcaba las
//! tareas y se iba. **Marcar no es limpiar**: el desmontaje lo hace `reap`, que
//! corre en el siguiente cambio de contexto, asi que quien pulsaba la tecla no
//! tenia forma de saber si habia pasado -- ni cuanto habia vuelto.
//!
//! ## El defecto era el MISMO un nivel mas arriba
//!
//! ```text
//!    la patada vieja   echaba al dueno de la pantalla y a nadie mas
//!    la purga v1       marcaba a todos... y no comprobaba nada
//! ```
//!
//! Las dos dejan la maquina en un estado que nadie puede nombrar. Y ese es el
//! problema de verdad del bucle en el que estaba el dueno: **no es que no se
//! limpie, es que no se sabe.**
//!
//! > Una vuelta a cero que no se puede comprobar no es un punto de partida:
//! > es una creencia.
//!
//! # El CONTRATO, y es lo unico que este fichero promete
//!
//! ```text
//!    Despues de `purgar()`, ninguna tarea de Ring 3 existe,
//!    y se dice CUANTOS MARCOS Y CUANTAS RANURAS volvieron.
//! ```
//!
//! No promete que vuelvan todos. Promete **decir cuantos**, que es lo que
//! convierte una fuga en un numero en vez de en una sospecha. Si un dia
//! `vueltos` es menor de lo que se fue, ahi esta la fuga y ahi esta su tamano.
//!
//! # Lo que NO devuelve, dicho en voz alta
//!
//! `revoke_all` cubre endpoint, pantalla, entrada, MMIO, prestamos, memoria,
//! consola, directorios, ficheros, paquete y familia. Lo que queda fuera:
//!
//! ```text
//!    proc::PROGRAMS   el registro HISTORICO de lanzamientos. A proposito:
//!                     es la bitacora, y una bitacora que se borra al limpiar
//!                     no sirve para investigar lo que acaba de pasar
//!    uconsole         las ultimas palabras de cada muerto. Por lo mismo
//!    la morgue        las pilas liberadas. Por lo mismo: es la prueba
//! ```
//!
//! Los tres son MEMORIA DE LO QUE PASO, no recursos de un proceso vivo. Que
//! sobrevivan a la purga no es una fuga: es el motivo de que existan.

use crate::ring0::task::scheduler;
use core::sync::atomic::{AtomicBool, Ordering};

/// **Hay una purga pedida?** La pone la tecla, la recoge el hilo del bus.
///
/// # *** POR QUE NO SE PURGA DONDE SE PULSA (corregido el 2026-08-31)
///
/// La primera version llamaba a [`purgar`] desde `segunda_llamada`, en
/// `dev/usb/rescate.rs`. Y ese sitio se alcanza por DOS caminos:
///
/// ```text
///    watch_rescue()      <- el hilo del bus. Hilo de kernel, pila propia. OK
///    tecla_del_dueno()   <- desde `poll_ascii`, o sea DENTRO DE UN SYSCALL
/// ```
///
/// ** Por el segundo, `purgar()` corre **dentro de una llamada de una tarea de
/// Ring 3**, y entonces hace dos cosas que no puede hacer:
///
/// 1. marca `Exited` **a la propia tarea que llamo** --es `is_user`--, y
/// 2. cede el CPU esperando a `reap`. Como ya esta muerta, **no vuelve nunca**.
///
/// El resultado es que la purga ocurre y el parte no se imprime jamas: se
/// limpia y no se cuenta, que es exactamente el defecto que este fichero vino a
/// arreglar, reaparecido por la puerta de atras.
///
/// > Un instrumento que se ejecuta en el contexto equivocado no da un dato
/// > peor: **da el mismo silencio que habia antes de escribirlo.**
///
/// *** La casa ya tenia la respuesta y esta escrita en `core/emergencia.rs`:
/// *"Solo se APUNTA... Quien lo recoge es el hilo del bus."* Esto es lo mismo,
/// con su propia bandera para no mezclar dos motivos --la patada del kernel y
/// la peticion del dueno-- en un solo camino.
static PEDIDA: AtomicBool = AtomicBool::new(false);

/// La tecla PIDE. No purga.
pub fn pedir() {
    PEDIDA.store(true, Ordering::SeqCst);
}

/// El hilo del bus RECOGE. Devuelve `true` si hubo purga.
///
/// [!] Aqui si se puede ceder el CPU: `bus_thread` es un hilo de kernel con su
/// propia pila y su propio turno, y `is_user` es falso -- la purga no se lleva
/// por delante a quien la esta ejecutando.
pub fn atender() -> bool {
    if !PEDIDA.swap(false, Ordering::SeqCst) {
        return false;
    }
    let parte = purgar();
    contar(&parte);
    true
}

/// Cuantas veces se cede el CPU esperando a que `reap` recoja.
///
/// [!] Un tope y no un bucle abierto. Esto corre en el hilo del bus, que es
/// quien lee el teclado: quedarse dando vueltas aqui deja la maquina sin la
/// unica tecla que la salva. **Ocho vueltas o se cuenta lo que haya** -- un
/// informe incompleto se puede leer; una tecla muerta, no.
const VUELTAS_MAX: u32 = 8;

/// El parte de la purga. Todo numeros: no hay ningun juicio aqui dentro.
#[derive(Clone, Copy)]
pub struct Parte {
    /// Tareas de Ring 3 que se cerraron.
    pub tareas: u32,
    pub marcos_antes: u64,
    pub marcos_despues: u64,
    /// Marcos de 4 KiB que volvieron. Es la fila que el dueno mira en `save`.
    pub vueltos: u64,
    pub ranuras_antes: u64,
    pub ranuras_despues: u64,
    /// Cuantas cesiones de CPU hicieron falta para que `reap` terminara.
    pub vueltas: u32,
    /// `true` = no queda ni una tarea de Ring 3. `false` = se acabo el plazo.
    pub completa: bool,
}

/// **Cierra Ring 3 entero, espera a que se recoja, y cuenta lo que volvio.**
///
/// # Por que se CEDE el CPU en vez de desmontar aqui
///
/// El desmontaje de verdad --devolver las hojas con `PTE_NUESTRA`, respetar lo
/// prestado, soltar las pilas-- lo hace `reap`, al final de `schedule_locked`.
/// Es el unico sitio del kernel que ya sabe hacerlo bien, y corre **despues**
/// del cambio de contexto, que es la garantia de que no tira el suelo que se
/// esta pisando.
///
/// *** Escribir aqui una segunda version de eso seria duplicar la funcion mas
/// delicada del kernel para usarla desde una tecla. Asi que esto no desmonta:
/// **marca, y luego le da al planificador las oportunidades que necesita.**
///
/// [!] Ceder es legitimo desde aqui y no en cualquier sitio: el rescate lo mira
/// `bus_thread`, un hilo de kernel con su propia pila y su propio turno. Desde
/// una interrupcion esto seria un error.
pub fn purgar() -> Parte {
    let (_, marcos_antes) = crate::ring0::mm::phys::stats();
    let ranuras_antes = scheduler::huecos_libres() as u64;

    let (tareas, _) = scheduler::limpieza_de_ring3();

    // ** Se cede hasta que no quede ninguna, con tope. Cada cesion es un
    // `schedule_locked`, y cada `schedule_locked` termina en `reap`.
    let mut vueltas = 0u32;
    let mut completa = false;
    while vueltas < VUELTAS_MAX {
        if !scheduler::queda_alguna_de_ring3() {
            completa = true;
            break;
        }
        scheduler::yield_current();
        vueltas += 1;
    }
    if !completa {
        completa = !scheduler::queda_alguna_de_ring3();
    }

    let (_, marcos_despues) = crate::ring0::mm::phys::stats();
    let ranuras_despues = scheduler::huecos_libres() as u64;

    // ** EL SUPERVISOR TAMBIEN VUELVE A CERO, y forma parte del contrato.
    //
    // `DESKTOP_ATTEMPTS` cuenta los relanzamientos y tiene tope dos. Si la
    // purga dice *"como al arrancar"* y deja el contador donde estaba, el
    // siguiente escritorio nace con menos vidas que el primero -- y eso es
    // exactamente un estado que sobrevive al reinicio que no lo es.
    unsafe { crate::ring0::core::desktop::DESKTOP_ATTEMPTS = 0 };

    Parte {
        tareas,
        marcos_antes,
        marcos_despues,
        vueltos: marcos_despues.saturating_sub(marcos_antes),
        ranuras_antes,
        ranuras_despues,
        vueltas,
        completa,
    }
}

/// El parte, en cuatro renglones del panel. Lo llama la tecla y lo llama el
/// shell: **el mismo texto por los dos caminos**, para que lo que se lee con
/// la maquina rota y lo que se lee probando sean comparables.
pub fn contar(p: &Parte) {
    use crate::ring0::core::dashboard::dashboard_log;
    use crate::ring0::cabina::format::Buf;

    dashboard_log("*** PURGA DE RING 3 ***");

    let mut l = Buf::new();
    l.txt("   tareas cerradas: ");
    l.dec(p.tareas as u64);
    l.txt("   ranuras: ");
    l.dec(p.ranuras_antes);
    l.txt(" -> ");
    l.dec(p.ranuras_despues);
    dashboard_log(l.as_str());

    let mut l2 = Buf::new();
    l2.txt("   marcos libres: ");
    l2.dec(p.marcos_antes);
    l2.txt(" -> ");
    l2.dec(p.marcos_despues);
    l2.txt("   VOLVIERON ");
    l2.dec(p.vueltos);
    dashboard_log(l2.as_str());

    let mut l3 = Buf::new();
    if p.completa {
        l3.txt("   Ring 3 VACIO en ");
        l3.dec(p.vueltas as u64);
        l3.txt(" cesion(es). La maquina esta como al arrancar.");
    } else {
        // [!] Y esto se dice fuerte. Una purga incompleta que se anuncia como
        // completa es el fallo que este fichero entero viene a impedir.
        l3.txt("   [!] QUEDA ALGO DE RING 3 tras ");
        l3.dec(VUELTAS_MAX as u64);
        l3.txt(" cesiones: NO es un punto de partida limpio");
    }
    dashboard_log(l3.as_str());

    crate::ring0::cabina::warn("purga", "tareas de Ring 3 cerradas", p.tareas as u64);
    crate::ring0::cabina::warn("purga", "marcos de 4 KiB devueltos", p.vueltos);
    if !p.completa {
        crate::ring0::cabina::fault("purga", "la purga NO vacio Ring 3", p.vueltas as u64);
    }
}
