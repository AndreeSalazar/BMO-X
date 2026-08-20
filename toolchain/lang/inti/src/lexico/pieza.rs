//! `lexico::pieza` -- el tipo de dato, sin una linea de logica.
//!
//! ## Por que los datos van en su propio fichero
//!
//! Porque los va a leer todo el mundo: el lexer los produce, el parser los
//! consume, y las sondas del censo los miran. Un tipo compartido que vive
//! dentro del modulo que lo produce acaba arrastrando las decisiones de ese
//! modulo a todos los demas.

use crate::aviso::Sitio;
use crate::palabras::Simbolo;

/// En que base se escribio un numero. Se guarda porque `0xFF` y `255` son el
/// mismo valor y **no son el mismo codigo**: un aviso tiene que devolverle a la
/// persona lo que ella escribio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Diez,
    Dieciseis,
}

/// Un numero, **todavia como texto**.
///
/// ** Esto es una decision y no una pereza: `numero` en INTI es **decimal
/// exacto**, y convertirlo aqui a `f64` para "ya tenerlo hecho" perderia la
/// exactitud en el primer paso del compilador -- justo la propiedad que el
/// lenguaje promete en la portada (`0.1 + 0.2` da `0.3`). El valor lo construye
/// quien sabe en que forma va a vivir, que es F2, no el lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Numero {
    /// Tal y como se escribio, sin separadores de millar.
    pub texto: String,
    pub base: Base,
    /// Lleva punto decimal. Un entero y un decimal no son el mismo tipo por
    /// dentro (`INTI_MAESTRO.md` 10.3).
    pub con_punto: bool,
}

/// Los signos que no son palabras. La lista completa cabe aqui, y esa es la
/// idea: `GRAMATICA.md` sec. 0 dice que solo se usan simbolos donde se aprenden
/// en el colegio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signo {
    ParenAbre,
    ParenCierra,
    CorcheteAbre,
    CorcheteCierra,
    LlaveAbre,
    LlaveCierra,
    Coma,
    DosPuntos,
    Punto,
    /// `=`. Compara en una expresion y asigna en una sentencia, y no hay
    /// ambiguedad porque asignar no es una expresion.
    Igual,
    Menor,
    Mayor,
    MenorIgual,
    MayorIgual,
    Mas,
    Menos,
    Por,
    Barra,
}

impl Signo {
    /// Como se escribe. Para los avisos: hay que poder decirle a la persona
    /// exactamente que caracter esperaba.
    pub fn texto(self) -> &'static str {
        match self {
            Signo::ParenAbre => "(",
            Signo::ParenCierra => ")",
            Signo::CorcheteAbre => "[",
            Signo::CorcheteCierra => "]",
            Signo::LlaveAbre => "{",
            Signo::LlaveCierra => "}",
            Signo::Coma => ",",
            Signo::DosPuntos => ":",
            Signo::Punto => ".",
            Signo::Igual => "=",
            Signo::Menor => "<",
            Signo::Mayor => ">",
            Signo::MenorIgual => "<=",
            Signo::MayorIgual => ">=",
            Signo::Mas => "+",
            Signo::Menos => "-",
            Signo::Por => "*",
            Signo::Barra => "/",
        }
    }

    /// Abre una pareja: dentro de una, el salto de linea no termina la
    /// sentencia.
    pub fn abre(self) -> bool {
        matches!(self, Signo::ParenAbre | Signo::CorcheteAbre | Signo::LlaveAbre)
    }

    /// La que cierra a esta, si abre.
    pub fn pareja(self) -> Option<Signo> {
        match self {
            Signo::ParenAbre => Some(Signo::ParenCierra),
            Signo::CorcheteAbre => Some(Signo::CorcheteCierra),
            Signo::LlaveAbre => Some(Signo::LlaveCierra),
            _ => None,
        }
    }
}

/// Que es una pieza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clase {
    /// Una palabra clave, ya traducida a simbolo. **El parser no ve cadenas de
    /// ningun idioma**: ve esto.
    Palabra(Simbolo),
    /// Un nombre del autor, en minuscula.
    Nombre(String),
    /// Un nombre de tipo o de registro: empieza por mayuscula.
    Tipo(String),
    Numero(Numero),
    /// El contenido ya sin comillas y con los escapes resueltos.
    Texto(String),
    Signo(Signo),
    /// Entra un nivel de sangria.
    Sangra,
    /// Sale un nivel.
    Desangra,
    /// Final de una sentencia. No se emite dentro de una pareja abierta, ni en
    /// las lineas vacias o de solo comentario.
    FinLinea,
    /// Se acabo el fichero.
    Fin,
}

/// Una pieza con su sitio. **El sitio no es opcional**: un aviso sin `[DONDE]`
/// incumple el contrato de cuatro partes, asi que aqui no hay forma de perderlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pieza {
    pub clase: Clase,
    pub sitio: Sitio,
}

impl Pieza {
    pub fn nueva(clase: Clase, sitio: Sitio) -> Self {
        Self { clase, sitio }
    }

    pub fn es(&self, s: Simbolo) -> bool {
        matches!(&self.clase, Clase::Palabra(x) if *x == s)
    }

    pub fn es_signo(&self, s: Signo) -> bool {
        matches!(&self.clase, Clase::Signo(x) if *x == s)
    }

    /// Como se nombra en un aviso. En castellano y sin jerga: el lector no
    /// tiene por que saber que existe una clasificacion de piezas.
    pub fn como_se_llama(&self) -> String {
        match &self.clase {
            Clase::Palabra(s) => format!("la palabra `{}`", s.clave().to_lowercase()),
            Clase::Nombre(n) => format!("el nombre `{}`", n),
            Clase::Tipo(t) => format!("el tipo `{}`", t),
            Clase::Numero(n) => format!("el numero {}", n.texto),
            Clase::Texto(_) => "un texto".to_string(),
            Clase::Signo(s) => format!("el signo `{}`", s.texto()),
            Clase::Sangra => "una sangria".to_string(),
            Clase::Desangra => "el final de un bloque".to_string(),
            Clase::FinLinea => "el final de la linea".to_string(),
            Clase::Fin => "el final del fichero".to_string(),
        }
    }
}
