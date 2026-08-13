//! **STATEMENTS** -- the shapes that do not produce a value.
//!
//! === Why this is a file of its own ===
//!
//! It is the smallest of the grammar halves and the most regular: nine forms,
//! each one a keyword followed by a predictable shape. Next to the declarators
//! --which wrap inside-out and need 946 lines-- these 305 read almost like a
//! table, and that contrast is worth being able to see.
//!
//! === Where the risk actually is ===
//!
//! Not in parsing these. In what the codegen does with them afterwards: the
//! corner cases that bite are `continue` inside a `switch` (it belongs to the
//! LOOP), `continue` in a `do..while` (it jumps to the CONDITION, not the top),
//! and `while (0)` running zero times where `do..while(0)` runs one.
//!
//! All three are green and are held down by `probe_control_flow` in the bench.
//! They are named here because this is the file somebody will open when one of
//! them goes red, and the answer will not be here.

use super::*;

impl Parser {
    // ---- Statements ----
    pub(super) fn parse_stmt(&mut self) -> Result<Stmt, CError> {
        // Lo mismo DENTRO de una funcion, que es donde se colaba callado.
        if *self.peek() == Token::Hash {
            return Err(CError::new(
                self.line(),
                "aqui no hay preprocesador todavia: '#define', '#include' y '#ifdef' no se procesan. Usa 'const int' o 'enum' para las constantes",
            ));
        }
        match self.peek() {
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::Do => self.parse_do(),
            Token::For => self.parse_for(),
            Token::Switch => self.parse_switch(),
            Token::Break => { self.advance(); self.skip_semicolon(); Ok(Stmt::Break) }
            Token::Continue => { self.advance(); self.skip_semicolon(); Ok(Stmt::Continue) }
            Token::Return => self.parse_return(),
            Token::OpenBrace => self.parse_block(),
            Token::Goto => {
                self.advance();
                let label = match self.advance() {
                    Token::Ident(s) => s,
                    t => return Err(CError::new(self.line(),format!("expected label name, got {:?}", t))),
                };
                self.skip_semicolon();
                Ok(Stmt::Goto(label))
            }
            Token::Semicolon => { self.advance(); Ok(Stmt::Block(vec![])) }
            _ => {
                // Try to parse as declaration if it starts with a type keyword
                if self.peek_is_type_start() {
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        return self.terminar_declaracion(typ, name);
                    }
                }
                self.parse_expr_stmt()
            }
        }
    }

    pub(super) fn parse_if(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        let then = Box::new(self.parse_stmt()?);
        let else_ = if *self.peek() == Token::Else { self.advance(); Some(Box::new(self.parse_stmt()?)) } else { None };
        Ok(Stmt::If(cond, then, else_))
    }

    pub(super) fn parse_while(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While(cond, body))
    }

    pub(super) fn parse_do(&mut self) -> Result<Stmt, CError> {
        self.advance();
        let body = Box::new(self.parse_stmt()?);
        self.expect(&Token::While)?;
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        self.skip_semicolon();
        Ok(Stmt::DoWhile(body, cond))
    }

    pub(super) fn parse_for(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        // check for declaration: for(int i = 0; ...)
        let has_decl = match self.peek() {
            Token::Int | Token::Char | Token::Short | Token::Long |
            Token::Void | Token::Unsigned | Token::Signed | Token::Float | Token::Double |
            Token::Struct | Token::Union | Token::Const | Token::Volatile => true,
            _ => false,
        };
        if has_decl {
            let save = self.pos;
            self.strip_qualifiers();
            let _typ = match self.parse_type_spec() {
                Ok(t) => t,
                Err(_) => { self.pos = save; return self.parse_for_expr(); }
            };
            let name = match self.advance() {
                Token::Ident(n) => n,
                _ => { self.pos = save; return self.parse_for_expr(); }
            };
            let init = if *self.peek() == Token::Assign { self.advance(); Some(self.parse_expr()?) } else { None };
            self.skip_semicolon();
            self.var_types.insert(name.clone(), _typ.clone());
            // wrap in Block: { type name = init; for(; cond; inc) body }
            let mut stmts = Vec::new();
            stmts.push(Stmt::DeclAssign(_typ, name, init));
            let cond = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
            self.skip_semicolon();
            let inc = if *self.peek() == Token::CloseParen { None } else { Some(self.parse_expr()?) };
            self.expect(&Token::CloseParen)?;
            let body = self.parse_stmt()?;
            stmts.push(Stmt::For(None, cond, inc, Box::new(body)));
            return Ok(Stmt::Block(stmts));
        }
        self.parse_for_expr()
    }

    pub(super) fn parse_for_expr(&mut self) -> Result<Stmt, CError> {
        let init = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
        self.skip_semicolon();
        let cond = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
        self.skip_semicolon();
        let inc = if *self.peek() == Token::CloseParen { None } else { Some(self.parse_expr()?) };
        self.expect(&Token::CloseParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For(init, cond, inc, body))
    }

    pub(super) fn parse_switch(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        self.expect(&Token::OpenBrace)?;
        let mut cases = Vec::new();
        let mut current = Vec::new();
        let mut current_val = None;
        loop {
            match self.peek() {
                Token::Case => {
                    if !current.is_empty() { cases.push(Case { value: current_val, stmts: std::mem::take(&mut current) }); }
                    self.advance();
                    // * La etiqueta de un `case` es una EXPRESION CONSTANTE.
                    //
                    // Se aceptaba un literal, una constante de enum o un
                    // caracter, y nada mas. `case -1:` moria con "expected int
                    // in case, got Minus" -- un mensaje que acusa al signo.
                    //
                    // Se lee con el mismo `parse_conditional` + `const_eval`
                    // que los valores de un enum: el signo, `1 << 3` y
                    // `MAX - 1` son la misma cosa para quien la escribe, y
                    // ahora tambien para quien la lee.
                    let e = self.parse_conditional()?;
                    let val = const_eval(&e).ok_or_else(|| {
                        CError::new(
                            self.line(),
                            "la etiqueta de un 'case' tiene que ser una constante que se \
                             pueda calcular al compilar".to_string(),
                        )
                    })?;
                    current_val = Some(val);
                    self.expect(&Token::Colon)?;
                }
                Token::Default => {
                    if !current.is_empty() { cases.push(Case { value: current_val, stmts: std::mem::take(&mut current) }); }
                    self.advance();
                    current_val = None;
                    self.expect(&Token::Colon)?;
                }
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(self.line(),"unexpected eof in switch")),
                _ => { current.push(self.parse_stmt()?); }
            }
        }
        if !current.is_empty() { cases.push(Case { value: current_val, stmts: current }); }
        Ok(Stmt::Switch(expr, cases))
    }

    pub(super) fn parse_return(&mut self) -> Result<Stmt, CError> {
        self.advance();
        if *self.peek() == Token::Semicolon { self.advance(); Ok(Stmt::Return(None)) }
        else { let e = self.parse_expr()?; self.skip_semicolon(); Ok(Stmt::Return(Some(e))) }
    }

    pub(super) fn parse_block(&mut self) -> Result<Stmt, CError> {
        self.advance();
        let mut stmts = Vec::new();
        loop {
            match self.peek() {
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(self.line(),"unexpected eof in block")),
                // * La directiva se caza AQUI, antes que nada. Es donde se
                // colaba: `try_parse_decl` miraba el `#`, decia "esto no es una
                // declaracion" y devolvia None sin consumirlo, y el bucle
                // seguia adelante -- asi que un `#define X 5` dentro de una
                // funcion compilaba y se ignoraba EN SILENCIO. El programa
                // corria con la X sin sustituir y nadie decia nada.
                Token::Hash => {
                    return Err(CError::new(
                        self.line(),
                        "aqui no hay preprocesador todavia: '#define', '#include' y '#ifdef' \
                         no se procesan. Usa 'const int' o 'enum' para las constantes",
                    ))
                }
                _ => {
                    // check for label: ident followed by colon
                    if let Token::Ident(name) = self.peek().clone() {
                        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
                            self.advance(); // consume ident
                            self.advance(); // consume colon
                            stmts.push(Stmt::Label(name));
                            continue;
                        }
                    }
                    // ** LO QUE UN BLOQUE ANIDADO NO SABIA HACER.
                    //
                    // El cuerpo de una funcion entendia `static`, `extern` y
                    // los declaradores separados por coma. Un bloque de dentro
                    // --el de un `if`, el de un `for`-- no, porque es OTRO
                    // bucle. Asi que lo mismo compilaba o no segun estuviera
                    // una llave mas adentro:
                    //
                    //   static mobj_t dummy_mobj;      "unexpected token: Static"
                    //   extern boolean advancedemo;    "unexpected token: Extern"
                    //   char *startname, *endname;     "unexpected token: Comma"
                    //
                    // Ninguno de los tres es raro: son p_mobj.c, d_net.c y
                    // p_spec.c. Dos bucles con la misma gramatica es como se
                    // llega a que uno sepa cosas que el otro no.
                    if *self.peek() == Token::Static {
                        self.advance();
                        let Some((typ, vname)) = self.try_parse_decl()? else {
                            return Err(CError::new(self.line(),
                                "static: esperaba una declaracion de variable"));
                        };
                        let quien = self.funcion_actual.clone();
                        let base = self.base_del_declarador.clone();
                        self.declarar_static_local(&quien, typ, vname)?;
                        // `static int lastlevel = -1, lastepisode = -1;` -- la
                        // coma tambien vale detras de un `static`, y este era
                        // el ultimo sitio donde no.
                        let mut mas = Vec::new();
                        self.declaradores_tras_coma(&base, &mut mas)?;
                        for (t2, n2) in mas {
                            self.declarar_static_local(&quien, t2, n2)?;
                        }
                        continue;
                    }
                    // `extern` DENTRO de una funcion: declara un nombre que
                    // vive en otro sitio. Se registra el tipo y no se emite
                    // nada -- que es todo lo que significa.
                    if *self.peek() == Token::Extern {
                        self.advance();
                        if let Some((typ, vname)) = self.try_parse_decl()? {
                            self.var_types.insert(vname, typ);
                            self.skip_semicolon();
                        }
                        continue;
                    }
                    // * Un `enum { ... };` DENTRO de una funcion.
                    //
                    // C lo permite y `am_map.c` declara asi las cuatro esquinas
                    // de su recorte. Las constantes se registran donde se
                    // registran todas --son constantes de compilacion, no
                    // ocupan memoria-- y aqui no queda ninguna sentencia.
                    if *self.peek() == Token::Enum {
                        self.parse_enum_spec()?;
                        self.skip_semicolon();
                        continue;
                    }
                    // Un prototipo dentro del cuerpo: se consume y ya.
                    if self.saltar_prototipo_local() {
                        continue;
                    }
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        let base = self.base_del_declarador.clone();
                        stmts.push(self.terminar_declaracion(typ, name)?);
                        let mut mas = Vec::new();
                        self.declaradores_tras_coma(&base, &mut mas)?;
                        for (t2, n2) in mas {
                            stmts.push(self.terminar_declaracion(t2, n2)?);
                        }
                        continue;
                    } else {
                        stmts.push(self.parse_stmt()?);
                    }
                }
            }
        }
        Ok(Stmt::Block(stmts))
    }

    pub(super) fn parse_expr_stmt(&mut self) -> Result<Stmt, CError> {
        let expr = self.parse_expr()?;
        self.skip_semicolon();
        match &expr {
            // Atajo para `printf("literal")` SIN argumentos variadicos: baja
            // directo a la puerta de consola, sin runtime ni imports.
            //
            // El `args.len() == 1` es la condicion que faltaba: antes
            // `printf("%d\n", x)` tambien entraba aqui y los argumentos se
            // DESCARTABAN en silencio -- el programa imprimia literalmente
            // "%d". Con mas de un argumento debe seguir por la ruta
            // variadica, que si los formatea.
            Expr::Call(name, args) if name == "printf" && args.len() == 1 => {
                if let Some(Expr::StringLit(s)) = args.first() {
                    return Ok(if s.ends_with('\n') { let mut t = s.clone(); t.pop(); Stmt::PrintfLn(t) } else { Stmt::Printf(s.clone()) });
                }
            }
            _ => {}
        }
        Ok(Stmt::Expr(expr))
    }
}
