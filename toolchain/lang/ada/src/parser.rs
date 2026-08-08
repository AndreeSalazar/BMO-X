//! El analisis de Ada. Lo que no se sabe compilar se RECHAZA con su motivo.
//!
//! ## El alcance, dicho entero
//!
//! Un procedimiento en UN fichero:
//!
//! ```ada
//! with Ada.Text_IO; use Ada.Text_IO;
//! procedure Cierre is
//!    type Saldo is delta 0.01 digits 12;
//!    Total : Saldo := 0.00;
//! begin
//!    Total := Total + 19.99;
//!    Put_Line("total:");
//!    Put_Line(Total);
//! end Cierre;
//! ```
//!
//! **Un fichero, una unidad.** Ada de verdad son especificacion y cuerpo con
//! orden de elaboracion (RM 10.2.1), y eso es semantica del lenguaje, no
//! comodidad: no se puede fingir. Lo que se hace aqui es acotar honestamente
//! --un procedimiento suelto, que el estandar permite-- y **rechazar con su
//! motivo** todo lo que pida el modelo de unidades: `package`, `with` de
//! cualquier cosa que no sea `Ada.Text_IO`, `separate`, genericos y tareas.
//!
//! Prometer que compila Ada entero seria el fallo de Vib-OS otra vez.

use crate::ast::*;
use crate::lexer::{lexar, Componente, Tok};

pub struct Parser {
    t: Vec<Componente>,
    i: usize,
}

impl Parser {
    pub fn nuevo(fuente: &str) -> Self {
        Self { t: lexar(fuente), i: 0 }
    }

    fn actual(&self) -> &Tok {
        &self.t[self.i.min(self.t.len() - 1)].tok
    }

    fn linea(&self) -> usize {
        self.t[self.i.min(self.t.len() - 1)].linea
    }

    fn avanzar(&mut self) {
        if self.i < self.t.len() - 1 {
            self.i += 1;
        }
    }

    /// El componente actual es esta palabra reservada?
    fn es_palabra(&self, p: &str) -> bool {
        matches!(self.actual(), Tok::Ident(s) if s == p)
    }

    fn es_simbolo(&self, s: &str) -> bool {
        matches!(self.actual(), Tok::Simbolo(x) if x == s)
    }

    fn comer_palabra(&mut self, p: &str) -> Result<(), AdaError> {
        if self.es_palabra(p) {
            self.avanzar();
            Ok(())
        } else {
            Err(AdaError::nuevo(self.linea(), format!("falta '{}'", p.to_ascii_lowercase())))
        }
    }

    fn comer_simbolo(&mut self, s: &str) -> Result<(), AdaError> {
        if self.es_simbolo(s) {
            self.avanzar();
            Ok(())
        } else {
            Err(AdaError::nuevo(self.linea(), format!("falta '{s}'")))
        }
    }

    fn nombre(&mut self) -> Result<String, AdaError> {
        match self.actual().clone() {
            Tok::Ident(s) => {
                self.avanzar();
                Ok(s)
            }
            _ => Err(AdaError::nuevo(self.linea(), "aqui esperaba un nombre")),
        }
    }

    // -- El programa -----------------------------------------------------

    pub fn programa(&mut self) -> Result<Programa, AdaError> {
        self.contexto()?;

        // `pragma ...;` se acepta y se ignora: un `pragma Restrictions` es una
        // promesa del programa sobre lo que NO va a usar, y este compilador ya
        // rechaza todo eso por su cuenta. Aceptarlo deja compilar fuente real.
        while self.es_palabra("PRAGMA") {
            while !self.es_simbolo(";") && !matches!(self.actual(), Tok::Fin) {
                self.avanzar();
            }
            self.comer_simbolo(";")?;
        }

        if self.es_palabra("PACKAGE") {
            return Err(AdaError::nuevo(
                self.linea(),
                "un package son DOS unidades de compilacion con orden de elaboracion, \
                 y este compilador es de un fichero = una unidad. Escribe un procedure",
            ));
        }
        if self.es_palabra("GENERIC") {
            return Err(AdaError::nuevo(self.linea(), "los genericos todavia no se compilan"));
        }
        if self.es_palabra("TASK") {
            return Err(AdaError::nuevo(
                self.linea(),
                "las tareas piden un planificador: el perfil de este compilador es \
                 ZFP secuencial (sin runtime), no Ravenscar",
            ));
        }

        self.comer_palabra("PROCEDURE")?;
        let nombre = self.nombre()?;
        if self.es_simbolo("(") {
            return Err(AdaError::nuevo(
                self.linea(),
                "un procedure con parametros todavia no se compila: el de arranque no los lleva",
            ));
        }
        self.comer_palabra("IS")?;

        let mut tipos = Vec::new();
        let mut declaraciones = Vec::new();
        while !self.es_palabra("BEGIN") {
            if matches!(self.actual(), Tok::Fin) {
                return Err(AdaError::nuevo(self.linea(), "falta 'begin'"));
            }
            self.declaracion(&mut tipos, &mut declaraciones)?;
        }
        self.comer_palabra("BEGIN")?;

        let cuerpo = self.sentencias(&["END"])?;
        self.comer_palabra("END")?;
        // `end Cierre;` -- el nombre repetido es opcional en el estandar, pero
        // si esta TIENE que coincidir. Es la comprobacion que caza un `end`
        // colocado en el sitio equivocado.
        if let Tok::Ident(n) = self.actual().clone() {
            if n != nombre {
                return Err(AdaError::nuevo(
                    self.linea(),
                    format!("el 'end {}' no cuadra con 'procedure {}'", n.to_ascii_lowercase(), nombre.to_ascii_lowercase()),
                ));
            }
            self.avanzar();
        }
        self.comer_simbolo(";")?;

        Ok(Programa { nombre, tipos, declaraciones, cuerpo })
    }

    /// `with Ada.Text_IO; use Ada.Text_IO;` -- y nada mas.
    fn contexto(&mut self) -> Result<(), AdaError> {
        while self.es_palabra("WITH") || self.es_palabra("USE") {
            let clausula = if self.es_palabra("WITH") { "with" } else { "use" };
            self.avanzar();
            let mut unidad = String::new();
            loop {
                match self.actual().clone() {
                    Tok::Ident(s) => {
                        unidad.push_str(&s);
                        self.avanzar();
                    }
                    Tok::Simbolo(s) if s == "." => {
                        unidad.push('.');
                        self.avanzar();
                    }
                    _ => break,
                }
            }
            self.comer_simbolo(";")?;
            // La unica unidad que existe aqui. Aceptar cualquier otra seria
            // prometer una biblioteca estandar que no esta.
            if unidad != "ADA.TEXT_IO" {
                return Err(AdaError::nuevo(
                    self.linea(),
                    format!(
                        "'{clausula} {}' — la unica unidad que este compilador provee es \
                         Ada.Text_IO. No hay biblioteca estandar detras, y decir que si \
                         seria mentir en la primera linea",
                        unidad.to_ascii_lowercase()
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Una declaracion: un tipo decimal o una variable.
    fn declaracion(
        &mut self,
        tipos: &mut Vec<TipoDecimal>,
        decls: &mut Vec<Declaracion>,
    ) -> Result<(), AdaError> {
        if self.es_palabra("TYPE") {
            self.avanzar();
            let nombre = self.nombre()?;
            self.comer_palabra("IS")?;
            // * `delta <d> digits <n>` -- el decimal de Annex F.
            if !self.es_palabra("DELTA") {
                return Err(AdaError::nuevo(
                    self.linea(),
                    format!(
                        "type {} is ... — este compilador solo declara tipos DECIMALES \
                         (`is delta 0.01 digits 12`), que es lo que hace falta para banca",
                        nombre.to_ascii_lowercase()
                    ),
                ));
            }
            self.avanzar();
            let delta = match self.actual().clone() {
                Tok::Numero(n) => {
                    self.avanzar();
                    n
                }
                _ => return Err(AdaError::nuevo(self.linea(), "tras 'delta' va el paso: 0.01")),
            };
            let escala = match delta.split_once('.') {
                Some((_, dec)) => dec.len() as u32,
                None => 0,
            };
            self.comer_palabra("DIGITS")?;
            let digitos = match self.actual().clone() {
                Tok::Numero(n) => {
                    self.avanzar();
                    n.parse::<u32>().unwrap_or(0)
                }
                _ => return Err(AdaError::nuevo(self.linea(), "tras 'digits' va cuantas cifras")),
            };
            // El limite de verdad: un entero de 64 bits con signo llega a 18
            // cifras. El Information Systems Annex exige 18 como minimo, asi
            // que el tipo que pide mas de lo que cabe se dice AQUI.
            if digitos > 18 {
                return Err(AdaError::nuevo(
                    self.linea(),
                    format!(
                        "digits {digitos}: no caben en un entero de 64 bits, que es donde \
                         vive el decimal exacto. El maximo es 18"
                    ),
                ));
            }
            self.comer_simbolo(";")?;
            tipos.push(TipoDecimal { nombre, escala, digitos });
            return Ok(());
        }

        // `Nombre : Tipo [:= inicial];`
        let nombre = self.nombre()?;
        self.comer_simbolo(":")?;
        if self.es_palabra("CONSTANT") {
            self.avanzar();
        }
        let tipo = self.nombre()?;
        let escala = match tipo.as_str() {
            "INTEGER" | "NATURAL" | "POSITIVE" => 0,
            otro => match tipos.iter().find(|t| t.nombre == otro) {
                Some(t) => t.escala,
                None => {
                    return Err(AdaError::nuevo(
                        self.linea(),
                        format!(
                            "el tipo '{}' no existe: declara uno con \
                             `type {} is delta 0.01 digits 12;` o usa Integer",
                            otro.to_ascii_lowercase(),
                            otro.to_ascii_lowercase()
                        ),
                    ))
                }
            },
        };
        let inicial = if self.es_simbolo(":=") {
            self.avanzar();
            match self.actual().clone() {
                Tok::Numero(n) => {
                    self.avanzar();
                    Some(n)
                }
                Tok::Simbolo(s) if s == "-" => {
                    self.avanzar();
                    match self.actual().clone() {
                        Tok::Numero(n) => {
                            self.avanzar();
                            Some(format!("-{n}"))
                        }
                        _ => return Err(AdaError::nuevo(self.linea(), "tras '-' va un numero")),
                    }
                }
                _ => {
                    return Err(AdaError::nuevo(
                        self.linea(),
                        "el valor inicial tiene que ser un numero: una expresion ahi \
                         pide elaboracion en ejecucion, que este compilador no hace",
                    ))
                }
            }
        } else {
            None
        };
        self.comer_simbolo(";")?;
        decls.push(Declaracion { nombre, tipo, escala, inicial });
        Ok(())
    }

    // -- Sentencias ------------------------------------------------------

    fn sentencias(&mut self, hasta: &[&str]) -> Result<Vec<Sentencia>, AdaError> {
        let mut out = Vec::new();
        loop {
            if matches!(self.actual(), Tok::Fin) {
                return Err(AdaError::nuevo(self.linea(), "el cuerpo se acaba sin cerrar"));
            }
            if let Tok::Ident(s) = self.actual() {
                if hasta.contains(&s.as_str()) {
                    return Ok(out);
                }
            }
            out.push(self.sentencia()?);
        }
    }

    fn sentencia(&mut self) -> Result<Sentencia, AdaError> {
        if self.es_palabra("IF") {
            self.avanzar();
            let cond = self.condicion()?;
            self.comer_palabra("THEN")?;
            let entonces = self.sentencias(&["ELSE", "ELSIF", "END"])?;
            if self.es_palabra("ELSIF") {
                // `elsif` es un `else` con un `if` dentro. Se rechaza y se dice
                // la salida en vez de compilarlo a medias.
                return Err(AdaError::nuevo(
                    self.linea(),
                    "elsif todavia no se compila: escribelo como 'else if ... end if;' anidado",
                ));
            }
            let si_no = if self.es_palabra("ELSE") {
                self.avanzar();
                self.sentencias(&["END"])?
            } else {
                Vec::new()
            };
            self.comer_palabra("END")?;
            self.comer_palabra("IF")?;
            self.comer_simbolo(";")?;
            return Ok(Sentencia::Si(cond, entonces, si_no));
        }

        if self.es_palabra("WHILE") {
            self.avanzar();
            let cond = self.condicion()?;
            self.comer_palabra("LOOP")?;
            let cuerpo = self.sentencias(&["END"])?;
            self.comer_palabra("END")?;
            self.comer_palabra("LOOP")?;
            self.comer_simbolo(";")?;
            return Ok(Sentencia::Mientras(cond, cuerpo));
        }

        if self.es_palabra("FOR") {
            return Err(AdaError::nuevo(
                self.linea(),
                "el bucle 'for' todavia no se compila: usa 'while' con su contador",
            ));
        }

        // `Put_Line(...)` / `Put(...)`
        if self.es_palabra("PUT_LINE") || self.es_palabra("PUT") {
            self.avanzar();
            self.comer_simbolo("(")?;
            let s = match self.actual().clone() {
                Tok::Texto(t) => {
                    self.avanzar();
                    Sentencia::PutLiteral(t)
                }
                Tok::Ident(n) => {
                    self.avanzar();
                    Sentencia::PutValor(n)
                }
                _ => {
                    return Err(AdaError::nuevo(
                        self.linea(),
                        "Put_Line admite un texto entre comillas o el nombre de una variable",
                    ))
                }
            };
            self.comer_simbolo(")")?;
            self.comer_simbolo(";")?;
            return Ok(s);
        }

        // `null;` -- la sentencia que no hace nada, y hay que escribirla.
        if self.es_palabra("NULL") {
            self.avanzar();
            self.comer_simbolo(";")?;
            return Ok(Sentencia::Nada);
        }

        // Asignacion: `Nombre := expr;`
        let nombre = self.nombre()?;
        if self.es_simbolo("=") {
            return Err(AdaError::nuevo(
                self.linea(),
                format!(
                    "'{} = ...' compara, no asigna. Para asignar es ':='",
                    nombre.to_ascii_lowercase()
                ),
            ));
        }
        self.comer_simbolo(":=")?;
        let e = self.expresion()?;
        self.comer_simbolo(";")?;
        Ok(Sentencia::Asignar(nombre, e))
    }

    fn condicion(&mut self) -> Result<Condicion, AdaError> {
        let izq = self.expresion()?;
        let op = match self.actual().clone() {
            Tok::Simbolo(s) if matches!(s.as_str(), "=" | "/=" | "<" | ">" | "<=" | ">=") => {
                self.avanzar();
                s
            }
            _ => {
                return Err(AdaError::nuevo(
                    self.linea(),
                    "falta la comparacion: =, /=, <, >, <= o >=",
                ))
            }
        };
        let der = self.expresion()?;
        Ok(Condicion { izq, op, der })
    }

    // -- Expresiones, con precedencia de verdad --------------------------

    fn expresion(&mut self) -> Result<Expr, AdaError> {
        let mut izq = self.termino()?;
        loop {
            let op = match self.actual() {
                Tok::Simbolo(s) if s == "+" => '+',
                Tok::Simbolo(s) if s == "-" => '-',
                _ => return Ok(izq),
            };
            self.avanzar();
            let der = self.termino()?;
            izq = Expr::Binaria(Box::new(izq), op, Box::new(der));
        }
    }

    fn termino(&mut self) -> Result<Expr, AdaError> {
        let mut izq = self.factor()?;
        loop {
            let op = match self.actual() {
                Tok::Simbolo(s) if s == "*" => '*',
                Tok::Simbolo(s) if s == "/" => '/',
                _ => return Ok(izq),
            };
            self.avanzar();
            let der = self.factor()?;
            izq = Expr::Binaria(Box::new(izq), op, Box::new(der));
        }
    }

    fn factor(&mut self) -> Result<Expr, AdaError> {
        match self.actual().clone() {
            Tok::Numero(n) => {
                self.avanzar();
                Ok(Expr::Literal(n))
            }
            Tok::Ident(n) => {
                self.avanzar();
                Ok(Expr::Nombre(n))
            }
            Tok::Simbolo(s) if s == "-" => {
                self.avanzar();
                match self.actual().clone() {
                    Tok::Numero(n) => {
                        self.avanzar();
                        Ok(Expr::Literal(format!("-{n}")))
                    }
                    _ => Err(AdaError::nuevo(self.linea(), "tras '-' va un numero")),
                }
            }
            Tok::Simbolo(s) if s == "(" => {
                self.avanzar();
                let e = self.expresion()?;
                self.comer_simbolo(")")?;
                Ok(e)
            }
            _ => Err(AdaError::nuevo(self.linea(), "aqui esperaba un numero, un nombre o '('")),
        }
    }
}
