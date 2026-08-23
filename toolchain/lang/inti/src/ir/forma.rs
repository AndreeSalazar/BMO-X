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
    /// **Regla 1, escondida dentro de una division**: `-2^63 entre -1` no cabe.
    ///
    /// ** Comparte codigo con `Desborde` --las dos son la Regla 1-- y es una
    /// variante aparte porque **mira DOS valores**: el cociente solo no cabe
    /// cuando el dividendo es el minimo Y el divisor es -1. Ninguna de las otras
    /// necesita mas de uno.
    ///
    /// Hasta el 2026-08-22 no la pedia nadie, y el silicio cortaba igual: `idiv`
    /// levanta `#DE` y el programa moria con una autopsia en vez de atrapar.
    Cociente,
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
            Comprobacion::Desborde | Comprobacion::Cociente => "E1001",
            Comprobacion::Indice => "E1002",
            Comprobacion::EntreCero => "E1003",
            Comprobacion::Conversion(_) => "E1012",
        }
    }

    /// Las cuatro, para poder recorrerlas. Una lista que se puede recorrer es
    /// la diferencia entre *"creo que faltaba una"* y un numero.
    pub const TODAS: [Comprobacion; 5] = [
        Comprobacion::Desborde,
        Comprobacion::Cociente,
        Comprobacion::Indice,
        Comprobacion::EntreCero,
        Comprobacion::Conversion(4),
    ];

    /// Como se llama en castellano, para un informe que lee una persona.
    pub fn nombre(self) -> &'static str {
        match self {
            Comprobacion::Desborde => "desborde",
            Comprobacion::Cociente => "cociente que no cabe",
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
            Comprobacion::Desborde
            | Comprobacion::Cociente
            | Comprobacion::EntreCero
            | Comprobacion::Conversion(_) => true,
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
                // [!] Y desde el 2026-08-23 esta frase esta a MEDIAS. `lista de
                // T` YA existe en ejecucion --`runtime/objetos/lista.inti`-- y
                // comprueba su indice contra `cuantos`, que vive a un `mov` de
                // distancia en su cabecera.
                //
                // Lo que sigue sin poder comprobarse es indexar un `bufer`, y
                // eso no va a cambiar: no es que falte la comprobacion, es que
                // **no existe la informacion**.
                //
                // *** Lo que falta para que esta fila diga `true` es que el
                // DESCENSO baje `a[i]` de una lista a `sitio_de` en vez de a la
                // aritmetica cruda. Mientras no lo haga, contestar que si aqui
                // seria prometer una comprobacion que el binario no lleva.
                "un `bufer` es una direccion y no lleva su longitud, asi que no hay contra                  que comprobar. En `lista de T` SI se comprueba, y falta que el descenso                  la use"
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
        /// **Se opera SIN SIGNO?** (2026-08-23)
        ///
        /// ** Es una bandera y no una tercera `Clase` a proposito. `Clase` dice
        /// con QUE ARITMETICA se opera --entera o de coma flotante-- y eso
        /// decide el juego de instrucciones entero. El signo no cambia el juego:
        /// cambia CUATRO instrucciones dentro del mismo.
        ///
        /// ```text
        ///    < > <= >=          setb/seta   en vez de  setl/setg
        ///    entre, resto       div         en vez de  idiv
        ///    desplaza derecha   shr         en vez de  sar
        ///    la Regla 1         jc          en vez de  jo
        /// ```
        ///
        /// Meterlo en `Clase` habria obligado a los 121 sitios que la miran a
        /// atender un caso que a 117 de ellos no les cambia nada.
        sin_signo: bool,
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
        /// **El segundo valor, para las reglas que miran dos.**
        ///
        /// ** `None` es lo normal: desbordar, dividir entre cero y convertir se
        /// deciden con UN valor. La unica que necesita dos es el cociente que no
        /// cabe -- y por eso el campo es un `Option` y no un segundo `Valor`
        /// obligatorio que las demas tendrian que rellenar con algo falso.
        contra: Option<Valor>,
        sitio: Sitio,
    },
    /// **La direccion de una tabla congelada.**
    ///
    /// ## ** Por que es una INSTRUCCION y no un `Valor`
    ///
    /// Un `Valor::Congelado(i)` habria sido mas corto de escribir y habria
    /// obligado a que **cada sitio que carga un valor** supiera apuntar una
    /// reubicacion -- veintitres sitios, y el dia que alguien anadiera el
    /// veinticuatro se le olvidaria. El resultado seria una tabla que se carga
    /// con la direccion sin rellenar: un cero, otra vez.
    ///
    /// Siendo una instruccion, el emisor la atiende en UN sitio, ahi tiene la
    /// lista de reubicaciones a mano, y todo lo de despues ve un temporal
    /// normal. Y los `match` cerrados obligan a atenderla en todos los
    /// recorridos -- el reparto de registros, el barrido, el informe.
    Direccion {
        destino: Temporal,
        /// Indice en `ModuloIr::congelados`.
        congelado: u32,
    },
    /// **La direccion del monton de ESTA TAREA** (2026-08-23).
    ///
    /// ## Por que es una instruccion y no un argumento mas
    ///
    /// Porque `a + b` no tiene donde llevarlo. Todas las funciones del monton
    /// reciben el monton por parametro --`pide(monton, cuantos)`-- y eso vale
    /// mientras lo escriba una persona. Un OPERADOR no tiene ese hueco: nadie
    /// escribe `a +(monton) b`.
    ///
    /// Asi que el monton de la tarea es **ambiente**, como en cualquier lenguaje
    /// con objetos, y esta instruccion es por donde se coge.
    ///
    /// ## ** Y es la misma decision que `Instr::Direccion`, por el mismo motivo
    ///
    /// Podria ser un `Valor::MontonDeLaTarea` y seria mas corto. Obligaria a los
    /// veintitres sitios que cargan un valor a saber resolverlo, y el dia que
    /// alguien anadiera el veinticuatro se le olvidaria. Siendo una instruccion,
    /// el emisor la atiende en UN sitio y todo lo de despues ve un temporal.
    ///
    /// [!] El slot vive en la seccion `Data` --la 1 en la numeracion de las
    /// reubicaciones-- y quien lo rellena es el arranque. Mientras eso no exista,
    /// el emisor la pone en `sin_emitir` en vez de bajarla a un cero.
    MontonDeLaTarea {
        destino: Temporal,
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

/// **Una tabla CONGELADA: sus bytes, ya hechos.**
///
/// ** Nace de una `constante` cuyo valor es una lista de literales. No crece, no
/// pide monton, y por eso cabe en `llano` -- es lo que la seccion 10.2 del
/// maestro llama CONGELADO: *"inmortal. Nadie lo cambia, nadie cuenta sus
/// referencias"*.
///
/// Los bytes van a `SectionKind::RoData = 0x02` del `.bex`, y el codigo llega a
/// ellos por una reubicacion -- **no van dentro de la seccion de codigo**. Meter
/// datos ahi romperia el barrido lineal, que es lo que hace que un `.ibex` se
/// pueda recorrer entero y es la exclusividad tecnica de INTI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Congelado {
    pub nombre: String,
    /// Los bytes, ya en el orden de esta maquina.
    ///
    /// [!] **Sin cabecera.** Si `clase` pide una, la pone el EMISOR: la forma de
    /// un objeto del monton la declara `bmo-abi`, y este crate no lo enlaza a
    /// proposito (ver la cabecera de su `Cargo.toml`). Poner aqui veinticuatro
    /// bytes a mano seria una segunda declaracion del mismo contrato.
    pub bytes: Vec<u8>,
    /// Cuanto mide un elemento. Hace falta para indexar.
    pub ancho: u32,
    /// **Que clase de cosa congelada es.**
    pub clase: ClaseCongelada,
}

/// *** LAS DOS COSAS QUE VIVEN CONGELADAS, y por que son la misma (2026-08-23).
///
/// El pozo de textos y las tablas constantes nacieron separados y **no eran dos
/// mecanismos**: el pozo existia aparte solo porque `RoData` no existia todavia,
/// asi que `Const::Texto` bajaba a un CERO y el emisor lo confesaba en su lista
/// de "sin emitir".
///
/// La seccion 10.2 del maestro ya los tenia juntos desde el principio:
///
/// ```text
///    CONGELADO   inmortal. Nadie lo cambia, nadie cuenta sus referencias.
///                literales, constantes, un modulo cargado
/// ```
///
/// ** Lo unico que los separa es si llevan cabecera de objeto, y eso lo decide
/// esta etiqueta -- no dos caminos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseCongelada {
    /// Una tabla de literales: `PRIMOS = [2, 3, 5]`. Bytes pelados.
    Tabla,
    /// Un literal de texto. El emisor le pone delante la cabecera de
    /// `bmo_abi::dynobj::texto`, **con el bit de INMORTAL puesto**.
    Texto,
}

#[derive(Debug, Clone, Default)]
pub struct ModuloIr {
    pub funciones: Vec<FuncionIr>,
    /// El pozo de textos. Se comparte, y por eso puede prestarse congelado.
    pub textos: Vec<String>,
    /// **Las tablas congeladas del modulo**, en el orden en que se declararon.
    pub congelados: Vec<Congelado>,
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
