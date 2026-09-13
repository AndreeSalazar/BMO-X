//! **EL RELOJ DEL ESCRITORIO**, repartido en sus dos mitades.
//!
//! [consumo] NADA      mide, y solo cuando el bucle le pasa por encima. El que
//!                     late es `main.rs` y el que duerme es `tick/roja.rs`
//!                     (L6h)
//!
//! [carril]  AMARILLO  el reparto, y hereda el color de lo que mas hace: MEDIR
//!
//! [cuesta]  TAREA -- medir mal no rompe nada; `roja.rs` SI, y por eso son dos
//!           ficheros. El coste de la carpeta es el del carril que manda.
//!
//! [riesgo]  RELOJ SILENCIO
//!           los dos entran por sus carriles. Aqui no hay ni logica ni pixel:
//!           solo el ESTADO que las dos mitades comparten.
//!
//! # *** POR QUE ESTE FICHERO SE PARTIO (L6g nivel 3, 2026-09-08)
//!
//! Lo dejo prometido su propia version anterior, con estas palabras:
//!
//! > *"este fichero tiene dos mitades con riesgo distinto --MEDIR (amarillo: si
//! > se equivoca, convence) y DAR EL TURNO (rojo: si se equivoca, se queda la
//! > maquina)-- y merecen carriles separados como los tiene el pulso"*.
//!
//! ** Y el mismo dia el metal dio la evidencia, dos veces, las dos en la mitad
//! que ACTUA y ninguna en la que mide:
//!
//! ```text
//!    `ceder` sin la invariante   el escritorio se quedo el nucleo y el
//!                                teclado del dueno parecio ignorado
//!    `WAIT` bloqueando de verdad ROTTEN CONTEXT: the seal is gone.
//!                                La maquina PARADA
//! ```
//!
//! *** Ninguna averia de este fichero ha venido nunca de contar mal. Las dos han
//! venido de **dar el turno**, que es una sola funcion. Ese es el corte, y no la
//! cuenta de lineas: 604 no obligan a nada.
//!
//! ```text
//!    roja.rs      DAR EL TURNO   se equivoca -> se queda la maquina
//!    amarilla.rs  MEDIR          se equivoca -> CONVENCE, que es peor
//!    mod.rs       el estado que las dos comparten, y nada mas
//! ```
//!
//! # Y el estado se queda AQUI a proposito
//!
//! Los campos son de las dos mitades --`ceder` cierra el tramo que `pulse`
//! abrio-- asi que repartirlos seria inventar una frontera donde no la hay. En
//! Rust un modulo hijo ve lo privado de su padre, asi que los carriles llegan a
//! ellos sin abrirlos a nadie mas: **el estado es de la carpeta, no del arbol**.

mod amarilla;
mod roja;

pub(crate) struct Tick {
    /// **Vueltas del bucle principal**, no fotogramas.
    ///
    /// * SE LLAMABA `frames`, Y ESE NOMBRE ERA EL FALLO. Este contador sube una
    /// vez por vuelta y el bucle no tiene freno --acaba en `yield_screen()` y
    /// vuelve--, asi que una vuelta que no pinta nada son unas pocas puertas.
    /// Quien leia `frames` entendia "fotogramas de pantalla" y calibraba contra
    /// sesenta por segundo; tres sitios lo hicieron. Ver `loops_per_second`.
    pub loops: u32,
    pub will_paint: bool,
    /// **Paso algo en esta vuelta**: una tecla, el raton, una superficie nueva
    /// o repintada, un hijo que nacio o murio. NO el cuarto de segundo.
    ///
    /// ** Es una pregunta distinta de `will_paint`, y juntarlas fue W4b del
    /// plan de vatios: el cuarto de segundo PINTA --el cursor parpadea, la
    /// barra se pone al dia-- pero no es que pase nada, y metido aqui
    /// reiniciaba el reposo cada 250 ms. El reposo escucha a esta; el pintor,
    /// a la otra.
    pub actividad: bool,
    pub repaint_field: bool,
    /// Where the mouse cursor was left. `u32::MAX` means "nowhere yet".
    pub ax: u32,
    pub ay: u32,
    /// A click is a BUTTON GOING DOWN, not "the button is down". Without the
    /// edge, holding it would type a hundred times a second.
    pub button_before: bool,
    /// El flanco del boton DERECHO, aparte del izquierdo.
    ///
    /// * Tiene que ser otro: los dos botones llegan en la misma mascara pero
    /// significan cosas distintas, y con un solo flanco mantener pulsado el
    /// derecho reabriria el menu en cada fotograma.
    pub derecho_before: bool,
    pub combo_before: bool,
    pub key_during_combo: bool,
    /// Which calculator key the pointer is over, if any. Carried as state
    /// because the highlight is repainted only WHEN IT CHANGES.
    pub calc_hover: Option<u8>,
    /// The rectangles left behind by windows whose app died, to give back to
    /// the desktop. A mailbox, not a state: filled and emptied in one turn.
    pub dead_boxes: [(u32, u32, u32, u32); crate::scene::surface::MAX],
    /// **How many passes of the main loop fit in a second**, last time a whole
    /// second could be closed. `0` until the first one closes.
    ///
    /// * IT IS MEASURED BECAUSE IT WAS BEING ASSUMED. Two grids timed their
    /// double click by counting `frames` against a constant whose comment said
    /// *"a los ~60 por segundo del escritorio son unos 400 ms"* -- and this
    /// loop has no pacing at all: it ends in `yield_screen()` and comes right
    /// back. Nobody had ever counted, so nobody knew whether that gesture was
    /// 400 ms or 4. The gesture now uses cycles (`scene::double_click`), and
    /// this number exists so the rhythm itself stops being a guess. It shows in
    /// F7, which is the window for when something feels slow.
    pub loops_per_second: u32,
    /// **True on the pass that opened a new quarter second.** Whoever refreshes
    /// on a rhythm reads this instead of counting passes.
    ///
    /// It is consumed in `desktop::paint::compose`, which runs on EVERY pass --
    /// if it were read only on passes that paint, the edge would be missed. The
    /// USB witness light does have that constraint, and that is why it keeps
    /// its own distance instead: see `scene::testigo::refrescar`.
    pub quarter: bool,
    /// **De cada segundo, cuantos ms se fueron DENTRO de la vuelta** -- todo
    /// el trabajo del compositor: mirar la entrada, componer y volcar.
    ///
    /// == *** PARA QUE ESTAN ESTAS DOS CIFRAS (2026-09-08) ================
    ///
    /// El dueno midio el pulso en el Ryzen y salio **50 vueltas por segundo**.
    /// El bucle vive --la aguja gira, hay reloj-- pero 50 vueltas son **20 ms
    /// por vuelta**, y esto no es un bucle con freno: no tiene ninguno.
    ///
    /// Y ahi la pregunta se vuelve a partir en dos, igual que la partio la
    /// aguja, porque una vuelta solo tiene dos mitades:
    ///
    /// ```text
    ///    el CUERPO    lo que hace el compositor antes de ceder
    ///    la PUERTA    lo que tarda en volver despues de ceder
    /// ```
    ///
    /// ** Las dos suenan igual desde fuera --"el escritorio va lento"-- y la
    /// cura no se parece en nada: una se arregla en Ring 3 y la otra en el
    /// planificador. Sin separarlas, cualquier arreglo es una apuesta.
    ///
    /// Van en **ms de cada segundo** y no en el peor caso a proposito: sumados
    /// dan ~1000, asi que se leen como un reparto y se ve de un vistazo cual de
    /// las dos mitades se queda el segundo.
    ///
    /// # [!!] MIDE RELOJ DE PARED, NO CPU. Y esto hay que leerlo antes de usarlo
    ///
    /// `cuerpo` es *"desde el principio de la vuelta hasta justo antes de
    /// ceder"*, y eso incluye **el tiempo en que a esta tarea la echaron del
    /// CPU**. Ring 3 no tiene hoy forma de preguntar su propio tiempo de CPU, y
    /// por eso no la tiene esto.
    ///
    /// *** LA PRIMERA LECTURA EN METAL LO DEMOSTRO, y casi me manda al sitio
    /// equivocado. Salio `cuerpo 1066` con 12.937 vueltas, o sea 82 us por
    /// vuelta. Y una vuelta en vacio son NUEVE PUERTAS: 2,36 us. Los otros 80
    /// no los gastaba el compositor -- **se los pasaba fuera del CPU**, porque
    /// `WAIT` estaba roto, el bucle no cedia nunca, y el reloj lo echaba a la
    /// fuerza cada cuatro milisegundos.
    ///
    /// ** Y de ahi sale tambien el otro sintoma que trajo el dueno: el ritmo le
    /// bailo entre 600 y 12.937 en el mismo arranque, y lo llamo *"el kernel
    /// borracho"*. No lo estaba:
    ///
    /// ```text
    ///    el bucle no cedia         -> su ritmo era EL SOBRANTE del CPU
    ///    el sobrante cambia        -> con el bus USB, con el shell, con
    ///                                 cualquiera que quisiera trabajar
    ///    `cuerpo` no cambia        -> el reloj de pared es el reloj de pared
    /// ```
    ///
    /// Montado en el latido el ritmo deja de ser un sobrante y pasa a ser una
    /// propiedad del reloj (1 kHz), asi que el baile se acaba **por
    /// construccion**, no por haber optimizado nada.
    ///
    /// [!] Asi que la lectura honesta es: `cuerpo` grande **no** dice "el
    /// compositor trabaja mucho". Dice *"entre el principio y el final de sus
    /// vueltas pasa mucho tiempo"*, y eso son dos cosas -- trabajar, o esperar
    /// de pie. Para separarlas hace falta que el kernel publique el tiempo de
    /// CPU de una tarea, y hoy no lo hace.
    pub cuerpo_ms: u32,
    /// De cada segundo, cuantos ms se fueron en `yield_screen`. Ver
    /// [`Tick::cuerpo_ms`].
    pub puerta_ms: u32,
    /// **PUERTAS POR VUELTA, por DIEZ.** `92` son 9,2 puertas por vuelta.
    ///
    /// == *** EL NUMERO QUE SOSTENIA TODO EL ARGUMENTO DE VATIOS, Y NADIE
    /// ==     LO HABIA MEDIDO (2026-09-12) ===============================
    ///
    /// El presupuesto de este bucle esta escrito en `main.rs` y se apoya en una
    /// sola cifra: *"una vuelta en vacio cruza NUEVE puertas"*, de ahi 8.721
    /// ciclos, de ahi el 0,06 % del CPU a 250 vueltas y el 14 % a 60.000.
    ///
    /// ** Esas nueve eran una cuenta A MANO, hecha leyendo el bucle. Y el
    /// instrumento para medirlas **ya existia y ya llegaba a Ring 3**:
    /// `INFO_SYSCALL_CUENTA`, que `meter.rs` sirve desde el 16-08 y
    /// `medida/coste` lee desde entonces. El unico bucle cuyo gasto decide los
    /// vatios del escritorio nunca pregunto.
    ///
    /// Es el mismo patron que la seccion `Resources` del BEF: estaba en el
    /// formato y nadie la escribia.
    ///
    /// # [!] QUE CUENTA DE VERDAD: LA MAQUINA, NO ESTE PROCESO
    ///
    /// `meter::doors()` es un contador **global del kernel**. Asi que esto es
    /// el trafico de TODA la maquina repartido entre las vueltas de ESTE bucle:
    ///
    /// ```text
    ///    escritorio solo    es suyo, y entonces se compara con las 9 a mano
    ///    con una app        son las suyas TAMBIEN, y el numero sube sin que
    ///                       el compositor haya cambiado nada
    /// ```
    ///
    /// ** Por eso se llama TRAFICO y no "mis puertas" -- el vocabulario lo puso
    /// `medida/coste`, que a lo mismo le dice `trafico_total`. Un nombre que
    /// promete menos de lo que mide es como se lee mal un instrumento bueno.
    ///
    /// # Lo que cuesta medirlo: UNA puerta por segundo
    ///
    /// No por vuelta. Leerlo en cada vuelta seria subir un 11 % las puertas
    /// para poder contar puertas -- medir el termometro, que es justo lo que la
    /// cabecera de `meter.rs` avisa. Una vez por segundo, sobre ~9.000 puertas,
    /// es el 0,01 %.
    pub trafico_x10: u32,
    /// La lectura anterior de `INFO_SYSCALL_CUENTA`. Se lee como DELTA.
    trafico_visto: u64,
    /// **De cada segundo, cuantas vueltas PINTARON algo.**
    ///
    /// == *** LA OTRA MITAD DE `loops_per_second` (2026-09-08) ============
    ///
    /// El ritmo dice a que velocidad gira. Esto dice **cuantas de esas vueltas
    /// sirvieron para algo**, y el cociente de los dos es el desperdicio:
    ///
    /// ```text
    ///    20000 vueltas, 4 pintan    19996 vueltas para descubrir que no
    ///    250 vueltas, 4 pintan      lo mismo, sin quemar el nucleo
    /// ```
    ///
    /// ** Y hace falta un numero porque la decision que viene se toma con el:
    /// si el bucle debe seguir girando o pasar a montarse en el LATIDO. Ver la
    /// nota del presupuesto en `main.rs`, junto al `yield_screen`.
    pub pintados_por_segundo: u32,
    /// Las que llevan pintado en el segundo en curso.
    pintados: u32,
    /// Los dos tramos del segundo en curso, en ciclos.
    suma_cuerpo: u64,
    suma_puerta: u64,
    /// **El LATIDO del hardware**, o `0` si no se pudo tomar.
    ///
    /// == *** EL SEGUNDO SYSCALL, ESTRENADO (2026-09-08) ==================
    ///
    /// Lo vio el dueno:
    ///
    /// > *"tengo 2 syscalls, INVOKE y WAIT, pero WAIT casi no se usaba.
    /// > Creo que es momento de darle su oportunidad."*
    ///
    /// Y era literal. `WAIT` se usaba en **UN** sitio de todo el repo --el
    /// `dormir_un_rato` del arranque, con esperable `0`, o sea un `sleep`-- y
    /// `latido_esperar` no lo llamaba **nadie**. El suelo S3 se construyo
    /// entero, se documento, se le puso envoltorio de userland, y no se
    /// estreno. Un sistema con dos puertas donde una solo sabe dormir un plazo
    /// no tiene dos puertas: tiene una y media.
    ///
    /// ** Y el sitio donde faltaba era este, el bucle que corre SIEMPRE. Antes
    /// acababa en `yield_screen()`: "quitame de en medio y devuelveme el turno
    /// en cuanto puedas", que es girar con buenos modales. Ahora dice lo que de
    /// verdad quiere -- **despiertame cuando lata el reloj** -- y esa frase solo
    /// se puede decir con `WAIT`.
    latido: u64,
    /// El testigo del ultimo latido visto. Ver `ceder`.
    visto: u64,
    /// **Cuantas vueltas DURMIERON de verdad** en el segundo en curso.
    ///
    /// == *** POR QUE ESTO NO SE DEDUCE, SE MIDE (2026-09-08) ==============
    ///
    /// La primera version decidia "durmio o no" comparando el valor que
    /// devuelve `WAIT` con el que se le paso. **Y eso tiene un agujero**: si
    /// `WAIT` falla --por ejemplo porque al handle le falta un derecho, que es
    /// justo lo que acababa de pasar-- devuelve un error con `value = 0`. Con
    /// `visto` en 0, la comparacion daba IGUAL y el bucle se creia dormido.
    ///
    /// ```text
    ///    una vuelta   cree que durmio  -> NO cede
    ///    la siguiente ve que no cuadra -> cede
    /// ```
    ///
    /// O sea medio giro, otra vez, y con el mismo disfraz. *** Deducir el
    /// estado de un mecanismo a partir de lo que devuelve ese mecanismo es
    /// preguntarle al sospechoso. El reloj no es sospechoso: si paso un
    /// milisegundo, durmio; si no paso nada, no durmio. Se mide y se acabo.
    dormidas: u32,
    /// Las del ultimo segundo cerrado, y es lo que hace HONESTO el letrero.
    dormidas_por_segundo: u32,
    /// == *** EL REPOSO (2026-09-11): si no pasa nada, no se pregunta ======
    ///
    /// Vueltas SEGUIDAS que no pintaron nada. El bucle pedia mil vueltas por
    /// segundo aunque el dueno se hubiera ido a dormir -- el `INT 16h` de
    /// COMMAND.COM, preguntar sin parar. Cuando lleva `REPOSO_TRAS` vueltas sin
    /// nada que ensenar, deja el latido y duerme un plazo fijo: la vuelta pasa
    /// de mil a ~125 por segundo. **Y la primera vuelta que pinta lo devuelve
    /// a mil.** Una tecla despues de un rato quieto se ve como mucho 8 ms
    /// tarde, que es lo que cuesta no despertar al CPU mil veces por segundo
    /// para nada. Ver `docs/plan/PLAN_VATIOS.md`, W4.
    ///
    /// [!] Es la version que NO pide interrupciones del USB: el teclado sigue
    /// viendose por sondeo, solo que ocho veces menos a menudo mientras nadie
    /// lo toca. La de verdad --dormir SOBRE la entrada-- es W3+W4 del plan.
    quietas: u32,
    /// Vueltas del segundo en curso que durmieron en REPOSO (no en el latido).
    reposos: u32,
    /// Las del ultimo segundo cerrado: es lo que pone "reposo" en la barra.
    pub reposos_por_segundo: u32,
    /// `rdtsc` justo antes de ceder, y `0` si todavia no se cedio nunca.
    cedio_en: u64,
    /// `rdtsc` del principio de esta vuelta. Reloj de PARED.
    inicio: u64,
    /// Ciclos de CPU PROPIOS al principio de esta vuelta. Ver `ceder`.
    cpu_inicio: u64,
    /// The open sample: when it started (cycles) and at which pass.
    sample_at: u64,
    sample_loops: u32,
    /// When the current quarter second started, in cycles.
    quarter_at: u64,
    /// ** EL UNICO LECTOR DE FRECUENCIA Y VATIOS DEL ESCRITORIO (2026-09-12).
    ///
    /// Muestrea los contadores que solo crecen una vez por cuarto de segundo y
    /// guarda lo que sale. Las vitales y los informes leen de aqui: antes cada
    /// uno cruzaba la puerta por su cuenta y le robaba el intervalo al otro --
    /// y a cualquier programa que midiera a la vez.
    pub consumo: bmo_juicio::consumo::Muestreo,
    /// The reference clock, asked **once** in the life of the process.
    ///
    /// `INFO_TSC_HZ` never changes, and asking it every pass would put a 969
    /// cycle syscall inside the loop this field exists to measure -- an
    /// instrument that changes what it measures. `u64::MAX` is what gets stored
    /// when the kernel answers `0`: the second never closes, no rate is ever
    /// published, and the question is not asked again.
    tsc_hz: u64,
}


/// What `tsc_hz` holds when the kernel could not give a reference clock.
///
/// Not `0`: zero means "not asked yet", and telling them apart is what keeps
/// the question from being asked once per pass forever.
pub(super) const NO_CLOCK: u64 = u64::MAX;

/// How often anything that refreshes on a rhythm should refresh.
///
/// A quarter of a second, and the number comes from the measurement itself: the
/// CPU rows of F7 are **differences between two readings**, so a window of 16 ms
/// makes a watt tremble instead of settle. Refreshing faster does not give more
/// information -- it gives the same information shaking.
pub(super) const QUARTER_MS: u64 = 250;

/// Passes between refreshes when there is no reference clock. Exactly what the
/// two callers used before this existed, so a machine without a calibrated TSC
/// is left no worse than it was.
pub(super) const QUARTER_LOOPS: u32 = 15;


impl Tick {
    /// **Un reloj recien puesto en hora.**
    ///
    /// ** Vive aqui y no en `Desktop::new` desde el corte del 2026-09-08, y no
    /// por gusto: lo exigio el compilador. Al salir `Tick` de `desktop/mod.rs`
    /// sus campos privados dejaron de verse desde alli, y un tipo que solo
    /// puede construir OTRO fichero lleva la mitad de su invariante fuera de
    /// casa. El error fue una segunda opinion sobre donde iba el corte.
    pub fn nuevo() -> Self {
        Self {
            loops: 0,
            will_paint: false,
            actividad: false,
            repaint_field: false,
            ax: u32::MAX,
            ay: u32::MAX,
            button_before: false,
            derecho_before: false,
            combo_before: false,
            key_during_combo: false,
            calc_hover: None,
            dead_boxes: [(0, 0, 0, 0); crate::scene::surface::MAX],
            loops_per_second: 0,
            cuerpo_ms: 0,
            puerta_ms: 0,
            trafico_x10: 0,
            trafico_visto: 0,
            pintados_por_segundo: 0,
            pintados: 0,
            suma_cuerpo: 0,
            suma_puerta: 0,
            latido: 0,
            visto: 0,
            dormidas: 0,
            dormidas_por_segundo: 0,
            quietas: 0,
            reposos: 0,
            reposos_por_segundo: 0,
            cedio_en: 0,
            inicio: 0,
            cpu_inicio: 0,
            quarter: false,
            sample_at: 0,
            sample_loops: 0,
            quarter_at: 0,
            consumo: bmo_juicio::consumo::Muestreo::nuevo(),
            tsc_hz: 0,
        }
    }
}
