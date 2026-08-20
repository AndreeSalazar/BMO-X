//! `palabras` -- el vocabulario, que es un fichero y no un `match`.
//!
//! ## Por que esto es un modulo entero para 49 cadenas
//!
//! Porque es lo que hace que **el idioma sea una columna y no un fork**. Las
//! palabras clave de INTI estan en castellano por una razon medida (los novatos
//! que programan en su lengua demuestran conceptos nuevos mas rapido), y eso
//! tiene un precio igual de real: nadie de fuera del idioma contribuye.
//!
//! Este modulo es el que hace ese precio **reversible**. El lexer nunca compara
//! contra `"funcion"`: pregunta al vocabulario. Cambiar de idioma es cambiar el
//! fichero que se carga, y no hay una sola linea del compilador que lo sepa.
//!
//! Es el patron de `intrinsics.toml` -- *"anadir una instruccion = 1 entrada
//! TOML, CERO Rust"* -- aplicado a las palabras.
//!
//! ## Y por que se CARGA en vez de venir incrustado
//!
//! Porque `tables/` es la raiz que consulta `bmo-mods`, y quien deje su version
//! en `$BMO_MODS` gana **sin bifurcar el repo**. Si el vocabulario viviera solo
//! dentro del binario, esa propiedad se perderia y un dialecto volveria a ser
//! un fork.
//!
//! El incrustado existe igual, pero como **respaldo**: un compilador que no
//! arranca porque no encuentra un fichero de datos es peor que uno que arranca
//! con lo que traia. Y como el respaldo entra por `include_str!` **del mismo
//! fichero**, no pueden divergir.

use std::collections::HashMap;

use bmo_mods::Roots;

/// El TOML que viaja dentro. Es literalmente el fichero de `tables/`, asi que
/// el respaldo y la fuente no pueden decir cosas distintas.
const INCRUSTADO: &str =
    include_str!("../../../../forge/sem-asm/tables/lang/inti/palabras.toml");

/// La ruta relativa a una raiz de tablas.
pub const RUTA: &str = "lang/inti/palabras.toml";

/// Declara los simbolos y su clave en la tabla de una vez.
///
/// La clave es lo que aparece en el TOML y **el simbolo es lo que ve el
/// parser**. Separarlos es el truco entero: el parser hace `match` sobre algo
/// que no es una cadena de ningun idioma.
macro_rules! simbolos {
    ($($var:ident => $clave:literal),* $(,)?) => {
        /// Una palabra clave, ya reconocida. El parser solo ve esto.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Simbolo {
            $($var),*
        }

        impl Simbolo {
            /// Todos, en el orden en que se declararon.
            pub const TODOS: &'static [Simbolo] = &[$(Simbolo::$var),*];

            /// La clave con la que aparece en `palabras.toml`.
            pub fn clave(self) -> &'static str {
                match self { $(Simbolo::$var => $clave),* }
            }
        }
    };
}

simbolos! {
    // el modulo
    Perfil => "PERFIL",
    Llano => "LLANO",
    Pleno => "PLENO",
    Usa => "USA",
    // declaraciones
    Funcion => "FUNCION",
    Devuelve => "DEVUELVE",
    Registro => "REGISTRO",
    Operacion => "OPERACION",
    // nombres y tipos
    Cambiante => "CAMBIANTE",
    Es => "ES",
    Un => "UN",
    De => "DE",
    A => "A",
    // control
    Si => "SI",
    Sino => "SINO",
    Para => "PARA",
    Cada => "CADA",
    En => "EN",
    Hasta => "HASTA",
    Repite => "REPITE",
    Veces => "VECES",
    Mientras => "MIENTRAS",
    Corta => "CORTA",
    Continua => "CONTINUA",
    // errores y perfiles
    Falla => "FALLA",
    Crudo => "CRUDO",
    Paralelo => "PARALELO",
    // operadores que son palabras
    Y => "Y",
    O => "O",
    No => "NO",
    Entre => "ENTRE",
    Resto => "RESTO",
    Elevado => "ELEVADO",
    Desplaza => "DESPLAZA",
    Izquierda => "IZQUIERDA",
    Derecha => "DERECHA",
    BitsY => "BITS_Y",
    BitsO => "BITS_O",
    BitsXor => "BITS_XOR",
    // valores
    Cierto => "CIERTO",
    Falso => "FALSO",
    Nada => "NADA",
    Quiza => "QUIZA",
    Error => "ERROR",
    Fallo => "FALLO",
    Valor => "VALOR",
    Motivo => "MOTIVO",
    Lista => "LISTA",
    Tabla => "TABLA",
}

/// Lo que puede salir mal al cargar el vocabulario. Son fallos de la TABLA, no
/// del programa que se esta compilando, y por eso no son un `Aviso`: un aviso
/// habla de tu codigo, esto habla de la instalacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rotura {
    /// El TOML no es TOML.
    NoEsToml(String),
    /// El idioma pedido no esta en el fichero.
    SinIdioma(String),
    /// Falta una palabra que el lenguaje necesita.
    FaltaPalabra { idioma: String, clave: String },
    /// Dos simbolos con el mismo texto: uno taparia al otro en silencio.
    TextoRepetido { texto: String, claves: (String, String) },
}

impl std::fmt::Display for Rotura {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rotura::NoEsToml(e) => write!(f, "palabras.toml no se puede leer: {}", e),
            Rotura::SinIdioma(i) => write!(f, "palabras.toml no trae el idioma `{}`", i),
            Rotura::FaltaPalabra { idioma, clave } => {
                write!(f, "a `[{}]` le falta la palabra {}", idioma, clave)
            }
            Rotura::TextoRepetido { texto, claves } => write!(
                f,
                "`{}` esta puesto en {} y en {}: uno taparia al otro",
                texto, claves.0, claves.1
            ),
        }
    }
}

/// Las palabras de un idioma, listas para preguntar.
#[derive(Debug, Clone)]
pub struct Vocabulario {
    idioma: String,
    por_texto: HashMap<String, Simbolo>,
    por_simbolo: HashMap<Simbolo, String>,
    /// Si las tildes valen como alias. Sale del propio fichero.
    alias_por_tilde: bool,
}

impl Vocabulario {
    /// El que viaja dentro del binario. Nunca falla salvo que alguien rompa la
    /// tabla del repo, y para eso esta el test.
    pub fn por_defecto() -> Result<Self, Rotura> {
        Self::desde_texto(INCRUSTADO, None)
    }

    /// El de las raices de `bmo-mods`: `$BMO_MODS` -> `mods/` -> `tables/`.
    /// Si no aparece en ninguna, se usa el incrustado **y se dice**.
    pub fn cargar(raices: &Roots) -> (Result<Self, Rotura>, Origen) {
        match raices.locate(RUTA) {
            Some(p) => match std::fs::read_to_string(&p) {
                Ok(t) => (Self::desde_texto(&t, None), Origen::Fichero(p)),
                Err(_) => (Self::por_defecto(), Origen::Incrustado),
            },
            None => (Self::por_defecto(), Origen::Incrustado),
        }
    }

    /// Un idioma concreto del mismo fichero. Es la prueba de que la columna
    /// inglesa no es decorativa.
    pub fn desde_texto(toml_txt: &str, idioma: Option<&str>) -> Result<Self, Rotura> {
        let raiz: toml::Value = toml_txt
            .parse()
            .map_err(|e: toml::de::Error| Rotura::NoEsToml(e.to_string()))?;

        let meta = raiz.get("meta");
        let idioma = idioma
            .map(|s| s.to_string())
            .or_else(|| {
                meta.and_then(|m| m.get("idioma_por_defecto"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "es".to_string());

        let alias_por_tilde = meta
            .and_then(|m| m.get("alias_por_tilde"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let tabla = raiz
            .get(&idioma)
            .and_then(|v| v.as_table())
            .ok_or_else(|| Rotura::SinIdioma(idioma.clone()))?;

        let mut por_texto = HashMap::new();
        let mut por_simbolo = HashMap::new();

        for s in Simbolo::TODOS {
            let texto = tabla
                .get(s.clave())
                .and_then(|v| v.as_str())
                .ok_or_else(|| Rotura::FaltaPalabra {
                    idioma: idioma.clone(),
                    clave: s.clave().to_string(),
                })?;

            // Dos simbolos con el mismo texto no es un empate: es que uno de
            // los dos deja de existir y nadie lo nota hasta que un programa
            // hace algo raro.
            if let Some(otro) = por_texto.insert(texto.to_string(), *s) {
                return Err(Rotura::TextoRepetido {
                    texto: texto.to_string(),
                    claves: (otro.clave().to_string(), s.clave().to_string()),
                });
            }
            por_simbolo.insert(*s, texto.to_string());
        }

        Ok(Self {
            idioma,
            por_texto,
            por_simbolo,
            alias_por_tilde,
        })
    }

    pub fn idioma(&self) -> &str {
        &self.idioma
    }

    /// Cuantas palabras reserva el lenguaje. Un lenguaje se juzga por lo que
    /// reserva, asi que este numero es publico a proposito.
    pub fn cuantas(&self) -> usize {
        self.por_simbolo.len()
    }

    /// Como se escribe un simbolo en este idioma.
    pub fn texto(&self, s: Simbolo) -> &str {
        // No puede faltar: `desde_texto` falla antes si falta alguno.
        &self.por_simbolo[&s]
    }

    /// La pregunta que hace el lexer: esta palabra, es clave?
    ///
    /// Si el fichero lo permite, la version acentuada vale igual: quien escribe
    /// `funcion` con tilde no tropieza, y el fichero canonico sigue siendo
    /// ASCII porque `ascii-sweep` lo exige.
    pub fn reconocer(&self, palabra: &str) -> Option<Simbolo> {
        if let Some(s) = self.por_texto.get(palabra) {
            return Some(*s);
        }
        if self.alias_por_tilde {
            let plano = sin_tildes(palabra);
            if plano != palabra {
                return self.por_texto.get(&plano).copied();
            }
        }
        None
    }
}

/// De donde salio el vocabulario. Se devuelve para que una herramienta pueda
/// decirlo: "no encuentro tu tabla y estoy usando la mia" tiene que poder
/// verse, porque el fallo silencioso aqui seria un dialecto que no se aplica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origen {
    Fichero(std::path::PathBuf),
    Incrustado,
}

/// Quita las tildes del castellano. No es normalizacion Unicode completa y no
/// pretende serlo: cubre exactamente las letras que un teclado espanol pone en
/// una palabra clave.
pub fn sin_tildes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{e1}' => 'a', // a con tilde
            '\u{e9}' => 'e',
            '\u{ed}' => 'i',
            '\u{f3}' => 'o',
            '\u{fa}' | '\u{fc}' => 'u',
            '\u{f1}' => 'n', // ene con virgulilla
            '\u{c1}' => 'A',
            '\u{c9}' => 'E',
            '\u{cd}' => 'I',
            '\u{d3}' => 'O',
            '\u{da}' | '\u{dc}' => 'U',
            '\u{d1}' => 'N',
            otro => otro,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El contrato que declara el propio fichero: las dos columnas tienen que
    /// tener EXACTAMENTE las mismas claves. Una clave suelta en un idioma seria
    /// una palabra que existe a medias.
    #[test]
    fn los_dos_idiomas_tienen_las_mismas_claves() {
        let raiz: toml::Value = INCRUSTADO.parse().expect("la tabla no es TOML");
        let es = raiz["es"].as_table().expect("falta [es]");
        let en = raiz["en"].as_table().expect("falta [en]");

        let solo_es: Vec<_> = es.keys().filter(|k| !en.contains_key(*k)).collect();
        let solo_en: Vec<_> = en.keys().filter(|k| !es.contains_key(*k)).collect();
        assert!(solo_es.is_empty(), "solo en castellano: {:?}", solo_es);
        assert!(solo_en.is_empty(), "solo en ingles: {:?}", solo_en);
    }

    /// El numero que declara `[meta]` y el que hay tienen que coincidir, y los
    /// dos con los simbolos que conoce el compilador. Son tres sitios y por eso
    /// el test mira los tres: cualquier pareja podria estar de acuerdo y
    /// equivocada.
    #[test]
    fn el_numero_de_palabras_cuadra_en_los_tres_sitios() {
        let raiz: toml::Value = INCRUSTADO.parse().unwrap();
        let declaradas = raiz["meta"]["palabras"].as_integer().unwrap() as usize;
        let en_la_tabla = raiz["es"].as_table().unwrap().len();
        let en_el_codigo = Simbolo::TODOS.len();
        assert_eq!(declaradas, en_la_tabla, "[meta].palabras no cuadra con [es]");
        assert_eq!(en_la_tabla, en_el_codigo, "[es] no cuadra con Simbolo::TODOS");
        assert_eq!(en_el_codigo, 49);
    }

    #[test]
    fn el_incrustado_carga() {
        let v = Vocabulario::por_defecto().expect("no carga");
        assert_eq!(v.idioma(), "es");
        assert_eq!(v.cuantas(), 49);
        assert_eq!(v.texto(Simbolo::Funcion), "funcion");
        assert_eq!(v.reconocer("mientras"), Some(Simbolo::Mientras));
        assert_eq!(v.reconocer("alumno"), None);
    }

    /// La afirmacion de la portada: el ingles es una columna. Se comprueba en
    /// vez de creerse.
    #[test]
    fn el_ingles_es_una_columna() {
        let v = Vocabulario::desde_texto(INCRUSTADO, Some("en")).expect("no carga en ingles");
        assert_eq!(v.idioma(), "en");
        assert_eq!(v.cuantas(), 49);
        assert_eq!(v.reconocer("while"), Some(Simbolo::Mientras));
        assert_eq!(v.reconocer("mutable"), Some(Simbolo::Cambiante));
        // Y lo que ya no es palabra clave en ese dialecto:
        assert_eq!(v.reconocer("mientras"), None);
    }

    /// Quien escribe con tildes no tropieza.
    #[test]
    fn la_tilde_es_un_alias() {
        let v = Vocabulario::por_defecto().unwrap();
        assert_eq!(v.reconocer("funci\u{f3}n"), Some(Simbolo::Funcion));
        assert_eq!(v.reconocer("qu\u{ed}za"), Some(Simbolo::Quiza));
        // Y un nombre cualquiera con tilde sigue sin ser palabra clave.
        assert_eq!(v.reconocer("a\u{f1}o"), None);
    }

    /// Un texto repetido taparia un simbolo en silencio.
    #[test]
    fn dos_palabras_iguales_se_denuncian() {
        let roto = "[meta]\nidioma_por_defecto = \"x\"\n[x]\nSI = \"si\"\nSINO = \"si\"\n";
        match Vocabulario::desde_texto(roto, None) {
            Err(Rotura::TextoRepetido { texto, .. }) => assert_eq!(texto, "si"),
            // Falta el resto de palabras, asi que puede fallar antes por eso:
            // lo que no puede es aceptarlo.
            Err(Rotura::FaltaPalabra { .. }) => {}
            otro => panic!("acepto dos palabras iguales: {:?}", otro),
        }
    }

    #[test]
    fn un_idioma_que_no_esta_se_dice() {
        match Vocabulario::desde_texto(INCRUSTADO, Some("quechua")) {
            Err(Rotura::SinIdioma(i)) => assert_eq!(i, "quechua"),
            otro => panic!("deberia faltar: {:?}", otro),
        }
    }

    /// Si la tabla del repo esta donde dice, el cargador la encuentra. Si no
    /// esta (un checkout raro), cae al incrustado en vez de morir.
    #[test]
    fn cargar_de_las_raices_no_puede_fallar() {
        let (v, origen) = Vocabulario::cargar(&Roots::find());
        let v = v.expect("ni la tabla ni el incrustado cargaron");
        assert_eq!(v.cuantas(), 49);
        match origen {
            Origen::Fichero(p) => assert!(p.ends_with(RUTA)),
            Origen::Incrustado => {}
        }
    }
}
