//! **CARRIL AMARILLO** -- lo que el pulso AFIRMA. Si se equivoca no falla:
//! convence, y eso es peor.
//!
//! [carril]  AMARILLO  es un instrumento. Su modo de fallo no es romperse: es
//!           seguir funcionando y decir algo que no es
//!
//! [cuesta]  NADA -- si esto se equivoca no se rompe nada. Ni una app, ni un
//!           dato, ni la maquina. Y ESA es exactamente la trampa: por eso el
//!           carril y el coste no dicen lo mismo, y por eso son dos etiquetas.
//!           El precedente esta escrito en la ley con nombre --`core/autopsy.rs`,
//!           coste NADA y carril AMARILLO-- y la razon es la misma: **manda la
//!           investigacion al sitio equivocado**.
//!
//! [riesgo]  SILENCIO RELOJ
//!           SILENCIO -- equivocarse aqui no da error: sigue, y da un dato malo.
//!                       Ya paso DOS VECES en un dia, y las dos costaron una
//!                       vuelta al metal. Ver "las dos veces que mintio".
//!           RELOJ    -- todo lo que dice depende de `INFO_TSC_HZ` y del canto
//!                       del cuarto de segundo. Sin reloj no hay ni una cifra
//!                       suya que signifique nada, y por eso lo DICE.
//!
//! # *** LAS DOS VECES QUE MINTIO, y por eso este carril existe aparte
//!
//! Este fichero no se separo por tamano. Se separo porque **las dos unicas
//! averias que ha tenido el pulso estaban las dos aqui dentro**, ninguna en el
//! dibujo, y las dos eran la misma frase:
//!
//! ```text
//!    08-09  el numero sin aguja    `loops_per_second` se calcula UNA vez por
//!                                  segundo -> entre dos calculos, el bucle
//!                                  vivo y el bucle muerto se ven IGUAL
//!    08-09  `pulso 0/s` sin reloj  el cero no era una medida baja: era una
//!                                  medida que nadie tomo -- y apuntaba al
//!                                  bucle, que es donde NO estaba el fallo
//! ```
//!
//! ** Ninguna de las dos rompio nada. Las dos costaron un arranque completo del
//! Ryzen, que es el recurso mas caro que tiene este proyecto. Un carril propio
//! con su letrero es lo que hace que la tercera se vea venir.
//!
//! # Las SEIS afirmaciones, y el porque de cada una
//!
//! Todo lo que hace este fichero es decidir **que es verdad**. Seis
//! decisiones, y ninguna toca un pixel:
//!
//! ```text
//!    1  la aguja avanza SIEMPRE      para que `quieta` signifique `muerto`
//!    2  sin reloj no hay numero      un cero es una medida, y no se tomo
//!    3  el numero en blanco si <100  dos cifras no son rendimiento: son el
//!                                    bucle sin turno, y tienen que gritar
//!    4  la mitad grande en blanco    es LA RESPUESTA: de que lado tirar
//!    5  al lado del ritmo, `pinta`   el ritmo dice lo rapido que gira; esto
//!                                    dice cuantas de esas vueltas SIRVIERON
//!    6  la caja se llama LATIDO o    por donde da el turno el bucle. Si el
//!       PULSO, y no es adorno        kernel no da el latido hay que SABERLO
//! ```
//!
//! *** La sexta es la que impide que este fichero cometa su tercera mentira. El
//! bucle pide el latido al arrancar y, si el kernel dice que no, sigue girando
//! **exactamente igual de bien**. Sin ese nombre en la caja, las dos formas se
//! verian identicas -- y eso es la definicion del `[riesgo] SILENCIO` que este
//! carril declara arriba. Un modo de reserva que no se anuncia es un modo de
//! reserva que se queda puesto para siempre.
//!
//! ** La quinta se anadio al preguntar el dueno si quedaba algo que exprimir,
//! y la contesta ella sola. El techo UTIL de ese bucle son **250 vueltas por
//! segundo** --lo pone el bus USB, que late cada 4 ms-- asi que la distancia
//! entre `pulso` y `pinta` no es una curiosidad: es el desperdicio, medido.
//! Ver el presupuesto escrito en `main.rs`, junto al `yield_screen`.

use crate::desktop;

/// **Lo que este medidor recibe en una vuelta.** Va junta y no como cuatro
/// parametros sueltos: son **una sola lectura** --el mismo instante del mismo
/// segundo-- y repartirla en la firma invita a pintar la mitad de un segundo
/// con la mitad de otro.
pub(crate) struct Lectura {
    /// Vueltas del bucle en el ultimo segundo cerrado.
    pub vueltas: u32,
    /// El kernel no publica reloj de referencia: `vueltas` no significa nada.
    pub sin_reloj: bool,
    /// De ese segundo, ms dentro de la vuelta. Ver `desktop::Tick::cuerpo_ms`.
    pub cuerpo_ms: u32,
    /// De ese segundo, ms esperando el turno.
    pub puerta_ms: u32,
    /// De ese segundo, cuantas vueltas PINTARON algo.
    pub pinta: u32,
    /// El bucle va montado en el LATIDO del hardware (`WAIT`), no girando.
    pub en_latido: bool,
}

/// **Lo que hay que ensenar, ya decidido.** El carril verde no vuelve a
/// preguntarse nada: recibe esto y lo pone en pantalla.
///
/// ** Que exista este tipo ES el corte. Mientras la decision y el pixel vivian
/// en la misma funcion, un cambio de SITIO y un cambio de SIGNIFICADO se leian
/// igual en el diff -- y las dos mentiras de arriba entraron por ahi.
pub(crate) struct Dictamen {
    /// El paso de la aguja que toca pintar.
    pub aguja: u8,
    /// El ritmo, o `None` cuando no hay reloj con que medirlo.
    pub ritmo: Option<u32>,
    /// El ritmo es tan bajo que hay que mirarlo. Ver la decision 3.
    pub alarma: bool,
    pub cuerpo_ms: u32,
    pub puerta_ms: u32,
    /// Vueltas que pintaron. Ver la decision 5.
    pub pinta: u32,
    /// Va montado en el latido. Ver la decision 6.
    pub en_latido: bool,
    /// El cuerpo se queda el segundo. Ver la decision 4.
    pub manda_cuerpo: bool,
}

/// **La aguja.** Avanza en CADA cuarto de segundo que este modulo recibe.
///
/// == *** POR QUE NO BASTABA EL NUMERO (2026-09-08) =========================
///
/// La primera version pintaba solo `loops_per_second`, y el dueno lo probo en
/// el Ryzen y trajo esto:
///
/// > *"veo pulso una sola vez y se congela"*
///
/// ** Y NO ERA LA MAQUINA: era el instrumento. `loops_per_second` se calcula
/// **una vez por segundo** --asi esta escrito en `Tick::pulse`-- asi que entre
/// dos calculos el numero es CONSTANTE. Correcto, y absolutamente inutil:
///
/// ```text
///    el bucle VIVO y el numero quieto      se ve igual
///    el bucle MUERTO                       se ve igual
/// ```
///
/// *** Un medidor cuyo estado sano se ve identico a su estado roto no mide: es
/// un adorno con cifras. Y encima costo una vuelta al metal descubrirlo.
static mut AGUJA: u8 = 0;

/// Los cuatro pasos de la aguja. Se eligen ASCII porque las fuentes de esta
/// casa lo son (ver `docs/identidad`), y porque los cuatro se distinguen de un
/// vistazo a la distancia a la que se mira una barra de tareas. El cuarto va
/// por su codigo ASCII --92 es la barra invertida-- porque escaparla dentro de
/// un literal es justo el tipo de detalle que se rompe al copiar el fichero.
const PASOS: [u8; 4] = [b'|', b'/', b'-', 92];

/// Por debajo de esto, el ritmo deja de ser un detalle de rendimiento.
///
/// ** No es un umbral de gusto. Este bucle no tiene freno ninguno: acaba en
/// `yield_screen()` y vuelve. Dos cifras por segundo no es "va justo", es
/// **alguien quedandose el turno** -- que es literalmente lo que aparecio el
/// 08-09 con un 50. Ver `desktop::Tick::cuerpo_ms`.
const RITMO_BAJO: u32 = 100;

/// **Leer, que aqui es decidir.** Avanza la aguja y contesta que hay que
/// ensenar. No toca la pantalla: ese es el otro carril.
pub(crate) fn leer(l: &Lectura) -> Dictamen {
    // ** LA AGUJA AVANZA SIEMPRE, y por eso el modulo entero repinta SIEMPRE
    // que le llega un cuarto. Es lo contrario de lo que hace el testigo --que
    // se calla si no cambio nada-- y es a proposito: aqui lo que se ensena no
    // es el valor, es que **haya latido**.
    let aguja = unsafe {
        AGUJA = AGUJA.wrapping_add(1);
        PASOS[(AGUJA as usize) % PASOS.len()]
    };
    Dictamen {
        aguja,
        // ** SIN RELOJ NO HAY NUMERO, y esa es toda la regla. El numero no
        // existe --nadie lo calculo: `Tick::pulse` sale por su rama de
        // emergencia antes-- asi que darlo seria inventarlo. `None` y no un
        // cero: un cero es una medida, y esa medida no se tomo.
        ritmo: if l.sin_reloj { None } else { Some(l.vueltas) },
        alarma: !l.sin_reloj && l.vueltas < RITMO_BAJO,
        cuerpo_ms: l.cuerpo_ms,
        puerta_ms: l.puerta_ms,
        pinta: l.pinta,
        en_latido: l.en_latido,
        // ** DE QUE LADO TIRAR. Las dos mitades suenan igual desde fuera --"el
        // escritorio va lento"-- y no se arreglan en el mismo sitio: una es de
        // Ring 3 y la otra del planificador. El empate cae del lado del cuerpo
        // porque es el unico de los dos que este proceso puede arreglar solo.
        manda_cuerpo: l.cuerpo_ms >= l.puerta_ms,
    }
}

/// La lectura de este cuarto, sacada del reloj del escritorio.
///
/// Vive aqui y no en quien pinta porque es **traduccion, no dibujo**: es el
/// unico sitio que sabe que `Tick` y `Lectura` hablan del mismo segundo. Antes
/// esa traduccion se escribia en `paint.rs`, o sea en el fichero que menos
/// tiene que saber de las dos cosas.
pub(crate) fn de(t: &desktop::Tick) -> Lectura {
    Lectura {
        vueltas: t.loops_per_second,
        sin_reloj: t.sin_reloj(),
        cuerpo_ms: t.cuerpo_ms,
        puerta_ms: t.puerta_ms,
        pinta: t.pintados_por_segundo,
        en_latido: t.en_latido(),
    }
}
