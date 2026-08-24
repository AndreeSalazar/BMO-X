//! **El reparto de trabajo.** Lo unico que separa doce nucleos encendidos de
//! doce nucleos que sirven para algo.
//!
//! === El hueco que tapa ===
//!
//! Los APs arrancaron --`12 de 12` en el Ryzen-- y se quedaban en `cli; hlt`
//! **para siempre**. Un nucleo despierto al que no se le puede dar una tarea es
//! exactamente igual de util que uno dormido, y cuesta lo mismo: nada.
//!
//! === Lo que NO es, y por que ===
//!
//! **Esto no es un planificador.** No hay colas, ni prioridades, ni cambio de
//! contexto, ni tareas de Ring 3 corriendo en otro nucleo. Es un reparto de
//! *una* funcion pura entre *n* partes, con una barrera al final:
//!
//! ```text
//!   el BSP publica   (funcion, cuantas partes)
//!   cada obrero      hace SU parte y se apunta
//!   el BSP espera    a que esten todas
//! ```
//!
//! Y ser tan poca cosa es lo que lo hace seguro **hoy**, con los **236**
//! `static mut` que hay en el kernel (eran 209 cuando esto se escribio, el
//! 2026-08-08; el numero se reconto el 11-08 y **sube solo**): un obrero que
//! solo calcula sobre su rango no toca ni uno.
//!
//! ** Ese contador es la medida real de lo que falta para SMP de verdad. El
//! trampolin --lo que la gente llama "hacer SMP"-- ya esta y arranco 12 de 12
//! a la primera. Lo que separa esto de un planificador multinucleo son esos 236
//! sitios, uno a uno: el anillo de CABINA, la cola del teclado, el bitmap de
//! marcos, el registro de programas, los contadores de USB. **Cada uno es una
//! carrera el dia que corra un segundo nucleo dentro del kernel.**
//!
//! Es el contrato del `docs/maestro/SMP_MAESTRO.md` -- *"de Cell se copia el reparto,
//! no el transporte"*--, y aqui el reparto cabe en cien lineas porque lo caro
//! de Cell era el transporte, que en un CCX con 32 MB de L3 compartida **no hay
//! que escribir**.
//!
//! === [!] El precio, dicho antes de que se note ===
//!
//! Un obrero en espera **gira** (`pause`), no duerme. Sacarlo de `hlt` pediria
//! una IPI, y para atender una IPI un AP necesita GS por-CPU y su propia TSS --
//! que es justo el trabajo que este modulo evita. Consecuencia real y medible:
//! con los doce en pie, once nucleos giran al 100 % y la maquina consume como
//! si estuviera trabajando.
//!
//! Por eso existe [`parar`], y por eso la orden lo dice en pantalla. Un coste
//! que no se anuncia es una trampa.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// La funcion que toca hacer. `0` = ninguna.
///
/// Se guarda como numero y no como `fn` porque un `AtomicPtr` a funcion no
/// existe y aqui no hace falta mas: el BSP publica, los obreros leen.
static TAREA: AtomicU64 = AtomicU64::new(0);
/// En cuantas partes se ha cortado el trabajo (contando la del BSP).
static PARTES: AtomicU32 = AtomicU32::new(0);
/// Cuantos obreros han terminado su parte.
static HECHOS: AtomicU32 = AtomicU32::new(0);
/// Sube con cada encargo. Es lo que distingue *"hay trabajo nuevo"* de *"sigue
/// el de antes"* sin tener que borrar nada entre medias.
static RONDA: AtomicU32 = AtomicU32::new(0);
/// Cuando se pone, los obreros vuelven a `hlt` y no salen mas.
static PARAR: AtomicBool = AtomicBool::new(false);

// =============== LOS TESTIGOS ===============
//
// * El 2026-08-08, en metal, `smp prueba` contesto `0.00x` -- o sea que
// `repartir` se rindio esperando. Y ahi se acababa la informacion: **"falto
// una parte" no dice cuantas partes llegaron**, ni si los obreros habian
// entrado siquiera al bucle.
//
// Un bit ("salio mal") no se puede depurar. Tres numeros si, y estos tres
// parten el camino en los tres sitios donde se puede romper:
//
//   ENTRARON  el AP llego a `obrero`      -> si es < 11, el fallo esta ANTES,
//                                            en el trampolin o en la pila
//   VIERON    el obrero vio la ronda nueva -> si entraron y no vieron, es la
//                                            publicacion de las atomicas
//   HECHOS    el obrero termino su parte   -> si vieron y no acabaron, la
//                                            faena murio a medias
//
// Es el metodo de las cinco sondas de XSAVE: un instrumento que puede MATAR la
// hipotesis vale mas que uno que la confirma.
/// Cuantos obreros han llegado vivos al bucle de trabajo.
static ENTRARON: AtomicU32 = AtomicU32::new(0);
/// Cuantas veces un obrero ha visto una ronda nueva y se ha puesto a ella.
static VIERON: AtomicU32 = AtomicU32::new(0);

/// `(entraron, vieron, hechos)` -- los tres testigos del ultimo reparto.
pub fn testigos() -> (u32, u32, u32) {
    (
        ENTRARON.load(Ordering::SeqCst),
        VIERON.load(Ordering::SeqCst),
        HECHOS.load(Ordering::SeqCst),
    )
}

/// La forma de una faena: `(mi parte, de cuantas)`.
pub type Faena = fn(u32, u32);

/// **El bucle del obrero.** No vuelve.
///
/// `indice` es 0..n-1 entre los APs; su parte es `indice + 1` porque la parte
/// `0` se la queda el BSP, que tambien trabaja -- tener un nucleo mirando como
/// trabajan los otros es desperdiciar justo el mas caliente de cache.
pub fn obrero(indice: u32) -> ! {
    // Lo primero que hace un obrero es decir que existe. Antes el unico
    // testigo era `VIVOS`, y ese se incrementa en el trampolin -- o sea, dice
    // que el nucleo arranco, no que llegara hasta aqui.
    ENTRARON.fetch_add(1, Ordering::SeqCst);
    let mut vista = 0u32;
    loop {
        if PARAR.load(Ordering::SeqCst) {
            // Punto de no retorno: sin IPI no hay quien lo despierte, y volver
            // a llamarlo es un INIT+SIPI entero. Esta bien asi -- es la forma
            // honesta de "desactivar" con lo que hay.
            loop {
                unsafe { core::arch::asm!("cli; hlt", options(nomem, nostack)) };
            }
        }
        let r = RONDA.load(Ordering::SeqCst);
        if r != vista {
            vista = r;
            let f = TAREA.load(Ordering::SeqCst);
            let partes = PARTES.load(Ordering::SeqCst);
            let mia = indice + 1;
            if f != 0 && mia < partes {
                // Se apunta ANTES de la faena: si el obrero muere dentro, la
                // diferencia entre `VIERON` y `HECHOS` es exactamente cuantos
                // se quedaron por el camino.
                VIERON.fetch_add(1, Ordering::SeqCst);
                let faena: Faena = unsafe { core::mem::transmute(f) };
                faena(mia, partes);
                HECHOS.fetch_add(1, Ordering::SeqCst);
            }
        }
        core::hint::spin_loop();
    }
}

/// **Reparte una faena y espera a que acabe.**
///
/// `obreros` es cuantos APs participan; el BSP hace la parte `0` siempre. Con
/// `obreros = 0` esto es un `faena(0, 1)` y ni siquiera toca las atomicas.
///
/// Devuelve `false` si alguien no llego a tiempo -- y entonces **el dato que se
/// haya calculado no vale**, porque falta una parte del rango.
pub fn repartir(faena: Faena, obreros: u32) -> bool {
    let partes = obreros + 1;
    if obreros == 0 {
        faena(0, 1);
        return true;
    }

    HECHOS.store(0, Ordering::SeqCst);
    VIERON.store(0, Ordering::SeqCst);
    PARTES.store(partes, Ordering::SeqCst);
    TAREA.store(faena as usize as u64, Ordering::SeqCst);
    // La ronda va LA ULTIMA: es la senal, y publicarla antes que los datos
    // dejaria a un obrero leyendo la faena de la ronda anterior con las partes
    // de la nueva.
    RONDA.fetch_add(1, Ordering::SeqCst);

    // El BSP hace la suya mientras los demas hacen las suyas.
    faena(0, partes);

    // Y espera, con tope. Un obrero que no contesta no puede colgar la maquina.
    //
    // * EL TOPE ES TIEMPO, NO VUELTAS, y el cambio importa. Antes eran
    // `2_000_000_000` vueltas de `pause`: un numero que **nadie sabe cuanto
    // dura**. En un Zen 3 un `pause` cuesta unos 35 ciclos, asi que eran ~19
    // segundos; en un CPU donde `pause` cuesta 10, son cinco. El mismo codigo,
    // el mismo tope escrito, y dos esperas distintas segun la maquina -- que es
    // justo lo que un tope no puede hacer.
    //
    // Con el TSC son dos segundos en cualquier sitio, y dos segundos es de
    // sobra: la faena de prueba entera en un solo nucleo dura decimas.
    let hz = crate::ring0::task::scheduler::tsc_freq();
    let limite = if hz > 0 { hz * 2 } else { u64::MAX };
    let t0 = crate::ring0::task::scheduler::rdtsc();
    while HECHOS.load(Ordering::SeqCst) < obreros {
        if crate::ring0::task::scheduler::rdtsc().wrapping_sub(t0) > limite {
            break;
        }
        core::hint::spin_loop();
    }
    let ok = HECHOS.load(Ordering::SeqCst) >= obreros;
    TAREA.store(0, Ordering::SeqCst);
    ok
}

/// **Desactivar los obreros**: vuelven a `hlt` y ahi se quedan.
///
/// Es la otra mitad del mando que pidio el dueno. No hay vuelta atras sin un
/// INIT+SIPI nuevo, y eso es correcto: "desactivado" tiene que significar
/// desactivado y no "durmiendo por si acaso".
pub fn parar() {
    PARAR.store(true, Ordering::SeqCst);
}

/// Estan parados?
pub fn parados() -> bool {
    PARAR.load(Ordering::SeqCst)
}

/// Volver a admitir trabajo. Solo tiene efecto para los que se despierten
/// DESPUES: los que ya entraron en `hlt` no salen solos.
pub fn reanudar() {
    PARAR.store(false, Ordering::SeqCst);
}

// =============== LA PRUEBA ===============

/// Cuantas vueltas da la faena de prueba **en total**, repartidas entre todos.
///
/// Elegido para que en un nucleo se note (decimas de segundo) y en doce siga
/// midiendose bien. Es una cuenta pura: ni memoria que compartir ni nada que
/// bloquear, o sea el caso MAS FAVORABLE que existe -- y decirlo importa, porque
/// la aceleracion que salga aqui es el techo, no lo que va a dar un programa
/// real.
const VUELTAS: u64 = 400_000_000;

static SUMAS: [AtomicU64; 16] = [const { AtomicU64::new(0) }; 16];

/// Una cuenta que el compilador no puede saltarse: cada vuelta depende de la
/// anterior, asi que no hay forma de plegarla ni de vectorizarla.
fn faena_prueba(parte: u32, de: u32) {
    let bloque = VUELTAS / de as u64;
    let desde = bloque * parte as u64;
    let hasta = if parte + 1 == de { VUELTAS } else { bloque * (parte as u64 + 1) };
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut i = desde;
    while i < hasta {
        h = h.wrapping_mul(6364136223846793005).wrapping_add(i);
        h ^= h >> 29;
        i += 1;
    }
    if (parte as usize) < SUMAS.len() {
        SUMAS[parte as usize].store(h, Ordering::SeqCst);
    }
}


/// Vueltas de la faena de ANCHO. Menos que las de latencia porque cada vuelta
/// hace ocho cuentas en vez de una: el tiempo de pared sale parecido.
const VUELTAS_ANCHO: u64 = 50_000_000;

/// *** LA SEGUNDA FAENA, Y ES LA QUE HACE HONESTO EL NUMERO (2026-08-24).
///
/// ## Por que una sola faena no puede contestar "cuanto acelera esta maquina"
///
/// La faena de arriba es **una cadena de dependencias, y lo dice su propio
/// comentario**: cada vuelta necesita el resultado de la anterior. Eso la hace
/// perfecta para comprobar que el reparto FUNCIONA -- y **la peor posible para
/// predecir lo que va a dar un trabajo de verdad**:
///
/// ```text
///    LATENCIA   una cadena, poco ILP. El nucleo esta casi parado esperando,
///               asi que el segundo hilo SMT llena esos huecos
///               -> hasta ~2x por nucleo. **El MEJOR caso**
///
///    ANCHO      cuentas independientes que saturan las unidades. El segundo
///               hilo no encuentra hueco porque no lo hay
///               -> ~1x por nucleo extra. **El caso REAL de un calculo denso**
/// ```
///
/// *** Y esto salio de una medida: el 2026-08-24 el Ryzen dio **11,59x sobre 12
/// hilos** --el 96,6%-- contra una prediccion escrita que decia *"~6x es el
/// techo honesto, dos hilos SMT comparten unidades de ejecucion"*.
///
/// **La prediccion no estaba equivocada: la faena no era la que suponia.** Y un
/// numero que solo vale para la faena que lo produjo, presentado como *"lo que
/// acelera esta maquina"*, **es un numero deshonesto** -- por bueno que sea.
///
/// El dueno lo dijo con la palabra exacta: *"el SMP si no es honesto a base de
/// Perfil no me sirve"*.
///
/// ## Ocho acumuladores, y por que ocho
///
/// Independientes, para que el CPU pueda lanzarlos a la vez y las unidades se
/// llenen. Ocho es mas que los puertos de ejecucion de un Zen 3, que es
/// justamente el punto: **la maquina tiene que quedarse sin sitio.**
///
/// [!] Y aqui SI se deja que el compilador vectorice, al reves que en la de
/// latencia. No es un descuido: **un motor de inferencia vectoriza**, y una
/// prueba que lo impidiera mediria un trabajo que nadie va a correr.
fn faena_ancho(parte: u32, de: u32) {
    let bloque = VUELTAS_ANCHO / de as u64;
    let desde = bloque * parte as u64;
    let hasta = if parte + 1 == de { VUELTAS_ANCHO } else { bloque * (parte as u64 + 1) };
    let mut a: [u64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let mut i = desde;
    while i < hasta {
        let mut k = 0usize;
        while k < 8 {
            a[k] = a[k].wrapping_add(i.wrapping_mul(k as u64 + 1));
            k += 1;
        }
        i += 1;
    }
    // Que el resultado SALGA: sin esto el compilador puede tirar el bucle
    // entero, y la prueba mediria lo rapido que es no hacer nada.
    let mut h = 0u64;
    for x in a.iter() {
        h ^= *x;
    }
    if (parte as usize) < SUMAS.len() {
        SUMAS[parte as usize].store(h, Ordering::SeqCst);
    }
}

/// Corre la misma cuenta con **un** nucleo y con **todos**, y devuelve
/// `(ticks_uno, ticks_todos, partes)`.
///
/// Se mide con `rdtsc` y no con el reloj de ticks porque esto dura decimas de
/// segundo: contar en milisegundos daria dos numeros tan cercanos que la
/// aceleracion saldria de la nada.
pub fn prueba(obreros: u32) -> (u64, u64, u32) {
    prueba_de(faena_prueba, obreros)
}

/// **La faena de ANCHO**: cuentas independientes que saturan las unidades.
/// Es la que predice un calculo denso. Ver [`faena_ancho`].
pub fn prueba_ancho(obreros: u32) -> (u64, u64, u32) {
    prueba_de(faena_ancho, obreros)
}

fn prueba_de(faena: Faena, obreros: u32) -> (u64, u64, u32) {
    // ** CON EL RELOJ SERIALIZADO, y esa palabra costo una tanda de fotos.
    //
    // Con `rdtsc()` a secas --que lleva `options(nomem)`-- esto contesto
    // `ticks con UN nucleo =37` para **cuatrocientos millones de vueltas**. El
    // reparto estaba bien: once obreros entraron, vieron y terminaron. Lo que
    // no media nada era el cronometro, porque nada ataba el trabajo a estar
    // ENTRE las dos lecturas. Ver `scheduler::rdtsc_serial`.
    use crate::ring0::task::scheduler::rdtsc_serial as reloj;
    let t0 = reloj();
    repartir(faena, 0);
    let uno = reloj().wrapping_sub(t0);

    let t1 = reloj();
    let ok = repartir(faena, obreros);
    let todos = reloj().wrapping_sub(t1);

    // Si alguien no llego, el numero de "todos" mide una carrera incompleta y
    // seria el mas bonito de los dos. Se devuelve 0 partes para que quien pinte
    // no pueda ensenarlo como si valiera.
    (uno, todos, if ok { obreros + 1 } else { 0 })
}

/// **La medida es fisicamente posible?**
///
/// La faena es una cadena de dependencias: cada vuelta necesita el resultado de
/// la anterior, asi que **ni un CPU perfecto podria hacer una vuelta por ciclo**
/// -- una multiplicacion entera ya cuesta tres. O sea que menos de `VUELTAS`
/// ticks para `VUELTAS` vueltas no es "muy rapido": es **imposible**, y lo que
/// esta roto es el cronometro.
///
/// ** Existe porque el 2026-08-11 el sistema enseno `37` ticks para 400 millones
/// de vueltas y **nadie sospecho del reloj**: se busco el fallo en el reparto,
/// que estaba bien. Un instrumento que no puede denunciarse a si mismo manda a
/// depurar el sitio equivocado, y eso cuesta arranques.
///
/// > Un numero que no puede ser cierto tiene que decirlo el, no el que mira.
pub fn medida_creible(ticks: u64) -> bool {
    ticks >= VUELTAS
}

/// El hash que dejo la ultima faena de la parte 0. **Cero = no se ejecuto.**
///
/// Es la prueba directa de que el trabajo ocurrio, independiente del reloj: si
/// esto trae un valor y los ticks son ridiculos, el que miente es el cronometro
/// y no hay que buscar mas lejos.
pub fn suma_testigo() -> u64 {
    SUMAS[0].load(Ordering::SeqCst)
}
