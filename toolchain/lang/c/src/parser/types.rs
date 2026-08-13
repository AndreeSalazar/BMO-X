//! **TYPE AND LAYOUT RESOLUTION** -- what a name refers to, and where in a
//! struct it lives.
//!
//! === Why this is a file of its own ===
//!
//! Because the parser of C cannot stay a parser. In most languages `a.b.c`
//! stays symbolic until a later pass resolves it; here the `Expr::Field` node
//! **carries the byte offset inside it**, so by the time the tree is built the
//! parser has already had to answer "what type is `a`, what does `b` point at,
//! and how far in is `c`".
//!
//! That is a type checker's job living inside a parser, and it has its own
//! state (`struct_sizes`, `struct_aligns`, `field_types`) that nothing else
//! here touches. Kept in the same drawer as the grammar it was invisible;
//! sitting on its own it is obvious that it is a second subsystem.
//!
//! === ** What being invisible cost, on 2026-08-13 ===
//!
//! `compute_struct_layout` measured members with `TypeSpec::stack_size()`,
//! which answers **0** for a `StructRef` -- from the bare AST there is no size
//! table. The codegen, meanwhile, consulted its own table. **Two offset
//! calculations that disagreed**, which is exactly what `bmo_abi::Disposicion`
//! was written to make impossible, and the disagreement was two files apart.
//!
//! And the layout rule underneath was wrong for anything holding an array:
//! alignment was derived from the member's SIZE. `char name[8]` is eight bytes
//! wide, same as a `long`, and aligns to one. Every struct DOOM casts onto the
//! raw bytes of a WAD came out shifted. See `probe_layout` in the bench.

use super::*;

impl Parser {
    pub(super) fn get_field_offset(&self, struct_name: &str, field: &str) -> Option<u32> {
        self.struct_fields.get(struct_name).and_then(|fields| {
            fields.iter().find(|(n, _, _)| n == field).map(|(_, off, _)| *off)
        })
    }

    /// Nombre del struct/union del que un TypeSpec ES valor directo.
    pub(super) fn struct_of(t: &TypeSpec) -> Option<&str> {
        match t {
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s),
            _ => None,
        }
    }

    /// Nombre del struct/union al que un TypeSpec APUNTA (un nivel de *).
    pub(super) fn pointee_struct_of(t: &TypeSpec) -> Option<&str> {
        match t {
            TypeSpec::Ptr(base) => Self::struct_of(base),
            _ => None,
        }
    }

    /// Tipo estatico de una expresion, hasta donde el parser puede saberlo.
    /// Devuelve None si no es resoluble (y el offset caera a 0 -- visible en tests).
    pub(super) fn resolve_expr_type(&self, expr: &Expr) -> Option<TypeSpec> {
        match expr {
            Expr::Var(n) => self.var_types.get(n).cloned(),
            Expr::Subscript(n, _, _) => {
                // arr[i]: el tipo del elemento
                match self.var_types.get(n)? {
                    TypeSpec::Ptr(base) => Some(base.as_ref().clone()),
                    TypeSpec::Array(base, _) => Some(base.as_ref().clone()),
                    t => Some(t.clone()),
                }
            }
            Expr::Deref(inner) => {
                match self.resolve_expr_type(inner)? {
                    TypeSpec::Ptr(base) => Some(*base),
                    // * `*tabla` sobre un ARRAY es su primer elemento.
                    //
                    // Un array decae a puntero en cuanto se usa en una
                    // expresion, y aqui no decaia: solo se aceptaba `Ptr`. El
                    // precio lo pagaba el modismo mas comun de C --
                    // `sizeof(t) / sizeof(*t)` para contar los elementos-- que
                    // aparece en nueve ficheros de DOOM y en casi todo
                    // programa que recorra una tabla.
                    TypeSpec::Array(base, _) => Some(*base),
                    _ => None,
                }
            }
            Expr::AddrOf(inner) => {
                let t = self.resolve_expr_type(inner)?;
                Some(TypeSpec::Ptr(Box::new(t)))
            }
            Expr::Field(base, fname, _, _) => {
                // base.f: tipo del campo f en el struct de base
                let s = self.resolve_struct_type(base)?;
                self.field_types.get(&(s, fname.clone())).cloned()
            }
            Expr::Arrow(base, fname, _, _) => {
                // base->f: tipo del campo f en el struct APUNTADO por base
                let t = self.resolve_expr_type(base)?;
                let s = Self::pointee_struct_of(&t)?.to_string();
                self.field_types.get(&(s, fname.clone())).cloned()
            }
            // * Los que faltaban, y los cuatro son EXACTOS.
            //
            // Aparecieron pidiendolos `sizeof`, que desde hoy acepta una
            // expresion. Se anaden solo estos y no la aritmetica: el tipo de
            // `a + b` pide las conversiones usuales de C, y **equivocarse aqui
            // no da un error, da un `memset` de la medida equivocada**. Sin
            // resolver, `sizeof` lo dice; adivinando, lo escribe.
            //
            // `p[i]` ya trae el tipo del elemento dentro del nodo, que es
            // justo lo que hace que este sea exacto y no una suposicion.
            Expr::IndexPtr(_, _, elem) => Some(elem.clone()),
            Expr::Cast(t, _) => Some(t.clone()),
            Expr::Int(_) => Some(TypeSpec::Int),
            Expr::CharLit(_) => Some(TypeSpec::Char),
            // En C `sizeof("abc")` son CUATRO: el literal es un array con su
            // cero, no un puntero.
            Expr::StringLit(s) => Some(TypeSpec::Array(
                Box::new(TypeSpec::Char),
                s.len() as u32 + 1,
            )),
            _ => None,
        }
    }

    /// Struct/union del que la expresion ES valor (para `expr.field`).
    pub(super) fn resolve_struct_type(&self, expr: &Expr) -> Option<String> {
        let t = self.resolve_expr_type(expr)?;
        match &t {
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s.clone()),
            // permisivo historico: p[i] con p: struct* ya cae en resolve_expr_type
            _ => None,
        }
    }

    pub(super) fn resolve_field_expr_offset(&self, expr: &Expr, field: &str) -> u32 {
        self.resolve_struct_type(expr)
            .and_then(|s| self.get_field_offset(&s, field))
            .unwrap_or(0)
    }

    /// Tipo del campo para `expr.field` (base por valor).
    pub(super) fn field_type_via_value(&self, expr: &Expr, field: &str) -> TypeSpec {
        self.resolve_struct_type(expr)
            .and_then(|s| self.field_types.get(&(s, field.to_string())).cloned())
            .unwrap_or(TypeSpec::Long)
    }

    /// Tipo del campo para `expr->field` (base puntero).
    pub(super) fn field_type_via_pointer(&self, expr: &Expr, field: &str) -> TypeSpec {
        self.resolve_expr_type(expr)
            .and_then(|t| Self::pointee_struct_of(&t).map(str::to_string))
            .and_then(|s| self.field_types.get(&(s, field.to_string())).cloned())
            .unwrap_or(TypeSpec::Long)
    }

    pub(super) fn resolve_arrow_expr_offset(&self, expr: &Expr, field: &str) -> u32 {
        // expr->field: expr es puntero a struct; funciona ANIDADO (a->b->c)
        // porque resolve_expr_type sigue los tipos de campo registrados.
        self.resolve_expr_type(expr)
            .and_then(|t| Self::pointee_struct_of(&t).map(str::to_string))
            .and_then(|s| self.get_field_offset(&s, field))
            .unwrap_or(0)
    }

    /// Tamano del elemento apuntado/contenido por `base` (para escalar subindices).
    /// * THE STRIDE OF ONE STEP, AND WHY IT IS NOT A `u8`.
    ///
    /// For `int grid[2][3]`, one step of the outer index is a whole ROW: three
    /// ints, twelve bytes. The old version answered 8 for any array-of-array
    /// (it fell through to a catch-all), so `grid[1][0]` read `grid[0][2]`.
    /// That compiles, runs, and prints a plausible number -- the failure mode
    /// this compiler's own test bench exists to catch.
    ///
    /// It returns `u32` because a row is not small: `gammatable[5][256]` steps
    /// 256 bytes, and a table of 1024 ints steps 4096. A `u8` here does not
    /// clamp, it WRAPS, which is the same bug with a bigger table.
    pub(super) fn pointee_size(&self, base: &TypeSpec) -> u32 {
        match base {
            TypeSpec::Char | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
            TypeSpec::Float => 4, TypeSpec::Double => 8,
            TypeSpec::Void => 1,
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => *self.struct_sizes.get(s.as_str()).unwrap_or(&8) as u32,
            TypeSpec::Array(inner, n) => self.pointee_size(inner).saturating_mul(*n),
            _ => 8,
        }
    }

    pub(super) fn element_size(&self, name: &str) -> u32 {
        if let Some(typ) = self.var_types.get(name) {
            match typ {
                TypeSpec::Char => 1, TypeSpec::UnsignedChar => 1,
                TypeSpec::Short => 2, TypeSpec::UnsignedShort => 2,
                TypeSpec::Int => 4, TypeSpec::UnsignedInt => 4,
                TypeSpec::Long | TypeSpec::UnsignedLong => 8,
                TypeSpec::Ptr(ref base) => self.pointee_size(base),
                TypeSpec::Array(ref base, _) => self.pointee_size(base),
                _ => 8,
            }
        } else { 8 }
    }

    /// La regla de disposicion **ya no esta aqui**: vive una sola vez en
    /// `bmo_abi::types::disposicion`. Estaba copiada a mano en tres sitios
    /// --este, `codegen::build_struct_layout` y el parser de C++-- y una
    /// divergencia entre ellas no da un error: da un programa que escribe en
    /// el campo de al lado.
    pub(super) fn compute_struct_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::Disposicion::nueva();
        for m in members {
            let sz = self.type_size(&m.typ);
            layout.push((m.name.clone(), d.coloca(sz, self.type_align(&m.typ)), sz));
            self.field_types.insert((name.to_string(), m.name.clone()), m.typ.clone());
        }
        self.struct_fields.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), d.total());
        self.struct_aligns.insert(name.to_string(), d.alineado());
    }

    pub(super) fn compute_union_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::DisposicionUnion::nueva();
        for m in members {
            let sz = self.type_size(&m.typ);
            layout.push((m.name.clone(), d.coloca(sz, self.type_align(&m.typ)), sz));
            self.field_types.insert((name.to_string(), m.name.clone()), m.typ.clone());
        }
        self.struct_fields.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), d.total());
        self.struct_aligns.insert(name.to_string(), d.alineado());
    }

    /// The size of a type **with the struct table in front of it**.
    ///
    /// [!] This is not `TypeSpec::stack_size()`, and the difference is not
    /// cosmetic: that one answers **0** for a `StructRef`, because from the
    /// bare AST there is no size table to consult. Using it here placed a
    /// member that was itself a struct with size zero, i.e. **on top of the
    /// next one** -- and in disagreement with the codegen, which does consult
    /// its table. Two offset calculations diverging is exactly what this class
    /// was written to prevent.
    ///
    /// It is the same defect already paid for once in `pointer_scale`: `p + 1`
    /// on a `struct T *` advanced ONE byte because the size was asked of the
    /// AST.
    pub(super) fn type_size(&self, typ: &TypeSpec) -> u32 {
        match typ {
            TypeSpec::StructRef(n) | TypeSpec::UnionRef(n) => {
                self.struct_sizes.get(n.as_str()).copied().unwrap_or(8)
            }
            TypeSpec::Array(t, n) => self.type_size(t).saturating_mul(*n),
            otro => otro.stack_size(),
        }
    }

    /// The alignment of a type. An array aligns like its ELEMENT and an
    /// aggregate like its most demanding member; neither follows from the total
    /// size. Twin of `codegen::type_align`, and the two have to agree.
    pub(super) fn type_align(&self, typ: &TypeSpec) -> u32 {
        match typ {
            TypeSpec::Array(t, _) => self.type_align(t),
            TypeSpec::StructRef(n) | TypeSpec::UnionRef(n) => {
                self.struct_aligns.get(n.as_str()).copied().unwrap_or(8)
            }
            TypeSpec::Ptr(_) => 8,
            otro => bmo_abi::types::alineado_de(self.type_size(otro)),
        }
    }
}
