//! `aviso` -- el mensaje de cuatro partes.
//!
//! ## Por que esto es un modulo y no un `String` dentro del lexer
//!
//! Porque **el mensaje de error es la interfaz principal del lenguaje**, no un
//! caso de borde. El dato que lo decide: el **73%** de los envios de codigo de
//! estudiantes llevan errores de sintaxis, y hasta los mejores lo hacen en el
//! 50% de los casos. Un programador ve mas mensajes de error que documentacion.
//!
//! Y siendo la interfaz principal, tiene que poder **probarse sola**: este
//! modulo no sabe que existe un lexer, ni un parser, ni INTI. Sabe formatear
//! cuatro campos y nada mas. Sus tests corren sin compilar una linea de nada.
//!
//! ## Las cuatro partes, y de donde salen
//!
//! No son de gusto: el estudio de CHI 2021 sobre mensajes para novatos midio
//! los factores que deciden si un mensaje se entiende -- **longitud, jerga,
//! estructura de la frase y vocabulario**. Las cuatro partes atacan los cuatro:
//!
//! ```text
//!    [QUE PASO]     una frase, en castellano, sin jerga de compilador
//!    [DONDE]        fichero, linea, y LA LINEA, con el dedo puesto
//!    [QUE HABIA]    los valores concretos, con el nombre que escribio el autor
//!    [QUE HACER]    codigo que se puede pegar
//! ```
//!
//! OJO: **Un aviso sin las cuatro partes es un fallo de test, no un detalle de
//! estilo.** Ver `pruebas::las_cuatro_partes_son_obligatorias`.

pub mod codigos;

pub use codigos::Codigo;

/// Donde pasa algo. Linea y columna cuentan desde 1 **porque las cuenta un
/// humano leyendo su editor**, y esa es la unica base que importa aqui.
///
/// (Los indices del lenguaje si son base 0 -- ver `GRAMATICA.md` sec. 9. No es
/// una incoherencia: alli el que cuenta es el hardware, aqui el que cuenta es
/// la persona.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sitio {
    pub linea: usize,
    pub columna: usize,
}

impl Sitio {
    pub fn nuevo(linea: usize, columna: usize) -> Self {
        Self { linea, columna }
    }
}

impl Default for Sitio {
    fn default() -> Self {
        Self::nuevo(1, 1)
    }
}

/// Un aviso completo. Los cuatro campos que no pueden faltar son `codigo`,
/// `que_paso`, `sitio` y `que_hacer`; `que_habia` puede estar vacio **solo**
/// cuando no hay ningun valor concreto del que hablar (por ejemplo, falta el
/// `perfil`: no hay nada que ensenar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    pub codigo: Codigo,
    /// Que paso. Una frase. Sin "token", sin "AST", sin "unexpected EOF".
    pub que_paso: String,
    pub sitio: Sitio,
    /// La linea de codigo tal cual, para poner el dedo encima.
    pub linea_fuente: String,
    /// Los valores concretos. Vacio si de verdad no hay ninguno.
    pub que_habia: String,
    /// Que hacer. En imperativo, y si se puede, codigo pegable.
    pub que_hacer: String,
}

impl Aviso {
    pub fn nuevo(codigo: Codigo, que_paso: impl Into<String>, sitio: Sitio) -> Self {
        Self {
            codigo,
            que_paso: que_paso.into(),
            sitio,
            linea_fuente: String::new(),
            que_habia: String::new(),
            que_hacer: String::new(),
        }
    }

    pub fn con_linea(mut self, linea: impl Into<String>) -> Self {
        self.linea_fuente = linea.into();
        self
    }

    pub fn con_habia(mut self, habia: impl Into<String>) -> Self {
        self.que_habia = habia.into();
        self
    }

    pub fn con_hacer(mut self, hacer: impl Into<String>) -> Self {
        self.que_hacer = hacer.into();
        self
    }

    /// Es un aviso (`A2xxx`) y no un error: el programa sigue compilando.
    pub fn es_aviso(&self) -> bool {
        self.codigo.0.starts_with('A')
    }

    /// Se atrapa en ejecucion (`E1xxx`): el frontend no lo emite, lo declara.
    pub fn es_de_ejecucion(&self) -> bool {
        self.codigo.0.starts_with("E1")
    }

    /// El texto que ve la persona.
    ///
    /// El nombre del fichero entra por argumento y no por campo porque un
    /// aviso puede nacer antes de saber como se llama lo que se esta leyendo
    /// (una cadena en un test, la entrada de un REPL). Guardar un nombre falso
    /// para rellenar el hueco es como se acaba imprimiendo `<stdin>` en un
    /// error sobre un fichero de disco.
    pub fn pintar(&self, fichero: &str) -> String {
        let mut s = String::new();
        s.push_str(&format!("{} {}\n", self.codigo, self.que_paso));
        s.push_str(&format!(
            "   en {}, linea {}:\n",
            fichero, self.sitio.linea
        ));
        if !self.linea_fuente.is_empty() {
            s.push_str(&format!("      {}\n", self.linea_fuente.trim_end()));
            // El dedo debajo de la columna exacta. Se cuenta en caracteres y
            // no en bytes: una tilde ocupa dos bytes y desplazaria la flecha.
            let sangria: String = std::iter::repeat(' ')
                .take(6 + self.sitio.columna.saturating_sub(1))
                .collect();
            s.push_str(&format!("{}^\n", sangria));
        }
        if !self.que_habia.is_empty() {
            s.push_str(&format!("   {}\n", self.que_habia));
        }
        if !self.que_hacer.is_empty() {
            s.push_str(&format!("   prueba: {}\n", self.que_hacer));
        }
        s
    }
}

/// Lo que devuelve una fase: lo que salio, y todo lo que hay que decir.
///
/// ## Por que no es `Result`
///
/// Porque un `Result` obliga a elegir entre **el resultado** y **los avisos**,
/// y las dos cosas hacen falta a la vez: un fichero con tres errores tiene que
/// dar los tres, no el primero. Parar en el primero convierte arreglar un
/// programa en un juego de adivinar cuantos quedan.
#[derive(Debug, Clone)]
pub struct Cosecha<T> {
    pub valor: T,
    pub avisos: Vec<Aviso>,
}

impl<T> Cosecha<T> {
    pub fn nueva(valor: T) -> Self {
        Self {
            valor,
            avisos: Vec::new(),
        }
    }

    pub fn con(valor: T, avisos: Vec<Aviso>) -> Self {
        Self { valor, avisos }
    }

    /// Hay algo que impide seguir. Los `A2xxx` no cuentan.
    pub fn hay_errores(&self) -> bool {
        self.avisos.iter().any(|a| !a.es_aviso())
    }

    pub fn errores(&self) -> impl Iterator<Item = &Aviso> {
        self.avisos.iter().filter(|a| !a.es_aviso())
    }

    /// Los codigos, en orden. Es lo que comparan las sondas del censo.
    pub fn codigos(&self) -> Vec<&'static str> {
        self.avisos.iter().map(|a| a.codigo.0).collect()
    }

    pub fn pintar(&self, fichero: &str) -> String {
        self.avisos
            .iter()
            .map(|a| a.pintar(fichero))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ejemplo() -> Aviso {
        Aviso::nuevo(
            codigos::TABULADOR,
            "Hay un tabulador donde va la sangria.",
            Sitio::nuevo(12, 1),
        )
        .con_linea("\tescribe \"hola\"")
        .con_habia("La sangria de INTI son cuatro espacios, siempre.")
        .con_hacer("cambia el tabulador por cuatro espacios")
    }

    /// La regla que `REGLAS.md` llama contrato y no estilo.
    #[test]
    fn las_cuatro_partes_son_obligatorias() {
        let a = ejemplo();
        let t = a.pintar("notas.inti");
        assert!(t.contains("E0010"), "falta el codigo");
        assert!(t.contains("tabulador donde va la sangria"), "falta que paso");
        assert!(t.contains("notas.inti, linea 12"), "falta donde");
        assert!(t.contains("cuatro espacios, siempre"), "falta que habia");
        assert!(t.contains("prueba:"), "falta que hacer");
    }

    /// Sin jerga. La lista negra es corta y concreta a proposito: son las
    /// palabras que un compilador escribe cuando habla consigo mismo.
    #[test]
    fn el_mensaje_no_tiene_jerga_de_compilador() {
        const JERGA: &[&str] = &[
            "token", "AST", "lexer", "parser", "EOF", "unexpected", "nonterminal",
        ];
        let t = ejemplo().pintar("notas.inti");
        for palabra in JERGA {
            assert!(
                !t.to_lowercase().contains(&palabra.to_lowercase()),
                "el mensaje dice `{}`, que solo significa algo dentro del compilador",
                palabra
            );
        }
    }

    /// El dedo tiene que caer bajo la columna, contando CARACTERES.
    #[test]
    fn el_dedo_apunta_a_la_columna() {
        let a = Aviso::nuevo(codigos::SIGNO_DESCONOCIDO, "Ese signo no es mio.", Sitio::nuevo(3, 5))
            .con_linea("x = 5 @ 2");
        let t = a.pintar("p.inti");
        let linea_dedo = t.lines().find(|l| l.trim() == "^").expect("no hay dedo");
        assert_eq!(linea_dedo.len(), 6 + 4 + 1, "el dedo no cae en la columna 5");
    }

    /// Una cosecha con solo avisos sigue siendo compilable.
    #[test]
    fn un_aviso_no_es_un_error() {
        let c = Cosecha::con(
            (),
            vec![Aviso::nuevo(codigos::DESPLAZA_DE_MAS, "Ese desplazamiento da cero.", Sitio::default())],
        );
        assert!(!c.hay_errores());
        assert_eq!(c.errores().count(), 0);
    }

    #[test]
    fn los_codigos_salen_en_orden() {
        let c = Cosecha::con(
            (),
            vec![
                Aviso::nuevo(codigos::TABULADOR, "a", Sitio::default()),
                Aviso::nuevo(codigos::COMILLA_SIMPLE, "b", Sitio::default()),
            ],
        );
        assert_eq!(c.codigos(), vec!["E0010", "E0011"]);
    }
}
