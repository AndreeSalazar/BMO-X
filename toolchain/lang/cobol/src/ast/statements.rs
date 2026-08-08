use crate::ast::{SyscallDef, Valor88};

/// Como se resuelve el ultimo digito de una operacion aritmetica.
///
/// **En un banco esto es una decision legal**, no un detalle de formato: medio
/// centimo repetido cuatro millones de veces es dinero, y hay jurisdicciones
/// que obligan al redondeo del banquero precisamente porque el clasico tiene
/// sesgo. Por eso van todos los modos del estandar con su nombre, y no "el
/// redondeo" a secas.
///
/// Es un tipo de COBOL --lo dice la clausula `ROUNDED`-- que se traduce al
/// `bmo_lower::redondeo::Modo` en el codegen. Los dos existen a proposito: uno
/// es la palabra del lenguaje y el otro la aritmetica compartida, y el dia que
/// Ada pida sus modos del Annex F no tendra que hablar de COBOL.
pub type Redondeo = bmo_lower::redondeo::Modo;

/// Un control de `PERFORM VARYING`: la variable, por donde empieza, cuanto
/// suma y cuando para.
///
/// [!] La condicion dice cuando **PARAR**, no cuando seguir. `UNTIL I > 12`
/// recorre del 1 al 12, no al 13. Es al reves que el `while` de casi todo lo
/// demas, y confundirlo da una vuelta de mas o de menos -- que sobre una tabla
/// es un subindice fuera de rango.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlBucle {
    pub variable: String,
    pub desde: String,
    pub paso: String,
    pub hasta_que: Condicion,
}

/// Las clausulas que acompanan a una operacion aritmetica.
///
/// * Las dos son **de banca**, no de sintaxis:
///
/// - `ROUNDED` decide el ultimo digito, y eso es una decision legal.
/// - `ON SIZE ERROR` decide **que pasa cuando el resultado no cabe**. Sin el,
///   un importe que se sale de su PICTURE se guarda truncado por arriba y el
///   programa sigue con un numero que no es. Con el, el campo **no se toca** y
///   el programa decide -- que es lo que separa un batch que se para diciendo
///   "este movimiento no cabe" de uno que cuadra mal y nadie sabe por que.
#[derive(Debug, Clone, PartialEq)]
pub struct Aritmetica {
    pub redondeo: Redondeo,
    /// `ON SIZE ERROR <stmts>`. `None` = no se declaro, y entonces el
    /// desbordamiento se guarda truncado, que es lo que dice el estandar.
    pub si_desborda: Option<Vec<CobolStatement>>,
    /// `NOT ON SIZE ERROR <stmts>` -- lo que se hace cuando SI cupo.
    pub si_cabe: Option<Vec<CobolStatement>>,
}

impl Default for Aritmetica {
    /// Sin clausulas: **truncar y no mirar si cabe**, que es lo que dice el
    /// estandar cuando no se escribe ninguna.
    fn default() -> Self {
        Aritmetica { redondeo: Redondeo::Truncar, si_desborda: None, si_cabe: None }
    }
}

impl Aritmetica {
    pub fn con(redondeo: Redondeo) -> Self {
        Aritmetica { redondeo, ..Default::default() }
    }
}

/// Que se imprime en un `DISPLAY`.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayArg {
    /// Entre comillas: sale tal cual.
    Literal(String),
    /// Un nombre de la DATA DIVISION: se formatea en EJECUCION con la escala
    /// de su PIC, porque el valor no se conoce al compilar.
    Variable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CobolStatement {
    /// `DISPLAY "texto"` o `DISPLAY VARIABLE`.
    ///
    /// Eran lo mismo --una `String` que siempre se imprimia literal-- y por eso
    /// el programa de ejemplo CALCULA 59.97 y luego imprime la cadena
    /// "total exacto: 59.97" escrita a mano. La aritmetica era de verdad; lo
    /// que se veia, no. Un `DISPLAY` que no sabe ensenar lo que acaba de
    /// calcular deja al lenguaje sin salida.
    Display(DisplayArg),
    Accept(String),
    Move(String, String),
    // -- Las cinco aritmeticas, cada una con su REDONDEO --
    //
    // La clausula `ROUNDED` viaja en la sentencia y no en el dato porque es de
    // la OPERACION: el mismo campo se redondea en una linea y se trunca en la
    // de abajo, y eso es deliberado -- el interes se redondea y el desglose de
    // un asiento se trunca para que la suma cuadre con el total.
    Add(String, String, Aritmetica),
    Subtract(String, String, Aritmetica),
    Multiply(String, String, Aritmetica),
    Divide(String, String, Aritmetica),
    Compute(String, String, Aritmetica),
    /// `IF <cond> ... [ELSE ...] END-IF`. Ver [`Condicion`] para como se
    /// combinan con `AND` y `OR`.
    If(Condicion, Vec<CobolStatement>, Vec<CobolStatement>),
    /// `PERFORM <n> TIMES ... END-PERFORM` -- el cuerpo va en el AST, no
    /// como una cuenta suelta: sin cuerpo no hay nada que repetir.
    PerformTimes(u32, Vec<CobolStatement>),
    /// * `PERFORM <parrafo> [THRU <parrafo>] [<n> TIMES | UNTIL <cond>]`.
    ///
    /// El **PERFORM fuera de linea**, que es como se escribe COBOL de verdad:
    /// el programa principal es una lista de `PERFORM` y el trabajo vive en
    /// parrafos con nombre. Un batch bancario entero cabe en cinco lineas
    /// legibles y luego se lee cada paso por separado.
    PerformFuera {
        desde: String,
        /// `THRU <otro>` -- ejecuta desde uno hasta otro, los dos incluidos.
        hasta: Option<String>,
        /// `<n> TIMES`.
        veces: Option<u32>,
        /// `UNTIL <cond>` -- se prueba ANTES de cada vuelta.
        hasta_que: Option<Condicion>,
    },
    /// * `EVALUATE ... WHEN ... END-EVALUATE` -- el `switch` de COBOL.
    ///
    /// Cada rama lleva la condicion **ya construida**: la forma con sujeto
    /// (`EVALUATE TIPO / WHEN 1`) se traduce a `TIPO = 1` en el parser, porque
    /// el sujeto se conoce ahi. `None` es el `WHEN OTHER`, que no compara nada.
    ///
    /// Que las dos formas --con sujeto y `EVALUATE TRUE`-- acaben en el mismo
    /// `Condicion` no es una casualidad de implementacion: **son la misma cosa**
    /// dicha de dos maneras, y por eso las dos heredan el cortocircuito y la
    /// precedencia sin una linea de mas en el codegen.
    Evaluate(Vec<(Option<Condicion>, Vec<CobolStatement>)>),
    /// `INSPECT <campo> TALLYING <n> FOR ALL "<c>"`.
    ///
    /// Contar apariciones de un caracter. En banca lo mas corriente es contar
    /// espacios para saber cuanto mide de verdad un campo que viene rellenado.
    InspectContar { campo: String, contador: String, buscado: char },
    /// `INSPECT <campo> REPLACING {ALL|LEADING} "<a>" BY "<b>"`.
    ///
    /// * `ALL` y `LEADING` no son lo mismo y la diferencia cambia un numero:
    /// sobre `"  12 34"`, `LEADING " " BY "0"` da `"0012 34"` y `ALL` daria
    /// `"0012034"`. Por eso son dos formas y no una con una opcion.
    InspectReemplazar { campo: String, viejo: char, nuevo: char, solo_delante: bool },
    /// `STRING <fuentes> DELIMITED BY SIZE INTO <destino>`.
    ///
    /// Las fuentes son literales o campos, y se pegan **en orden** hasta llenar
    /// el destino.
    StringInto { fuentes: Vec<String>, destino: String },
    /// * `PERFORM VARYING <v> FROM <x> BY <y> UNTIL <cond> ... END-PERFORM`.
    ///
    /// El bucle CON INDICE, que es como se recorre una tabla. Con `AFTER`
    /// encadenado se recorren varias dimensiones:
    ///
    /// ```text
    ///   PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3
    ///           AFTER   J FROM 1 BY 1 UNTIL J > 4
    /// ```
    ///
    /// * Los controles van en UNA LISTA y no anidados en el AST porque la
    /// diferencia entre `VARYING` y `AFTER` es solo la posicion: el segundo se
    /// **reinicia** cada vez que el primero avanza, y eso lo da el orden.
    ///
    /// El `PERFORM <parrafo> VARYING ...` no necesita otra variante: el parser lo
    /// deja como un `PerformVarying` cuyo cuerpo es un `PERFORM` del parrafo.
    PerformVarying { controles: Vec<ControlBucle>, cuerpo: Vec<CobolStatement> },
    /// `GO TO <parrafo>` -- un salto, sin vuelta.
    ///
    /// Es la otra mitad de `PERFORM ... THRU X-SALIR`: el descarte. Sin el, saltar
    /// al parrafo de salida hay que escribirlo con un interruptor y un `IF`, que
    /// dice lo mismo con tres lineas mas y una variable que alguien tendra que
    /// entender dentro de diez anos.
    ///
    /// * **No confundir con `PERFORM`**: `PERFORM X` ejecuta X y VUELVE;
    /// `GO TO X` se va y no vuelve. Fingir uno con el otro es lo que hacia el
    /// ejemplo del nivel 8 antes de que esto existiera, y estaba dicho ahi
    /// mismo porque el trabajo de debajo se hacia igual.
    GoTo(String),
    /// `EXIT` -- no hace nada, y ese es su trabajo.
    ///
    /// Es el destino de un `PERFORM ... THRU X-SALIR`: un parrafo vacio al que
    /// saltar cuando hay que salir antes de tiempo. Emitir "nada" es correcto;
    /// rechazarlo obligaria a inventar una sentencia de mentira.
    Exit,
    /// `PERFORM UNTIL <cond> ... END-PERFORM`. Prueba ANTES de cada
    /// iteracion (`WITH TEST BEFORE`, el default del estandar).
    PerformUntil(Condicion, Vec<CobolStatement>),
    /// `OPEN INPUT|OUTPUT <fichero>`. El modo decide si se abre para leer o
    /// se CREA para escribir, y son dos puertas distintas del kernel.
    Open(String, String),
    /// `CLOSE <fichero>`. En un fichero de salida **es donde el contenido
    /// llega al disco**: sin esto no se guarda nada.
    Close(String),
    /// `READ <fichero> AT END <stmts> [NOT AT END <stmts>] END-READ`.
    ///
    /// `AT END` no es un adorno de sintaxis: es la UNICA forma de que un
    /// `PERFORM UNTIL` sobre un fichero termine. Un `READ` que no lo lleva
    /// compilaba antes a un error explicito, y ahora compila a un bucle que no
    /// para -- asi que el parser lo exige.
    Read(String, Vec<CobolStatement>, Vec<CobolStatement>),
    /// `WRITE <registro>`. Escribe el valor del registro como una linea.
    Write(String),
    StopRun,
    Syscall(SyscallDef, Vec<String>),
    Expr(String),
}

/// Una condicion COMPUESTA: comparaciones unidas con `AND` y `OR`.
///
/// Era una `Vec<CobolCondition>` conjugada siempre con AND, y el `OR` se
/// rechazaba con un error explicito. Eso bloqueaba tres cosas de golpe: un `88`
/// con `THRU`, un `88` con varios valores, y el `WHEN a, b, c` de `EVALUATE`.
///
/// Es un ARBOL y no una lista porque `A OR B AND C` no significa lo mismo que
/// `(A OR B) AND C`: **`AND` liga mas fuerte que `OR`**, como en el estandar y
/// como en cualquier lenguaje. Una lista plana no puede representar esa
/// diferencia, y elegir mal cambia a que rama va el programa sin que nada
/// avise.
#[derive(Debug, Clone, PartialEq)]
pub enum Condicion {
    Simple(CobolCondition),
    /// Las dos. Se evalua en **cortocircuito**: si la primera falla, la segunda
    /// ni se calcula.
    Y(Box<Condicion>, Box<Condicion>),
    /// Cualquiera de las dos, tambien en cortocircuito.
    O(Box<Condicion>, Box<Condicion>),
}

impl Condicion {
    /// Une con `AND`, que es lo que hace un `Vec` de comparaciones.
    pub fn y(izq: Condicion, der: Condicion) -> Condicion {
        Condicion::Y(Box::new(izq), Box::new(der))
    }

    pub fn o(izq: Condicion, der: Condicion) -> Condicion {
        Condicion::O(Box::new(izq), Box::new(der))
    }

    /// * Un conjunto de valores comparado contra UN campo, convertido en la
    /// condicion que de verdad es.
    ///
    /// ```text
    ///   VALUE 1.          ->  X = 1
    ///   VALUE 1 THRU 5.   ->  X >= 1 AND X <= 5
    ///   VALUE 6, 7.       ->  X = 6 OR X = 7
    /// ```
    ///
    /// Vive aqui y no en el codegen porque **la usan dos sitios que no se
    /// conocen**: los nombres de condicion del nivel 88 y el `WHEN` de un
    /// `EVALUATE` con sujeto. Son la misma pregunta --"esta este campo en este
    /// conjunto?"-- y tenerla dos veces seria copiar el mismo error de extremo
    /// abierto en dos gramaticas distintas.
    ///
    /// Un rango lleva los dos extremos INCLUIDOS, que es lo que dice el
    /// estandar y lo que espera quien escribe `1 THRU 5` pensando en cinco.
    pub fn de_valores(campo: &str, valores: &[Valor88]) -> Option<Condicion> {
        let mut acc: Option<Condicion> = None;
        for v in valores {
            let c = match v {
                Valor88::Uno(x) => {
                    Condicion::Simple(CobolCondition::Equal(campo.to_string(), x.clone()))
                }
                Valor88::Rango(desde, hasta) => Condicion::y(
                    Condicion::Simple(CobolCondition::GreaterOrEqual(
                        campo.to_string(),
                        desde.clone(),
                    )),
                    Condicion::Simple(CobolCondition::LessOrEqual(campo.to_string(), hasta.clone())),
                ),
            };
            acc = Some(match acc {
                None => c,
                Some(izq) => Condicion::o(izq, c),
            });
        }
        acc
    }
}

/// Una comparacion simple: el operando de la izquierda, el de la derecha, y que
/// se pregunta.
///
/// Cada operando es un nombre de dato o un literal; el codegen lo resuelve
/// mirando si esta declarado en la DATA DIVISION.
#[derive(Debug, Clone, PartialEq)]
pub enum CobolCondition {
    /// Un NOMBRE DE CONDICION a secas (`IF FIN-DE-FICHERO`), declarado con un
    /// nivel 88. El parser no puede resolverlo --no conoce los datos--, asi que
    /// lo pasa por nombre y lo expande el codegen, que si sabe de quien es y
    /// puede decirlo cuando no existe.
    Nombre(String),
    Equal(String, String),
    NotEqual(String, String),
    Greater(String, String),
    Less(String, String),
    GreaterOrEqual(String, String),
    LessOrEqual(String, String),
}
