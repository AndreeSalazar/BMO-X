//! `disposicion` -- cuanto mide cada cosa y donde esta cada campo.
//!
//! ## Que problema resuelve, dicho con el sintoma
//!
//! Antes de este modulo, `p.x` se bajaba a `p`. El campo se **ignoraba**, sin un
//! aviso, y el programa compilaba. Y `a[i]` se bajaba a la DIRECCION del
//! elemento en vez de a su valor, asi que leer un array daba punteros.
//!
//! Las dos cosas son el mismo agujero: **INTI no sabia cuanto mide nada**. Sin
//! eso no hay campo que valga, porque un campo es una direccion mas un
//! desplazamiento, y el desplazamiento sale de las medidas de los de antes.
//!
//! ## ** Por que es un modulo aparte, y no unas lineas dentro del descenso
//!
//! Por lo mismo que `perfil` no vive dentro de `sintaxis`: son dos trabajos.
//!
//! ```text
//!    disposicion   COMPRUEBA y calcula el plano   -> avisos
//!    ir            BAJA usando el plano           -> instrucciones
//! ```
//!
//! Si el descenso comprobara, tendria que decidir que hacer con lo que no
//! cuadra **mientras esta emitiendo**, que es como se acaba emitiendo algo
//! plausible para un programa que esta mal. Asi el descenso recibe un plano ya
//! validado y no tiene ninguna decision que tomar.
//!
//! ## Lo que este modulo NO sabe
//!
//! Ninguna maquina. Las medidas salen de `medidas.toml`, y la unica razon por
//! la que eso importa: **el dia que INTI compile para 32 bits, lo que cambia es
//! `puntero = 4` en una tabla**, no un fichero de Rust.

use std::collections::HashMap;

use bmo_mods::Roots;

use crate::arbol::{Bloque, Decl, Expr, Funcion, Modulo, Repeticion, Sent, Tipo};
use crate::aviso::{codigos, Aviso, Cosecha, Sitio};

pub const RUTA: &str = "lang/inti/medidas.toml";

const INCRUSTADA: &str = include_str!("../../../../forge/sem-asm/tables/lang/inti/medidas.toml");

/// Cuanto mide cada tipo con nombre.
#[derive(Debug, Clone, Default)]
pub struct Medidas {
    bytes: HashMap<String, u32>,
    /// Los tipos que se operan con la aritmetica de coma flotante.
    ///
    /// ** Es una LISTA y no un `match` por lo mismo que las medidas: el dia que
    /// haya una maquina sin coma flotante, la lista se queda vacia y `a + b` de
    /// flotantes deja de compilar -- en vez de compilar a algo que el silicio no
    /// tiene. El compilador no se entera de que cambio nada.
    flotantes: Vec<String>,
    /// Los que se operan con la aritmetica de enteros. Las dos listas juntas
    /// son el catalogo de conversiones que el lenguaje admite.
    enteros: Vec<String>,
}

impl Medidas {
    pub fn por_defecto() -> Self {
        Self::desde_texto(INCRUSTADA)
    }

    pub fn cargar(raices: &Roots) -> Self {
        match raices.locate(RUTA).and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(t) => Self::desde_texto(&t),
            None => Self::por_defecto(),
        }
    }

    /// Las medidas que dice ESTE texto de tabla.
    ///
    /// ** Es publica por una sola razon, y es la que justifica el modulo
    /// entero: `tests/segunda_maquina.rs` necesita darle a INTI la tabla de una
    /// maquina que no existe y comprobar que el compilador la obedece. Sin esta
    /// puerta, la frase *"cambiar de maquina es cambiar una tabla"* solo se
    /// podria comprobar teniendo la segunda maquina -- o sea, nunca hasta que
    /// fuera tarde para arreglarlo.
    pub fn desde_tabla(t: &str) -> Self {
        Self::desde_texto(t)
    }

    fn desde_texto(t: &str) -> Self {
        let raiz: toml::Value = match t.parse() {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut bytes = HashMap::new();
        // Las dos secciones se juntan en una sola tabla a proposito: la
        // diferencia entre "mide 4 en todas partes" y "mide 4 en esta maquina"
        // es del que ESCRIBE la tabla, no del que la lee. Al leerla, un tipo
        // mide lo que mide.
        for seccion in ["bytes", "bytes_de_esta_maquina"] {
            if let Some(t) = raiz.get(seccion).and_then(|v| v.as_table()) {
                for (k, v) in t {
                    if let Some(n) = v.as_integer() {
                        bytes.insert(k.clone(), n as u32);
                    }
                }
            }
        }
        let lista = |cual: &str| -> Vec<String> {
            raiz.get("clase")
                .and_then(|v| v.get(cual))
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            bytes,
            flotantes: lista("flotantes"),
            enteros: lista("enteros"),
        }
    }

    /// Este tipo, se opera con coma flotante?
    pub fn es_flotante(&self, nombre: &str) -> bool {
        self.flotantes.iter().any(|f| f == nombre)
    }

    /// Este tipo, se opera con enteros?
    pub fn es_entero(&self, nombre: &str) -> bool {
        self.enteros.iter().any(|f| f == nombre)
    }

    /// Todos los nombres que son una conversion.
    ///
    /// ** Existe para el analisis de NOMBRES, y ese es el punto: `flotante64(n)`
    /// se escribe como una llamada, asi que quien busca nombres desconocidos
    /// tiene que saber que existe -- o denuncia `flotante64` como un error de
    /// escritura. Le llega desde aqui, de la MISMA tabla que decide como se
    /// baja, y no de una segunda lista en otro fichero.
    ///
    /// Dos listas de lo mismo acaban discrepando, y el dia que discrepen el
    /// compilador aprobaria un nombre que luego no sabe emitir.
    pub fn conversiones(&self) -> Vec<String> {
        let mut v = self.flotantes.clone();
        v.extend(self.enteros.iter().cloned());
        v
    }

    /// Este nombre, es una conversion? `flotante64(n)`, `entero32(f)`.
    ///
    /// ** Se escribe como una llamada porque asi se lee, no porque lo sea. La
    /// gramatica no gasta ni una regla en esto -- es la decision 7 de
    /// `GRAMATICA.md` aplicada otra vez: si algo se lee bien con la forma que
    /// ya existe, no se le inventa una.
    pub fn es_conversion(&self, nombre: &str) -> bool {
        self.es_flotante(nombre) || self.es_entero(nombre)
    }

    pub fn de(&self, nombre: &str) -> Option<u32> {
        self.bytes.get(nombre).copied()
    }
}

/// Donde esta un campo dentro de su registro, y cuanto ocupa.
#[derive(Debug, Clone)]
pub struct Hueco {
    pub desplazamiento: u32,
    pub medida: u32,
    pub tipo: Tipo,
}

/// Un registro, ya medido.
#[derive(Debug, Clone)]
pub struct Registro {
    pub medida: u32,
    pub alineacion: u32,
    campos: Vec<(String, Hueco)>,
}

impl Registro {
    pub fn campo(&self, nombre: &str) -> Option<&Hueco> {
        self.campos.iter().find(|(n, _)| n == nombre).map(|(_, h)| h)
    }

    /// Los campos en el orden en que se escribieron.
    ///
    /// ** En el orden ESCRITO, y no reordenados por medida para ahorrar huecos.
    /// Reordenar ahorraria bytes y romperia la unica cosa que un registro
    /// promete de verdad: que su disposicion se pueda predecir mirando el
    /// fuente. Un `registro` de INTI tiene que poder describir algo que ya
    /// existe --una estructura del kernel, una cabecera de fichero-- y para eso
    /// el orden es el contrato.
    pub fn campos(&self) -> &[(String, Hueco)] {
        &self.campos
    }
}

/// El plano del modulo: lo que mide cada tipo y donde esta cada campo.
#[derive(Debug, Clone, Default)]
pub struct Plano {
    medidas: Medidas,
    registros: HashMap<String, Registro>,
}

impl Plano {
    /// Cuanto mide un tipo. `None` si no se puede saber.
    ///
    /// ** `None` no es "cero": es *"no lo se"*, y quien pregunte tiene que
    /// denunciarlo en vez de seguir con un numero inventado. Un desplazamiento
    /// calculado con una medida inventada apunta a un sitio que existe, y por
    /// eso el programa no se rompe -- hace otra cosa.
    pub fn medida_de(&self, t: &Tipo) -> Option<u32> {
        match t {
            Tipo::Nombre(n) => self
                .medidas
                .de(n)
                .or_else(|| self.registros.get(n).map(|r| r.medida)),
            // Un bufer es una direccion. Lo que mide su ELEMENTO se pregunta
            // con `elemento`, que es otra pregunta.
            Tipo::Bufer(_) => self.medidas.de("bufer"),
            _ => None,
        }
    }

    pub fn alineacion_de(&self, t: &Tipo) -> Option<u32> {
        match t {
            Tipo::Nombre(n) => match self.registros.get(n) {
                Some(r) => Some(r.alineacion),
                None => self.medidas.de(n),
            },
            Tipo::Bufer(_) => self.medidas.de("bufer"),
            _ => None,
        }
    }

    pub fn registro(&self, nombre: &str) -> Option<&Registro> {
        self.registros.get(nombre)
    }

    /// Este nombre, es una conversion de numero?
    pub fn es_conversion(&self, nombre: &str) -> bool {
        self.medidas.es_conversion(nombre)
    }

    /// Y de las dos clases, a cual convierte.
    pub fn convierte_a_flotante(&self, nombre: &str) -> bool {
        self.medidas.es_flotante(nombre)
    }

    /// El tipo de una expresion, si esta escrito en algun sitio.
    ///
    /// ** Vive en el plano y no en cada usuario porque **los dos que preguntan
    /// tienen que dar la misma respuesta**: el que comprueba y el que emite. Dos
    /// copias de esta funcion serian dos criterios, y el dia que discreparan el
    /// compilador aprobaria un acceso y emitiria otro.
    pub fn tipo_de(&self, e: &Expr, tipos: &HashMap<String, Tipo>) -> Option<Tipo> {
        match e {
            Expr::Nombre(n, _) => tipos.get(n).cloned(),
            Expr::Campo { que, nombre, .. } => {
                let Tipo::Nombre(r) = self.tipo_de(que, tipos)? else {
                    return None;
                };
                self.registro(&r)?.campo(nombre).map(|h| h.tipo.clone())
            }
            Expr::Indice { que, .. } => {
                let t = self.tipo_de(que, tipos)?;
                self.elemento(&t).map(|(t, _)| t)
            }
            _ => None,
        }
    }

    /// Esta expresion, se opera con coma flotante?
    ///
    /// ## ** Por que la contesta el plano y no el descenso
    ///
    /// Por lo mismo que `tipo_de`: **los dos que preguntan tienen que dar la
    /// misma respuesta**. Si el descenso lo decidiera por su cuenta, el dia que
    /// discrepara del comprobador el compilador aprobaria una suma de enteros y
    /// emitiria una de flotantes -- y los dos bits caben en los mismos ocho
    /// bytes, asi que nadie se enteraria hasta ver un numero raro.
    ///
    /// ## De donde sale la respuesta, en orden
    ///
    /// ```text
    ///    3.5              lleva punto     -> flotante
    ///    flotante64(x)    se pidio        -> flotante
    ///    a + b            si alguno lo es -> flotante
    ///    x                lo dice su tipo -> la tabla decide
    /// ```
    ///
    /// OJO: `a + b` con uno de cada NO es una conversion implicita -- eso lo
    /// prohibe la regla del censo `v05`. Es que si uno es flotante, la
    /// aritmetica es de flotantes; que el otro pueda estar ahi es una pregunta
    /// de tipos, y la contesta quien comprueba, no quien emite.
    pub fn es_flotante(&self, e: &Expr, tipos: &HashMap<String, Tipo>) -> bool {
        matches!(self.clase_de(e, tipos), Some(crate::arbol::Clase::Flotante))
    }

    /// Con que aritmetica se opera esto -- y **`None` cuando no se sabe**.
    ///
    /// ## ** La diferencia entre `None` y `Entero` es la que hace posible
    /// comprobar tipos
    ///
    /// `es_flotante` devuelve un `bool`, asi que un tipo desconocido y un entero
    /// contestan lo mismo: *no*. Para BAJAR sirve --si no consta que es
    /// flotante, se opera con enteros-- pero para COMPROBAR no: denunciar
    /// `a + b` porque uno es flotante y del otro no se sabe nada seria un aviso
    /// que salta de mas, y un aviso que salta de mas se desactiva en una semana.
    ///
    /// ```text
    ///    Some(Flotante)   consta, y es de coma flotante
    ///    Some(Entero)     consta, y es de enteros
    ///    None             no consta -- un literal, o algo sin tipo escrito
    /// ```
    ///
    /// ** Y `None` para un literal es una decision, no un hueco: `a * 2` con `a`
    /// flotante es correcto. El `2` no es "un entero que se convierte": es un
    /// numero que todavia no ha elegido forma. Eso NO es conversion implicita
    /// --lo que `v05` prohibe-- porque no hay dos tipos, hay uno y un literal.
    pub fn clase_de(
        &self,
        e: &Expr,
        tipos: &HashMap<String, Tipo>,
    ) -> Option<crate::arbol::Clase> {
        use crate::arbol::Clase;
        match e {
            // Un literal con punto es de coma flotante en `llano` **porque
            // `decimal` no existe alli**: `biblioteca.toml` lo prohibe por no
            // decir su medida. En `pleno` la respuesta sera la otra, y este
            // modulo no trabaja en `pleno`.
            //
            // Y uno SIN punto no dice de que es: `2` vale en las dos
            // aritmeticas, y por eso contesta `None` y no `Entero`.
            Expr::Numero(n, _) => n.con_punto.then_some(Clase::Flotante),
            Expr::Binaria {
                op,
                izquierda,
                derecha,
                ..
            } => {
                // Comparar da un `logico`, no un numero. Preguntarle su clase
                // de aritmetica no tiene sentido, y contestar `Entero` haria que
                // `si (a < b)` pareciera un entero mal puesto.
                if es_de_comparar(*op) {
                    return None;
                }
                self.clase_de(izquierda, tipos)
                    .or_else(|| self.clase_de(derecha, tipos))
            }
            Expr::Unaria { valor, .. } => self.clase_de(valor, tipos),
            // `flotante64(x)` dice de que es lo que sale, y lo dice el nombre
            // que se escribio. Una conversion en INTI se pide, no se supone.
            Expr::Llamada { que, .. } => match &**que {
                Expr::Nombre(n, _) if self.medidas.es_flotante(n) => Some(Clase::Flotante),
                Expr::Nombre(n, _) if self.medidas.es_entero(n) => Some(Clase::Entero),
                _ => None,
            },
            _ => match self.tipo_de(e, tipos) {
                Some(Tipo::Nombre(n)) if self.medidas.es_flotante(&n) => Some(Clase::Flotante),
                Some(Tipo::Nombre(n)) if self.medidas.es_entero(&n) => Some(Clase::Entero),
                // Un `bufer` es una direccion, y una direccion es un entero.
                Some(Tipo::Bufer(_)) => Some(Clase::Entero),
                _ => None,
            },
        }
    }

    /// La clase que tiene un tipo ESCRITO, sin mirar ninguna expresion.
    pub fn clase_del_tipo(&self, t: &Tipo) -> Option<crate::arbol::Clase> {
        use crate::arbol::Clase;
        match t {
            Tipo::Nombre(n) if self.medidas.es_flotante(n) => Some(Clase::Flotante),
            Tipo::Nombre(n) if self.medidas.es_entero(n) => Some(Clase::Entero),
            Tipo::Bufer(_) => Some(Clase::Entero),
            _ => None,
        }
    }

    /// El tipo y la medida de lo que hay dentro de un `bufer de T`.
    pub fn elemento(&self, t: &Tipo) -> Option<(Tipo, u32)> {
        match t {
            Tipo::Bufer(dentro) => self
                .medida_de(dentro)
                .map(|m| ((**dentro).clone(), m)),
            _ => None,
        }
    }
}

/// Este operador produce un `logico` en vez de un numero?
///
/// ** Lo usan dos preguntas distintas --de que clase es esto, y sirve como
/// condicion-- y por eso esta suelto: dos copias serian dos criterios sobre la
/// misma lista de operadores.
pub fn es_de_comparar(op: crate::arbol::Op) -> bool {
    use crate::arbol::Op;
    matches!(
        op,
        Op::Igual
            | Op::NoEs
            | Op::Menor
            | Op::Mayor
            | Op::MenorIgual
            | Op::MayorIgual
            | Op::EsUn
            | Op::Y
            | Op::O
    )
}

/// Los tipos que se conocen dentro de una funcion, por nombre.
///
/// ## ** De donde salen, y de donde NO
///
/// Solo de lo que esta ESCRITO: los parametros (`p es Punto`) y las
/// declaraciones con tipo (`cambiante m es Punto = ...`). No hay inferencia.
///
/// Es una decision, no una carencia por rellenar despues. Con inferencia,
/// cambiar una linea de arriba cambia en silencio el ancho de un acceso a
/// memoria de veinte lineas mas abajo -- y el compilador no diria nada porque
/// no habria pasado nada raro. En un lenguaje que escribe sistema, **el ancho
/// de un acceso tiene que estar escrito en algun sitio que se pueda leer**.
///
/// Lo que no esta declarado no es un error por si mismo: solo lo es si alguien
/// intenta sacarle un campo o indexarlo.
pub fn tipos_de(f: &Funcion) -> HashMap<String, Tipo> {
    let mut m = HashMap::new();
    for p in &f.parametros {
        if let Some(t) = &p.tipo {
            m.insert(p.nombre.clone(), t.clone());
        }
    }
    recoge_bloque(&f.cuerpo, &mut m);
    m
}

fn recoge_bloque(b: &Bloque, m: &mut HashMap<String, Tipo>) {
    for s in b {
        match s {
            Sent::Asigna {
                destino,
                tipo: Some(t),
                ..
            } => {
                if let Expr::Nombre(n, _) = destino {
                    m.insert(n.clone(), t.clone());
                }
            }
            Sent::Si { ramas, sino, .. } => {
                for (_, cuerpo) in ramas {
                    recoge_bloque(cuerpo, m);
                }
                if let Some(c) = sino {
                    recoge_bloque(c, m);
                }
            }
            Sent::Repite { cuerpo, .. } => recoge_bloque(cuerpo, m),
            // OJO: `para cada x en xs` declara `x`, pero su tipo sale del tipo
            // de `xs`, y eso es una pregunta que este modulo todavia no
            // contesta. Se deja marcado en vez de meter una suposicion: un tipo
            // supuesto aqui elegiria el ancho de un acceso a memoria.
            Sent::ParaCada { cuerpo, .. } => recoge_bloque(cuerpo, m),
            Sent::Crudo { cuerpo, .. } => recoge_bloque(cuerpo, m),
            Sent::Paralelo { cuerpo, .. } => recoge_bloque(cuerpo, m),
            _ => {}
        }
    }
}

/// Mide el modulo entero y comprueba todo acceso a campo y todo indice.
pub fn comprobar(m: &Modulo, medidas: Medidas) -> Cosecha<Plano> {
    let mut plano = Plano {
        medidas,
        registros: HashMap::new(),
    };
    let mut avisos = Vec::new();

    // ** ESTE MODULO SOLO TRABAJA EN `llano`, y no es una excepcion comoda.
    //
    // En `pleno` la pregunta es OTRA. Un campo alli no es "una direccion mas un
    // desplazamiento fijo": un `texto` crece, lo que se guarda es una
    // referencia, y ese modelo no esta construido. Medir un `registro` de
    // `pleno` con las reglas de `llano` daria dos cosas y las dos malas:
    // denunciar `nombre es texto` --que es correcto alli-- y, si no lo
    // denunciara, inventarle una disposicion que luego no sera la suya.
    //
    // En `llano` si es la pregunta correcta, porque `llano` no tiene otra: sin
    // monton, todo lo que se toca es una direccion con una medida.
    //
    // El dia que `pleno` tenga su modelo, la fila que hace falta es
    // `referencia = 8` en `medidas.toml` y esta puerta se abre. Esta aqui
    // arriba, entera y con su motivo, en vez de repartida en `if`s por dentro.
    if !matches!(m.perfil, crate::arbol::Perfil::Llano) {
        return Cosecha::con(plano, avisos);
    }

    // 1. Medir los registros. En el orden en que se declararon, que es lo que
    //    permite que uno lleve otro dentro sin resolver dependencias: si el de
    //    dentro se declaro despues, no se sabe medir, y se dice.
    for d in &m.declaraciones {
        if let Decl::Registro {
            nombre,
            campos,
            sitio,
            ..
        } = d
        {
            let mut huecos: Vec<(String, Hueco)> = Vec::new();
            let mut donde = 0u32;
            let mut alineacion = 1u32;

            for c in campos {
                let Some(t) = &c.tipo else {
                    avisos.push(
                        Aviso::nuevo(
                            codigos::CAMPO_SIN_TIPO,
                            format!("El campo `{}` no dice de que tipo es.", c.nombre),
                            c.sitio,
                        )
                        .con_habia(
                            "Sin tipo no hay medida, y sin medida no se sabe donde empieza \
                             el campo siguiente."
                                .to_string(),
                        )
                        .con_hacer("escribe por ejemplo `x es entero64`"),
                    );
                    continue;
                };
                let (Some(medida), Some(al)) = (plano.medida_de(t), plano.alineacion_de(t)) else {
                    avisos.push(
                        Aviso::nuevo(
                            codigos::SIN_MEDIDA,
                            format!("No se sabe cuanto mide el campo `{}`.", c.nombre),
                            c.sitio,
                        )
                        .con_habia(
                            "Un registro tiene que poder medirse entero en compilacion. Si el \
                             tipo es otro registro, tiene que estar declarado ANTES."
                                .to_string(),
                        )
                        .con_hacer("usa un tipo con medida (`entero64`, `natural32`...)"),
                    );
                    continue;
                };
                // El hueco de alineacion: se sube hasta el siguiente multiplo.
                donde = (donde + al - 1) / al * al;
                huecos.push((
                    c.nombre.clone(),
                    Hueco {
                        desplazamiento: donde,
                        medida,
                        tipo: t.clone(),
                    },
                ));
                donde += medida;
                alineacion = alineacion.max(al);
            }

            // El registro entero se redondea a su propia alineacion, para que
            // uno detras de otro en un array siga cuadrando.
            let medida = (donde + alineacion - 1) / alineacion * alineacion;
            let _ = sitio;
            plano.registros.insert(
                nombre.clone(),
                Registro {
                    medida,
                    alineacion,
                    campos: huecos,
                },
            );
        }
    }

    // 2. Comprobar cada uso.
    for d in &m.declaraciones {
        match d {
            Decl::Funcion(f) => revisa_funcion(f, &plano, &mut avisos),
            Decl::Operacion { funcion, .. } => revisa_funcion(funcion, &plano, &mut avisos),
            Decl::Registro { operaciones, .. } => {
                for f in operaciones {
                    revisa_funcion(f, &plano, &mut avisos);
                }
            }
            Decl::Constante { .. } => {}
        }
    }

    Cosecha::con(plano, avisos)
}

fn revisa_funcion(f: &Funcion, plano: &Plano, avisos: &mut Vec<Aviso>) {
    let tipos = tipos_de(f);
    let mut v = Revision {
        plano,
        tipos: &tipos,
        avisos,
        dentro_de_crudo: false,
    };
    v.bloque(&f.cuerpo);
}

struct Revision<'a> {
    plano: &'a Plano,
    tipos: &'a HashMap<String, Tipo>,
    avisos: &'a mut Vec<Aviso>,
    /// ** Indexar un bufer pide `crudo`, y hay que llevar la cuenta aqui porque
    /// `perfil` --que es quien la lleva para los nombres-- no sabe cuales de
    /// estos indices son bufers y cuales no. Saberlo pide el plano, y el plano
    /// es de este modulo.
    dentro_de_crudo: bool,
}

impl Revision<'_> {
    fn bloque(&mut self, b: &Bloque) {
        for s in b {
            self.sentencia(s);
        }
    }

    fn sentencia(&mut self, s: &Sent) {
        match s {
            Sent::Asigna { destino, valor, .. } => {
                self.expresion(destino);
                self.expresion(valor);
            }
            Sent::Si { ramas, sino, .. } => {
                for (cond, cuerpo) in ramas {
                    self.expresion(cond);
                    self.bloque(cuerpo);
                }
                if let Some(c) = sino {
                    self.bloque(c);
                }
            }
            Sent::Repite { forma, cuerpo, .. } => {
                match forma {
                    Repeticion::Mientras(c) | Repeticion::Veces(c) => self.expresion(c),
                    Repeticion::Siempre => {}
                }
                self.bloque(cuerpo);
            }
            Sent::ParaCada {
                desde,
                hasta,
                cuerpo,
                ..
            } => {
                self.expresion(desde);
                if let Some(h) = hasta {
                    self.expresion(h);
                }
                self.bloque(cuerpo);
            }
            Sent::Crudo { cuerpo, .. } => {
                let antes = self.dentro_de_crudo;
                self.dentro_de_crudo = true;
                self.bloque(cuerpo);
                self.dentro_de_crudo = antes;
            }
            Sent::Paralelo { cuerpo, .. } => self.bloque(cuerpo),
            Sent::Devuelve { valor: Some(e), .. } => self.expresion(e),
            Sent::Expresion(e) => self.expresion(e),
            Sent::Falla { motivo, .. } => self.expresion(motivo),
            _ => {}
        }
    }

    fn expresion(&mut self, e: &Expr) {
        match e {
            Expr::Campo { que, nombre, sitio } => {
                self.expresion(que);
                self.mira_campo(que, nombre, *sitio);
            }
            Expr::Indice { que, indice, sitio } => {
                self.expresion(que);
                self.expresion(indice);
                self.mira_indice(que, *sitio);
            }
            Expr::Binaria {
                op,
                izquierda,
                derecha,
                sitio,
            } => {
                self.expresion(izquierda);
                self.expresion(derecha);
                self.mira_operacion(*op, e, *sitio);
            }
            Expr::Unaria { valor, .. } => self.expresion(valor),
            Expr::Llamada { que, argumentos, .. } => {
                self.expresion(que);
                for a in argumentos {
                    self.expresion(&a.valor);
                }
            }
            _ => {}
        }
    }

    fn tipo_de(&self, e: &Expr) -> Option<Tipo> {
        self.plano.tipo_de(e, self.tipos)
    }

    /// Esta operacion, existe para lo que se le esta dando?
    ///
    /// ** Solo hay una familia que no: **los bits sobre un flotante**. Y no es
    /// una carencia del emisor que ya se anadira -- es que la pregunta no tiene
    /// sentido. Los ocho bytes de un `flotante64` son signo, exponente y
    /// mantisa; `f | 1` no enciende el bit de las unidades de nada, toca el
    /// exponente y devuelve un numero que no se parece a ninguno de los dos.
    ///
    /// El resto SI existen: sumar, restar, multiplicar, dividir y las seis
    /// comparaciones estan todas en IEEE-754 con su resultado escrito.
    fn mira_operacion(&mut self, op: crate::arbol::Op, e: &Expr, sitio: Sitio) {
        use crate::arbol::Op;
        let de_bits = matches!(
            op,
            Op::BitsY
                | Op::BitsO
                | Op::BitsXor
                | Op::DesplazaIzquierda
                | Op::DesplazaDerecha
                | Op::Resto
                | Op::Entre
        );
        if !de_bits || !self.plano.es_flotante(e, self.tipos) {
            return;
        }
        self.avisos.push(
            Aviso::nuevo(
                codigos::FLOTANTE_SIN_BITS,
                "Esta operacion no existe para un numero de coma flotante.".to_string(),
                sitio,
            )
            .con_habia(
                "Los ocho bytes de un flotante son signo, exponente y mantisa, no un                  numero en binario. Operarlos a bits no toca lo que parece que toca."
                    .to_string(),
            )
            .con_hacer("usa `/` para dividir, o convierte a entero primero si lo que quieres son los bits"),
        );
    }

    fn mira_campo(&mut self, que: &Expr, nombre: &str, sitio: Sitio) {
        let Some(t) = self.tipo_de(que) else {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::SIN_MEDIDA,
                    format!("No se sabe de que tipo es esto, asi que `.{}` no se puede resolver.", nombre),
                    sitio,
                )
                .con_habia(
                    "Un campo es una direccion mas un desplazamiento, y el desplazamiento sale \
                     del tipo. Sin el tipo escrito no hay desplazamiento que valga."
                        .to_string(),
                )
                .con_hacer("declara el tipo: `p es Punto`"),
            );
            return;
        };
        let Tipo::Nombre(r) = &t else {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CAMPO_DESCONOCIDO,
                    format!("Esto no es un registro, asi que no tiene `.{}`.", nombre),
                    sitio,
                )
                .con_hacer("los campos solo existen en lo que declara `registro`"),
            );
            return;
        };
        let Some(reg) = self.plano.registro(r) else {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CAMPO_DESCONOCIDO,
                    format!("`{}` no es un registro declarado en este fichero.", r),
                    sitio,
                )
                .con_hacer("declaralo con `registro`, o revisa el nombre"),
            );
            return;
        };
        if reg.campo(nombre).is_none() {
            let tiene: Vec<&str> = reg.campos().iter().map(|(n, _)| n.as_str()).collect();
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CAMPO_DESCONOCIDO,
                    format!("`{}` no tiene ningun campo `{}`.", r, nombre),
                    sitio,
                )
                .con_habia(format!("Tiene: {}.", tiene.join(", ")))
                .con_hacer("revisa el nombre del campo"),
            );
        }
    }

    fn mira_indice(&mut self, que: &Expr, sitio: Sitio) {
        let Some(t) = self.tipo_de(que) else {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::SIN_MEDIDA,
                    "No se sabe de que tipo es esto, asi que no se puede indexar.".to_string(),
                    sitio,
                )
                .con_habia(
                    "Un indice es una direccion mas el numero por LA MEDIDA DEL ELEMENTO. Sin \
                     saber que hay dentro, no hay medida."
                        .to_string(),
                )
                .con_hacer("declara el tipo: `pantalla es bufer de natural32`"),
            );
            return;
        };
        // ** Un bufer NO lleva su longitud, asi que no hay contra que
        // comprobar el indice. No es que la comprobacion se haya olvidado: no
        // existe informacion para hacerla, y por eso esto tiene que ir dentro
        // de `crudo` -- que es justo lo que `crudo` significa.
        //
        // `lista de T` si lleva longitud, y por eso `pleno` la comprueba y no
        // pide `crudo`. La misma regla de siempre: al otro lado, hay alguien
        // que comprueba?
        if self.plano.elemento(&t).is_some() && !self.dentro_de_crudo {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::METAL_SIN_CRUDO,
                    "Indexar un `bufer` tiene que ir dentro de un bloque `crudo`.".to_string(),
                    sitio,
                )
                .con_habia(
                    "Un `bufer` no lleva su longitud dentro, asi que no hay contra que \
                     comprobar el indice. `lista de <tipo>` si la lleva, y esa no pide \
                     `crudo` -- pero es de `pleno`."
                        .to_string(),
                )
                .con_hacer("mete la linea dentro de un bloque `crudo`"),
            );
        }
        if self.plano.elemento(&t).is_none() {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CAMPO_DESCONOCIDO,
                    "Esto no se puede indexar.".to_string(),
                    sitio,
                )
                .con_habia(
                    "En `llano` lo que se indexa es un `bufer de <tipo>`. `lista de <tipo>` \
                     lleva su longitud dentro y es de `pleno`."
                        .to_string(),
                )
                .con_hacer("declaralo como `bufer de <tipo>`"),
            );
        }
    }
}

#[cfg(test)]
mod pruebas;
