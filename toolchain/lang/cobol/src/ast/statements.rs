use crate::ast::SyscallDef;

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
    Add(String, String),
    Subtract(String, String),
    Multiply(String, String),
    Divide(String, String),
    Compute(String, String),
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
