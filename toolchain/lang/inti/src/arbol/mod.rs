//! `arbol` -- la forma de un programa de INTI. Datos puros.
//!
//! ## Por que esto es un modulo y no vive dentro del parser
//!
//! Porque el arbol lo va a leer **todo lo que viene despues**: el analisis de
//! nombres, el de perfiles, la IR y el emisor. Si viviera dentro de quien lo
//! construye, cada uno de esos tendria que importar el parser entero para mirar
//! un nodo -- y a partir de ahi, cualquier decision del parser se convierte en
//! una decision de todos.
//!
//! Este fichero **no tiene una sola funcion que decida nada**. Es la forma, y
//! nada mas.
//!
//! ## La regla del sitio
//!
//! Cada nodo lleva su [`Sitio`]. No es opcional y no se puede olvidar: un aviso
//! sin `[DONDE]` incumple el contrato de cuatro partes, y el sitio solo se sabe
//! aqui -- despues ya no hay texto al que volver.

use crate::aviso::Sitio;
use crate::lexico::Numero;

/// El perfil del modulo. Es lo primero de un fichero y no tiene valor por
/// defecto: `GRAMATICA.md` sec. 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perfil {
    /// Sin monton, sin contador de referencias. Escribe sistema.
    Llano,
    /// Con texto, listas, tablas, congelado y tareas.
    Pleno,
}

impl Perfil {
    pub fn nombre(self) -> &'static str {
        match self {
            Perfil::Llano => "llano",
            Perfil::Pleno => "pleno",
        }
    }
}

/// Un fichero entero.
#[derive(Debug, Clone)]
pub struct Modulo {
    pub perfil: Perfil,
    pub sitio_perfil: Sitio,
    /// Lo que se importa, en orden.
    pub usa: Vec<(String, Sitio)>,
    pub declaraciones: Vec<Decl>,
}

/// Un tipo, tal y como se escribio. **No esta resuelto**: `Nombre("numero")` y
/// `Nombre("Alumno")` salen iguales de aqui, y decidir cual existe es trabajo
/// de otro. Mezclar las dos cosas es como un parser acaba necesitando saber que
/// tipos hay declarados para poder leer una linea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tipo {
    Nombre(String),
    /// `lista de T`
    Lista(Box<Tipo>),
    /// `bufer de T` -- una DIRECCION, y de que estan hechos los elementos.
    ///
    /// ** No lleva su longitud dentro, y esa es toda la diferencia con
    /// `lista de T`: por eso `bufer` vive en `llano` y `lista` en `pleno`, y por
    /// eso indexar un bufer pide `crudo` -- no hay nadie que compruebe el
    /// limite porque no hay limite guardado en ningun sitio.
    ///
    /// Es lo que hace falta para escribir un framebuffer sin pagar nada.
    Bufer(Box<Tipo>),
    /// `tabla de T a U`
    Tabla(Box<Tipo>, Box<Tipo>),
    /// `quiza T` -- puede no haber valor, y no se puede usar sin mirarlo.
    Quiza(Box<Tipo>),
}

/// Lo que devuelve una funcion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TipoRetorno {
    pub tipo: Tipo,
    /// `devuelve numero o error`. Si es cierto, quien llama **tiene** que mirar
    /// el resultado (`E0060`).
    pub puede_fallar: bool,
}

#[derive(Debug, Clone)]
pub struct Campo {
    pub nombre: String,
    pub tipo: Option<Tipo>,
    pub defecto: Option<Expr>,
    pub sitio: Sitio,
}

#[derive(Debug, Clone)]
pub struct Parametro {
    pub nombre: String,
    /// Sin esto, el parametro no se puede cambiar dentro (`E0033`).
    pub cambiante: bool,
    pub tipo: Option<Tipo>,
    pub defecto: Option<Expr>,
    pub sitio: Sitio,
}

#[derive(Debug, Clone)]
pub struct Funcion {
    pub nombre: String,
    pub parametros: Vec<Parametro>,
    pub retorno: Option<TipoRetorno>,
    pub cuerpo: Bloque,
    pub sitio: Sitio,
}

#[derive(Debug, Clone)]
pub enum Decl {
    /// Nivel superior: se congela al cargar el modulo, y por eso no admite
    /// `cambiante` (`E0002`).
    Constante {
        nombre: String,
        valor: Expr,
        sitio: Sitio,
    },
    Registro {
        nombre: String,
        campos: Vec<Campo>,
        /// ** Las operaciones escritas DENTRO del registro.
        ///
        /// `operacion suma(a, b)` dentro de `registro Punto` es lo mismo que
        /// `operacion Punto suma(a, b)` fuera -- **el mismo nodo por dos
        /// caminos**, como pasa con la llamada sin parentesis. Dentro no se
        /// repite el nombre del tipo porque ya se dijo en la linea de arriba.
        operaciones: Vec<Funcion>,
        sitio: Sitio,
    },
    Funcion(Funcion),
    /// `operacion Punto suma(a, b)` -- rellena una ranura numerada del tipo.
    /// No es un metodo: no hay `self` ni herencia.
    Operacion {
        tipo: String,
        funcion: Funcion,
    },
}

impl Decl {
    pub fn sitio(&self) -> Sitio {
        match self {
            Decl::Constante { sitio, .. } | Decl::Registro { sitio, .. } => *sitio,
            Decl::Funcion(f) => f.sitio,
            Decl::Operacion { funcion, .. } => funcion.sitio,
        }
    }

    pub fn nombre(&self) -> &str {
        match self {
            Decl::Constante { nombre, .. } | Decl::Registro { nombre, .. } => nombre,
            Decl::Funcion(f) => &f.nombre,
            Decl::Operacion { funcion, .. } => &funcion.nombre,
        }
    }
}

pub type Bloque = Vec<Sent>;

/// Las tres formas del unico bucle. Son tres y no tres bucles distintos porque
/// lo caro de un bucle es componer el plan, no elegir la palabra.
#[derive(Debug, Clone)]
pub enum Repeticion {
    /// `repite` -- a proposito infinito; se sale con `corta`.
    Siempre,
    /// `repite 10 veces`
    Veces(Expr),
    /// `repite mientras <condicion>`
    Mientras(Expr),
}

#[derive(Debug, Clone)]
pub enum Sent {
    /// `x = 1`, `cambiante y es entero32 = 0`, `p.x = 3`, `a[i] = 3`.
    ///
    /// **Asignar es una SENTENCIA y nunca una expresion**, y de ahi sale que
    /// `=` pueda significar igual en los dos sitios sin ambiguedad.
    Asigna {
        destino: Expr,
        cambiante: bool,
        tipo: Option<Tipo>,
        valor: Expr,
        sitio: Sitio,
    },
    /// `si` con sus `sino si` y su `sino`. Las ramas van en orden.
    Si {
        ramas: Vec<(Expr, Bloque)>,
        sino: Option<Bloque>,
        sitio: Sitio,
    },
    /// `para cada x en lista` y `para cada i en 0 hasta 10`.
    ///
    /// El `hasta` va aqui y no como una expresion de rango porque **un rango no
    /// es un valor en INTI**: no se puede guardar en una variable. Tenerlo como
    /// expresion obligaria a inventar un tipo que el lenguaje no tiene.
    ParaCada {
        nombre: String,
        desde: Expr,
        hasta: Option<Expr>,
        cuerpo: Bloque,
        sitio: Sitio,
    },
    Repite {
        forma: Repeticion,
        cuerpo: Bloque,
        sitio: Sitio,
    },
    Devuelve {
        valor: Option<Expr>,
        sitio: Sitio,
    },
    Falla {
        motivo: Expr,
        sitio: Sitio,
    },
    Corta(Sitio),
    Continua(Sitio),
    /// La unica ventana sin comprobar. Solo en `llano` (`E0071`), y el
    /// compilador la **cuenta**.
    Crudo {
        cuerpo: Bloque,
        sitio: Sitio,
    },
    /// Solo en `pleno`. Lo que cruza tiene que estar congelado (`E0080`).
    Paralelo {
        cuerpo: Bloque,
        sitio: Sitio,
    },
    /// Una expresion suelta: casi siempre una llamada.
    Expresion(Expr),
}

impl Sent {
    pub fn sitio(&self) -> Sitio {
        match self {
            Sent::Asigna { sitio, .. }
            | Sent::Si { sitio, .. }
            | Sent::ParaCada { sitio, .. }
            | Sent::Repite { sitio, .. }
            | Sent::Devuelve { sitio, .. }
            | Sent::Falla { sitio, .. }
            | Sent::Crudo { sitio, .. }
            | Sent::Paralelo { sitio, .. } => *sitio,
            Sent::Corta(s) | Sent::Continua(s) => *s,
            Sent::Expresion(e) => e.sitio(),
        }
    }
}

/// Los operadores de dos lados. El orden de esta lista **no** es la
/// precedencia: la precedencia vive en el parser, que es quien la aplica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Suma,
    Resta,
    Por,
    /// `/` -- divide de verdad. `5 / 2` da `2.5`.
    Divide,
    /// `entre` -- cociente entero. Una palabra distinta y no un simbolo
    /// parecido, que es la sorpresa 10 de Python.
    Entre,
    Resto,
    Elevado,
    /// `=` en posicion de expresion.
    Igual,
    /// `no es`
    NoEs,
    /// `es un` -- pregunta el TIPO, no el valor.
    EsUn,
    Menor,
    Mayor,
    MenorIgual,
    MayorIgual,
    Y,
    O,
    BitsY,
    BitsO,
    BitsXor,
    DesplazaIzquierda,
    DesplazaDerecha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpUno {
    /// `-x`
    Menos,
    /// `no x`
    No,
}

/// Un argumento de una llamada. Puede ir por nombre: `Alumno(nombre: "ana")`.
#[derive(Debug, Clone)]
pub struct Argumento {
    pub nombre: Option<String>,
    pub valor: Expr,
}

/// Lo que hay detras de `o si no`: un valor, o un bloque que corta el paso.
#[derive(Debug, Clone)]
pub enum Respaldo {
    Valor(Box<Expr>),
    Bloque(Bloque),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Numero(Numero, Sitio),
    Texto(String, Sitio),
    Logico(bool, Sitio),
    /// `nada`. **No es un nulo**: es lo que devuelve una funcion que no
    /// devuelve, y un valor que puede faltar se declara `quiza T`.
    Nada(Sitio),
    /// Un nombre en minuscula: variable o funcion.
    Nombre(String, Sitio),
    /// Un nombre en mayuscula: tipo o registro.
    Tipo(String, Sitio),
    Lista(Vec<Expr>, Sitio),
    Tabla(Vec<(Expr, Expr)>, Sitio),
    Binaria {
        op: Op,
        izquierda: Box<Expr>,
        derecha: Box<Expr>,
        sitio: Sitio,
    },
    Unaria {
        op: OpUno,
        valor: Box<Expr>,
        sitio: Sitio,
    },
    /// `f(a, b)` y tambien `cuenta de lista`, que es la misma llamada escrita
    /// como una frase. **Salen el mismo nodo**: `de` no es un operador, es
    /// azucar, y por eso no cuesta gramatica.
    Llamada {
        que: Box<Expr>,
        argumentos: Vec<Argumento>,
        sitio: Sitio,
    },
    Indice {
        que: Box<Expr>,
        indice: Box<Expr>,
        sitio: Sitio,
    },
    Campo {
        que: Box<Expr>,
        nombre: String,
        sitio: Sitio,
    },
    /// `abrir("x") o si no ""`.
    OSiNo {
        intento: Box<Expr>,
        respaldo: Respaldo,
        sitio: Sitio,
    },
}

impl Expr {
    pub fn sitio(&self) -> Sitio {
        match self {
            Expr::Numero(_, s)
            | Expr::Texto(_, s)
            | Expr::Logico(_, s)
            | Expr::Nada(s)
            | Expr::Nombre(_, s)
            | Expr::Tipo(_, s)
            | Expr::Lista(_, s)
            | Expr::Tabla(_, s) => *s,
            Expr::Binaria { sitio, .. }
            | Expr::Unaria { sitio, .. }
            | Expr::Llamada { sitio, .. }
            | Expr::Indice { sitio, .. }
            | Expr::Campo { sitio, .. }
            | Expr::OSiNo { sitio, .. } => *sitio,
        }
    }

    /// Sirve como destino de una asignacion. Se pregunta aqui y no en el parser
    /// porque la respuesta es una propiedad de la FORMA, no de como se leyo.
    pub fn es_destino(&self) -> bool {
        matches!(
            self,
            Expr::Nombre(..) | Expr::Campo { .. } | Expr::Indice { .. }
        )
    }
}
