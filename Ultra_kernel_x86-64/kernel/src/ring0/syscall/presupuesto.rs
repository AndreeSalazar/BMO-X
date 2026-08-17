//! `syscall::presupuesto` -- **lo que una puerta TIENE PERMITIDO costar.**
//!
//! # Por que existe
//!
//! El 2026-08-16 una puerta paso de 2663 a ~1050 ciclos en tres piezas, y cada
//! una se justifico con una medida. Pero **nada en el arbol impide que la
//! cuarta la devuelva a 2000**: el metro sabe decir cuanto cuesta hoy y no sabe
//! decir cuanto DEBERIA costar. Un numero sin contrato es una anecdota.
//!
//! Este fichero convierte *"optimizar"* en *"no incumplir"*, que es la misma
//! forma que ya tiene el censo de extensiones: se **declara** lo que se espera,
//! y lo que grita es la diferencia entre lo declarado y lo real.
//!
//! ** Y no es una idea nueva: en motores de tiempo real y de juego se llama
//! presupuesto de ciclos y es practica normal. Lo raro era no tenerlo.
//!
//! # DOS numeros por fila, y son dos contratos distintos
//!
//! ```text
//!    techo   lo que NO puede empeorar.  Es un trinquete contra regresiones.
//!    meta    a donde tiene que llegar.  Es la deuda, escrita.
//! ```
//!
//! El `techo` sale de la **ultima medida confirmada en metal**, no de un deseo:
//! si algo lo cruza, alguien acaba de meter trabajo en el camino y hay que
//! saberlo el mismo dia, no tres meses despues. La `meta` sale del analisis del
//! suelo fisico de esta maquina, y hasta que se alcance la fila dice, sin
//! adornos, que el trabajo no esta terminado.
//!
//! ** UNA FILA QUE CUMPLE EL TECHO Y NO LA META NO ESTA BIEN: esta en plazo.
//!
//! # De donde sale la meta, que es la parte que hay que poder discutir
//!
//! ```text
//!    cruce (`syscall` + `sysretq`)   ~150   IRREDUCIBLE
//!    prologo + epilogo                ~60
//!    dispatch (el Rust)              ~190
//!    ------------------------------------
//!    puerta pelada                    400
//! ```
//!
//! Los ~150 del cruce no son un objetivo: son el suelo. Salen de la calibracion
//! que dio `c/coste.bex` --un `rdtsc` mide **69 ciclos**, y `syscall`/`sysret`
//! son de la misma familia microcodificada pero hacen mas-- y coinciden con lo
//! que Liedtke consiguio con L4 en los 90 (~250 ciclos en un 486). El coste de
//! cruzar un anillo de privilegio es lo unico de esta cuenta que no ha bajado
//! en treinta anos.
//!
//! # Como se lee, y por que NO se comprueba en el arranque
//!
//! El censo de extensiones grita en CABINA al arrancar porque su verdad ya
//! existe entonces. La de este fichero **no**: al arrancar no se ha servido ni
//! una puerta y el metro esta vacio. Un presupuesto solo se puede juzgar contra
//! trafico real.
//!
//! Por eso lo lee `c/coste.bex` desde Ring 3, que es donde ya vive la medida --
//! y ademas es el unico sitio al que Eddi puede llegar desde el escritorio.
//!
//! [!] Estos numeros viajan a Ring 3 EMPAQUETADOS, `meta << 32 | techo`, por la
//! misma razon que `INFO_CPU_EXT_AVERIAS` empaqueta cuatro: son datos de la
//! misma fila y separarlos en dos campos permitiria leer uno y no el otro, que
//! es justo el error que hace decir *"cumple"* a algo que no llego a la meta.

/// # EL MARGEN DEL TRINQUETE, y por que no es cero
///
/// La primera version puso cada `techo` clavado en la ultima medida. La tanda
/// siguiente, **con el kernel byte a byte identico**, dio 915 donde la anterior
/// dio 895 y el presupuesto grito `SE PASA -- REGRESION`.
///
/// No habia regresion: hay ruido. El minimo de 16 bloques sigue dependiendo del
/// estado de la maquina --cache, historia del arranque, que mas hubiera listo--
/// y entre tandas se mueve un ~2%.
///
/// ** UN TRINQUETE MAS APRETADO QUE EL RUIDO NO ES ESTRICTO: ES UNA ALARMA
/// ALEATORIA. Y una alarma que salta sola se acaba ignorando, que es peor que
/// no tenerla.
///
/// Asi que el techo se pone en **la peor medida observada mas un 5%**: por
/// encima del ruido medido (2,2%) y muy por debajo de lo que mueve una pieza de
/// verdad (las de hoy movieron del 30% al 60%). Una regresion real sigue
/// gritando; el ruido, no.
pub const MARGEN_DE_RUIDO_POR_CIENTO: u64 = 5;

/// Una fila del presupuesto: lo que no puede empeorar y a donde tiene que ir.
pub struct Fila {
    /// Ultima medida confirmada en metal. Cruzarlo es una REGRESION.
    pub techo: u32,
    /// El objetivo que sale del analisis. No alcanzarlo es DEUDA, no fallo.
    pub meta: u32,
    /// Por que la meta es esa. Vive aqui para que cambiarla obligue a
    /// reescribir el motivo, no solo la cifra.
    pub porque: &'static str,
}

impl Fila {
    /// `meta << 32 | techo`, que es como cruza a Ring 3.
    pub const fn empaquetado(&self) -> u64 {
        ((self.meta as u64) << 32) | self.techo as u64
    }
}

/// **La puerta pelada**: `INVOKE` de `BMO_OP_PID` sobre la tarea actual, medida
/// desde Ring 3. Es el suelo del sistema: no resuelve ningun handle, asi que
/// nada puede costar menos que esto.
///
/// Un trinquete se aprieta con lo que YA se consiguio, nunca con lo que se
/// cree que se va a conseguir. Historia de este techo:
///
/// ```text
///    2618   antes de todo
///    1625   pieza 1 (el XSAVE que no tenia por que existir)
///     895   pieza 2 (`sysretq`) + los cuatro sellos fuera   <- HOY, 242 ns
/// ```
///
/// Se aprieta DESPUES de cada tanda que el metal confirma, no antes: cuando
/// aqui ponia 1625, la pieza 2 todavia era una estimacion mia de ~1050 y salio
/// en 895. Si hubiera puesto 1050 y la pieza saliera en 1100, el trinquete
/// habria gritado por una mejora.
pub const PUERTA_PELADA: Fila = Fila {
    // 915 fue la peor de las tres tandas, +5% de margen de ruido.
    techo: 960,
    // ** LA META BAJA DE 400 A 300, y no por optimismo: por medida. Se puso 400
    // contando 190 para `dispatch`, y `dispatch` resulto ser ~90. La cuenta
    // buena es 150 de cruce + 60 de prologo/epilogo + 90 de Rust.
    meta: 300,
    porque: "150 de cruce irreducible + 60 de prologo/epilogo + 90 de dispatch",
};

/// **La mitad de Rust**: lo que tarda `dispatch` por dentro, que es lo unico
/// que el metro sabe medir solo.
///
/// ** ESTA FILA SE REESCRIBIO ENTERA CUANDO LA VENTANA SE LIMPIO.
///
/// Decia `techo 320, meta 190` porque las cuatro primeras tandas midieron
/// **309-319**. Ese numero era falso: la ventana llevaba dentro un `printf`, y
/// una puerta de consola cuesta ~2,1 M ciclos. Con la ventana cerrada antes de
/// imprimir, las dos implementaciones dan **84 (C) y 99 (Rust)**.
///
/// O sea que **la mitad Rust de una puerta nunca fue el 12%: es el 10%, y son
/// ~90 ciclos.** El 309 era un `printf` disfrazado de dispatcher.
///
/// [!] Y de esos ~90 una parte grande es el PROPIO METRO: `meter::start`/`stop`
/// son dos `rdtsc`, y uno cuesta 69 ciclos medidos. La meta de 60 esta puesta
/// contra eso -- alcanzarla pasa por sacar el metro de `dispatch`, no por
/// afinar el `match` de dos brazos.
pub const DISPATCH: Fila = Fila {
    // 99 fue la peor de las dos implementaciones, +5% redondeado hacia arriba.
    techo: 105,
    meta: 60,
    porque: "de los ~90 medidos, buena parte son los dos rdtsc del propio metro",
};

/// **Lo que cuesta resolver una capability**: la fila 4 menos la fila 3.
///
/// ** ESTA FILA EXISTE POR UNA ANOMALIA, y es el mejor argumento de todo el
/// fichero. Resolver un handle costaba 83 ciclos, de los que 76 caian dentro de
/// `dispatch` y 7 en el stub -- ruido, y correcto: **el stub no sabe que
/// operacion se pidio**. Con la pieza 1 puesta el mismo hueco salio en 342, con
/// **257 de ellos en el stub**, que es un sitio donde no pueden estar.
///
/// Nadie lo habria visto si no se hubieran comparado las dos tandas a mano. Un
/// trinquete lo habria gritado solo, y por eso esta fila se declara aunque su
/// techo sea, hoy, un numero que no me gusta.
///
/// ** Y LA ANOMALIA SOBREVIVIO A LAS DOS PIEZAS, o sea que es REAL y no era el
/// instrumento:
///
/// ```text
///                    total    dispatch   stub
///    antes            +83       +76       +7     correcto
///    pieza 1         +342       +85     +257     <- aparece
///    pieza 2         +327       +84     +243     <- sigue
///    ventana limpia  +338       +92     +246     <- y sigue
/// ```
///
/// La cuarta fila es la que la confirma del todo: se midio con la ventana ya
/// cerrada antes de imprimir, o sea sin la contaminacion que hundio todo lo
/// demas de este fichero. **La mitad de `dispatch` es correcta en las cuatro**
/// --~85, la capability, donde tiene que estar-- y los ~246 del stub siguen
/// exactamente igual.
///
/// La mitad de `dispatch` se comporta perfecto en las tres tandas: ~85, que es
/// la capability y esta donde tiene que estar. Lo que no puede existir son los
/// **243 en el stub**, porque el stub no sabe que operacion se pidio.
///
/// [!] No hay explicacion, y despues de fallar dos veces razonando sobre este
/// camino no se va a poner una tercera hipotesis por escrito. Lo que la
/// resuelve es UNA sonda concreta: una fila mas en `c/coste.bex` que use un
/// handle REAL con la operacion mas barata que exista. Si esa fila tambien
/// carga los 243, es el camino del handle; si no, es `BMO_ARCH_TAMANO`.
pub const HANDLE: Fila = Fila {
    // 338 fue la peor observada, +5% de margen de ruido.
    techo: 355,
    meta: 80,
    porque: "~90 en dispatch es correcto; los ~246 del stub son la anomalia viva",
};
