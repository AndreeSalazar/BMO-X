//! `perfil` -- la frontera entre `llano` y `pleno`, comprobada.
//!
//! ## Que hace
//!
//! Recorre el arbol y contesta una sola pregunta: **esto cabe en el perfil que
//! declaro el fichero?** Nada mas. No sabe si los nombres existen, no sabe si
//! los tipos cuadran, y no emite un solo byte.
//!
//! ## Por que es un modulo y no un `if` dentro del parser
//!
//! Porque el parser **avanza** y esto **decide**. Un `crudo` es sintacticamente
//! igual de valido en los dos perfiles; lo que cambia es si esta permitido, y
//! esa es una pregunta sobre el modulo entero, no sobre la linea.
//!
//! Y porque es la ley que sostiene la promesa mas fuerte del lenguaje: *"en
//! `llano`, usar algo que asigna memoria es un error de compilacion con nombre
//! y sitio, no una sorpresa en ejecucion"*. Una promesa asi no puede vivir
//! repartida.
//!
//! ## La regla, dicha entera
//!
//! ```text
//!    llano                        pleno
//!    ------------------------     ------------------------
//!    sin texto/lista/tabla        todo
//!    tamanos exactos              `numero` vale
//!    `crudo` SI                   `crudo` no (E0071)
//!    `en paralelo` no             `en paralelo` si
//! ```
//!
//! ★ Y la que decide donde hace falta `crudo`: **no marca "bajo nivel", marca
//! "aqui nadie comprueba por ti"**. Por eso `invoca` no lo necesita --al otro
//! lado hay un kernel que valida una capability-- y `entrada_puerto` si.

use std::collections::HashSet;

use bmo_mods::Roots;

use crate::arbol::*;
use crate::aviso::{codigos, Aviso, Cosecha, Sitio};

/// La ruta relativa a una raiz de tablas.
pub const RUTA: &str = "lang/inti/biblioteca.toml";

const INCRUSTADA: &str =
    include_str!("../../../../forge/sem-asm/tables/lang/inti/biblioteca.toml");

/// Lo que el compilador sabe de la biblioteca sin conocerla.
///
/// Sale de `biblioteca.toml` por el mismo motivo que las palabras: **son datos
/// sobre la biblioteca, no sobre el lenguaje**. Si vivieran aqui, anadir una
/// operacion de sistema obligaria a recompilar el compilador.
#[derive(Debug, Clone)]
pub struct Catalogo {
    crecen: HashSet<String>,
    sin_tamano: HashSet<String>,
    piden_crudo: HashSet<String>,
}

impl Catalogo {
    pub fn por_defecto() -> Self {
        Self::desde_texto(INCRUSTADA)
    }

    pub fn cargar(raices: &Roots) -> Self {
        match raices.locate(RUTA).and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(t) => Self::desde_texto(&t),
            None => Self::por_defecto(),
        }
    }

    /// Si la tabla esta rota se usa una vacia **y el analisis deja de acusar**.
    ///
    /// Es a proposito: una tabla ilegible no puede convertirse en "todo esta
    /// prohibido", porque entonces un fichero de datos corrupto pararia
    /// compilaciones correctas y el mensaje hablaria del programa del usuario
    /// en vez de la instalacion.
    fn desde_texto(t: &str) -> Self {
        let raiz: toml::Value = match t.parse() {
            Ok(v) => v,
            Err(_) => return Self::vacio(),
        };
        let lista = |seccion: &str, clave: &str| -> HashSet<String> {
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
            crecen: lista("llano", "tipos_que_crecen"),
            sin_tamano: lista("llano", "tipos_sin_tamano"),
            piden_crudo: lista("crudo", "nombres"),
        }
    }

    fn vacio() -> Self {
        Self {
            crecen: HashSet::new(),
            sin_tamano: HashSet::new(),
            piden_crudo: HashSet::new(),
        }
    }
}

/// Lo que sale del analisis, aparte de los avisos.
#[derive(Debug, Clone, Default)]
pub struct Informe {
    /// Cuantos bloques `crudo` tiene el modulo.
    ///
    /// ★★ Este numero es el que convierte *"cuanto de mi programa esta atado a
    /// esta maquina?"* en un dato. Va al informe del `.bex` para que
    /// `bmo-verify` pueda exigirlo firmado.
    pub bloques_crudo: usize,
}

/// Comprueba un modulo contra su perfil.
pub fn comprobar(m: &Modulo, cat: &Catalogo) -> Cosecha<Informe> {
    let mut v = Vigia {
        perfil: m.perfil,
        cat,
        avisos: Vec::new(),
        informe: Informe::default(),
        dentro_de_crudo: false,
    };

    for d in &m.declaraciones {
        v.declaracion(d);
    }

    Cosecha::con(v.informe, v.avisos)
}

struct Vigia<'c> {
    perfil: Perfil,
    cat: &'c Catalogo,
    avisos: Vec<Aviso>,
    informe: Informe,
    dentro_de_crudo: bool,
}

impl<'c> Vigia<'c> {
    fn llano(&self) -> bool {
        self.perfil == Perfil::Llano
    }

    fn declaracion(&mut self, d: &Decl) {
        match d {
            Decl::Constante { valor, .. } => self.expresion(valor),
            Decl::Registro { campos, .. } => {
                for c in campos {
                    if let Some(t) = &c.tipo {
                        self.tipo(t, c.sitio);
                    }
                    if let Some(e) = &c.defecto {
                        self.expresion(e);
                    }
                }
            }
            Decl::Funcion(f) => self.funcion(f),
            Decl::Operacion { funcion, .. } => self.funcion(funcion),
        }
    }

    fn funcion(&mut self, f: &Funcion) {
        for p in &f.parametros {
            match &p.tipo {
                Some(t) => self.tipo(t, p.sitio),
                None if self.llano() => self.falta_tipo(&p.nombre, p.sitio),
                None => {}
            }
        }
        if let Some(r) = &f.retorno {
            self.tipo(&r.tipo, f.sitio);
        }
        self.bloque(&f.cuerpo);
    }

    fn bloque(&mut self, b: &Bloque) {
        for s in b {
            self.sentencia(s);
        }
    }

    fn sentencia(&mut self, s: &Sent) {
        match s {
            Sent::Asigna { tipo, valor, .. } => {
                if let Some(t) = tipo {
                    self.tipo(t, s.sitio());
                }
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
            Sent::Repite { forma, cuerpo, .. } => {
                match forma {
                    Repeticion::Veces(e) | Repeticion::Mientras(e) => self.expresion(e),
                    Repeticion::Siempre => {}
                }
                self.bloque(cuerpo);
            }
            Sent::Devuelve { valor, .. } => {
                if let Some(e) = valor {
                    self.expresion(e);
                }
            }
            Sent::Falla { motivo, .. } => self.expresion(motivo),
            Sent::Corta(_) | Sent::Continua(_) => {}
            Sent::Crudo { cuerpo, sitio } => self.crudo(cuerpo, *sitio),
            Sent::Paralelo { cuerpo, sitio } => self.paralelo(cuerpo, *sitio),
            Sent::Expresion(e) => self.expresion(e),
        }
    }

    fn crudo(&mut self, cuerpo: &Bloque, sitio: Sitio) {
        if !self.llano() {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::CRUDO_EN_PLENO,
                    "`crudo` no existe en el perfil `pleno`.",
                    sitio,
                )
                .con_habia(
                    "La ventana sin comprobar es del perfil de sistema, y alli se cuenta \
                     y se puede exigir firmada. En `pleno` no hay nada que abrir."
                        .to_string(),
                )
                .con_hacer("si de verdad tocas el metal, empieza el fichero con `perfil llano`"),
            );
        }
        self.informe.bloques_crudo += 1;
        let antes = self.dentro_de_crudo;
        self.dentro_de_crudo = true;
        self.bloque(cuerpo);
        self.dentro_de_crudo = antes;
    }

    fn paralelo(&mut self, cuerpo: &Bloque, sitio: Sitio) {
        if self.llano() {
            self.avisos.push(
                Aviso::nuevo(
                    codigos::LLANO_NO_ADMITE,
                    "`en paralelo` no existe en el perfil `llano`.",
                    sitio,
                )
                .con_habia(
                    "Una tarea necesita su propio monton, y en `llano` no hay monton."
                        .to_string(),
                )
                .con_hacer("cambia el fichero a `perfil pleno`"),
            );
        }
        self.bloque(cuerpo);
    }

    fn expresion(&mut self, e: &Expr) {
        match e {
            Expr::Texto(_, sitio) if self.llano() => {
                self.crece("un texto", *sitio);
            }
            Expr::Lista(v, sitio) => {
                if self.llano() {
                    self.crece("una lista", *sitio);
                }
                for x in v {
                    self.expresion(x);
                }
            }
            Expr::Tabla(v, sitio) => {
                if self.llano() {
                    self.crece("una tabla", *sitio);
                }
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
                que, argumentos, ..
            } => {
                // El nombre se comprueba al visitarlo, no aqui: hacerlo en los
                // dos sitios daba el mismo aviso dos veces, y un aviso repetido
                // es peor que uno que falta -- el lector deja de contar.
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
                self.expresion(intento);
                match respaldo {
                    Respaldo::Valor(v) => self.expresion(v),
                    Respaldo::Bloque(b) => self.bloque(b),
                }
            }
            Expr::Nombre(n, sitio) => self.quiza_pide_crudo(n, *sitio),
            _ => {}
        }
    }

    /// El nombre toca el metal y no esta dentro de un `crudo`.
    fn quiza_pide_crudo(&mut self, nombre: &str, sitio: Sitio) {
        if self.dentro_de_crudo || !self.cat.piden_crudo.contains(nombre) {
            return;
        }
        self.avisos.push(
            Aviso::nuevo(
                codigos::METAL_SIN_CRUDO,
                format!("`{}` tiene que ir dentro de un bloque `crudo`.", nombre),
                sitio,
            )
            .con_habia(
                "`crudo` no marca \"esto es de bajo nivel\": marca \"aqui nadie comprueba \
                 por ti\". Al otro lado de un puerto no hay ningun kernel que valide nada."
                    .to_string(),
            )
            .con_hacer("mete la linea dentro de un bloque `crudo`"),
        );
    }

    fn crece(&mut self, que: &str, sitio: Sitio) {
        self.avisos.push(
            Aviso::nuevo(
                codigos::LLANO_NO_ADMITE,
                format!("En el perfil `llano` no se puede usar {}.", que),
                sitio,
            )
            .con_habia(
                "Lo que crece pide memoria, y `llano` no tiene monton: por eso puede \
                 escribir un manejador de interrupciones."
                    .to_string(),
            )
            .con_hacer("cambia el fichero a `perfil pleno`, o usa un tamano fijo"),
        );
    }

    fn falta_tipo(&mut self, nombre: &str, sitio: Sitio) {
        self.avisos.push(
            Aviso::nuevo(
                codigos::FALTA_TAMANO,
                format!("En `llano`, `{}` tiene que decir su tipo.", nombre),
                sitio,
            )
            .con_habia(
                "Sin tipo no hay tamano, y sin tamano no se puede reservar en la pila. \
                 La obligacion sale del perfil, no del gusto."
                    .to_string(),
            )
            .con_hacer(format!("escribe `{} es entero32`", nombre)),
        );
    }

    fn tipo(&mut self, t: &Tipo, sitio: Sitio) {
        match t {
            Tipo::Nombre(n) => {
                if !self.llano() {
                    return;
                }
                if self.cat.crecen.contains(n) {
                    self.crece(&format!("`{}`", n), sitio);
                } else if self.cat.sin_tamano.contains(n) {
                    self.avisos.push(
                        Aviso::nuevo(
                            codigos::FALTA_TAMANO,
                            format!("En el perfil `llano` no existe `{}`.", n),
                            sitio,
                        )
                        .con_habia(
                            "Hay que decir el tamano exacto. Sin tamano no se puede elegir \
                             la instruccion ni reservar en la pila."
                                .to_string(),
                        )
                        .con_hacer("usa `entero32`, `natural8`, `flotante64`..."),
                    );
                }
            }
            Tipo::Lista(t) => {
                if self.llano() {
                    self.crece("una lista", sitio);
                }
                self.tipo(t, sitio);
            }
            Tipo::Tabla(k, v) => {
                if self.llano() {
                    self.crece("una tabla", sitio);
                }
                self.tipo(k, sitio);
                self.tipo(v, sitio);
            }
            Tipo::Quiza(t) => self.tipo(t, sitio),
        }
    }
}

#[cfg(test)]
mod pruebas;
