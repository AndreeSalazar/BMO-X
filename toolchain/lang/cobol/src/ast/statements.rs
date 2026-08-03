use crate::ast::{SyscallDef, Valor88};

/// Cómo se resuelve el último dígito de una operación aritmética.
///
/// **En un banco esto es una decisión legal**, no un detalle de formato: medio
/// céntimo repetido cuatro millones de veces es dinero, y hay jurisdicciones
/// que obligan al redondeo del banquero precisamente porque el clásico tiene
/// sesgo. Por eso van todos los modos del estándar con su nombre, y no "el
/// redondeo" a secas.
///
/// Es un tipo de COBOL —lo dice la cláusula `ROUNDED`— que se traduce al
/// `bmo_lower::redondeo::Modo` en el codegen. Los dos existen a propósito: uno
/// es la palabra del lenguaje y el otro la aritmética compartida, y el día que
/// Ada pida sus modos del Annex F no tendrá que hablar de COBOL.
pub type Redondeo = bmo_lower::redondeo::Modo;

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
    /// Eran lo mismo —una `String` que siempre se imprimia literal— y por eso
    /// el programa de ejemplo CALCULA 59.97 y luego imprime la cadena
    /// "total exacto: 59.97" escrita a mano. La aritmetica era de verdad; lo
    /// que se veia, no. Un `DISPLAY` que no sabe ensenar lo que acaba de
    /// calcular deja al lenguaje sin salida.
    Display(DisplayArg),
    Accept(String),
    Move(String, String),
    // ── Las cinco aritméticas, cada una con su REDONDEO ──
    //
    // La cláusula `ROUNDED` viaja en la sentencia y no en el dato porque es de
    // la OPERACIÓN: el mismo campo se redondea en una línea y se trunca en la
    // de abajo, y eso es deliberado — el interés se redondea y el desglose de
    // un asiento se trunca para que la suma cuadre con el total.
    Add(String, String, Redondeo),
    Subtract(String, String, Redondeo),
    Multiply(String, String, Redondeo),
    Divide(String, String, Redondeo),
    Compute(String, String, Redondeo),
    /// `IF <cond> ... [ELSE ...] END-IF`. Ver [`Condicion`] para cómo se
    /// combinan con `AND` y `OR`.
    If(Condicion, Vec<CobolStatement>, Vec<CobolStatement>),
    /// `PERFORM <n> TIMES ... END-PERFORM` — el cuerpo va en el AST, no
    /// como una cuenta suelta: sin cuerpo no hay nada que repetir.
    PerformTimes(u32, Vec<CobolStatement>),
    /// ★ `PERFORM <párrafo> [THRU <párrafo>] [<n> TIMES | UNTIL <cond>]`.
    ///
    /// El **PERFORM fuera de línea**, que es como se escribe COBOL de verdad:
    /// el programa principal es una lista de `PERFORM` y el trabajo vive en
    /// párrafos con nombre. Un batch bancario entero cabe en cinco líneas
    /// legibles y luego se lee cada paso por separado.
    PerformFuera {
        desde: String,
        /// `THRU <otro>` — ejecuta desde uno hasta otro, los dos incluidos.
        hasta: Option<String>,
        /// `<n> TIMES`.
        veces: Option<u32>,
        /// `UNTIL <cond>` — se prueba ANTES de cada vuelta.
        hasta_que: Option<Condicion>,
    },
    /// ★ `EVALUATE … WHEN … END-EVALUATE` — el `switch` de COBOL.
    ///
    /// Cada rama lleva la condición **ya construida**: la forma con sujeto
    /// (`EVALUATE TIPO / WHEN 1`) se traduce a `TIPO = 1` en el parser, porque
    /// el sujeto se conoce ahí. `None` es el `WHEN OTHER`, que no compara nada.
    ///
    /// Que las dos formas —con sujeto y `EVALUATE TRUE`— acaben en el mismo
    /// `Condicion` no es una casualidad de implementación: **son la misma cosa**
    /// dicha de dos maneras, y por eso las dos heredan el cortocircuito y la
    /// precedencia sin una línea de más en el codegen.
    Evaluate(Vec<(Option<Condicion>, Vec<CobolStatement>)>),
    /// `EXIT` — no hace nada, y ese es su trabajo.
    ///
    /// Es el destino de un `PERFORM … THRU X-SALIR`: un párrafo vacío al que
    /// saltar cuando hay que salir antes de tiempo. Emitir "nada" es correcto;
    /// rechazarlo obligaría a inventar una sentencia de mentira.
    Exit,
    /// `PERFORM UNTIL <cond> ... END-PERFORM`. Prueba ANTES de cada
    /// iteración (`WITH TEST BEFORE`, el default del estándar).
    PerformUntil(Condicion, Vec<CobolStatement>),
    /// `OPEN INPUT|OUTPUT <fichero>`. El modo decide si se abre para leer o
    /// se CREA para escribir, y son dos puertas distintas del kernel.
    Open(String, String),
    /// `CLOSE <fichero>`. En un fichero de salida **es donde el contenido
    /// llega al disco**: sin esto no se guarda nada.
    Close(String),
    /// `READ <fichero> AT END <stmts> [NOT AT END <stmts>] END-READ`.
    ///
    /// `AT END` no es un adorno de sintaxis: es la ÚNICA forma de que un
    /// `PERFORM UNTIL` sobre un fichero termine. Un `READ` que no lo lleva
    /// compilaba antes a un error explícito, y ahora compila a un bucle que no
    /// para — así que el parser lo exige.
    Read(String, Vec<CobolStatement>, Vec<CobolStatement>),
    /// `WRITE <registro>`. Escribe el valor del registro como una línea.
    Write(String),
    StopRun,
    Syscall(SyscallDef, Vec<String>),
    Expr(String),
}

/// Una condición COMPUESTA: comparaciones unidas con `AND` y `OR`.
///
/// Era una `Vec<CobolCondition>` conjugada siempre con AND, y el `OR` se
/// rechazaba con un error explícito. Eso bloqueaba tres cosas de golpe: un `88`
/// con `THRU`, un `88` con varios valores, y el `WHEN a, b, c` de `EVALUATE`.
///
/// Es un ÁRBOL y no una lista porque `A OR B AND C` no significa lo mismo que
/// `(A OR B) AND C`: **`AND` liga más fuerte que `OR`**, como en el estándar y
/// como en cualquier lenguaje. Una lista plana no puede representar esa
/// diferencia, y elegir mal cambia a qué rama va el programa sin que nada
/// avise.
#[derive(Debug, Clone, PartialEq)]
pub enum Condicion {
    Simple(CobolCondition),
    /// Las dos. Se evalúa en **cortocircuito**: si la primera falla, la segunda
    /// ni se calcula.
    Y(Box<Condicion>, Box<Condicion>),
    /// Cualquiera de las dos, también en cortocircuito.
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

    /// ★ Un conjunto de valores comparado contra UN campo, convertido en la
    /// condición que de verdad es.
    ///
    /// ```text
    ///   VALUE 1.          →  X = 1
    ///   VALUE 1 THRU 5.   →  X >= 1 AND X <= 5
    ///   VALUE 6, 7.       →  X = 6 OR X = 7
    /// ```
    ///
    /// Vive aquí y no en el codegen porque **la usan dos sitios que no se
    /// conocen**: los nombres de condición del nivel 88 y el `WHEN` de un
    /// `EVALUATE` con sujeto. Son la misma pregunta —"¿está este campo en este
    /// conjunto?"— y tenerla dos veces sería copiar el mismo error de extremo
    /// abierto en dos gramáticas distintas.
    ///
    /// Un rango lleva los dos extremos INCLUIDOS, que es lo que dice el
    /// estándar y lo que espera quien escribe `1 THRU 5` pensando en cinco.
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

/// Una comparación simple: el operando de la izquierda, el de la derecha, y qué
/// se pregunta.
///
/// Cada operando es un nombre de dato o un literal; el codegen lo resuelve
/// mirando si está declarado en la DATA DIVISION.
#[derive(Debug, Clone, PartialEq)]
pub enum CobolCondition {
    /// Un NOMBRE DE CONDICIÓN a secas (`IF FIN-DE-FICHERO`), declarado con un
    /// nivel 88. El parser no puede resolverlo —no conoce los datos—, así que
    /// lo pasa por nombre y lo expande el codegen, que sí sabe de quién es y
    /// puede decirlo cuando no existe.
    Nombre(String),
    Equal(String, String),
    NotEqual(String, String),
    Greater(String, String),
    Less(String, String),
    GreaterOrEqual(String, String),
    LessOrEqual(String, String),
}
