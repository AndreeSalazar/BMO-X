//! `ir::forma` -- QUE FORMA TIENE UNA INSTRUCCION.
//!
//! ## Por que soy un fichero y no un trozo del de al lado (L6b)
//!
//! Porque contesto una pregunta distinta que `descenso`:
//!
//! ```text
//!    forma      QUE es una instruccion       -> tipos, y cero decisiones
//!    descenso   COMO se llega a una          -> recorre el arbol y decide
//! ```
//!
//! Es el mismo corte que separa `arbol` de `sintaxis`, una capa mas abajo: uno
//! define la forma y el otro la construye. Y tiene la misma consecuencia util:
//! **esto lo mira todo el mundo --el emisor, el marco, las pruebas-- y no pasa
//! nada**, porque un fichero que solo define una forma no puede colar una
//! decision dentro de quien lo lee.
//!
//! ** El dia que exista un segundo emisor, este fichero es lo unico que los dos
//! comparten. Por eso no puede tener logica: lo que este aqui lo hereda todo el
//! mundo, y lo que se hereda no se discute.

use crate::arbol::{Op, OpUno};
use crate::aviso::Sitio;

pub use crate::arbol::Clase;

/// Un valor con nombre y sin sitio. Donde vive lo decide el emisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Temporal(pub u32);

/// Una ranura local. Es un INDICE, no una direccion: el marco lo reparte quien
/// sabe el ancho de un puntero, y este modulo no lo sabe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Etiqueta(pub u32);

/// Una constante, **todavia sin convertir**.
///
/// El decimal sigue siendo texto por el mismo motivo que en el lexer: `numero`
/// es decimal exacto, y pasarlo por un binario intermedio perderia la exactitud
/// que el lenguaje promete en la portada.
#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    Entero(i64),
    /// El PATRON DE BITS de un flotante IEEE-754, no su texto.
    ///
    /// ** Y aqui esta la linea que separa las dos cosas que en castellano se
    /// llaman igual:
    ///
    /// ```text
    ///    Flotante   IEEE-754 binario. Mide lo que dice su nombre. Es `llano`
    ///    Decimal    exacto, todavia texto, y NO dice cuanto mide. Es `pleno`
    /// ```
    ///
    /// Son dos variantes y no una con una bandera porque **no se convierten la
    /// una en la otra sin perder algo**: 0,1 no existe en binario, y el dia que
    /// alguien "unifique" estas dos el decimal exacto deja de serlo en silencio.
    /// Ese es exactamente el motivo por el que el lexer tampoco convierte.
    Flotante(u64),
    Decimal(String),
    /// Indice en el pozo de textos del modulo.
    Texto(u32),
    Logico(bool),
    Nada,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Temporal(Temporal),
    Local(Local),
    Const(Const),
    /// Un nombre que este modulo no sabe resolver: una funcion, o algo que trae
    /// un `usa`. El emisor lo resuelve contra sus tablas.
    Nombre(String),
}

/// Que se comprueba, y con que codigo se atrapa.
///
/// ** Cada variante es una fila de `REGLAS.md`. Tenerlas como instrucciones
/// **y no como codigo suelto dentro del emisor** es lo que deja contarlas: un
/// test puede exigir que una suma traiga la suya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comprobacion {
    /// Regla 1: la operacion se paso de la cuenta.
    Desborde,
    /// Regla 3: dividir entre cero.
    EntreCero,
    /// Regla 2: indice fuera de rango.
    Indice,
    /// Regla 12: convertir un flotante que no cabe, **en tantos bytes**.
    ///
    /// ** Lleva el ancho porque sin el la pregunta no tiene respuesta: 1e10
    /// cabe de sobra en un `entero64` y no cabe en un `entero32`. Una
    /// comprobacion que no sabe contra que mide es una que aprueba todo.
    Conversion(u32),
}

impl Comprobacion {
    /// El codigo con el que atrapa.
    pub fn codigo(self) -> &'static str {
        match self {
            Comprobacion::Desborde => "E1001",
            Comprobacion::Indice => "E1002",
            Comprobacion::EntreCero => "E1003",
            Comprobacion::Conversion(_) => "E1012",
        }
    }

    /// Las cuatro, para poder recorrerlas. Una lista que se puede recorrer es
    /// la diferencia entre *"creo que faltaba una"* y un numero.
    pub const TODAS: [Comprobacion; 4] = [
        Comprobacion::Desborde,
        Comprobacion::Indice,
        Comprobacion::EntreCero,
        Comprobacion::Conversion(4),
    ];

    /// Como se llama en castellano, para un informe que lee una persona.
    pub fn nombre(self) -> &'static str {
        match self {
            Comprobacion::Desborde => "desborde",
            Comprobacion::Indice => "indice fuera de rango",
            Comprobacion::EntreCero => "dividir entre cero",
            Comprobacion::Conversion(_) => "conversion que no cabe",
        }
    }

    /// **Llega a bytes hoy?**
    ///
    /// ## ** Por que esto vive aqui y no dentro del emisor
    ///
    /// Porque es una pregunta sobre la REGLA y no sobre una maquina: la 2 no
    /// sale en ninguna, y por el mismo motivo en todas -- un `bufer` no lleva su
    /// longitud, y eso no depende del procesador. Ponerlo en el emisor haria que
    /// cada maquina nueva tuviera que volver a decidirlo, y la segunda decidiria
    /// distinto.
    ///
    /// *** Y este comentario ya nombro dos procesadores en su primera version.
    /// Lo casco `agnostico.rs`, que es la prueba que prohibe que el frontend
    /// nombre una maquina -- **hasta en la prosa**. Se deja escrito porque
    /// demuestra por que esa prueba mira los comentarios y no solo el codigo: el
    /// dia que el frontend "sepa" de una maquina, lo va a saber primero alguien
    /// que lea, y despues alguien que escriba.
    ///
    /// *** El emisor tiene un `match` que decide lo mismo. **Hay una prueba que
    /// exige que los dos digan lo mismo**, porque dos listas que dicen lo mismo
    /// se separan el dia que alguien toca una: es exactamente el fallo que este
    /// proyecto lleva persiguiendo desde el censo de las diez sondas.
    pub fn llega_a_bytes(self) -> bool {
        match self {
            Comprobacion::Desborde | Comprobacion::EntreCero | Comprobacion::Conversion(_) => true,
            Comprobacion::Indice => false,
        }
    }

    /// Y si no llega, **por que**. Vacio si llega.
    ///
    /// ** Un "no" sin motivo produce una pregunta que hay que ir a buscar al
    /// codigo. Este es el mismo criterio que `E0073`: distinguir *"esta
    /// prohibido"* de *"todavia no se hacerlo"* es la mitad del valor del aviso.
    pub fn por_que_no(self) -> &'static str {
        match self {
            Comprobacion::Indice => {
                "un `bufer` es una direccion y no lleva su longitud, asi que no hay contra \
                 que comprobar. Nace con `lista de T`"
            }
            _ => "",
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    Mueve {
        destino: Temporal,
        origen: Valor,
    },
    Binaria {
        destino: Temporal,
        op: Op,
        clase: Clase,
        izquierda: Valor,
        derecha: Valor,
    },
    Unaria {
        destino: Temporal,
        op: OpUno,
        valor: Valor,
    },
    /// La comprobacion que hace que no haya comportamiento indefinido.
    ///
    /// Va **detras** de la operacion que la necesita y mira su resultado. Si
    /// falla, atrapa con su codigo -- y atrapar en INTI es devolver un error,
    /// no abortar.
    Comprueba {
        que: Comprobacion,
        sobre: Valor,
        sitio: Sitio,
    },
    /// Cambia de clase de numero: `flotante64(n)`, `entero64(f)`.
    ///
    /// ** Es una INSTRUCCION y no una llamada, y esa es la decision. Escrito
    /// `flotante64(n)` parece una llamada y en C lo seria; aqui no, porque una
    /// llamada cuesta una convencion entera y esto es un `mov` con nombre.
    ///
    /// Y hay una razon mejor: **una conversion es el sitio donde vive la Regla
    /// 12**. Convertir 1e30 a `entero32` no tiene resultado, y lo que no tiene
    /// resultado atrapa. Eso se puede exigir de una instruccion; de una llamada
    /// a una funcion cualquiera, no.
    Convierte {
        destino: Temporal,
        valor: Valor,
        desde: Clase,
        hacia: Clase,
    },
    Llama {
        destino: Option<Temporal>,
        que: Valor,
        argumentos: Vec<Valor>,
    },
    /// Lee `ancho` bytes de una direccion.
    ///
    /// ** `ancho` va en BYTES, no en el nombre de un registro de la maquina:
    /// "8" es verdad en toda maquina y "qword" solo en una. Traducirlo a la
    /// instruccion es trabajo del emisor, y ese es el reparto entero.
    ///
    /// Esta instruccion **no comprueba nada**, y esa es su definicion. Por eso
    /// los nombres que la generan piden `crudo`: al otro lado de una direccion
    /// cruda no hay ningun kernel que valide. No es un descuido de la IR --
    /// es lo que se pidio.
    Lee {
        destino: Temporal,
        direccion: Valor,
        ancho: u32,
    },
    /// Escribe `ancho` bytes en una direccion.
    Escribe {
        direccion: Valor,
        valor: Valor,
        ancho: u32,
    },
    /// Un intrinseco de la maquina, por NOMBRE. El emisor lo busca en las
    /// tablas de la arquitectura; este modulo no sabe que hay detras.
    Metal {
        destino: Option<Temporal>,
        nombre: String,
        argumentos: Vec<Valor>,
    },
    Guarda {
        destino: Local,
        valor: Valor,
    },
    Etiqueta(Etiqueta),
    Salta(Etiqueta),
    SaltaSi {
        cond: Valor,
        cierto: Etiqueta,
        falso: Etiqueta,
    },
    Devuelve(Option<Valor>),
}

#[derive(Debug, Clone)]
pub struct FuncionIr {
    pub nombre: String,
    /// Cuantas de las locales son parametros.
    ///
    /// Son las PRIMERAS, y por eso basta un numero. El emisor lo necesita para
    /// guardarlos donde la maquina los deje al entrar -- pero **cuales son esos
    /// sitios es cosa suya**: aqui solo se dice cuantos.
    pub parametros: u32,
    /// Cuantas ranuras locales pide. El TAMANO de cada una lo decide el emisor
    /// con el perfil de la maquina: aqui solo se cuentan.
    pub locales: u32,
    pub temporales: u32,
    pub instrucciones: Vec<Instr>,
}

#[derive(Debug, Clone, Default)]
pub struct ModuloIr {
    pub funciones: Vec<FuncionIr>,
    /// El pozo de textos. Se comparte, y por eso puede prestarse congelado.
    pub textos: Vec<String>,
}

impl ModuloIr {
    /// Cuantas comprobaciones anti-UB emitio el modulo entero.
    ///
    /// ** Este numero es el precio del "sin comportamiento indefinido", y se
    /// puede leer. La seccion 6.3 del maestro dice que cuesta ~1%; aqui esta
    /// **cuantas son**, para que el dia de la medida se sepa contra que.
    pub fn comprobaciones(&self) -> usize {
        self.funciones
            .iter()
            .flat_map(|f| f.instrucciones.iter())
            .filter(|i| matches!(i, Instr::Comprueba { .. }))
            .count()
    }

    /// Cuantas instrucciones de la maquina toca el modulo.
    ///
    /// ** Es el hermano del contador de bloques `crudo`, y mide otra cosa: un
    /// `crudo` dice *"aqui nadie comprueba"*, y este dice *"aqui se habla con
    /// el silicio"*. Un programa puede tener mucho de lo primero y nada de lo
    /// segundo --el monton, sin ir mas lejos-- y al reves.
    ///
    /// Va a CABINA como numero por el mismo motivo que los demas: *"este
    /// programa se esta atando mas a la maquina que el mes pasado"* deja de ser
    /// una impresion.
    pub fn instrucciones(&self) -> usize {
        self.funciones
            .iter()
            .flat_map(|f| f.instrucciones.iter())
            .filter(|i| matches!(i, Instr::Metal { .. }))
            .count()
    }
}
