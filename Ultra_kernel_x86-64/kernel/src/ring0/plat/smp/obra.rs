//! **El reparto de trabajo.** Lo único que separa doce núcleos encendidos de
//! doce núcleos que sirven para algo.
//!
//! ═══ El hueco que tapa ═══
//!
//! Los APs arrancaron —`12 de 12` en el Ryzen— y se quedaban en `cli; hlt`
//! **para siempre**. Un núcleo despierto al que no se le puede dar una tarea es
//! exactamente igual de útil que uno dormido, y cuesta lo mismo: nada.
//!
//! ═══ Lo que NO es, y por qué ═══
//!
//! **Esto no es un planificador.** No hay colas, ni prioridades, ni cambio de
//! contexto, ni tareas de Ring 3 corriendo en otro núcleo. Es un reparto de
//! *una* función pura entre *n* partes, con una barrera al final:
//!
//! ```text
//!   el BSP publica   (función, cuántas partes)
//!   cada obrero      hace SU parte y se apunta
//!   el BSP espera    a que estén todas
//! ```
//!
//! Y ser tan poca cosa es lo que lo hace seguro **hoy**, con los 209 `static
//! mut` que hay en el kernel: un obrero que sólo calcula sobre su rango no toca
//! ni uno. Es el contrato del `docs/SMP_MAESTRO.md` — *"de Cell se copia el
//! reparto, no el transporte"*—, y aquí el reparto cabe en cien líneas porque
//! lo caro de Cell era el transporte, que en un CCX con 32 MB de L3 compartida
//! **no hay que escribir**.
//!
//! ═══ ⚠️ El precio, dicho antes de que se note ═══
//!
//! Un obrero en espera **gira** (`pause`), no duerme. Sacarlo de `hlt` pediría
//! una IPI, y para atender una IPI un AP necesita GS por-CPU y su propia TSS —
//! que es justo el trabajo que este módulo evita. Consecuencia real y medible:
//! con los doce en pie, once núcleos giran al 100 % y la máquina consume como
//! si estuviera trabajando.
//!
//! Por eso existe [`parar`], y por eso la orden lo dice en pantalla. Un coste
//! que no se anuncia es una trampa.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// La función que toca hacer. `0` = ninguna.
///
/// Se guarda como número y no como `fn` porque un `AtomicPtr` a función no
/// existe y aquí no hace falta más: el BSP publica, los obreros leen.
static TAREA: AtomicU64 = AtomicU64::new(0);
/// En cuántas partes se ha cortado el trabajo (contando la del BSP).
static PARTES: AtomicU32 = AtomicU32::new(0);
/// Cuántos obreros han terminado su parte.
static HECHOS: AtomicU32 = AtomicU32::new(0);
/// Sube con cada encargo. Es lo que distingue *"hay trabajo nuevo"* de *"sigue
/// el de antes"* sin tener que borrar nada entre medias.
static RONDA: AtomicU32 = AtomicU32::new(0);
/// Cuando se pone, los obreros vuelven a `hlt` y no salen más.
static PARAR: AtomicBool = AtomicBool::new(false);

/// La forma de una faena: `(mi parte, de cuántas)`.
pub type Faena = fn(u32, u32);

/// **El bucle del obrero.** No vuelve.
///
/// `indice` es 0..n-1 entre los APs; su parte es `indice + 1` porque la parte
/// `0` se la queda el BSP, que también trabaja — tener un núcleo mirando cómo
/// trabajan los otros es desperdiciar justo el más caliente de caché.
pub fn obrero(indice: u32) -> ! {
    let mut vista = 0u32;
    loop {
        if PARAR.load(Ordering::SeqCst) {
            // Punto de no retorno: sin IPI no hay quien lo despierte, y volver
            // a llamarlo es un INIT+SIPI entero. Está bien así — es la forma
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
/// `obreros` es cuántos APs participan; el BSP hace la parte `0` siempre. Con
/// `obreros = 0` esto es un `faena(0, 1)` y ni siquiera toca las atómicas.
///
/// Devuelve `false` si alguien no llegó a tiempo — y entonces **el dato que se
/// haya calculado no vale**, porque falta una parte del rango.
pub fn repartir(faena: Faena, obreros: u32) -> bool {
    let partes = obreros + 1;
    if obreros == 0 {
        faena(0, 1);
        return true;
    }

    HECHOS.store(0, Ordering::SeqCst);
    PARTES.store(partes, Ordering::SeqCst);
    TAREA.store(faena as usize as u64, Ordering::SeqCst);
    // La ronda va LA ÚLTIMA: es la señal, y publicarla antes que los datos
    // dejaría a un obrero leyendo la faena de la ronda anterior con las partes
    // de la nueva.
    RONDA.fetch_add(1, Ordering::SeqCst);

    // El BSP hace la suya mientras los demás hacen las suyas.
    faena(0, partes);

    // Y espera, con tope. Un obrero que no contesta no puede colgar la máquina:
    // el número de vueltas es generoso pero finito, por lo mismo que el bring-up
    // no espera para siempre a un AP que no arranca.
    let mut vueltas = 0u64;
    while HECHOS.load(Ordering::SeqCst) < obreros && vueltas < 2_000_000_000 {
        core::hint::spin_loop();
        vueltas += 1;
    }
    let ok = HECHOS.load(Ordering::SeqCst) >= obreros;
    TAREA.store(0, Ordering::SeqCst);
    ok
}

/// **Desactivar los obreros**: vuelven a `hlt` y ahí se quedan.
///
/// Es la otra mitad del mando que pidió el dueño. No hay vuelta atrás sin un
/// INIT+SIPI nuevo, y eso es correcto: "desactivado" tiene que significar
/// desactivado y no "durmiendo por si acaso".
pub fn parar() {
    PARAR.store(true, Ordering::SeqCst);
}

/// ¿Están parados?
pub fn parados() -> bool {
    PARAR.load(Ordering::SeqCst)
}

/// Volver a admitir trabajo. Sólo tiene efecto para los que se despierten
/// DESPUÉS: los que ya entraron en `hlt` no salen solos.
pub fn reanudar() {
    PARAR.store(false, Ordering::SeqCst);
}

// ═══════════════ LA PRUEBA ═══════════════

/// Cuántas vueltas da la faena de prueba **en total**, repartidas entre todos.
///
/// Elegido para que en un núcleo se note (décimas de segundo) y en doce siga
/// midiéndose bien. Es una cuenta pura: ni memoria que compartir ni nada que
/// bloquear, o sea el caso MÁS FAVORABLE que existe — y decirlo importa, porque
/// la aceleración que salga aquí es el techo, no lo que va a dar un programa
/// real.
const VUELTAS: u64 = 400_000_000;

static SUMAS: [AtomicU64; 16] = [const { AtomicU64::new(0) }; 16];

/// Una cuenta que el compilador no puede saltarse: cada vuelta depende de la
/// anterior, así que no hay forma de plegarla ni de vectorizarla.
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

/// Corre la misma cuenta con **un** núcleo y con **todos**, y devuelve
/// `(ticks_uno, ticks_todos, partes)`.
///
/// Se mide con `rdtsc` y no con el reloj de ticks porque esto dura décimas de
/// segundo: contar en milisegundos daría dos números tan cercanos que la
/// aceleración saldría de la nada.
pub fn prueba(obreros: u32) -> (u64, u64, u32) {
    let t0 = crate::ring0::task::scheduler::rdtsc();
    repartir(faena_prueba, 0);
    let uno = crate::ring0::task::scheduler::rdtsc().wrapping_sub(t0);

    let t1 = crate::ring0::task::scheduler::rdtsc();
    let ok = repartir(faena_prueba, obreros);
    let todos = crate::ring0::task::scheduler::rdtsc().wrapping_sub(t1);

    // Si alguien no llegó, el número de "todos" mide una carrera incompleta y
    // sería el más bonito de los dos. Se devuelve 0 partes para que quien pinte
    // no pueda enseñarlo como si valiera.
    (uno, todos, if ok { obreros + 1 } else { 0 })
}
