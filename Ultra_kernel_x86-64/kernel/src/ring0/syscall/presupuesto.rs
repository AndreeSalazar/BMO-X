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
    techo: 895,
    meta: 400,
    porque: "150 de cruce irreducible + 60 de prologo/epilogo + 190 de dispatch",
};

/// **La mitad de Rust**: lo que tarda `dispatch` por dentro, que es lo unico
/// que el metro sabe medir solo.
///
/// [!] La meta es 190 y no 311 porque **el 311 medido incluye el propio metro**:
/// `meter::start`/`stop` son dos `rdtsc` y `coste.bex` midio que uno cuesta 69,
/// no los ~25 que `meter.rs` estimo. Entre 70 y 140 de esos 311 son la regla,
/// no lo medido. La meta esta puesta contra el `dispatch` de verdad.
pub const DISPATCH: Fila = Fila {
    techo: 320,
    meta: 190,
    porque: "el 311 medido lleva dentro 70-140 del propio metro",
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
///              total    dispatch   stub
///    antes      +83       +76       +7     correcto
///    pieza 1   +342       +85     +257     <- aparece
///    pieza 2   +327       +84     +243     <- sigue
/// ```
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
    techo: 327,
    meta: 80,
    porque: "84 en dispatch es correcto; los 243 del stub son la anomalia viva",
};
