//! El árbol de Ada. Sólo lo que este compilador sabe emitir.

/// Dónde falló, y por qué.
#[derive(Debug, Clone, PartialEq)]
pub struct AdaError {
    pub linea: usize,
    pub mensaje: String,
}

impl AdaError {
    pub fn nuevo(linea: usize, mensaje: impl Into<String>) -> Self {
        Self { linea, mensaje: mensaje.into() }
    }
}

impl core::fmt::Display for AdaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "linea {}: {}", self.linea, self.mensaje)
    }
}

/// Un tipo declarado por el programa.
///
/// ★ **El decimal de Ada es de primera clase, y por eso Ada está aquí.**
/// `type Saldo is delta 0.01 digits 12;` no es una convención ni una librería:
/// es un TIPO, y el compilador sabe que sus valores son múltiplos de 0.01. Se
/// guarda como entero escalado —céntimos— exactamente igual que un `PIC` de
/// COBOL, porque es exactamente lo mismo: el Annex F de Ada copió las reglas
/// de COBOL a propósito.
#[derive(Debug, Clone, PartialEq)]
pub struct TipoDecimal {
    pub nombre: String,
    /// Cuántos decimales. `delta 0.01` → 2. Es la escala.
    pub escala: u32,
    /// Cuántas cifras significativas admite. `digits 12` → 12.
    pub digitos: u32,
}

/// Una variable declarada.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaracion {
    pub nombre: String,
    /// El tipo escrito: `Integer`, o el nombre de un tipo decimal propio.
    pub tipo: String,
    /// La escala ya resuelta: 0 para `Integer`.
    pub escala: u32,
    /// `:= <valor>` inicial, si lo hay.
    pub inicial: Option<String>,
}

/// Una expresión aritmética, ya en árbol.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Un literal numérico tal cual se escribió.
    Literal(String),
    /// Una variable.
    Nombre(String),
    /// `izq <op> der`, con `op` en `+ - * /`.
    Binaria(Box<Expr>, char, Box<Expr>),
}

/// Una comparación.
#[derive(Debug, Clone, PartialEq)]
pub struct Condicion {
    pub izq: Expr,
    /// `=`, `/=`, `<`, `>`, `<=`, `>=`.
    pub op: String,
    pub der: Expr,
}

/// Lo que el programa hace.
#[derive(Debug, Clone, PartialEq)]
pub enum Sentencia {
    /// `null;` — no hacer nada, DICHO. Ada no permite un `begin end` vacío: si
    /// el cuerpo no hace nada, hay que escribirlo. Se representa en el árbol en
    /// vez de tirarla al analizar, porque "aquí no pasa nada a propósito" es
    /// información y un hueco no lo es.
    Nada,
    /// `Put_Line("texto");`
    PutLiteral(String),
    /// `Put_Line(X);` — el valor, con su escala.
    PutValor(String),
    /// `X := <expr>;`
    Asignar(String, Expr),
    /// `if <cond> then <s> [else <s>] end if;`
    Si(Condicion, Vec<Sentencia>, Vec<Sentencia>),
    /// `while <cond> loop <s> end loop;`
    Mientras(Condicion, Vec<Sentencia>),
}

/// El programa entero: un procedimiento con sus declaraciones y su cuerpo.
#[derive(Debug, Clone, PartialEq)]
pub struct Programa {
    pub nombre: String,
    pub tipos: Vec<TipoDecimal>,
    pub declaraciones: Vec<Declaracion>,
    pub cuerpo: Vec<Sentencia>,
}
