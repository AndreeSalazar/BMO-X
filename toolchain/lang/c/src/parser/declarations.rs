//! **DECLARATIONS AND DECLARATORS** -- the hard half of C's grammar.
//!
//! === Why this is a file of its own, and it is the biggest one ===
//!
//! Because in C a declaration is not "type, then name". It is a **type built
//! inside-out around a name**: `int (*f)(int)` is a pointer to a function, and
//! the pointer-ness is written to the left of the name while the function-ness
//! is written to the right. `char *v[8]` is an array of pointers, and `char
//! (*v)[8]` is a pointer to an array, and the difference is one pair of
//! brackets.
//!
//! That is why this half is 946 lines and the statements are 305. Statements
//! nest; declarators **wrap**, and every wrap needs its own method.
//!
//! === What lives here that is not obviously a declaration ===
//!
//! `parse_type_spec`, `strip_qualifiers` and `peek_is_type_start` -- the three
//! that answer *"is what comes next a type?"*. In a language without that
//! question a parser can be written blind; in C it is the question that decides
//! whether `a * b;` is a multiplication or the declaration of a pointer, and
//! there is no way to answer it without knowing which names are typedefs.
//!
//! [!] Spanish names survive here (`declarar_static_local`,
//! `declaradores_tras_coma`, `cerrar_array_incompleto`). That is old debt
//! tracked in the language memory, not new code -- moving a method does not
//! make it new. Anything written from here on goes in English.

use super::*;

impl Parser {
    pub(super) fn try_parse_function(&mut self) -> Result<Tope, CError> {
        let save = self.pos;
        let start_line = self.line();
        // `static int f(){...}` -- el `static` de una funcion es enlace interno,
        // y aqui solo hay una unidad de traduccion. Se acepta y se sigue.
        if *self.peek() == Token::Static {
            self.advance();
        }
        let ret_type = match self.parse_type_spec() {
            Ok(t) => t,
            Err(_) => { self.pos = save; return Ok(Tope::NoEsFuncion); }
        };
        let Token::Ident(name) = self.peek().clone() else { self.pos = save; return Ok(Tope::NoEsFuncion); };
        self.advance();
        if *self.peek() != Token::OpenParen { self.pos = save; return Ok(Tope::NoEsFuncion); }
        self.advance();
        let mut params = Vec::new();
        let mut anonimos = 0usize;
        let mut variadica = false;
        while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
            // * `...` -- el resto de los argumentos, sin nombre ni tipo.
            //
            // Va SIEMPRE al final, y por eso se corta el bucle aqui: lo que
            // viniera detras no seria un parametro de nadie.
            if *self.peek() == Token::Puntos {
                self.advance();
                variadica = true;
                break;
            }
            if *self.peek() == Token::Void && (self.pos + 1 >= self.tokens.len() || self.tokens[self.pos + 1] == Token::CloseParen) {
                self.advance(); break;
            }
            let ptype = self.parse_type_spec()?;
            // * A PARAMETER THAT IS A POINTER TO FUNCTION:
            //   `void P_PathTraverse(..., boolean (*trav)(intercept_t *))`
            //
            // The declarator was understood for globals, members, locals and
            // typedefs, and not here -- so a callback could be declared, stored
            // and called, but never PASSED. The message, "expected param name,
            // got OpenParen", blames the parenthesis.
            //
            // It was the first error in 24 of DOOM's files, and it is not a
            // corner of the language there: `p_map.c` and `p_sight.c` are built
            // on passing the traverser in.
            if *self.peek() == Token::OpenParen
                && self.tokens.get(self.pos + 1) == Some(&Token::Star)
            {
                let (pname, ptype) = self.parse_fnptr_tail()?;
                self.var_types.insert(pname.clone(), ptype.clone());
                params.push(Param { typ: ptype, name: pname });
                if *self.peek() == Token::Comma { self.advance(); }
                continue;
            }
            // * El nombre del parametro es OPCIONAL.
            //
            // `int f(int);` es C legal y es como se escriben los prototipos en
            // las cabeceras de cualquier programa de verdad -- DOOM incluido.
            // Aqui se exigia nombre siempre, asi que un prototipo moria con
            // "expected param name, got CloseParen": un mensaje que acusa al
            // programa de algo que el estandar permite.
            //
            // Sin nombre no se puede referenciar dentro del cuerpo, y por eso
            // solo aparece en declaraciones. Se le pone uno inventado para que
            // el resto del compilador no tenga que saber que puede faltar.
            let pname = match self.peek().clone() {
                Token::Ident(n) => { self.advance(); n }
                Token::Comma | Token::CloseParen => {
                    anonimos += 1;
                    format!("_anon{}", anonimos)
                }
                t => return Err(CError::new(self.line(),format!("expected param name, got {:?}", t))),
            };
            // * El tipo de un PARAMETRO tambien se registra.
            //
            // Solo se guardaba el de las variables locales, asi que dentro de
            // `int suma(struct P p)` el parser no sabia que `p` era un struct:
            // `p.x` salia como un campo de offset 0 y tipo `long`, y los tres
            // campos leian **la misma direccion y ocho bytes**. Daba
            // `0x200000001` -- las dos primeras `int` juntas -- en vez de 1.
            //
            // Mientras un parametro solo pudo ser un escalar esto no se notaba:
            // ningun escalar tiene campos que consultar.
            // * `void f(patch_t *c[])` -- un PARAMETRO declarado como array.
            //
            // En C un array como parametro ES un puntero (decae al llamar), y
            // aqui ni siquiera se leian los corchetes: el `[` sobraba y el
            // error acusaba al tipo. `wi_stuff.c` pasa asi sus tablas de
            // graficos.
            let ptype = if *self.peek() == Token::OpenBracket {
                let t = self.parse_array_suffix(ptype)?;
                match t {
                    TypeSpec::Array(base, _) => TypeSpec::Ptr(base),
                    otro => otro,
                }
            } else {
                ptype
            };
            self.var_types.insert(pname.clone(), ptype.clone());
            params.push(Param { typ: ptype, name: pname });
            if *self.peek() == Token::Comma { self.advance(); }
        }
        self.expect(&Token::CloseParen)?;
        // * PROTOTIPO: `int f(int a);` -- declarar sin definir.
        //
        // Sin esto no se puede llamar a una funcion antes de escribirla, y eso
        // no es una comodidad: **la recursion mutua es imposible sin ella**. Un
        // programa de cincuenta ficheros --DOOM son unos cincuenta-- esta lleno
        // de funciones que se llaman en circulo, y ninguna puede ir "antes" de
        // todas las demas. Era el hueco mas caro de los que quedaban, y no se
        // sabia que estaba: el lexer no tiene la culpa de nada aqui.
        //
        // No emite codigo. Lo unico que deja es el tipo de retorno anotado,
        // para que una llamada anterior a la definicion sepa que recibe.
        if *self.peek() == Token::Semicolon {
            self.advance();
            self.var_types.insert(name.clone(), ret_type);
            return Ok(Tope::Prototipo);
        }
        // After expect advances past ), pos should be at {
        if self.pos >= self.tokens.len() || *self.peek() != Token::OpenBrace { self.pos = save; return Ok(Tope::NoEsFuncion); }
        self.advance();
        // Cada funcion empieza sin `static` heredadas de la anterior: el mapa
        // ES el ambito.
        self.static_alias.clear();
        let mut var_count = 0u32;
        let mut var_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut body = Vec::new();
        // Quien es la funcion, para las `static` locales de CUALQUIER bloque
        // suyo -- incluidos los anidados, que hasta ahora no podian tener una.
        self.funcion_actual = name.clone();
        loop {
            match self.peek() {
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(self.line(),"unexpected eof in function body")),
                _ => {
                    // check for label: ident followed by colon
                    if let Token::Ident(name) = self.peek().clone() {
                        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
                            self.advance();
                            self.advance();
                            body.push(Stmt::Label(name));
                            continue;
                        }
                    }
                    // `extern` en el cuerpo: declara un nombre que vive en otro
                    // sitio. Se registra el tipo y no se emite nada. Estaba en
                    // `parse_block` y faltaba aqui, que es el bucle del cuerpo
                    // de la funcion -- las mismas dos gramaticas otra vez.
                    if *self.peek() == Token::Extern {
                        self.advance();
                        if let Some((t, vname)) = self.try_parse_decl()? {
                            self.var_types.insert(vname, t);
                            self.skip_semicolon();
                        }
                        continue;
                    }
                    // * Una local `static` NO es una local: se va a las
                    // globales y aqui no queda nada.
                    if *self.peek() == Token::Static {
                        self.advance();
                        let Some((typ, vname)) = self.try_parse_decl()? else {
                            return Err(CError::new(self.line(),
                                "static: esperaba una declaracion de variable"));
                        };
                        let base = self.base_del_declarador.clone();
                        self.declarar_static_local(&name, typ, vname)?;
                        // `static int lastlevel = -1, lastepisode = -1;`
                        // La coma tambien vale detras de un `static`.
                        let mut mas = Vec::new();
                        self.declaradores_tras_coma(&base, &mut mas)?;
                        for (t2, n2) in mas {
                            self.declarar_static_local(&name, t2, n2)?;
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
                        var_count += 1;
                        var_names.push(name.clone());
                        body.push(self.terminar_declaracion(typ, name)?);
                        // `int a, b;` -- los de detras de la coma comparten el
                        // tipo BASE y traen su propio `*` y su propio `[n]`.
                        let mut mas = Vec::new();
                        self.declaradores_tras_coma(&base, &mut mas)?;
                        for (t2, n2) in mas {
                            var_count += 1;
                            var_names.push(n2.clone());
                            body.push(self.terminar_declaracion(t2, n2)?);
                        }
                    } else {
                        body.push(self.parse_stmt()?);
                    }
                }
            }
        }
        Ok(Tope::Funcion(Function { ret_type, name, params, var_count, var_names, body, line: start_line, variadica }))
    }

    /// **Una `static` dentro de una funcion.**
    ///
    /// Aqui `static` si cambia lo que el programa hace, y en dos cosas a la vez:
    ///
    /// 1. **Sobrevive entre llamadas.** No puede vivir en la pila, que se
    ///    deshace al volver: vive donde viven las globales.
    /// 2. **Su inicializador corre UNA vez**, no en cada llamada. Por eso el
    ///    valor viaja con la global y **no se emite ninguna sentencia** en el
    ///    cuerpo -- si se emitiera una asignacion, un contador `static int n=0`
    ///    se pondria a cero en cada llamada y pareceria que no cuenta nada.
    ///
    /// Lo que NO cambia es su ambito: el nombre solo se ve dentro de su
    /// funcion, y dos funciones pueden tener cada una su `static int n`. De ahi
    /// el renombrado: la global se llama `funcion.variable` --con un punto, que
    /// un identificador de C no puede contener, asi que no puede chocar con
    /// nada que el programa escriba-- y el mapa de alias traduce.
    pub(super) fn declarar_static_local(
        &mut self,
        funcion: &str,
        typ: TypeSpec,
        name: String,
    ) -> Result<(), CError> {
        let real = format!("{}.{}", funcion, name);
        // * `static event_t st_notify = { ... };` -- una LISTA tambien.
        //
        // Solo se admitia una expresion, asi que una `static` local con
        // inicializador de agregado moria con "unexpected token: OpenBrace".
        // Y una `static` local con lista no es rara: es como se escribe una
        // tabla que no hace falta fuera de su funcion -- `am_map.c` guarda ahi
        // el evento que manda al pulsar una tecla.
        if *self.peek() == Token::Assign
            && self.tokens.get(self.pos + 1) == Some(&Token::OpenBrace)
        {
            self.advance();
            let escrituras = self.parse_inicializador(&typ)?;
            let typ = self.cerrar_array_incompleto(typ, &escrituras);
            self.skip_semicolon();
            self.var_types.insert(real.clone(), typ.clone());
            self.static_alias.insert(name, real.clone());
            self.globales_pendientes
                .push(GlobalDecl::VarLista(typ, real, escrituras));
            return Ok(());
        }
        let init = if *self.peek() == Token::Assign {
            self.advance();
            Some(self.parse_assign()?)
        } else {
            None
        };
        self.skip_semicolon();
        self.var_types.insert(real.clone(), typ.clone());
        self.static_alias.insert(name, real.clone());
        self.globales_pendientes.push(GlobalDecl::Var(typ, real, init));
        Ok(())
    }

    /// Los declaradores que van detras de una coma: `int a, *b, c[4];`.
    ///
    /// * Cada uno tiene su propio `*` y su propio `[n]`, y **comparte solo el
    /// tipo BASE**. Es el detalle de C que mas se salta al implementarlo: en
    /// `int *a, b;` la `b` es un `int`, **no** un puntero. El asterisco es del
    /// declarador, no del tipo -- y quien lo trate al reves compila el programa
    /// y le cambia el significado.
    pub(super) fn declaradores_tras_coma(
        &mut self,
        base: &TypeSpec,
        salida: &mut Vec<(TypeSpec, String)>,
    ) -> Result<(), CError> {
        while *self.peek() == Token::Comma {
            self.declaradores_tras_coma_uno(base, salida)?;
        }
        Ok(())
    }

    /// One declarator after one comma. Split out of the loop above so that the
    /// file-scope caller can stop between declarators -- at file scope each one
    /// may carry its own initializer, and reading them all first would leave
    /// the `=` behind.
    pub(super) fn declaradores_tras_coma_uno(
        &mut self,
        base: &TypeSpec,
        salida: &mut Vec<(TypeSpec, String)>,
    ) -> Result<(), CError> {
        self.expect(&Token::Comma)?;
        let mut typ = base.clone();
        while *self.peek() == Token::Star {
            self.advance();
            typ = TypeSpec::Ptr(Box::new(typ));
        }
        let Token::Ident(name) = self.peek().clone() else {
            return Err(CError::new(self.line(),
                "esperaba otro nombre despues de la coma en la declaracion"));
        };
        self.advance();
        if *self.peek() == Token::OpenBracket {
            typ = self.parse_array_suffix(typ)?;
        }
        salida.push((typ, name));
        Ok(())
    }

    /// The declarators after the comma of a FILE-SCOPE declaration, pushed as
    /// globals of their own.
    ///
    /// A wrapper over `declaradores_tras_coma` so that both scopes share one
    /// reader for `int *a, b[4], c;`. Each one may also carry its own `=`,
    /// which is why the initializer is read here and not by the caller.
    pub(super) fn declaradores_globales_tras_coma(
        &mut self,
        base: &TypeSpec,
        globals: &mut Vec<GlobalDecl>,
    ) -> Result<(), CError> {
        while *self.peek() == Token::Comma {
            let mut mas = Vec::new();
            // Reads exactly one declarator: the helper loops on commas, and
            // the initializer belongs to the one just read.
            let antes = self.pos;
            self.declaradores_tras_coma_uno(base, &mut mas)?;
            if self.pos == antes {
                break;
            }
            for (typ, name) in mas {
                if *self.peek() == Token::Assign
                    && self.tokens.get(self.pos + 1) == Some(&Token::OpenBrace)
                {
                    self.advance();
                    let escrituras = self.parse_inicializador(&typ)?;
                    let typ = self.cerrar_array_incompleto(typ, &escrituras);
                    self.var_types.insert(name.clone(), typ.clone());
                    globals.push(GlobalDecl::VarLista(typ, name, escrituras));
                    continue;
                }
                let init = if *self.peek() == Token::Assign {
                    self.advance();
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                self.var_types.insert(name.clone(), typ.clone());
                globals.push(GlobalDecl::Var(typ, name, init));
            }
        }
        Ok(())
    }

    pub(super) fn try_parse_decl(&mut self) -> Result<Option<(TypeSpec, String)>, CError> {
        let save = self.pos;
        if !self.peek_is_type_start() {
            return Ok(None);
        }
        let mut typ = match self.parse_type_spec() {
            Ok(t) => t,
            Err(_) => { self.pos = save; return Ok(None); }
        };
        // puntero a funcion: RETTYPE (*name)(params) -- variable de tipo puntero.
        // Es lo que sostiene las vtables de C++ y las tablas de drivers.
        if *self.peek() == Token::OpenParen
            && self.tokens.get(self.pos + 1) == Some(&Token::Star)
        {
            match self.parse_fnptr_tail() {
                Ok((fname, ftyp)) => {
                    if *self.peek() != Token::Semicolon && *self.peek() != Token::Assign {
                        self.pos = save; return Ok(None);
                    }
                    return Ok(Some((ftyp, fname)));
                }
                Err(_) => { self.pos = save; return Ok(None); }
            }
        }
        let Token::Ident(name) = self.peek().clone() else { self.pos = save; return Ok(None); };
        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::OpenParen {
            self.pos = save; return Ok(None);
        }
        self.advance();
        // * El MISMO lector de corchetes que el nivel de fichero.
        //
        // Este camino tenia su propia copia, que leia una sola dimension y
        // exigia una medida dentro. Asi que lo que ya funcionaba fuera de una
        // funcion volvia a fallar dentro de ella:
        //
        //   byte endtrack[] = {0xFF, 0x2F, 0x00};   "unexpected token: CloseBracket"
        //   short caja[2][4];                       el segundo [4] sobraba
        //
        // Dos copias de la misma regla es exactamente como se llega a que una
        // sepa algo que la otra no. Ahora es `parse_array_suffix` en los dos.
        if *self.peek() == Token::OpenBracket {
            typ = self.parse_array_suffix(typ)?;
        }
        // * La COMA tambien cierra un declarador: `int a, b;`.
        //
        // Antes solo valian `;` y `=`, asi que `int a, b;` no se reconocia como
        // declaracion y caia al camino de las expresiones -- donde `b` no existe
        // todavia. Lo destapo una sonda de `memcpy` que declaraba
        // `char a[4],b[4];` y acusaba a `memcpy`, que estaba perfecto.
        if *self.peek() != Token::Semicolon
            && *self.peek() != Token::Assign
            && *self.peek() != Token::Comma
        {
            self.pos = save; return Ok(None);
        }
        Ok(Some((typ, name)))
    }

    /// Consume the `{ ... }` of a struct or a union and return its members.
    /// Assumes the cursor is on the `{`.
    ///
    /// It lives on its own so that the TAGGED form (`struct P { ... };`) and
    /// the untagged one (`typedef struct { ... } P;`) read the same body with
    /// the same code. They used to be one path, which is why only the tagged
    /// one existed.
    pub(super) fn parse_aggregate_body(&mut self) -> Result<Vec<StructMember>, CError> {
        self.expect(&Token::OpenBrace)?;
        let mut members = Vec::new();
        while *self.peek() != Token::CloseBrace && *self.peek() != Token::Eof {
            let mtype = self.parse_type_spec()?;
            // The base type, before the declarator's stars, for the members
            // after a comma.
            let base = self.base_del_declarador.clone();
            // A pointer-to-function member: `void (*action)(void);`
            if *self.peek() == Token::OpenParen
                && self.tokens.get(self.pos + 1) == Some(&Token::Star)
            {
                let (mname, mtyp) = self.parse_fnptr_tail()?;
                self.skip_semicolon();
                members.push(StructMember { typ: mtyp, name: mname });
                continue;
            }
            let mname = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(CError::new(self.line(),format!("expected member name, got {:?}", t))),
            };
            // * `char name[8];` -- un ARRAY como miembro.
            //
            // Faltaba, y el error que salia --"expected type, got
            // OpenBracket"-- mandaba a mirar el tipo, que estaba perfecto. La
            // sonda lo encontro en la union, pero fallaba **igual en un
            // struct**: es el declarador, no el agregado.
            //
            // El tamano y el alineado salen solos: `stack_size()` de un
            // `Array(t,n)` ya es `t*n`, y el reparto de offsets se calcula
            // con eso.
            // Same reader as everywhere else, which is what buys `short
            // bbox[2][4];` -- a two-dimensional MEMBER. This branch had its
            // own bracket code that read one dimension and demanded a literal,
            // so the second `[4]` was left in front of the parser.
            //
            // `doomdata.h` is the on-disk map format: nodes carry their
            // bounding boxes exactly like that, and every file that can read a
            // level reaches it.
            let mtype = if *self.peek() == Token::OpenBracket {
                self.parse_array_suffix(mtype)?
            } else {
                mtype
            };
            // * Campo de bits: `unsigned a:3;`.
            //
            // Se ACEPTA la sintaxis y se le da al campo su tipo entero
            // entero -- **sin empaquetar**. Y se dice aqui por que, porque es
            // una decision y no un descuido: empaquetar de verdad obliga a que
            // cada lectura lleve su desplazamiento y su mascara, y cada
            // escritura sea leer-modificar-escribir. Eso es correcto solo si
            // se hace entero; a medias da campos que se pisan.
            //
            // Mientras no este, un `unsigned a:3` ocupa sus cuatro bytes y
            // **guarda lo que le metas**: el programa hace lo que dice, solo
            // que la estructura mide mas. Lo que NO vale es un layout binario
            // ajeno -- ver BRECHA.md.
            if *self.peek() == Token::Colon {
                self.advance();
                match self.advance() {
                    Token::IntLit(_) => {}
                    t => return Err(CError::new(self.line(), format!(
                        "'{mname}:': la anchura de un campo de bits es un numero, no {t:?}"))),
                }
            }
            members.push(StructMember { typ: mtype, name: mname });
            // * `int data1, data2, data3, data4;` INSIDE the aggregate.
            //
            // One member per line was the assumption, and C does not make it.
            // `d_event.h` -- the event every input in DOOM travels in -- packs
            // its four payload fields on one line, so this single line was the
            // first error in twenty files that never even reach their own code.
            //
            // Same reader as the other two scopes: the type is the BASE, and
            // each name brings its own `*` and its own `[n]`.
            let mut mas = Vec::new();
            self.declaradores_tras_coma(&base, &mut mas)?;
            for (t2, n2) in mas {
                members.push(StructMember { typ: t2, name: n2 });
            }
            self.skip_semicolon();
        }
        self.expect(&Token::CloseBrace)?;
        Ok(members)
    }

    /// Un PROTOTIPO dentro de un cuerpo: `void WI_unloadData(void);`
    ///
    /// C lo permite y `wi_stuff.c` lo usa para declarar una funcion justo antes
    /// de llamarla. Aqui no declara nada --el compilador ya admite llamar a lo
    /// que se defina despues-- pero hay que CONSUMIRLO: sin esto caia en el
    /// camino de las expresiones y el error acusaba al tipo ("unexpected token:
    /// Void"), que es lo unico de la linea que estaba bien.
    ///
    /// Devuelve si consumio algo. Si no lo era, deja el cursor donde estaba.
    pub(super) fn saltar_prototipo_local(&mut self) -> bool {
        let guardado = self.pos;
        if !self.peek_is_type_start() {
            return false;
        }
        if self.parse_type_spec().is_err() {
            self.pos = guardado;
            return false;
        }
        let Token::Ident(_) = self.peek().clone() else {
            self.pos = guardado;
            return false;
        };
        self.advance();
        if *self.peek() != Token::OpenParen {
            self.pos = guardado;
            return false;
        }
        // La lista de parametros, equilibrada.
        self.advance();
        let mut hondo = 1;
        while hondo > 0 {
            match self.advance() {
                Token::OpenParen => hondo += 1,
                Token::CloseParen => hondo -= 1,
                Token::Eof => { self.pos = guardado; return false; }
                _ => {}
            }
        }
        // Solo es un prototipo si termina en `;`. Si sigue una llave, es una
        // definicion anidada, y eso no es C.
        if *self.peek() != Token::Semicolon {
            self.pos = guardado;
            return false;
        }
        self.advance();
        true
    }

    /// `lvalue++` / `lvalue--` sobre algo que no es un nombre suelto.
    ///
    /// El valor de un post-incremento es el ANTERIOR, asi que se escribe la
    /// asignacion y se deshace por fuera: `(x += 1) - 1`. Exacto para enteros.
    ///
    /// Sobre un PUNTERO no lo es --`+1` avanza un elemento y `-1` restaria un
    /// byte-- y por eso ahi se rechaza con el motivo en vez de emitir algo que
    /// casi acierta.
    pub(super) fn post_sobre_lvalue(&mut self, expr: Expr, mas: bool) -> Result<Expr, CError> {
        if let Some(TypeSpec::Ptr(_)) = self.resolve_expr_type(&expr) {
            return Err(CError::new(
                self.line(),
                "'++'/'--' detras de un puntero que no es una variable suelta todavia no \
                 se compila: usa `p = p + 1` y di cual quieres",
            ));
        }
        let op: fn(Box<Expr>, Box<Expr>) -> Expr = if mas { Expr::Add } else { Expr::Sub };
        let asignacion = asignacion_con_uno(expr, op).ok_or_else(|| {
            CError::new(self.line(), "'++'/'--' necesita algo a lo que se pueda asignar")
        })?;
        // Deshacer por fuera: el valor de la expresion es el de antes.
        Ok(if mas {
            Expr::Sub(Box::new(asignacion), Box::new(Expr::Int(1)))
        } else {
            Expr::Add(Box::new(asignacion), Box::new(Expr::Int(1)))
        })
    }

    /// A tag for an aggregate that was written without one.
    ///
    /// The layout tables are keyed by name, so an untagged struct still needs
    /// one -- it just needs to be a name no source file can collide with.
    pub(super) fn anon_tag(&mut self, is_union: bool) -> String {
        self.anon_aggregates += 1;
        let kind = if is_union { "union" } else { "struct" };
        format!("<anon {kind} {}>", self.anon_aggregates)
    }

    /// Consume an `enum` specifier: `enum [tag] [{ constants }]`.
    ///
    /// One function for the three shapes C allows, because they are the same
    /// grammar and splitting them is how they drifted apart before:
    ///
    /// ```text
    ///   enum tag { A, B };          a definition
    ///   enum { A, B };              the SAME, with no tag -- legal, and it
    ///                               used to fail with "expected enum name"
    ///   typedef enum { A } thing_t; a definition inside a typedef
    /// ```
    ///
    /// The tag is parsed and dropped on purpose: an enum in this compiler is
    /// `int` plus a table of constants, so the tag names nothing that outlives
    /// this call. What matters is the constants, and those are global.
    ///
    /// The value of a constant is a CONSTANT EXPRESSION, not an integer
    /// literal. DOOM needs exactly that -- `sk_noitems = -1` and
    /// `INVULNTICS = (30*TICRATE)` -- and requiring a literal rejected both.
    pub(super) fn parse_enum_spec(&mut self) -> Result<(), CError> {
        self.expect(&Token::Enum)?;
        if let Token::Ident(_) = self.peek() {
            self.advance();
        }
        // `enum tag x;` names an existing enum and defines nothing.
        if *self.peek() != Token::OpenBrace {
            return Ok(());
        }
        self.advance();

        let mut val = 0i64;
        loop {
            match self.advance() {
                Token::Ident(en) => {
                    if *self.peek() == Token::Assign {
                        self.advance();
                        let e = self.parse_conditional()?;
                        val = const_eval(&e).ok_or_else(|| {
                            CError::new(
                                self.line(),
                                format!("enum '{en}': the value is not a constant expression"),
                            )
                        })?;
                    }
                    // The constant resolves to its VALUE where it is used (see
                    // parse_primary); its type stays int.
                    self.var_types.insert(en.clone(), TypeSpec::Int);
                    self.enum_constants.insert(en.clone(), val);
                }
                Token::CloseBrace => break,
                t => {
                    return Err(CError::new(
                        self.line(),
                        format!("expected enum constant, got {t:?}"),
                    ))
                }
            }
            val += 1;
            if *self.peek() == Token::Comma {
                self.advance();
            }
        }
        Ok(())
    }

    /// Consume la cola de un puntero a funcion: `(*name)(param-types)`.
    /// Asume estar en el `(` inicial. Devuelve el nombre. El tipo del
    /// puntero es opaco (se trata como Ptr): las llamadas son indirectas.
    pub(super) fn parse_fnptr_tail(&mut self) -> Result<(String, TypeSpec), CError> {
        self.expect(&Token::OpenParen)?;
        self.expect(&Token::Star)?;
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(CError::new(self.line(), format!("expected fnptr name, got {:?}", t))),
        };
        // `static int (*wipes[])(int, int, int)` -- una TABLA de punteros a
        // funcion. Los corchetes van dentro del parentesis, entre el nombre y
        // el cierre, y no se leian. `f_wipe.c` guarda ahi los tres efectos de
        // transicion del juego.
        let mut typ = TypeSpec::Ptr(Box::new(TypeSpec::Void));
        if *self.peek() == Token::OpenBracket {
            // Los corchetes hacen del declarador una TABLA de punteros, y ese
            // dato tiene que salir de aqui: si se pierde, el tipo queda escalar
            // y su lista de inicializacion contesta "sobran valores".
            typ = self.parse_array_suffix(typ)?;
        }
        self.expect(&Token::CloseParen)?;
        // saltar la lista de parametros ( ... ) balanceada
        self.expect(&Token::OpenParen)?;
        let mut depth = 1;
        while depth > 0 {
            match self.advance() {
                Token::OpenParen => depth += 1,
                Token::CloseParen => depth -= 1,
                Token::Eof => return Err(CError::new(self.line(), "eof en lista de parametros de fnptr")),
                _ => {}
            }
        }
        Ok((name, typ))
    }

    pub(super) fn parse_type_and_name(&mut self) -> Result<(TypeSpec, String), CError> {
        let mut typ = self.parse_type_spec()?;
        // puntero a funcion en globals/params: RETTYPE (*name)(params)
        if *self.peek() == Token::OpenParen
            && self.tokens.get(self.pos + 1) == Some(&Token::Star)
        {
            let (fname, ftyp) = self.parse_fnptr_tail()?;
            return Ok((ftyp, fname));
        }
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(CError::new(self.line(),format!("expected identifier, got {:?}", t))),
        };
        // array declarator [size] -- el tamano SE GUARDA (antes se tiraba)
        if *self.peek() == Token::OpenBracket {
            typ = self.parse_array_suffix(typ)?;
        }
        Ok((typ, name))
    }

    /// The `[...]` of a declarator, with or without a size inside.
    ///
    /// * WHY AN EMPTY `[]` IS A LENGTH OF ZERO AND NOT AN ERROR
    ///
    /// `int t[] = { 10, 20, 30 };` and `extern int t[];` are both ordinary C
    /// and both used to die on the bracket -- `parse_expr` was called on a `]`
    /// and reported "unexpected token: CloseBracket", which names the symbol
    /// and not the situation.
    ///
    /// Zero here means INCOMPLETE, not empty. When an initializer follows, the
    /// length is whatever the initializer wrote (see `cerrar_array_incompleto`)
    /// -- which is the C rule: the list is what says how long the array is.
    /// Without an initializer it stays incomplete, which is exactly what
    /// `extern int t[];` claims: the array is somebody else's.
    /// * AND IT CONSUMES EVERY BRACKET, NOT JUST THE FIRST.
    ///
    /// `extern const byte gammatable[5][256];` -- DOOM's gamma tables, in
    /// `tables.h`, which almost every file reaches through `r_local.h`. Only
    /// the first `[5]` was read, so the `[256]` was left in front of the
    /// parser, which asked for a type and got a bracket. 39 files.
    ///
    /// The dimensions fold from the RIGHT, because that is what they mean:
    /// `[5][256]` is five arrays of 256, not an array of five-by-256.
    pub(super) fn parse_array_suffix(&mut self, base: TypeSpec) -> Result<TypeSpec, CError> {
        let mut dims = Vec::new();
        while *self.peek() == Token::OpenBracket {
            self.advance();
            if *self.peek() == Token::CloseBracket {
                self.advance();
                dims.push(0);
                continue;
            }
            let size_expr = self.parse_expr()?;
            self.expect(&Token::CloseBracket)?;
            // A size that cannot be computed is an ERROR, not a 1.
            //
            // It used to fall back to one element, and that is the worst
            // possible answer: the program compiles, the array is a single
            // slot, and every write past the first lands on whatever follows
            // it. Same rule as a global the compiler cannot evaluate.
            match const_eval(&size_expr) {
                Some(n) if n > 0 => dims.push(n as u32),
                Some(n) => {
                    return Err(CError::new(
                        self.line(),
                        format!("un array no puede medir {n}"),
                    ))
                }
                None => {
                    return Err(CError::new(
                        self.line(),
                        "la medida de un array tiene que ser una constante que se pueda \
                         calcular al compilar".to_string(),
                    ))
                }
            }
        }
        let mut typ = base;
        for n in dims.into_iter().rev() {
            typ = TypeSpec::Array(Box::new(typ), n);
        }
        Ok(typ)
    }

    /// Give an incomplete array the length its initializer just implied.
    ///
    /// The writes carry absolute offsets, so the last one plus one element is
    /// the length. Anything that is not an incomplete array passes through.
    pub(super) fn cerrar_array_incompleto(
        &self,
        typ: TypeSpec,
        escrituras: &[Escritura],
    ) -> TypeSpec {
        let TypeSpec::Array(elem, 0) = &typ else { return typ };
        let tam = self.tamano_de(elem).max(1);
        let n = escrituras
            .iter()
            .map(|e| e.offset / tam + 1)
            .max()
            .unwrap_or(0);
        TypeSpec::Array(elem.clone(), n.max(1))
    }

    pub(super) fn peek_is_type_start(&self) -> bool {
        match self.peek() {
            Token::Int | Token::Void | Token::Char | Token::Short | Token::Long |
            Token::Unsigned | Token::Signed | Token::Float | Token::Double |
            Token::Struct | Token::Union | Token::Enum | Token::Const | Token::Volatile => true,
            Token::Ident(name) => self.typedefs.contains_key(name),
            _ => false,
        }
    }

    pub(super) fn strip_qualifiers(&mut self) {
        loop {
            match self.peek() {
                Token::Const | Token::Volatile => { self.advance(); }
                // * `inline` and its GCC spellings are consumed and dropped.
                //
                // Not laziness: `inline` is a REQUEST, and the standard says a
                // conforming compiler may ignore it. BMO C does not inline, so
                // honouring it and ignoring it produce the same program -- the
                // only difference was that the word made the file stop.
                //
                // `__inline__` and `__forceinline` are here because DOOM's
                // `m_misc.c` and `sha1.c` reach for them behind an `#ifdef`
                // that resolves to whatever the host compiler was.
                //
                // The day there IS an inliner, this is where it stops being a
                // no-op, and nothing else has to move.
                Token::Ident(n)
                    if n == "inline" || n == "__inline" || n == "__inline__"
                        || n == "__forceinline" =>
                {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    pub(super) fn parse_type_spec(&mut self) -> Result<TypeSpec, CError> {
        self.strip_qualifiers();
        let base = match self.advance() {
            Token::Void => TypeSpec::Void,
            Token::Char => TypeSpec::Char,
            Token::Short => TypeSpec::Short,
            Token::Int => TypeSpec::Int,
            Token::Long => {
                if self.features.long_long && *self.peek() == Token::Long { self.advance(); TypeSpec::LongLong } else { TypeSpec::Long }
            }
            Token::Unsigned => {
                match self.peek() {
                    Token::Char => { self.advance(); TypeSpec::UnsignedChar }
                    Token::Short => { self.advance(); TypeSpec::UnsignedShort }
                    Token::Int => { self.advance(); TypeSpec::UnsignedInt }
                    Token::Long => {
                        self.advance();
                        if *self.peek() == Token::Long { self.advance(); TypeSpec::UnsignedLongLong }
                        else { TypeSpec::UnsignedLong }
                    }
                    _ => TypeSpec::UnsignedInt,
                }
            }
            Token::Signed => { self.advance(); TypeSpec::Int }
            Token::Float => TypeSpec::Float,
            Token::Double => TypeSpec::Double,
            // * `struct`/`union`, WITH or WITHOUT a tag, with or without a body.
            //
            // Only `struct P` was understood here, so the two shapes C code
            // actually uses for a one-off type both failed on the brace:
            //
            //   typedef struct { ... } thing_t;   "expected struct name, got OpenBrace"
            //   typedef union  { ... } action_t;  "expected union name, got OpenBrace"
            //
            // That is 34 of DOOM's 81 files, and `d_think.h` -- the union at
            // the centre of every thinker in the game -- is one of them.
            //
            // An untagged aggregate still gets a tag, because the layout table
            // is keyed by name. It is generated with characters an identifier
            // cannot contain, so it can never collide with a real one.
            tok @ (Token::Struct | Token::Union) => {
                let is_union = tok == Token::Union;
                let name = match self.peek() {
                    Token::Ident(_) => match self.advance() {
                        Token::Ident(n) => n,
                        _ => unreachable!(),
                    },
                    _ => self.anon_tag(is_union),
                };
                if *self.peek() == Token::OpenBrace {
                    let members = self.parse_aggregate_body()?;
                    if is_union {
                        self.compute_union_layout(&name, &members);
                        self.globales_pendientes
                            .push(GlobalDecl::Union(name.clone(), members));
                    } else {
                        self.compute_struct_layout(&name, &members);
                        self.globales_pendientes
                            .push(GlobalDecl::Struct(name.clone(), members));
                    }
                }
                if is_union { TypeSpec::UnionRef(name) } else { TypeSpec::StructRef(name) }
            }
            // * An `enum` IS a type, and `int` is the type it is.
            //
            // Without this arm the specifier was only understood at file
            // scope, so `typedef enum { A, B } thing_t;` failed with "expected
            // type, got Enum" -- and that is the single most common way C code
            // declares an enum. It reached 30 of DOOM's 81 files.
            //
            // The token is pushed back for `parse_enum_spec`, which owns the
            // whole shape (optional tag, optional body) so the two places
            // cannot disagree about what an enum looks like.
            Token::Enum => {
                self.pos -= 1;
                self.parse_enum_spec()?;
                TypeSpec::Int
            }
            Token::Ident(name) => {
                if let Some(typ) = self.typedefs.get(&name).cloned() {
                    typ
                } else {
                    return Err(CError::new(self.line(),format!("expected type, got {:?}", Token::Ident(name))));
                }
            }
            t => return Err(CError::new(self.line(),format!("expected type, got {:?}", t))),
        };
        // * El tipo BASE, **antes** de los asteriscos. Lo necesitan los
        // declaradores que vengan detras de una coma: en `int *a, b;` la `b`
        // es un `int`, no un puntero -- el asterisco es del DECLARADOR.
        self.base_del_declarador = base.clone();
        // punteros multinivel: int **pp, char ***ppp, ...
        let mut typ = base;
        while *self.peek() == Token::Star {
            self.advance();
            typ = TypeSpec::Ptr(Box::new(typ));
            // * `char * const p` -- el calificador va DETRAS del asterisco, y
            // ahi califica al PUNTERO, no a lo apuntado.
            //
            // Solo se quitaban los de delante, asi que esto moria con
            // "expected identifier, got Const": el parser pedia el nombre y se
            // encontraba una palabra clave que en ese sitio es legal.
            //
            // Se consume y se tira, como el de delante: BMO C no comprueba
            // constancia, y fingir que si seria peor que no hacerlo.
            self.strip_qualifiers();
        }
        Ok(typ)
    }
}
