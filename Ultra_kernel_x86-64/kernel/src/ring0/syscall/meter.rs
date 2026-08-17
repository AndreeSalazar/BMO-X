//! `syscall::meter` -- how many cycles the kernel spends inside a door.
//!
//! ```text
//!    [eje]     LATENCIA -- pays SIZE (two counters) to buy a NUMBER
//!    [camino]  P1 la puerta -- 100% of every door
//!    [fila]    DISPATCH (techo 105, meta 60) -- and this file is INSIDE it
//!    [gen]     ABUELO -- it counts. It does not know what it is counting
//!    [exige]   R-CPU5 (doble testigo), R-TIME1 (el metro se resta a si mismo),
//!              R-TIME3 (el minimo es la respuesta; la media es otra cosa)
//! ```
//!
//! ** THE TAG IS NOT DECORATION: `[gen] ABUELO` is what forbids this file from
//! ever asking *"which operation was this?"*. The day it needs to know, the cut
//! is wrong -- the caller passes the answer down. See `META-KERNEL_HARD.md` L7.
//!
//! # Why this exists
//!
//! On 2026-08-16 `c/coste.bex` measured a door from Ring 3: **2615 cycles
//! minimum**, against 20 for a plain function call. That number decided a real
//! design question (see `docs/PYTHON_MAESTRO.md` section 4b) but it could not
//! answer the next one: **where do those cycles go?**
//!
//! The named suspects were the unconditional `xsave64`/`xrstor64` with
//! `RFBM = -1` and the `iretq`, both in `entry.rs`. Adding them up honestly
//! gives a few hundred cycles, not two thousand -- so the suspicion was
//! **unconfirmed, and acting on it would have been surgery on a guess**, on the
//! exact code that produced the `#GP` in `xrstor` that cost five probes to
//! find.
//!
//! This splits the number in half without changing a single behaviour:
//!
//! ```text
//!    total (medido desde Ring 3)   2615
//!      - lo que tarda `dispatch`   <- lo que mide esto
//!      = lo que tarda el STUB      <- pushes, xsave, xrstor, iretq
//! ```
//!
//! If the stub half is small, the suspects are innocent and the cost is in
//! Rust. If it is nearly all of it, the surgery is justified and there is a
//! number behind it instead of a hypothesis.
//!
//! # THE ANSWER, measured on the Ryzen the same day (2026-08-16)
//!
//! ```text
//!    puerta pelada  2663  =  dispatch 318  (12%)  +  stub 2345  (88%)
//!    puerta+handle  2746  =  dispatch 394         +  stub 2352
//! ```
//!
//! **88% of a door is this file's neighbours, not this file's callers.** The
//! Rust half -- resolve, dispatch, answer -- is 318 cycles; everything else is
//! the pushes, the `xsave64`, the `xrstor64` and the `iretq`. The suspects were
//! correctly named, and the surgery on `entry.rs` now has a number behind it.
//!
//! And the number nobody had: **resolving a capability costs 83 cycles, 76 of
//! them inside `dispatch` and 7 in the stub** (0.3%, noise). The stub does not
//! know which operation was asked for, which is what the design says. That
//! split is also what validates the meter: the two quantities moved exactly
//! where the code that does the work lives.
//!
//! [!] `dispatch` is read as a MEAN and the total as a MINIMUM, so 318 is a
//! ceiling for the Rust half and **2345 is a FLOOR for the stub**. Which way
//! the bias runs is stated because it happens to run in favour of the
//! conclusion, and that is exactly when it must be said out loud.
//!
//! # LA SEGUNDA MITAD: el reparto DENTRO del stub (2026-08-16, tarde)
//!
//! El primer reparto dijo *"el 88% esta en el ensamblador"* y ahi se quedo. Se
//! cambio `xsave64` por `xsaveopt64` --el sospechoso nombrado-- y **compro 45
//! ciclos de 2345, el 2%**. O sea que el sospechoso principal era inocente y la
//! pregunta *"cual de las cinco piezas"* seguia sin contestar.
//!
//! Y sumando a mano lo que hay en ese camino --`syscall`, `swapgs`, 20 pushes,
//! la cabecera, el `xsaveopt64`, el `xrstor64`, 15 pops, el `iretq`-- salen
//! **unos 700 de 2299**. Faltan 1.600 ciclos que **no estan en la lista de
//! sospechosos**: el modelo del camino esta incompleto, no solo mal ordenado.
//! Eso no se arregla con una tercera corazonada, se arregla con dos `rdtsc` mas.
//!
//! ```text
//!    A   tras los 15 pushes          -> [ULTIMO] = A
//!    B   tras el `xsaveopt64`        -> GUARDA   += B - A
//!    C   meter::start()              \  CYCLES += D - C
//!    D   meter::stop()               /  (ya existia, SIN TOCAR)
//!    D'  al volver de `call dispatch`-> [ULTIMO] = D'
//!    E   tras el `xrstor64`          -> RESTAURA += E - D'
//!
//!    resto = total - GUARDA - CYCLES - RESTAURA
//!          = `syscall` + `swapgs` + pushes + los dos puentes + pops + `iretq`
//! ```
//!
//! ** `D'` esta en el ENSAMBLADOR y no dentro de `stop`, aunque dentro de `stop`
//! saldria mas barato: metido ahi, los dos contadores volatiles y el epilogo de
//! `dispatch` caerian dentro de `RESTAURA`, que es donde ese codigo no esta.
//! Puesto aqui **`start`/`stop` no cambian ni un byte**, y ese es justo el
//! control que hace comparable el 319 de esta manana.
//!
//! **`resto` es la cifra que decide.** Si es pequeno, el coste esta en codigo
//! que se puede reescribir. Si se lleva los 1.600 que no cuadran, esta en las
//! DOS TRANSICIONES DE PRIVILEGIO y entonces la accion no es afinar el stub: es
//! `sysretq` en vez de `iretq` para el camino normal, o agrupar las llamadas --
//! que es justo la pregunta que `docs/PYTHON_MAESTRO.md` tiene abierta.
//!
//! [!] **Lo que este instrumento se cobra, dicho antes de leerlo**: cuatro
//! `rdtsc` mas (~25 ciclos cada uno) y veinte instrucciones, ~115 ciclos sobre
//! 2618 -- un **4,4%**. Y no se reparte a partes iguales:
//!
//! ```text
//!    GUARDA    +3     las tres instrucciones que siguen al sello A
//!    CYCLES     0     no se toca
//!    RESTAURA   0     el sello D' cierra justo antes de empezar
//!    resto    ~+90    los sellos B, D' y E enteros caen aqui
//! ```
//!
//! O sea que **el sesgo engorda `resto`**, que es justo la casilla que espero
//! ver grande: el instrumento empuja hacia mi propia hipotesis. Por eso
//! `c/coste.bex` mide un `rdtsc` suelto en su ultima fila -- para que restar
//! esos 90 sea una resta y no una estimacion. Si `resto` sale en 1.600, 90
//! arriba o abajo no cambian la lectura; si sale en 200, la cambian entera, y
//! entonces es la correccion la que manda.
//!
//! # LO QUE MIDIO, EN EL RYZEN, EL MISMO DIA
//!
//! ```text
//!    1. bucle vacio    min   44
//!    2. llamada        min   64
//!    3. puerta pelada  min 1625   (era 2618 esta manana)
//!       dispatch 311, stub 1314
//!       guardar 30, devolver 30, resto 1254
//!    5. rdtsc suelto   min  113   -> 113 - 44 = 69 ciclos POR SELLO
//! ```
//!
//! **La puerta paso de 2618 a 1625, y descontando los cuatro sellos (~276) a
//! ~1350: la mitad.** `guardar` y `devolver` en 30 cada uno --las dos casillas
//! que la pieza 1 atacaba-- y `dispatch` en 311 contra 318/319 de antes: el
//! control aguanta, asi que las que se movieron se movieron de verdad.
//!
//! ** Y UNA CONCLUSION MIA QUE ESTO CORRIGE. Por la manana quedo escrito que
//! *"el guardado del estado extendido NO es donde se van los 2.300"*. Era
//! cierto **del guardado** y falso **del estado extendido**: el experimento del
//! `xsaveopt64` solo tocaba la mitad de guardar y compro 45, pero quitar la
//! maquinaria ENTERA --guardar, devolver y validar-- compro ~1.000. El caro era
//! el `xrstor64` con sus nueve lecturas de cabecera, y aquel experimento era
//! **estructuralmente incapaz de verlo**. El fallo no fue la medida: fue
//! generalizar de una mitad al todo.
//!
//! [!] Y lo que NO se puede decir: la pieza quito siete cosas a la vez. Se sabe
//! que el conjunto vale ~1.000 y que la mitad de guardar era <=150, o sea que
//! devolver+validar era ~850-950. **Repartir esos 950 entre el `xrstor64` y las
//! comprobaciones pediria otra sonda**, y hoy no hace falta.
//!
//! [!] **Y el instrumento cuesta el DOBLE de lo que este fichero estimo**: 69
//! ciclos por sello medidos contra los ~25 que decia la estimacion, o sea ~276
//! y no ~115. Sacarlo con un `cfg` vale hoy mas que todo lo que compro el
//! `xsaveopt64`.
//!
//! [!] **El control que valida la tanda**: `CYCLES` no se toca. Si `dispatch`
//! vuelve a salir ~319 el instrumento nuevo no rompio el viejo; si se mueve,
//! el reparto nuevo no se lee hasta saber por que.
//!
//! [!] Y por eso `start`/`stop` siguen usando `rdtsc()` --el NO serializante--
//! aunque `rdtsc_serial()` existe una puerta mas alla y seria mas correcto:
//! cambiarlo en la misma tanda que anade las etapas dejaria el 319 sin poder
//! compararse con el 318 y el 319 de esta manana. **Un instrumento se cambia de
//! una cosa cada vez.** El sesgo que introduce son decenas de ciclos, no miles,
//! asi que no toca ninguna de las conclusiones de arriba.
//!
//! # It is read as a DELTA, not as an absolute
//!
//! There is no reset operation on purpose. A caller reads the pair before and
//! after its own loop and divides the differences, which measures exactly the
//! population it cares about and cannot be polluted by whatever the machine did
//! while booting. Adding a reset would also add the question *"who is allowed
//! to reset it"*, and nobody needs that answer.
//!
//! [!] Reading the pair costs two doors, and those two are counted too. Over a
//! block of thousands it is under a tenth of a percent; over a block of ten it
//! is the whole measurement.
//!
//! # What it costs, said out loud
//!
//! Two `rdtsc` reads per door, about fifty cycles, plus two stores. That is
//! ~2% of the number being measured, it inflates BOTH sides equally, and it is
//! disclosed here rather than discovered later by someone comparing against a
//! run from before this file existed.
//!
//! # Why `static mut` and not atomics
//!
//! A `lock xadd` is tens of cycles **on the very path being measured**, which
//! would be measuring the thermometer. These are plain volatile counters, the
//! same shape `plat::trap::registrar_publicacion` already uses one line away.
//!
//! [!] Consequence, stated because it will matter: with more than one core awake
//! these counters **undercount** -- two cores writing the same word lose one of
//! the two. They are a meter, not an accountant. The day APs run user code this
//! becomes per-CPU or it becomes wrong.

/// How many doors have been served since boot.
static mut DOORS: u64 = 0;

/// Accumulated TSC ticks spent inside `dispatch`, for those doors.
static mut CYCLES: u64 = 0;

/// **El sello anterior, rodando.** Lo escribe la etapa A (en el stub) y lo
/// vuelve a escribir [`stop`]; lo leen la etapa B y la etapa E para restar.
///
/// Un solo hueco y no cuatro porque las etapas van EN ORDEN y ninguna se salta:
/// entre A y E el camino es recto y las interrupciones estan enmascaradas
/// (`MSR_SFMASK` apaga `IF` en la entrada y solo el `iretq` la devuelve). Ese
/// es el hecho que hace que una sola casilla baste **y** que las restas no
/// puedan salir negativas -- en el stub no hay nada que se cuele en medio.
///
/// [!] `pub` porque **lo escribe el ensamblador de `entry.rs`** por `sym`, no
/// el Rust de este fichero. Es la unica razon; nadie mas debe tocarlo.
///
/// [!] ** Y CON DOS NUCLEOS SIRVIENDO PUERTAS ESTO NO ES QUE CUENTE DE MENOS:
/// ES QUE MIENTE. Los contadores de arriba pierden sumas cuando dos nucleos
/// escriben la misma palabra, y una suma perdida se nota. Esta casilla es
/// distinta -- si el nucleo B escribe su sello A entre el sello A y el sello B
/// del nucleo A, la resta que sale no es pequena: **es una diferencia entre
/// sellos de dos puertas distintas**, y puede salir negativa y envolver a un
/// numero cercano a `u64::MAX` que envenena la suma entera. Hoy no puede pasar
/// --el informe dice `en pie 1 de 12` y las APs no ejecutan codigo de usuario--
/// pero el dia que lo hagan, esto se vuelve por-CPU ANTES que los otros dos.
pub static mut ETAPA_ULTIMO: u64 = 0;

/// Ciclos de A a B: `sub`/`and rsp`, el back-pointer y el sello.
///
/// ** HOY VALE CERO, Y ESO NO ES UN FALLO: es un instrumento retirado.
///
/// Los cuatro sellos que llenaban esta casilla y [`CICLOS_RESTAURA`] vivieron
/// dentro de `entry.rs` lo justo para contestar --**guardar 30, dispatch 311,
/// devolver 30, resto 1254**-- y se sacaron el mismo dia. Costaban 69 ciclos
/// cada uno, medidos por la fila 5 de `coste.bex` y no estimados: ~276 sobre
/// 1625, **un 17%**. Un instrumento que ya dio su numero y sigue cobrando es un
/// peaje.
///
/// El campo se queda --y `c/coste.bex` dice *"NO MEDIDO"* en vez de imprimir un
/// reparto de ceros-- porque el hueco vale mas que el borrado: el dia que haya
/// que volver a partir el stub, el cableado hasta Ring 3 ya esta hecho y solo
/// hay que devolver los cuatro sellos, que estan en el git de este dia.
pub static mut CICLOS_GUARDA: u64 = 0;

/// Ciclos de D' a E: **lo que cuesta DEVOLVER el contexto**, y depende de por
/// que via se salga.
///
/// ```text
///    via rapida (misma tarea)  borrar el sello y `mov rsp, rbp`.  ~5
///    via lenta  (hubo cambio)  sello + cabecera + `xrstor64`.     ~cientos
/// ```
///
/// Las dos suman en el mismo sitio a proposito: la cifra que sale es la MEDIA
/// PONDERADA de las dos vias, o sea **lo que cuesta devolver un contexto en la
/// mezcla real de esta maquina**. Un bucle de puertas que no cede sale casi
/// todo por la rapida y esta casilla se va a ~5; una carga que cambia de tarea
/// constantemente la sube. Esa diferencia, leida contra `dispatch`, dice cuanto
/// esta cediendo el sistema sin tener que instrumentar el planificador.
pub static mut CICLOS_RESTAURA: u64 = 0;

// == THE CLASS HISTOGRAM: not how much a door costs, but WHICH doors happen ==
//
// `DOORS` answers *how many*. It cannot answer *which*, and without that the
// per-class costs already measured --875 / 1125 / 2,2 M-- cannot be turned into
// a share of the machine. A cost per call without calls per second is not a
// percentage (`docs/CENSO_DE_EJES.md`, R-CENSO3).
//
// ** WHAT THIS FILE IS ALLOWED TO KNOW: nothing. It receives an index and adds
// one. The meaning of each index lives in the ABI, and the decision of which
// index applies lives in `dispatch`, which already holds those facts in
// registers. That split is L7 and it is the reason this file has no `match` on
// operations -- see the header tag.

/// How many boxes the histogram has. Mirrors `bmo_abi::SYSCALL_CLASS_COUNT`;
/// the build guard compares the two.
pub const CLASS_COUNT: usize = 4;

/// Doors served, split by class. Index meaning: see `SYSCALL_CLASS_*` in the
/// ABI -- deliberately not repeated here, because repeating it here is how a
/// file starts knowing things it must not know.
static mut DOORS_BY_CLASS: [u64; CLASS_COUNT] = [0; CLASS_COUNT];

/// Add one to a box.
///
/// `#[inline(always)]` and a bare volatile add: this runs on the path being
/// measured, so it must cost what it looks like it costs.
///
/// [!] **What it costs, said out loud**: one load, one add, one store, about
/// three cycles, inside the metered window -- so it lands in the `DISPATCH`
/// row, whose ceiling is 105 with a 5% noise margin. It is disclosed here
/// rather than discovered by someone comparing against a run from before this
/// existed, which is the same courtesy the two counters above already got.
///
/// [!] An out-of-range index is dropped **on purpose, and it is a documented
/// caller idiom, not a mute failure**: `dispatch` passes `SYSCALL_CLASS_COUNT`
/// for the door that is none of the four (the retired syscall number), rather
/// than inventing a box for it. The drop is not silent either -- the four boxes
/// are read against [`doors`] and **the remainder is what was dropped**. A
/// count that lands nowhere moves that remainder; it does not hide.
#[inline(always)]
pub fn count_class(class: usize) {
    if class >= CLASS_COUNT {
        return;
    }
    unsafe {
        let p = core::ptr::addr_of_mut!(DOORS_BY_CLASS).cast::<u64>().add(class);
        p.write_volatile(p.read_volatile().wrapping_add(1));
    }
}

/// Doors served in one class since boot. Read as a DELTA, like everything else
/// in this file. Out of range answers `0`, which is what "no such box" means.
pub fn doors_of_class(class: usize) -> u64 {
    if class >= CLASS_COUNT {
        return 0;
    }
    unsafe { core::ptr::addr_of!(DOORS_BY_CLASS).cast::<u64>().add(class).read_volatile() }
}

/// Take the reading at the start of a door. Pairs with [`stop`].
#[inline(always)]
pub fn start() -> u64 {
    crate::ring0::task::scheduler::rdtsc()
}

/// Close the reading opened by [`start`] and fold it in.
///
/// `saturating_sub` and not `-`: the TSC is invariant on this CPU, but a value
/// that went backwards would be a fact about the machine, not a reason to
/// underflow into a number near `u64::MAX` that then poisons every average
/// computed from the sum. A lost sample is visible; a huge one is a lie.
#[inline(always)]
pub fn stop(started: u64) {
    let elapsed = crate::ring0::task::scheduler::rdtsc().saturating_sub(started);
    unsafe {
        let n = core::ptr::addr_of_mut!(DOORS);
        n.write_volatile(n.read_volatile().wrapping_add(1));
        let c = core::ptr::addr_of_mut!(CYCLES);
        c.write_volatile(c.read_volatile().wrapping_add(elapsed));
    }
}

/// Doors served since boot.
pub fn doors() -> u64 {
    unsafe { core::ptr::addr_of!(DOORS).read_volatile() }
}

/// Ticks spent inside `dispatch` for those doors.
pub fn cycles() -> u64 {
    unsafe { core::ptr::addr_of!(CYCLES).read_volatile() }
}

/// Ticks guardando el contexto -- la etapa A a B. Ver [`CICLOS_GUARDA`].
pub fn ciclos_guarda() -> u64 {
    unsafe { core::ptr::addr_of!(CICLOS_GUARDA).read_volatile() }
}

/// Ticks devolviendo el contexto -- la etapa D' a E. Ver [`CICLOS_RESTAURA`].
pub fn ciclos_restaura() -> u64 {
    unsafe { core::ptr::addr_of!(CICLOS_RESTAURA).read_volatile() }
}
