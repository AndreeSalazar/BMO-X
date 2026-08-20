//! `nombres` -- quien es cada nombre, y si se puede cambiar.
//!
//! ## Que hace
//!
//! Lleva la cuenta de los nombres vivos y contesta tres preguntas:
//!
//! ```text
//!    este nombre, existe?          -> si no, "no se que es `x`" con sugerencia
//!    se puede cambiar?             -> E0030 si no se declaro `cambiante`
//!    se declaro con un valor?      -> E0031 si no
//! ```
//!
//! Es lo que el parser **no podia** hacer: para saber si algo es `cambiante`
//! hay que recordar lo que se declaro antes, y el parser avanza.
//!
//! ## El alcance, dicho entero
//!
//! ```text
//!    un nombre pertenece al bloque donde NACIO, y muere al salir de el
//!    un bloque interior LEE y ESCRIBE lo de su funcion
//!    ninguna funcion escribe lo de otra                <- y por eso...
//!    ...no existen `global` ni `nonlocal`
//!    lo de nivel superior se CONGELA: escribirlo es E0032
//! ```
//!
//! ** Con esas cinco lineas, `UnboundLocalError` --la sorpresa 7 de Python-- no
//! tiene donde aparecer, y las dos palabras clave que Python necesito para
//! taparla no hacen falta.
//!
//! ## OJO: La correccion del 2026-08-19, y la cazo el censo
//!
//! La segunda linea decia antes *"un bloque interior NO puede escribir fuera"*,
//! con ambito por BLOQUE. Suena mas estricto y por tanto mejor. **Rompia el
//! bucle mas basico del lenguaje**:
//!
//! ```text
//!    cambiante quedan = cierto
//!    repite mientras quedan
//!        quedan = falso        <- prohibido, y entonces el bucle no termina
//! ```
//!
//! El ambito es de **FUNCION**, no de bloque, y eso no debilita nada: como no
//! hay funciones anidadas (`E0101`), *"escribir fuera de mi funcion"* solo
//! puede significar tocar lo de nivel superior -- que esta congelado y tiene su
//! propio codigo. **La regla se cumple sola por una decision que ya estaba
//! tomada.**
//!
//! ## Y la parte de facilidad: "quisiste decir"
//!
//! Cuando un nombre no existe, se busca el mas parecido entre los que si. Es la
//! caracteristica de Rust y Elm que mas se cita cuando alguien explica por que
//! sus errores se entienden, y aqui sale casi gratis porque **la biblioteca
//! comun esta en una tabla**: hay una lista contra la que comparar.

use std::collections::HashMap;

use bmo_mods::Roots;

use crate::arbol::*;
use crate::aviso::{codigos, Aviso, Cosecha, Sitio};
// ** La tabla de modulos se MUDO a `tablas`, y no por gusto: la necesitan tres
// piezas de esta misma generacion, asi que tenerla aqui las ataba entre si.
// Lo dijo `tests/linaje.rs` antes que nadie. Ver `tablas/mod.rs`.
pub use crate::tablas::Modulos;

pub const RUTA: &str = "lang/inti/comun.toml";

const INCRUSTADA: &str = include_str!("../../../../forge/sem-asm/tables/lang/inti/comun.toml");


/// Los nombres que estan sin pedirlos.
///
/// Salen de `comun.toml` porque **la facilidad de un lenguaje vive en su
/// biblioteca, no en su gramatica**: es la leccion de Python mirada de cerca, y
/// la de ABC al reves.
#[derive(Debug, Clone, Default)]
pub struct Comun {
    ambos: Vec<String>,
    solo_pleno: Vec<String>,
    pueden_fallar: Vec<String>,
    modifican: Vec<String>,
}

impl Comun {
    pub fn por_defecto() -> Self {
        Self::desde_texto(INCRUSTADA)
    }

    pub fn cargar(raices: &Roots) -> Self {
        match raices.locate(RUTA).and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(t) => Self::desde_texto(&t),
            None => Self::por_defecto(),
        }
    }

    fn desde_texto(t: &str) -> Self {
        let raiz: toml::Value = match t.parse() {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let claves = |seccion: &str| -> Vec<String> {
            raiz.get(seccion)
                .and_then(|v| v.as_table())
                .map(|t| t.keys().cloned().collect())
                .unwrap_or_default()
        };
        let lista = |seccion: &str, clave: &str| -> Vec<String> {
            raiz.get(seccion)
                .and_then(|s| s.get(clave))
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            ambos: claves("ambos"),
            solo_pleno: claves("pleno"),
            pueden_fallar: lista("avisos", "pueden_fallar"),
            modifican: lista("avisos", "modifican_la_coleccion"),
        }
    }

    /// Las de la biblioteca que pueden fallar.
    ///
    /// Salen de la tabla porque son un dato **sobre la biblioteca**: quien
    /// anada una que falla lo dice ahi, y la comprobacion queda cubierta sin
    /// tocar el compilador.
    pub fn pueden_fallar(&self) -> &[String] {
        &self.pueden_fallar
    }

    /// Esta operacion modifica la coleccion que recibe?
    pub fn modifica(&self, nombre: &str) -> bool {
        self.modifican.iter().any(|s| s == nombre)
    }

    /// Los nombres disponibles en un perfil.
    pub fn en(&self, perfil: Perfil) -> Vec<&str> {
        let mut v: Vec<&str> = self.ambos.iter().map(|s| s.as_str()).collect();
        if perfil == Perfil::Pleno {
            v.extend(self.solo_pleno.iter().map(|s| s.as_str()));
        }
        v
    }

    /// Existe, pero en el otro perfil. Sirve para dar el aviso bueno: *"`cuenta`
    /// existe, pero no en `llano`"* dice mucho mas que *"no se que es"*.
    pub fn solo_en_pleno(&self, nombre: &str) -> bool {
        self.solo_pleno.iter().any(|s| s == nombre)
    }
}

/// Lo que se sabe de un nombre declarado.
#[derive(Debug, Clone, Copy)]
struct Ficha {
    cambiante: bool,
    sitio: Sitio,
    /// Nacio en un bloque de fuera del que se esta mirando ahora.
    de_fuera: bool,
}

/// Comprueba los nombres de un modulo.
pub fn comprobar(m: &Modulo, comun: &Comun, extra: &[String]) -> Cosecha<()> {
    let mut v = Vigia {
        perfil: m.perfil,
        comun,
        avisos: Vec::new(),
        ambitos: Vec::new(),
        conocidos: Vec::new(),
        recorriendo: Vec::new(),
        pueden_fallar: Vec::new(),
        la_funcion_falla: false,
    };

    // Quien puede fallar se sabe ANTES de mirar ningun cuerpo: si no, una
    // funcion que llama a otra declarada mas abajo se libraria.
    for d in &m.declaraciones {
        let f = match d {
            Decl::Funcion(f) => f,
            Decl::Operacion { funcion, .. } => funcion,
            _ => continue,
        };
        if f.retorno.as_ref().map(|r| r.puede_fallar).unwrap_or(false) {
            v.pueden_fallar.push(f.nombre.clone());
        }
    }
    for n in comun.pueden_fallar() {
        v.pueden_fallar.push(n.to_string());
    }

    // El ambito del modulo: lo de nivel superior y lo que traen los `usa`.
    v.entra();
    for n in comun.en(m.perfil) {
        v.declara_de_fuera(n);
    }
    for n in extra {
        v.declara_de_fuera(n);
    }
    for d in &m.declaraciones {
        v.declara_de_fuera(d.nombre());
        if let Decl::Constante { nombre, sitio, .. } = d {
            v.conocidos.push(nombre.clone());
            let _ = sitio;
        }
    }

    for d in &m.declaraciones {
        v.declaracion(d);
    }
    v.sale();

    Cosecha::con((), v.avisos)
}

struct Vigia<'c> {
    perfil: Perfil,
    comun: &'c Comun,
    avisos: Vec<Aviso>,
    ambitos: Vec<HashMap<String, Ficha>>,
    conocidos: Vec<String>,
    /// Las colecciones que se estan recorriendo ahora mismo.
    ///
    /// Es una pila porque los bucles se anidan, y hay que saber **todas** las
    /// abiertas: tocar la del bucle de fuera desde el de dentro es igual de
    /// malo que tocar la propia.
    recorriendo: Vec<String>,
    /// Las funciones que pueden fallar: las que declararon `o error` y las que
    /// la tabla marca.
    pueden_fallar: Vec<String>,
    /// La funcion que se esta mirando declaro `o error`? Solo entonces
    /// `devuelve f(...)` cuenta como mirar el resultado: lo pasa a quien llamo.
    la_funcion_falla: bool,
}

impl<'c> Vigia<'c> {
    /// Entra en un BLOQUE: lo de la funcion se ve y **se puede escribir**. Lo
    /// que se declare aqui muere al salir.
    fn entra(&mut self) {
        let nuevo = self.ambitos.last().cloned().unwrap_or_default();
        self.ambitos.push(nuevo);
    }

    /// Entra en una FUNCION: lo de fuera se ve y **no se puede escribir**,
    /// porque lo unico que hay fuera de una funcion es el nivel superior, y el
    /// nivel superior esta congelado.
    fn entra_funcion(&mut self) {
        let mut nuevo = HashMap::new();
        if let Some(anterior) = self.ambitos.last() {
            for (k, f) in anterior {
                nuevo.insert(k.clone(), Ficha { de_fuera: true, ..*f });
            }
        }
        self.ambitos.push(nuevo);
    }

    fn sale(&mut self) {
        self.ambitos.pop();
    }

    fn declara_de_fuera(&mut self, nombre: &str) {
        if let Some(a) = self.ambitos.last_mut() {
            a.insert(
                nombre.to_string(),
                Ficha {
                    cambiante: false,
                    sitio: Sitio::default(),
                    de_fuera: true,
                },
            );
        }
    }

    fn declara(&mut self, nombre: &str, cambiante: bool, sitio: Sitio) {
        if let Some(a) = self.ambitos.last_mut() {
            a.insert(
                nombre.to_string(),
                Ficha {
                    cambiante,
                    sitio,
                    de_fuera: false,
                },
            );
        }
    }

    fn ficha(&self, nombre: &str) -> Option<Ficha> {
        self.ambitos.last().and_then(|a| a.get(nombre)).copied()
    }

    fn declaracion(&mut self, d: &Decl) {
        match d {
            Decl::Constante { valor, .. } => self.expresion(valor),
            Decl::Registro { operaciones, .. } => {
                for f in operaciones {
                    self.funcion(f);
                }
            }
            Decl::Funcion(f) => self.funcion(f),
            Decl::Operacion { funcion, .. } => self.funcion(funcion),
        }
    }

    fn funcion(&mut self, f: &Funcion) {
        let antes = self.la_funcion_falla;
        self.la_funcion_falla = f.retorno.as_ref().map(|r| r.puede_fallar).unwrap_or(false);
        self.entra_funcion();
        for p in &f.parametros {
            // Un parametro nace en la funcion, no fuera: por eso `de_fuera` es
            // falso y por eso cambiarlo sin `cambiante` se puede denunciar con
            // su codigo propio.
            self.declara(&p.nombre, p.cambiante, p.sitio);
        }
        self.bloque_sin_ambito(&f.cuerpo);
        self.sale();
        self.la_funcion_falla = antes;
    }

    fn bloque(&mut self, b: &Bloque) {
        self.entra();
        self.bloque_sin_ambito(b);
        self.sale();
    }

    fn bloque_sin_ambito(&mut self, b: &Bloque) {
        for s in b {
            self.sentencia(s);
        }
    }

    fn sentencia(&mut self, s: &Sent) {
        match s {
            Sent::Asigna {
                destino,
                cambiante,
                valor,
                sitio,
                ..
            } => {
                self.expresion_mirada(valor);
                self.asigna(destino, *cambiante, *sitio);
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
            Sent::ParaCada {
                nombre,
                desde,
                hasta,
                cuerpo,
                sitio,
            } => {
                self.expresion(desde);
                if let Some(h) = hasta {
                    self.expresion(h);
                }
                self.entra();
                // El nombre del bucle nace en el bucle y no se puede cambiar
                // dentro: cambiarlo seria pelearse con el que recorre.
                self.declara(nombre, false, *sitio);
                // La coleccion queda marcada como "en uso" mientras dure.
                if let Expr::Nombre(coleccion, _) = desde {
                    self.recorriendo.push(coleccion.clone());
                }
                self.bloque_sin_ambito(cuerpo);
                if matches!(desde, Expr::Nombre(..)) {
                    self.recorriendo.pop();
                }
                self.sale();
            }
            Sent::Repite { forma, cuerpo, .. } => {
                match forma {
                    Repeticion::Veces(e) | Repeticion::Mientras(e) => self.expresion(e),
                    Repeticion::Siempre => {}
                }
                self.bloque(cuerpo);
            }
            Sent::Devuelve { valor, .. } => {
                if let Some(e) = valor {
                    // Devolver un resultado que falla SOLO cuenta como mirarlo
                    // si esta funcion tambien declaro `o error`: entonces no lo
                    // esta ignorando, lo esta pasando.
                    if self.la_funcion_falla {
                        self.expresion_mirada(e);
                    } else {
                        self.expresion(e);
                    }
                }
            }
            Sent::Falla { motivo, .. } => self.expresion(motivo),
            Sent::Corta(_) | Sent::Continua(_) => {}
            Sent::Crudo { cuerpo, .. } | Sent::Paralelo { cuerpo, .. } => self.bloque(cuerpo),
            Sent::Expresion(e) => self.expresion(e),
        }
    }

    /// Una expresion en una posicion donde **si** se mira el resultado.
    ///
    /// Son tres, y no hay una cuarta:
    ///
    /// ```text
    ///    r = divide(a, b)              se guarda para mirarlo despues
    ///    divide(a, b) o si no 0        se mira ahora mismo
    ///    devuelve divide(a, b)         se pasa a quien llamo... si la funcion
    ///                                  tambien declaro `o error`
    /// ```
    ///
    /// ** La comprobacion es de POSICION y no de nivel superior, que es donde
    /// estuvo mal media hora: `escribe(divide(10, 0))` esconde la llamada
    /// dentro de otra, y es exactamente el caso que hay que cazar -- pasarle a
    /// alguien un valor que puede no existir.
    fn expresion_mirada(&mut self, e: &Expr) {
        match e {
            Expr::Llamada {
                que, argumentos, ..
            } => {
                self.expresion(que);
                for a in argumentos {
                    self.expresion(&a.valor);
                }
            }
            otra => self.expresion(otra),
        }
    }

    /// Una llamada que puede fallar, escrita donde nadie mira el resultado.
    ///
    /// Es la promesa mas fuerte del sistema de errores: **ignorar un error es
    /// un error de COMPILACION**. Sin esto, `o si no` seria una costumbre en
    /// vez de una regla, y el `except:` pelado de Python volveria por la puerta
    /// de atras: no escribir nada.
    fn quiza_ignora_un_error(&mut self, que: &Expr, sitio: Sitio) {
        {
            if let Expr::Nombre(n, _) = que {
                if self.pueden_fallar.iter().any(|x| x == n) {
                    self.avisos.push(
                        Aviso::nuevo(
                            codigos::ERROR_IGNORADO,
                            format!("`{}` puede fallar, y aqui nadie mira el resultado.", n),
                            sitio,
                        )
                        .con_habia(
                            "En INTI un error es un DATO, no algo que salta solo. Si nadie lo mira, el programa sigue como si nada con un valor que no existe."
                                .to_string(),
                        )
                        .con_hacer(format!(
                            "guardalo y mira `si fallo`, o escribe `{}(...) o si no <valor>`",
                            n
                        )),
                    );
                }
            }
        }
    }

    fn asigna(&mut self, destino: &Expr, cambiante: bool, sitio: Sitio) {
        let nombre = match destino {
            Expr::Nombre(n, _) => n.clone(),
            // `p.x = 3` y `a[i] = 3` tocan lo de dentro, no el nombre: aqui solo
            // se mira que el nombre exista.
            otro => {
                self.expresion(otro);
                return;
            }
        };

        match self.ficha(&nombre) {
            None => {
                // Nace aqui.
                self.declara(&nombre, cambiante, sitio);
            }
            Some(f) if cambiante => {
                // Volver a declarar tapa lo de fuera, y eso vale; declarar dos
                // veces en el mismo bloque, no.
                // ** Redeclarar algo que viene de FUERA (la biblioteca comun o
                // el nivel superior) vale y lo tapa: `cambiante suma = 0` tiene
                // que poder escribirse aunque exista `suma` en la biblioteca.
                // Si no, cincuenta nombres comunes dejarian de servir como
                // variables, que es un precio absurdo por una lista de ayuda.
                //
                // Lo que NO vale es redeclarar algo de la MISMA funcion: ahi
                // habria dos con el mismo nombre y el lector no sabria cual.
                if !f.de_fuera {
                    self.avisos.push(
                        Aviso::nuevo(
                            codigos::NO_ES_CAMBIANTE,
                            format!("`{}` ya existe en esta funcion.", nombre),
                            sitio,
                        )
                        .con_habia(format!("Nacio en la linea {}.", f.sitio.linea))
                        .con_hacer("quita el `cambiante`, o llamalo de otra forma"),
                    );
                }
                self.declara(&nombre, true, sitio);
            }
            Some(f) if f.de_fuera => {
                // ** Viene de fuera de la FUNCION, y lo unico que hay fuera de
                // una funcion es el nivel superior: congelado al cargar el
                // modulo. Por eso no hacen falta `global` ni `nonlocal`.
                self.avisos.push(
                    Aviso::nuevo(
                        codigos::CONGELADO,
                        format!("`{}` es de fuera de esta funcion y esta congelado.", nombre),
                        sitio,
                    )
                    .con_habia(
                        "Lo de nivel superior se congela cuando el modulo termina de cargarse, \
                         y por eso se puede prestar a otra tarea sin cerrojos. Por eso INTI no \
                         necesita `global` ni `nonlocal`."
                            .to_string(),
                    )
                    .con_hacer("devuelvelo desde la funcion y asignalo donde haga falta"),
                );
            }
            Some(f) if !f.cambiante => {
                self.avisos.push(
                    Aviso::nuevo(
                        codigos::NO_ES_CAMBIANTE,
                        format!("`{}` se fijo y no se puede cambiar.", nombre),
                        sitio,
                    )
                    .con_habia(format!(
                        "Se le dio su valor en la linea {}, sin `cambiante`.",
                        f.sitio.linea
                    ))
                    .con_hacer(format!("escribe `cambiante {} = ...` donde nace", nombre)),
                );
            }
            Some(_) => {}
        }
    }

    fn expresion(&mut self, e: &Expr) {
        match e {
            Expr::Nombre(n, sitio) => self.usa(n, *sitio),
            Expr::Lista(v, _) => {
                for x in v {
                    self.expresion(x);
                }
            }
            Expr::Tabla(v, _) => {
                for (k, val) in v {
                    self.expresion(k);
                    self.expresion(val);
                }
            }
            Expr::Binaria {
                izquierda, derecha, ..
            } => {
                self.expresion(izquierda);
                self.expresion(derecha);
            }
            Expr::Unaria { valor, .. } => self.expresion(valor),
            Expr::Llamada {
                que,
                argumentos,
                sitio,
            } => {
                self.quiza_muta_iterando(que, argumentos, *sitio);
                self.quiza_ignora_un_error(que, *sitio);
                self.expresion(que);
                for a in argumentos {
                    self.expresion(&a.valor);
                }
            }
            Expr::Indice { que, indice, .. } => {
                self.expresion(que);
                self.expresion(indice);
            }
            Expr::Campo { que, .. } => self.expresion(que),
            Expr::OSiNo {
                intento, respaldo, ..
            } => {
                // `o si no` es exactamente mirar el resultado.
                self.expresion_mirada(intento);
                match respaldo {
                    Respaldo::Valor(v) => self.expresion(v),
                    Respaldo::Bloque(b) => self.bloque(b),
                }
            }
            _ => {}
        }
    }

    /// Tocar la coleccion que se esta recorriendo: el bug clasico de borrar
    /// mientras se itera. Aqui **no compila**.
    ///
    /// La lista de lo que cuenta como "tocar" sale de la biblioteca y no de una
    /// constante escrita aqui: quien anada una operacion que modifica una lista
    /// lo dice en la tabla, y esta comprobacion queda cubierta sola.
    fn quiza_muta_iterando(&mut self, que: &Expr, argumentos: &[Argumento], sitio: Sitio) {
        let nombre = match que {
            Expr::Nombre(n, _) => n,
            _ => return,
        };
        if !self.comun.modifica(nombre) {
            return;
        }
        for a in argumentos {
            if let Expr::Nombre(objetivo, _) = &a.valor {
                if self.recorriendo.iter().any(|c| c == objetivo) {
                    self.avisos.push(
                        Aviso::nuevo(
                            codigos::MUTA_ITERANDO,
                            format!(
                                "`{}` se esta recorriendo ahora mismo, y `{}` la modifica.",
                                objetivo, nombre
                            ),
                            sitio,
                        )
                        .con_habia(
                            "Cambiar una coleccion mientras se recorre es el bug que en otros lenguajes se salta un elemento sin avisar. Aqui no compila."
                                .to_string(),
                        )
                        .con_hacer("junta lo que quieras cambiar en otra lista y hazlo al salir"),
                    );
                    return;
                }
            }
        }
    }

    fn usa(&mut self, nombre: &str, sitio: Sitio) {
        if self.ficha(nombre).is_some() {
            return;
        }

        // ** Existe, pero en el otro perfil. Decirlo asi vale mucho mas que
        // "no se que es": el que escribe ya sabe que existe.
        if self.perfil == Perfil::Llano && self.comun.solo_en_pleno(nombre) {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::LLANO_NO_ADMITE,
                    format!("`{}` existe, pero no en el perfil `llano`.", nombre),
                    sitio,
                )
                .con_habia(
                    "Pide memoria, y `llano` no tiene monton: por eso puede escribir un \
                     manejador de interrupciones."
                        .to_string(),
                )
                .con_hacer("cambia el fichero a `perfil pleno`, o hazlo con medidas fijas"),
            );
            return;
        }

        let mut aviso = Aviso::nuevo(
            codigos::NOMBRE_DESCONOCIDO,
            format!("No se que es `{}`.", nombre),
            sitio,
        );
        match self.parecido(nombre) {
            Some(p) => {
                aviso = aviso
                    .con_habia(format!("Hay uno muy parecido: `{}`.", p))
                    .con_hacer(format!("escribe `{}`", p));
            }
            None => {
                aviso = aviso
                    .con_habia(
                        "No esta declarado en este bloque ni lo trae ningun `usa`.".to_string(),
                    )
                    .con_hacer("declaralo antes, o anade el `usa` del modulo que lo trae");
            }
        }
        self.avisos.push(aviso);
    }

    /// El nombre vivo mas parecido, si hay alguno a distancia 1 o 2.
    ///
    /// Es la caracteristica de Rust y Elm que mas se cita cuando alguien
    /// explica por que sus errores se entienden, y aqui sale casi gratis
    /// **porque la biblioteca comun esta en una tabla**: hay una lista contra
    /// la que comparar.
    fn parecido(&self, nombre: &str) -> Option<String> {
        let mut mejor: Option<(usize, String)> = None;
        let candidatos = self
            .ambitos
            .last()
            .map(|a| a.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        for c in candidatos.iter().chain(self.conocidos.iter()) {
            let d = distancia(nombre, c);
            // Con nombres muy cortos, distancia 2 empareja cualquier cosa.
            let tope = if nombre.len() <= 4 { 1 } else { 2 };
            if d <= tope && mejor.as_ref().map(|(md, _)| d < *md).unwrap_or(true) {
                mejor = Some((d, c.clone()));
            }
        }
        mejor.map(|(_, n)| n)
    }
}

/// Distancia de edicion, en caracteres y no en bytes.
fn distancia(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut fila: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut anterior = fila[0];
        fila[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let sustituir = anterior + usize::from(ca != cb);
            anterior = fila[j + 1];
            fila[j + 1] = sustituir.min(fila[j] + 1).min(fila[j + 1] + 1);
        }
    }
    fila[b.len()]
}

#[cfg(test)]
mod pruebas;
