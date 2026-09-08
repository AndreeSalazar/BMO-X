//! **EL RITMO DEL ESCRITORIO**: cuantas vueltas da el bucle, cuando toca
//! refrescar, en que se le va el segundo y por donde devuelve el turno.
//!
//! [carril]  AMARILLO  hereda el color de lo que mas hace: MEDIR. Casi todo
//!           lo de aqui es un instrumento, y un instrumento no se rompe --
//!           convence. Ver `scene/pulso/amarilla.rs`, que lo pinta
//!
//! [cuesta]  TAREA -- medir mal no rompe nada, pero `ceder` SI: es lo unico
//!           del fichero que no mide sino que ACTUA, y una equivocacion ahi
//!           se queda el CPU. Ya paso dos veces el 2026-09-08: la primera se
//!           llevo el teclado del dueno, la segunda dejo el escritorio en
//!           **un fotograma cada diez segundos**.
//!
//! [riesgo]  RELOJ SILENCIO
//!           RELOJ    -- todo sale de `INFO_TSC_HZ` y del latido del kernel.
//!                       Sin reloj no hay ni una cifra que signifique algo, y
//!                       sin latido `ceder` degrada a girar.
//!           SILENCIO -- ninguno de sus fallos da error. Los tres que ha
//!                       tenido --el numero sin aguja, el cero sin reloj y el
//!                       `WAIT` que no dormia-- compilaban, corrian y
//!                       mentian. Los tres se cazaron EN EL METAL.
//!
//! # *** POR QUE ESTO SALE DE `desktop/mod.rs` (2026-09-08)
//!
//! Lo pidio el dueno --*"romper el archivo"*-- y llegaba justo a tiempo:
//! `mod.rs` estaba en **976 lineas, veinticuatro del techo de L6a**. La
//! siguiente cifra que hubiera hecho falta medir la habria bloqueado el
//! guardian.
//!
//! ** Y el corte no se eligio por la cuenta: `Tick` era **mas de la mitad del
//! fichero** --500 de esas 976-- y es lo unico de ahi que no describe el
//! escritorio sino su RELOJ. El resto de `mod.rs` son las cosas que el
//! escritorio TIENE (ventanas, calculadora, salida); esto es lo que el
//! escritorio HACE cada vuelta.
//!
//! El remedio es el que esta casa ya aplico cuando `contrato.py` cruzo las
//! mil: **mecanico -- mover texto, y demostrable**. Ni una linea de logica
//! cambia en este commit, y por eso se puede comprobar leyendo el diff.
//!
//! [!] El corte SIGUIENTE ya se ve desde aqui, y se dice para no olvidarlo:
//! este fichero tiene dos mitades con riesgo distinto --**medir** (amarillo:
//! si se equivoca, convence) y **dar el turno** (rojo: si se equivoca, se
//! queda la maquina)-- y merecen carriles separados como los tiene el pulso.
//! No se hace en la misma tanda que el arreglo del planificador: un corte
//! mecanico y un cambio de comportamiento en el mismo diff es un diff que no
//! se puede leer.

use bmo_userland as bmo;

/// What only lives for one turn of the loop, plus the two edge detectors that
/// have to remember the turn before.
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
    /// `rdtsc` justo antes de ceder, y `0` si todavia no se cedio nunca.
    cedio_en: u64,
    /// `rdtsc` del principio de esta vuelta.
    inicio: u64,
    /// The open sample: when it started (cycles) and at which pass.
    sample_at: u64,
    sample_loops: u32,
    /// When the current quarter second started, in cycles.
    quarter_at: u64,
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
const NO_CLOCK: u64 = u64::MAX;

/// How often anything that refreshes on a rhythm should refresh.
///
/// A quarter of a second, and the number comes from the measurement itself: the
/// CPU rows of F7 are **differences between two readings**, so a window of 16 ms
/// makes a watt tremble instead of settle. Refreshing faster does not give more
/// information -- it gives the same information shaking.
const QUARTER_MS: u64 = 250;

/// Passes between refreshes when there is no reference clock. Exactly what the
/// two callers used before this existed, so a machine without a calibrated TSC
/// is left no worse than it was.
const QUARTER_LOOPS: u32 = 15;

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
            pintados_por_segundo: 0,
            pintados: 0,
            suma_cuerpo: 0,
            suma_puerta: 0,
            latido: 0,
            visto: 0,
            dormidas: 0,
            dormidas_por_segundo: 0,
            cedio_en: 0,
            inicio: 0,
            quarter: false,
            sample_at: 0,
            sample_loops: 0,
            quarter_at: 0,
            tsc_hz: 0,
        }
    }

    /// **One pass of the main loop.** Counts it, raises the rhythm edge, and
    /// closes the second when it is due.
    ///
    /// `bmo::ciclos()` is `rdtsc`, not a syscall -- a couple of dozen cycles --
    /// so reading it every pass is affordable. What is not affordable is kept
    /// out on purpose: see `tsc_hz`.
    pub fn pulse(&mut self) {
        self.loops = self.loops.wrapping_add(1);
        let now = bmo::ciclos();
        if self.tsc_hz == 0 {
            let hz = bmo::info(bmo::INFO_TSC_HZ);
            self.tsc_hz = if hz == 0 { NO_CLOCK } else { hz };
            self.sample_at = now;
            self.sample_loops = self.loops;
            self.quarter_at = now;
            self.inicio = now;
            self.quarter = false;
            return;
        }
        if self.tsc_hz == NO_CLOCK {
            // Sin reloj no hay ritmo que medir: se cuenta como se contaba.
            self.inicio = now;
            self.quarter = self.loops % QUARTER_LOOPS == 0;
            return;
        }
        // ** SE CIERRA EL TRAMO DE LA PUERTA. Entre `cediendo()` y este
        // instante no corrio ni una linea del compositor: lo que haya pasado
        // ahi es tiempo que el escritorio ESPERO, no que gasto.
        if self.cedio_en != 0 {
            self.suma_puerta = self.suma_puerta.wrapping_add(now.wrapping_sub(self.cedio_en));
        }
        self.inicio = now;
        self.quarter = now.wrapping_sub(self.quarter_at) >= self.ciclos_de(QUARTER_MS);
        if self.quarter {
            self.quarter_at = now;
        }
        if now.wrapping_sub(self.sample_at) >= self.tsc_hz {
            self.loops_per_second = self.loops.wrapping_sub(self.sample_loops);
            // El reparto del segundo que se cierra, y a cero para el siguiente.
            // Se publica en ms enteros: decimas de ms en una barra de tareas es
            // precision que nadie puede usar leyendo de lejos.
            let por_ms = self.tsc_hz / 1_000;
            if por_ms > 0 {
                self.cuerpo_ms = (self.suma_cuerpo / por_ms) as u32;
                self.puerta_ms = (self.suma_puerta / por_ms) as u32;
            }
            self.pintados_por_segundo = self.pintados;
            self.pintados = 0;
            self.dormidas_por_segundo = self.dormidas;
            self.dormidas = 0;
            self.suma_cuerpo = 0;
            self.suma_puerta = 0;
            self.sample_at = now;
            self.sample_loops = self.loops;
        }
    }

    /// **Esta vuelta pinto.** Una suma, sin cruzar ninguna puerta.
    ///
    /// Lo llama `compose`, que es el unico sitio donde `will_paint` ya es
    /// definitivo: la recogida de entrada todavia lo puede subir.
    pub fn anota_pintado(&mut self) {
        if self.will_paint {
            self.pintados = self.pintados.wrapping_add(1);
        }
    }

    /// **Tomar el LATIDO.** Una vez en la vida del proceso, al arrancar.
    ///
    /// Si el kernel dice que no, `latido` se queda en `0` y `ceder` sigue
    /// girando como giraba. **Nunca peor que antes** -- y se ve en la barra,
    /// porque la caja del pulso cambia de nombre segun por donde este.
    pub fn tomar_latido(&mut self) {
        if let Some(h) = bmo::latido_tomar() {
            self.latido = h;
            // El testigo arranca en `0` y se pone al dia SOLO en la primera
            // vuelta. No se pregunta la cuenta aqui, y eso no es pereza: ver
            // "el testigo se saca del propio WAIT" en `ceder`.
            self.visto = 0;
        }
    }

    /// **El escritorio va montado en el reloj del hardware Y ESO FUNCIONA.**
    ///
    /// ** No dice "tengo el handle": dice "he dormido". La version anterior
    /// contestaba lo primero, y por eso la barra puso `latido` con toda la
    /// confianza del mundo mientras `WAIT` volvia en el acto trece mil veces
    /// por segundo. El letrero existe para distinguir los dos modos, asi que
    /// tiene que mirar el modo, no el permiso.
    ///
    /// Basta con que haya dormido ALGUNA vez en el ultimo segundo: una vuelta
    /// con trabajo de verdad no duerme, y eso esta bien. Lo que no puede pasar
    /// desapercibido es que no duerma NINGUNA.
    pub fn en_latido(&self) -> bool {
        self.latido != 0 && self.dormidas_por_segundo > 0
    }

    /// El plazo de seguridad del `WAIT`, en nanosegundos.
    ///
    /// ** NO se pone `0` --que seria "solo el latido"-- y el motivo esta escrito
    /// dos veces en esta casa: un bloqueo que solo despierta un aviso se cuelga
    /// para siempre el dia que el aviso no llegue. Ver `dormir_un_rato`, que ya
    /// se lo encontro.
    ///
    /// 50 ms es el suelo: si el latido se parara, el escritorio seguiria dando
    /// 20 vueltas por segundo y **el pulso lo diria en la barra** en vez de
    /// quedarse negro. Un instrumento tiene que sobrevivir a lo que mide.
    const PLAZO_NS: u64 = 50_000_000;

    /// **Devolver el turno**, que es lo que cierra cada vuelta.
    ///
    /// Hace dos cosas y las dos van juntas a proposito: cierra el tramo del
    /// CUERPO --un `rdtsc`, sin cruzar ninguna puerta-- y da el turno. Que sea
    /// un solo sitio es lo que garantiza que las dos mitades del segundo sumen:
    /// medir en un metodo y ceder en otro deja un hueco entre los dos que no
    /// cuenta nadie.
    ///
    /// # Las dos formas de dar el turno, y en que se diferencian
    ///
    /// ```text
    ///    yield_screen   "quitame de en medio y devuelveme el turno ya"
    ///                   -> vuelve en cuanto no haya nadie mejor. Girar con
    ///                      buenos modales
    ///    WAIT(latido)   "despiertame cuando lata el reloj"
    ///                   -> el nucleo queda LIBRE hasta entonces
    /// ```
    ///
    /// *** El techo util de este bucle son 250 vueltas/s --lo pone el bus USB,
    /// que late cada 4 ms-- y el latido va a 1 kHz, o sea CUATRO VECES por
    /// encima de lo que la entrada puede refrescar. No se pierde ni un evento y
    /// se deja de quemar el nucleo para descubrir que no ha pasado nada.
    ///
    /// # *** EL TESTIGO SE SACA DEL PROPIO `WAIT`, y la primera version no
    ///
    /// La primera version releia la cuenta con `latido_cuenta` antes de esperar.
    /// **Y no funciono en el metal**, con un numero que no dejaba dudas:
    ///
    /// ```text
    ///    latido 12937/s   pinta 3   cuerpo 1066   puerta 29
    /// ```
    ///
    /// Trece mil vueltas pidiendo dormir mil veces, 29 ms de puerta en todo un
    /// segundo, y el cuerpo quedandose el resto. **`WAIT` no durmio ni una vez.**
    ///
    /// La cadena entera, y cada eslabon estaba escrito:
    ///
    /// ```text
    ///    latido::claim   cap::grant(..., RIGHT_WAIT, ...)   solo ese derecho
    ///                    y su comentario lo dice: "sobre este handle no se
    ///                    lee ni se escribe nada, se espera"
    ///    latido_cuenta   es un INVOKE -> resuelve con RIGHT_READ -> FALLA
    ///    .unwrap_or(0)   se traga el fallo -> `visto` = 0 PARA SIEMPRE
    ///    WAIT            `current != observed` (0) -> vuelve EN EL ACTO
    /// ```
    ///
    /// ** Y lo caro no fue no dormir: fue **dejar de ceder**. Este metodo habia
    /// sustituido al `yield_screen()` incondicional, asi que el escritorio se
    /// quedo el nucleo entero y el teclado y el raton del dueno parecieron
    /// ignorados. Una optimizacion que se apaga sola tiene que apagarse HACIA
    /// EL LADO SEGURO, y esta se apagaba hacia el peor.
    ///
    /// # Como se sabe si durmio: SE MIRA EL RELOJ, no lo que contesto
    ///
    /// El valor que devuelve `WAIT` es *advisory*, y encima **miente cuando
    /// falla**: un error trae `value = 0`, que con el testigo en 0 se confunde
    /// con "durmio". Preguntarle al mecanismo por su propio estado es
    /// preguntarle al sospechoso -- ver [`Tick::dormidas`].
    ///
    /// El reloj no es sospechoso:
    ///
    /// ```text
    ///    paso ~un milisegundo   durmio     -> el testigo avanza uno
    ///    no paso nada           NO durmio  -> se pone al dia con lo que
    ///                                         contesto **y SE CEDE IGUAL**
    /// ```
    ///
    /// ** Y esa ultima linea es la invariante entera: el bucle no puede acabar
    /// una vuelta sin soltar el turno, haga `WAIT` lo que haga. Si el mecanismo
    /// se rompe, esto degrada a lo que habia antes --girar CEDIENDO-- que es
    /// lento y no se lleva el teclado por delante.
    pub fn ceder(&mut self) {
        let antes = bmo::ciclos();
        let con_reloj = self.tsc_hz != 0 && self.tsc_hz != NO_CLOCK;
        if con_reloj {
            self.suma_cuerpo = self.suma_cuerpo
                .wrapping_add(antes.wrapping_sub(self.inicio));
            self.cedio_en = antes;
        }
        if self.latido == 0 {
            bmo::yield_screen();
            return;
        }
        // *** EL TESTIGO SE RELEE, y esta es la tercera version de esta linea.
        //
        // La segunda lo sacaba del valor que devuelve `WAIT`, para no cruzar una
        // puerta. **Y eso solo vale si el bucle es MAS RAPIDO que el latido.**
        // El metal dijo que no lo es:
        //
        // ```text
        //    pulso 4/s   pinta 4   cuerpo 2   puerta 1237
        // ```
        //
        // Cuatro vueltas por segundo, o sea 300 ms por vuelta, o sea **300
        // latidos entre dos vueltas**. Con el testigo sacado de la vuelta
        // anterior llega caducado SIEMPRE, `current != observed`, y `WAIT`
        // vuelve en el acto -> se cede -> 300 ms -> y otra vez. Un circulo
        // vicioso que se alimenta de su propia lentitud.
        //
        // ** Releer cuesta UNA puerta: 969 ciclos sobre una vuelta de 300 ms es
        // la tres millonesima parte. Y ahora se PUEDE, porque el mismo dia se
        // arreglo el derecho que faltaba en `latido::claim`. Las dos mitades del
        // arreglo eran una sola pieza y las separe: esto lo junta.
        self.visto = bmo::latido_cuenta(self.latido).unwrap_or(self.visto);
        let vuelve = bmo::latido_esperar(self.latido, self.visto, Self::PLAZO_NS);
        // ** EL JUEZ ES EL RELOJ. Un latido son 1.000 us y una puerta 0,26, asi
        // que el umbral --la decima parte de un latido-- esta a 380 veces una
        // puerta y a 10 veces por debajo de un latido. No hay forma de
        // confundir las dos cosas.
        let durmio = con_reloj
            && bmo::ciclos().wrapping_sub(antes) >= (self.ciclos_de(1) / 10).max(1);
        if durmio {
            self.dormidas = self.dormidas.wrapping_add(1);
        } else {
            // No durmio. `vuelve` no se usa para nada --la cuenta se relee
            // arriba-- y lo unico que importa aqui es que **ceder no es
            // opcional**: si el mecanismo no duerme, esto degrada a girar
            // cediendo, que es lento y no se lleva el teclado por delante.
            let _ = vuelve;
            bmo::yield_screen();
        }
    }

    /// **No hay reloj de referencia**: el kernel contesto `0` a `INFO_TSC_HZ`.
    ///
    /// == ** POR QUE ESTO SE PUBLICA (2026-09-08) ==========================
    ///
    /// Sin reloj, `pulse` sale por su rama de emergencia y **`loops_per_second`
    /// no se calcula nunca**: se queda en el `0` con el que nacio. Eso esta
    /// escrito arriba, en el propio campo... y aun asi la barra pintaba
    /// `pulso 0/s`, que cualquiera lee como *"el bucle esta muerto"*.
    ///
    /// ```text
    ///    lo que pasa       no tengo con que medir el ritmo
    ///    lo que se veia    el ritmo es CERO
    /// ```
    ///
    /// ** Son dos cosas distintas y se veian iguales -- la misma clase de fallo
    /// que la aguja acaba de arreglar, un escalon mas abajo. Un instrumento que
    /// no sabe la respuesta tiene que decir QUE NO LA SABE, no dar un cero: un
    /// cero es una medida, y esa medida nunca se tomo.
    pub fn sin_reloj(&self) -> bool {
        self.tsc_hz == NO_CLOCK
    }

    /// The same quarter second as `quarter`, in cycles, for whoever cannot read
    /// the edge because they are not called on every pass.
    pub fn quarter_cycles(&self) -> u64 {
        self.ciclos_de(QUARTER_MS)
    }

    /// How many cycles `ms` milliseconds are **on this machine**.
    ///
    /// It is handed out instead of the frequency because the caller should not
    /// have to do this conversion again: three places got the same conversion
    /// wrong by assuming a rate instead of asking for one.
    ///
    /// ** ZERO when there is no reference clock -- and zero means *"no spacing
    /// is possible, look every time"*, not *"never"*. A caller that spaces two
    /// `OP_INFO` reads is then paying a few hundred cycles it did not have to;
    /// a caller that never looks again leaves a light lying about the bus. Of
    /// the two ways to be wrong without a clock, that is the cheap one.
    pub fn ciclos_de(&self, ms: u64) -> u64 {
        if self.tsc_hz == 0 || self.tsc_hz == NO_CLOCK {
            return 0;
        }
        (self.tsc_hz / 1_000) * ms
    }
}
