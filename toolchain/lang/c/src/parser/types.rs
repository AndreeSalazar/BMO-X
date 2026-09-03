//! **LA DISPOSICION, del lado del frontend** -- cuanto mide cada agregado y
//! donde cae cada campo.
//!
//! # *** 2026-09-02: ESTE FICHERO YA NO ES UN COMPROBADOR DE TIPOS
//!
//! Lo era, y lo decia aqui:
//!
//! > *"Because the parser of C cannot stay a parser [...] the `Expr::Field`
//! > node **carries the byte offset inside it** [...] That is a type checker's
//! > job living inside a parser."*
//!
//! Era cierto, y era el diagnostico correcto de un problema que nadie habia
//! arreglado. **El nodo ya no carga el offset**: `Expr::Field` y `Expr::Arrow`
//! solo NOMBRAN el campo, y quien lo resuelve es quien tiene la tabla en el
//! momento de emitir. Con eso murieron los nueve metodos que hacian de
//! comprobador --`resolve_field_expr_offset`, `field_type_via_*`,
//! `resolve_arrow_expr_offset`, `pointee_size`, `element_size`...-- y el
//! fichero paso de 232 lineas a 139.
//!
//! Lo que queda es lo unico que de verdad es del frontend: **colocar** los
//! agregados que declara el programa, y contestar las dos preguntas del
//! `trait Ambito`. La colocacion viaja en el `Program` y el codegen la coteja
//! contra la suya -- ver `codegen::cotejar_disposicion`.
//!
//! [!] Y esto es lo que hace que el fallo del 02-09 no pueda volver: aquel
//! offset 0 se grababa AQUI, al parsear, cuando `resolve_expr_type` no sabia
//! tipar `tope - 1`. Ya no hay nada que grabar.
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
    /// Nombre del struct/union del que un TypeSpec ES valor directo.
    pub(super) fn struct_of(t: &TypeSpec) -> Option<&str> {
        match t {
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s),
            _ => None,
        }
    }

    /// Tipo estatico de una expresion.
    ///
    /// ** DELEGA EN EL JUEZ UNICO (`crate::tipos`). Hasta el 2026-09-02 esta
    /// funcion tenia su propia copia de la pregunta, y el codegen tenia otra
    /// que sabia cosas distintas -- ver la cabecera de `tipos.rs`. Lo que
    /// resolvia esta y no aquella (y al reves) era el fallo, no el reparto.
    ///
    /// Sigue devolviendo `None` cuando no se puede saber, y el llamante sigue
    /// convirtiendolo en 0. Esa parte no cambia aqui: cambia CUANTAS formas de
    /// C caen en el `None`.
    pub(super) fn resolve_expr_type(&self, expr: &Expr) -> Option<TypeSpec> {
        crate::tipos::tipo_de(self, expr)
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

/// El parser contesta la unica pregunta que el juez le hace.
///
/// [!] `var_types` incluye globales y locales porque el parser las registra en
/// el mismo mapa segun las va viendo. Si algun dia se separan, este es el
/// sitio donde se vuelven a juntar -- y no dentro del juez.
impl crate::tipos::Ambito for Parser {
    fn tipo_de_variable(&self, nombre: &str) -> Option<TypeSpec> {
        self.var_types.get(nombre).cloned()
    }

    fn tipo_de_campo(&self, agregado: &str, campo: &str) -> Option<TypeSpec> {
        self.field_types
            .get(&(agregado.to_string(), campo.to_string()))
            .cloned()
    }

    /// [!] Sale de `var_types` y no de una tabla propia **porque ahi es donde
    /// esta**: un prototipo anota su tipo de retorno bajo el nombre de la
    /// funcion, y lo dice su propio comentario en `declarations.rs` --
    /// *"lo unico que deja es el tipo de retorno anotado, para que una llamada
    /// anterior a la definicion sepa que recibe"*.
    fn tipo_de_retorno(&self, funcion: &str) -> Option<TypeSpec> {
        self.var_types.get(funcion).cloned()
    }
}
