//! **Listas de inicialización**: `{1, 2}`, `{.x = 1, .y = 2}`, `{[3] = 7}`.
//!
//! ═══ Por qué esto es un fichero aparte ═══
//!
//! Es el único sitio de C donde el **tipo** y la **sintaxis** tienen que
//! mirarse a la vez: no se puede leer `{1, 2}` sin saber qué se está
//! inicializando, porque las mismas llaves significan dos campos, dos elementos
//! o un array de arrays según lo que haya a la izquierda del `=`.
//!
//! Y es la parte que CRECE. Cada agregado nuevo que se quiera soportar
//! —uniones, arrays de structs, los rangos `[1 ... 5] =` de GCC, los literales
//! compuestos `(struct P){…}`— aterriza aquí y en ningún otro sitio. Metido en
//! `parse_stmt`, cada una de esas cosas engordaría una función que ya decide
//! catorce cosas más.
//!
//! ═══ Cómo lo hacen los compiladores maestros ═══
//!
//! Hay **dos escuelas**, y elegir mal se paga durante años:
//!
//! - **GCC** (`gcc/c/c-typeck.c`) — una *pila de constructores* incremental:
//!   `push_init_level` / `set_init_index` / `set_init_label` /
//!   `process_init_element` / `pop_init_level`. Cada nivel lleva un cursor
//!   (`constructor_fields` o `constructor_index`) y un designador lo
//!   **reposiciona**; luego cada elemento lo consume y lo avanza. Ventaja: no
//!   hace falta tener la lista entera en memoria, que importa cuando alguien
//!   escribe una tabla de cien mil entradas. Coste: la lógica queda repartida
//!   por el parser, y `c-typeck.c` pasa de las 16.000 líneas.
//!
//! - **Clang** (`clang/lib/Sema/SemaInit.cpp`, `InitListChecker`) —
//!   **desazucarar**. Parsea la lista tal cual, y luego la reescribe a una
//!   forma *posicional* completamente expandida (la pareja `InitListExpr`
//!   sintáctica / semántica, más `FillInEmptyInitializations`). El generador de
//!   código **nunca ve un designador**.
//!
//! - **chibicc** (Rui Ueyama) — la misma idea de Clang en ~300 líneas: un árbol
//!   `Initializer` que espeja el tipo, y después se aplana a asignaciones.
//!
//! - **TCC** (Bellard, `tccgen.c:decl_initializer`) — una sola pasada que
//!   calcula el offset y emite ahí mismo. Lo más pequeño que funciona.
//!
//! - **MSVC** — el contraejemplo, y por eso vale la pena nombrarlo: su
//!   compilador de C **no tuvo designated initializers hasta 2020**
//!   (VS 2019 16.8, con `/std:c11`). Veinte años de `#ifdef _MSC_VER` en medio
//!   mundo. Un frontend que no termina el estándar que dice hablar se lo cobra
//!   el ecosistema, no él.
//!
//! ═══ Lo que hace BMO, y por qué ═══
//!
//! **Desazucarado, como Clang y chibicc.** La salida es un `Vec<Escritura>`:
//! una lista plana de *(offset, tipo, valor)*. Tres razones:
//!
//! 1. El codegen de BMO es un emisor directo, sin IR intermedia. Si los
//!    designadores le llegaran, tendría que saber de disposiciones de structs
//!    **por segunda vez** — ya las sabe el parser, y dos copias de un cálculo
//!    de offsets divergen.
//! 2. *Contratos y formatos, nunca cerebros*: `Escritura` es un formato. El
//!    codegen guarda bytes en offsets, que es una tabla, no un intérprete.
//! 3. Se audita leyendo una función. La pila incremental de GCC es mejor
//!    ingeniería para el problema de GCC —listas gigantes— y BMO no lo tiene:
//!    aquí no hay allocator y los programas son pequeños.
//!
//! ═══ Las reglas de C99 que se respetan ═══
//!
//! - **§6.7.9/21 — lo no mencionado vale CERO.** No es cosa de este fichero:
//!   el codegen borra el objeto entero antes de escribir. Lo que sí es cosa de
//!   aquí es no fingir que se escribió.
//! - **Un designador reposiciona el cursor, y lo siguiente sigue DESDE AHÍ.**
//!   `{[2] = 'c', 'd'}` pone la `d` en el índice 3, no en el 0. Es la regla que
//!   más se olvida al implementar esto a mano.
//! - **Cadenas de designadores**: `.a.b = 3` y `[1].c = 4` son legales.
//! - **El último gana**: `{.x = 1, .x = 2}` deja 2, y sale solo de emitir en
//!   orden.
//! - `int x = {5}` — un escalar entre llaves es legal.

use super::Parser;
use crate::ast::*;
use crate::lexer::Token;
use crate::CError;

impl Parser {
    /// Termina una declaración ya reconocida: el `= …` opcional y el `;`.
    ///
    /// ★ Existe porque esto estaba **copiado en tres sitios** —el cuerpo de una
    /// función, un bloque anidado y `parse_stmt`— con el mismo `if Assign {
    /// parse_expr }` en cada uno. Al añadir las listas `{ … }` sólo aprendió
    /// uno de los tres, y `int a[4] = {…}` seguía sin compilar dentro de un
    /// `if`. Tres copias de una regla se quedan viejas en dos.
    ///
    /// Registrar el tipo en `var_types` también estaba repetido y también viaja
    /// aquí: es parte de "declarar", no del sitio donde se declara.
    pub(super) fn terminar_declaracion(
        &mut self,
        typ: TypeSpec,
        name: String,
    ) -> Result<Stmt, CError> {
        self.var_types.insert(name.clone(), typ.clone());
        if *self.peek() != Token::Assign {
            self.skip_semicolon();
            return Ok(Stmt::DeclAssign(typ, name, None));
        }
        self.advance();
        // `= {` es una LISTA, no una expresión: se resuelve contra el tipo y
        // sale un conjunto de escrituras.
        if *self.peek() == Token::OpenBrace {
            let escrituras = self.parse_inicializador(&typ)?;
            self.skip_semicolon();
            return Ok(Stmt::DeclInit(typ, name, escrituras));
        }
        // `char s[8] = "hola"` — SIN llaves, y aun así es un agregado. Es la
        // única excepción de C a "un agregado se inicializa con llaves", y sin
        // ella lo que se guardaba en el array era el PUNTERO a la cadena.
        if let (TypeSpec::Array(_, _), Token::StringLit(_)) = (&typ, self.peek()) {
            let mut escrituras = Vec::new();
            self.cadena_a_array(&typ, 0, &mut escrituras)?;
            self.skip_semicolon();
            return Ok(Stmt::DeclInit(typ, name, escrituras));
        }
        // ★ `parse_assign`, no `parse_expr`. Y esto es gramática de C, no un
        // atajo: el inicializador de un declarador es una
        // *assignment-expression*, **no** una *expression* — precisamente para
        // que la coma pueda separar declaradores.
        //
        // Con `parse_expr` aquí, `int a = 20, b = 22;` se leía como
        // `a = (20, b = 22)` usando el operador coma: la `a` acababa valiendo
        // 22 y `b` no se declaraba nunca. La gramática del estándar tiene ese
        // escalón exactamente por este motivo.
        let valor = self.parse_assign()?;
        self.skip_semicolon();
        Ok(Stmt::DeclAssign(typ, name, Some(valor)))
    }

    /// Punto de entrada: `= { … }` para un objeto de tipo `tipo`.
    ///
    /// Devuelve las escrituras en orden de aparición, con offsets **absolutos**
    /// desde el principio del objeto.
    pub(super) fn parse_inicializador(
        &mut self,
        tipo: &TypeSpec,
    ) -> Result<Vec<Escritura>, CError> {
        let mut out = Vec::new();
        self.lista(tipo, 0, &mut out)?;
        Ok(out)
    }

    /// Una lista `{ … }` para `tipo`, escribiendo a partir de `base`.
    fn lista(
        &mut self,
        tipo: &TypeSpec,
        base: u32,
        out: &mut Vec<Escritura>,
    ) -> Result<(), CError> {
        self.expect(&Token::OpenBrace)?;
        // El cursor del nivel: qué subobjeto toca si NO viene designador.
        let mut cursor = 0usize;

        loop {
            if *self.peek() == Token::CloseBrace {
                self.advance();
                return Ok(());
            }
            if *self.peek() == Token::Eof {
                return Err(CError::new(self.line(), "falta la } de la lista de inicializacion"));
            }

            // ¿Designador, o seguimos por donde ibamos?
            let (t_sub, off_sub, siguiente) = if matches!(*self.peek(), Token::Dot | Token::OpenBracket)
            {
                self.designadores(tipo, base)?
            } else {
                let (t, o) = self.subobjeto_por_indice(tipo, cursor)?;
                (t, base + o, cursor + 1)
            };
            cursor = siguiente;

            // El valor.
            if *self.peek() == Token::OpenBrace {
                // Anidado: `{ {1,2}, {3,4} }`.
                self.lista(&t_sub, off_sub, out)?;
            } else if matches!(
                (&t_sub, self.peek()),
                (TypeSpec::Array(_, _), Token::StringLit(_))
            ) {
                self.cadena_a_array(&t_sub, off_sub, out)?;
            } else {
                let valor = self.parse_assign()?;
                out.push(Escritura { offset: off_sub, tipo: t_sub, valor });
            }

            match self.peek() {
                Token::Comma => {
                    self.advance();
                }
                Token::CloseBrace => {}
                t => {
                    return Err(CError::new(
                        self.line(),
                        format!("en una lista de inicializacion esperaba , o }}, no {t:?}"),
                    ))
                }
            }
        }
    }

    /// Una cadena de designadores (`.a`, `[3]`, `.a.b`, `[1].c`) seguida de `=`.
    ///
    /// Devuelve `(tipo del subobjeto, offset absoluto, cursor del nivel de
    /// arriba tras este elemento)`.
    ///
    /// ★ El tercer valor es la regla que más se olvida: tras `[2] = 'c'`, el
    /// elemento siguiente SIN designador va al índice **3**, no al 0. El cursor
    /// del nivel actual se reposiciona con el PRIMER designador de la cadena.
    fn designadores(
        &mut self,
        tipo: &TypeSpec,
        base: u32,
    ) -> Result<(TypeSpec, u32, usize), CError> {
        let mut t_actual = tipo.clone();
        let mut off = base;
        let mut cursor_nivel: Option<usize> = None;

        loop {
            match self.peek().clone() {
                Token::Dot => {
                    self.advance();
                    let Token::Ident(campo) = self.advance() else {
                        return Err(CError::new(self.line(), "tras . esperaba el nombre de un campo"));
                    };
                    let (t, o, idx) = self.subobjeto_por_nombre(&t_actual, &campo)?;
                    if cursor_nivel.is_none() {
                        cursor_nivel = Some(idx + 1);
                    }
                    t_actual = t;
                    off += o;
                }
                Token::OpenBracket => {
                    self.advance();
                    let idx = self.indice_constante()?;
                    self.expect(&Token::CloseBracket)?;
                    let (t, o) = self.subobjeto_por_indice(&t_actual, idx)?;
                    if cursor_nivel.is_none() {
                        cursor_nivel = Some(idx + 1);
                    }
                    t_actual = t;
                    off += o;
                }
                _ => break,
            }
        }

        self.expect(&Token::Assign)?;
        Ok((t_actual, off, cursor_nivel.unwrap_or(0)))
    }

    /// El índice de un `[n]` designador. Tiene que conocerse al COMPILAR: el
    /// offset donde se escribe no puede depender de algo que sólo se sabe al
    /// ejecutar.
    fn indice_constante(&mut self) -> Result<usize, CError> {
        match self.advance() {
            Token::IntLit(n) if n >= 0 => Ok(n as usize),
            Token::Ident(nombre) => match self.enum_constants.get(&nombre) {
                Some(&v) if v >= 0 => Ok(v as usize),
                _ => Err(CError::new(
                    self.line(),
                    format!("'{nombre}' no es una constante: el indice de un designador se resuelve al compilar"),
                )),
            },
            t => Err(CError::new(
                self.line(),
                format!("el indice de un designador tiene que ser una constante, no {t:?}"),
            )),
        }
    }

    /// El subobjeto número `i`: campo `i` de un struct, elemento `i` de un
    /// array. Devuelve `(tipo, offset relativo)`.
    fn subobjeto_por_indice(
        &self,
        tipo: &TypeSpec,
        i: usize,
    ) -> Result<(TypeSpec, u32), CError> {
        match tipo {
            TypeSpec::Array(elem, n) => {
                if i >= *n as usize {
                    return Err(CError::new(
                        self.line(),
                        format!("el array tiene {n} elementos y se inicializa el {i}"),
                    ));
                }
                Ok(((**elem).clone(), i as u32 * self.tamano_de(elem)))
            }
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => {
                let campos = self.struct_fields.get(s).ok_or_else(|| {
                    CError::new(self.line(), format!("no conozco el struct '{s}'"))
                })?;
                let (nombre, off, _) = campos.get(i).ok_or_else(|| {
                    CError::new(
                        self.line(),
                        format!("'{s}' tiene {} campos y se inicializa el {i}", campos.len()),
                    )
                })?;
                let t = self
                    .field_types
                    .get(&(s.clone(), nombre.clone()))
                    .cloned()
                    .unwrap_or(TypeSpec::Long);
                Ok((t, *off))
            }
            // Un escalar entre llaves: `int x = {5}`. Legal, y el único
            // subobjeto es él mismo.
            otro if i == 0 => Ok((otro.clone(), 0)),
            _ => Err(CError::new(
                self.line(),
                "sobran valores: un escalar solo admite uno".to_string(),
            )),
        }
    }

    /// `"hola"` → una escritura por byte dentro de un `char[]`.
    ///
    /// Una cadena que inicializa un array **no es un puntero**: son sus bytes,
    /// copiados dentro. Guardar el puntero —que es lo que pasaba antes— deja un
    /// `char[8]` con una dirección en los primeros ocho bytes y basura detrás,
    /// y un `%s` imprime lo que haya en `.rodata` a partir de ahí.
    ///
    /// El cero final entra **si cabe**: `char c[4] = "hola"` son cuatro letras
    /// sin terminador y es legal en C; `char c[5] = "hola"` sí lo lleva. Poner
    /// uno de más pisaría el campo siguiente.
    fn cadena_a_array(
        &mut self,
        tipo: &TypeSpec,
        base: u32,
        out: &mut Vec<Escritura>,
    ) -> Result<(), CError> {
        let TypeSpec::Array(elem, n) = tipo else {
            return Err(CError::new(self.line(), "una cadena solo inicializa un array"));
        };
        if !matches!(**elem, TypeSpec::Char | TypeSpec::UnsignedChar) {
            return Err(CError::new(
                self.line(),
                "una cadena solo inicializa un array de char",
            ));
        }
        let Token::StringLit(s) = self.advance() else {
            return Err(CError::new(self.line(), "esperaba una cadena"));
        };
        let bytes = s.as_bytes();
        if bytes.len() > *n as usize {
            return Err(CError::new(
                self.line(),
                format!("la cadena ocupa {} bytes y el array tiene {n}", bytes.len()),
            ));
        }
        for (i, b) in bytes.iter().enumerate() {
            out.push(Escritura {
                offset: base + i as u32,
                tipo: TypeSpec::Char,
                valor: Expr::CharLit(*b),
            });
        }
        Ok(())
    }

    /// El tamaño REAL de un tipo, structs incluidos.
    ///
    /// ★ `TypeSpec::stack_size()` devuelve **0** para `StructRef` y `UnionRef`
    /// —el tamaño no está en el tipo, está en la tabla de disposiciones— y
    /// usarla aquí ponía todos los elementos de un `struct P v[2]` en el mismo
    /// offset: `v[1]` escribía encima de `v[0]`. Compilaba, corría, y daba
    /// números que parecían plausibles.
    fn tamano_de(&self, tipo: &TypeSpec) -> u32 {
        match tipo {
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => {
                self.struct_sizes.get(s).copied().unwrap_or(8)
            }
            TypeSpec::Array(elem, n) => self.tamano_de(elem) * n,
            otro => otro.stack_size(),
        }
    }

    /// El campo llamado `nombre`. Devuelve `(tipo, offset relativo, indice)`.
    fn subobjeto_por_nombre(
        &self,
        tipo: &TypeSpec,
        nombre: &str,
    ) -> Result<(TypeSpec, u32, usize), CError> {
        let Some(s) = Self::struct_of(tipo) else {
            return Err(CError::new(
                self.line(),
                format!("'.{nombre}' pero esto no es un struct ni una union"),
            ));
        };
        let campos = self
            .struct_fields
            .get(s)
            .ok_or_else(|| CError::new(self.line(), format!("no conozco el struct '{s}'")))?;
        let idx = campos.iter().position(|(n, _, _)| n == nombre).ok_or_else(|| {
            CError::new(self.line(), format!("'{s}' no tiene un campo '{nombre}'"))
        })?;
        let off = campos[idx].1;
        let t = self
            .field_types
            .get(&(s.to_string(), nombre.to_string()))
            .cloned()
            .unwrap_or(TypeSpec::Long);
        Ok((t, off, idx))
    }
}
