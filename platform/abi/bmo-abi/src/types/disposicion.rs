//! **La disposición de un agregado**: dónde cae cada miembro y cuánto mide el
//! conjunto.
//!
//! ═══ Por qué esto es una sola función y no tres ═══
//!
//! La regla estaba escrita **tres veces** en el proyecto, idéntica y copiada a
//! mano:
//!
//! - `lang/c/parser/mod.rs::compute_struct_layout` — el parser de C, porque
//!   los nodos `Expr::Field` llevan el offset **dentro** del AST.
//! - `lang/c/codegen/mod.rs::build_struct_layout` — el codegen de C, porque
//!   recalcula la disposición al emitir.
//! - `lang/cpp/parser.rs` — el parser de C++, por lo mismo que el de C.
//!
//! Y ninguna de las tres es evitable por su lado: quien resuelve `p.x` tiene
//! que saber el offset, y quien reserva la pila tiene que saber el tamaño.
//! Lo evitable era que la **regla** estuviera tres veces.
//!
//! Es exactamente el riesgo que avisa la cabecera de `parser/inicializador.rs`
//! —*"dos copias de un cálculo de offsets divergen"*— y llevaba dos copias
//! antes de que C++ añadiera la tercera. Una divergencia aquí no da un error:
//! da un programa que escribe en el campo de al lado.
//!
//! ═══ Por qué vive en `bmo-abi` ═══
//!
//! Porque **la disposición de un agregado ES el ABI**. Es lo que define SysV
//! para x86-64, y es lo que hace que dos trozos de código compilados por
//! separado se pongan de acuerdo sobre dónde está un campo. Los tres frontends
//! ya dependen de esta crate, así que no crea ninguna arista nueva.
//!
//! No va en `bmo-lower` a propósito: la regla de esa crate es *"L1 sólo
//! contiene lo expresable en la superficie congelada por valor"*, y una
//! disposición de memoria no se expresa en `INVOKE`.
//!
//! ═══ Por qué es un CURSOR y no una función que devuelve una lista ═══
//!
//! Porque cada llamante ya tiene su propia forma de guardar los miembros —el
//! parser de C tiene `StructMember`, el de C++ tiene `MemberVar`, el codegen
//! tiene tuplas— y devolver un `Vec` obligaría a los tres a traducir a un
//! cuarto formato para leerlo. Con un cursor, cada uno recorre lo suyo y
//! pregunta *"¿dónde cae un miembro de este tamaño?"*.
//!
//! Además `bmo_abi` es `no_std`: un cursor no asigna nada.
//!
//! ```ignore
//! let mut d = Disposicion::nueva();
//! for m in miembros {
//!     let offset = d.coloca(tamaño_de(m));
//!     // …guardar (m, offset) donde le convenga al llamante
//! }
//! let total = d.total();
//! ```

/// El alineado de un miembro, dado su tamaño.
///
/// **Es el tamaño, tapado a 8 y con mínimo 1.** El tope de 8 es lo que hace
/// que un `char[16]` no exija alinear la estructura a 16 —un array se alinea
/// como su elemento, no como el conjunto— y el mínimo de 1 evita dividir por
/// cero con un miembro de tamaño 0.
pub const fn alineado_de(tam: u32) -> u32 {
    let a = if tam > 8 { 8 } else { tam };
    if a < 1 { 1 } else { a }
}

/// Redondea `v` hacia arriba al múltiplo de `a` más cercano.
pub const fn alinear(v: u32, a: u32) -> u32 {
    let a = if a < 1 { 1 } else { a };
    (v + a - 1) / a * a
}

/// **Cuántas ranuras de pila ocupa un argumento de `bytes` bytes.**
///
/// Ésta es *la convención de llamada de BMO*, y por eso vive aquí y no dentro
/// de un frontend: BMO **no pasa argumentos en registros**, los pasa por la
/// pila en ranuras de 8 bytes, derecha a izquierda. Un agregado ocupa
/// `techo(tamaño/8)` ranuras.
///
/// Estaba escondida en `lang/c/codegen/agregados.rs` como `pub(super)`, y a la
/// vez **documentada como ABI** en `lang/cpp/CPP_ABI.md`. Una regla que un
/// documento llama ABI y el árbol guarda dentro de un lenguaje es una regla
/// que el segundo lenguaje copia — y ahí empieza la divergencia.
///
/// ★ Un agregado de 8 bytes o menos **también** ocupa una ranura entera. Podría
/// caber en un registro, pero tratarlo distinto obligaría al llamante y a la
/// función a ponerse de acuerdo sobre el tamaño, y ése es justo el desacuerdo
/// que produce basura silenciosa. Una regla, sin casos de esquina — al
/// contrario que la clasificación por *eightbytes* de SysV, que existe porque
/// SysV sí usa registros.
pub const fn ranuras(bytes: u32) -> u32 {
    if bytes <= 8 { 1 } else { (bytes + 7) / 8 }
}

/// Cursor que coloca miembros uno detrás de otro, respetando el alineado.
///
/// Vale para `struct`, para una clase de C++ y para un registro de COBOL: no
/// sabe de qué lenguaje viene, y ése es el punto.
#[derive(Debug, Clone, Copy)]
pub struct Disposicion {
    off: u32,
    max_align: u32,
}

impl Disposicion {
    pub const fn nueva() -> Self {
        // El alineado mínimo de un agregado vacío es 1: `struct {}` mide 0 y
        // no obliga a nadie a nada.
        Self { off: 0, max_align: 1 }
    }

    /// Coloca un miembro de `tam` bytes y devuelve **su offset**.
    pub fn coloca(&mut self, tam: u32) -> u32 {
        let a = alineado_de(tam);
        if a > self.max_align { self.max_align = a; }
        let off = alinear(self.off, a);
        self.off = off + tam;
        off
    }

    /// El tamaño total, **redondeado al alineado del miembro más grande**.
    ///
    /// El relleno del final no es un capricho: sin él, un array de la
    /// estructura tendría el segundo elemento mal alineado.
    pub const fn total(&self) -> u32 {
        alinear(self.off, self.max_align)
    }

    /// El alineado que exige el agregado entero.
    pub const fn alineado(&self) -> u32 { self.max_align }

    /// Los bytes ocupados **sin** el relleno del final. Para quien necesite
    /// saber dónde acaba el último miembro.
    pub const fn ocupado(&self) -> u32 { self.off }
}

/// La disposición de una **unión**: todos los miembros en el offset 0, y el
/// tamaño es el del más grande.
#[derive(Debug, Clone, Copy)]
pub struct DisposicionUnion {
    max: u32,
    max_align: u32,
}

impl DisposicionUnion {
    pub const fn nueva() -> Self { Self { max: 0, max_align: 1 } }

    /// Coloca un miembro de `tam` bytes. Devuelve su offset, que en una unión
    /// **siempre es 0** — se devuelve igual para que el llamante escriba el
    /// mismo bucle que con un struct.
    pub fn coloca(&mut self, tam: u32) -> u32 {
        if tam > self.max { self.max = tam; }
        let a = alineado_de(tam);
        if a > self.max_align { self.max_align = a; }
        0
    }

    pub const fn total(&self) -> u32 { self.max }
    pub const fn alineado(&self) -> u32 { self.max_align }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El caso que decide todo: un `char` seguido de un `int` deja **tres
    /// bytes de hueco**. Si el relleno no estuviera, el `int` empezaría en el
    /// byte 1 y una escritura de cuatro bytes cruzaría la palabra.
    #[test]
    fn char_luego_int_deja_hueco() {
        let mut d = Disposicion::nueva();
        assert_eq!(d.coloca(1), 0); // char c
        assert_eq!(d.coloca(4), 4); // int n
        assert_eq!(d.total(), 8);
        assert_eq!(d.alineado(), 4);
    }

    #[test]
    fn todo_del_mismo_tamano_va_pegado() {
        let mut d = Disposicion::nueva();
        assert_eq!(d.coloca(4), 0);
        assert_eq!(d.coloca(4), 4);
        assert_eq!(d.coloca(4), 8);
        assert_eq!(d.total(), 12);
    }

    /// El relleno del FINAL. Sin él, `struct { int n; char c; }` mediría 5 y
    /// el segundo elemento de un array empezaría en un offset impar.
    #[test]
    fn el_final_tambien_se_rellena() {
        let mut d = Disposicion::nueva();
        assert_eq!(d.coloca(4), 0); // int
        assert_eq!(d.coloca(1), 4); // char
        assert_eq!(d.ocupado(), 5);
        assert_eq!(d.total(), 8);
    }

    /// ★ El tope de 8. Un `char[16]` mide 16 pero **se alinea como un byte**:
    /// un array se alinea como su elemento. Sin el tope, esta estructura
    /// exigiría alineado 16 y mediría 32.
    #[test]
    fn el_alineado_se_tapa_en_ocho() {
        assert_eq!(alineado_de(16), 8);
        assert_eq!(alineado_de(1), 1);
        assert_eq!(alineado_de(0), 1);
        let mut d = Disposicion::nueva();
        assert_eq!(d.coloca(8), 0);
        assert_eq!(d.coloca(16), 8);
        assert_eq!(d.total(), 24);
        assert_eq!(d.alineado(), 8);
    }

    #[test]
    fn el_agregado_vacio_mide_cero() {
        let d = Disposicion::nueva();
        assert_eq!(d.total(), 0);
        assert_eq!(d.alineado(), 1);
    }

    #[test]
    fn la_union_solapa_todo_en_cero() {
        let mut u = DisposicionUnion::nueva();
        assert_eq!(u.coloca(4), 0);
        assert_eq!(u.coloca(1), 0);
        assert_eq!(u.coloca(8), 0);
        assert_eq!(u.total(), 8);
    }

    /// ★ Un agregado pequeño ocupa una ranura ENTERA. Si los de 8 bytes o
    /// menos fueran por registro, el llamante y la función tendrían que
    /// ponerse de acuerdo sobre el tamaño — y ese desacuerdo produce basura
    /// silenciosa. Una regla, sin casos de esquina.
    #[test]
    fn un_agregado_pequeno_ocupa_una_ranura_entera() {
        assert_eq!(ranuras(1), 1);
        assert_eq!(ranuras(8), 1);
        assert_eq!(ranuras(9), 2);
        assert_eq!(ranuras(12), 2);
        assert_eq!(ranuras(16), 2);
        assert_eq!(ranuras(17), 3);
        // Un tipo de tamaño cero sigue ocupando su sitio: cero ranuras
        // desalinearía todo lo que venga detrás.
        assert_eq!(ranuras(0), 1);
    }

    #[test]
    fn alinear_no_mueve_lo_que_ya_esta_alineado() {
        assert_eq!(alinear(8, 4), 8);
        assert_eq!(alinear(9, 4), 12);
        assert_eq!(alinear(0, 8), 0);
        assert_eq!(alinear(5, 1), 5);
    }
}
