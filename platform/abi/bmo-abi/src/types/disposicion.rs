//! **La disposicion de un agregado**: donde cae cada miembro y cuanto mide el
//! conjunto.
//!
//! === Por que esto es una sola funcion y no tres ===
//!
//! La regla estaba escrita **tres veces** en el proyecto, identica y copiada a
//! mano:
//!
//! - `lang/c/parser/mod.rs::compute_struct_layout` -- el parser de C, porque
//!   los nodos `Expr::Field` llevan el offset **dentro** del AST.
//! - `lang/c/codegen/mod.rs::build_struct_layout` -- el codegen de C, porque
//!   recalcula la disposicion al emitir.
//! - `lang/cpp/parser.rs` -- el parser de C++, por lo mismo que el de C.
//!
//! Y ninguna de las tres es evitable por su lado: quien resuelve `p.x` tiene
//! que saber el offset, y quien reserva la pila tiene que saber el tamano.
//! Lo evitable era que la **regla** estuviera tres veces.
//!
//! Es exactamente el riesgo que avisa la cabecera de `parser/inicializador.rs`
//! --*"dos copias de un calculo de offsets divergen"*-- y llevaba dos copias
//! antes de que C++ anadiera la tercera. Una divergencia aqui no da un error:
//! da un programa que escribe en el campo de al lado.
//!
//! === Por que vive en `bmo-abi` ===
//!
//! Porque **la disposicion de un agregado ES el ABI**. Es lo que define SysV
//! para x86-64, y es lo que hace que dos trozos de codigo compilados por
//! separado se pongan de acuerdo sobre donde esta un campo. Los tres frontends
//! ya dependen de esta crate, asi que no crea ninguna arista nueva.
//!
//! No va en `bmo-lower` a proposito: la regla de esa crate es *"L1 solo
//! contiene lo expresable en la superficie congelada por valor"*, y una
//! disposicion de memoria no se expresa en `INVOKE`.
//!
//! === Por que es un CURSOR y no una funcion que devuelve una lista ===
//!
//! Porque cada llamante ya tiene su propia forma de guardar los miembros --el
//! parser de C tiene `StructMember`, el de C++ tiene `MemberVar`, el codegen
//! tiene tuplas-- y devolver un `Vec` obligaria a los tres a traducir a un
//! cuarto formato para leerlo. Con un cursor, cada uno recorre lo suyo y
//! pregunta *"donde cae un miembro de este tamano?"*.
//!
//! Ademas `bmo_abi` es `no_std`: un cursor no asigna nada.
//!
//! ```ignore
//! let mut d = Disposicion::nueva();
//! for m in miembros {
//!     let offset = d.coloca(tamano_de(m));
//!     // ...guardar (m, offset) donde le convenga al llamante
//! }
//! let total = d.total();
//! ```

/// El alineado de un miembro, dado su tamano.
///
/// **Es el tamano, tapado a 8 y con minimo 1.** El tope de 8 es lo que hace
/// que un `char[16]` no exija alinear la estructura a 16 --un array se alinea
/// como su elemento, no como el conjunto-- y el minimo de 1 evita dividir por
/// cero con un miembro de tamano 0.
pub const fn alineado_de(tam: u32) -> u32 {
    let a = if tam > 8 { 8 } else { tam };
    if a < 1 { 1 } else { a }
}

/// Redondea `v` hacia arriba al multiplo de `a` mas cercano.
pub const fn alinear(v: u32, a: u32) -> u32 {
    let a = if a < 1 { 1 } else { a };
    (v + a - 1) / a * a
}

/// **Cuantas ranuras de pila ocupa un argumento de `bytes` bytes.**
///
/// Esta es *la convencion de llamada de BMO*, y por eso vive aqui y no dentro
/// de un frontend: BMO **no pasa argumentos en registros**, los pasa por la
/// pila en ranuras de 8 bytes, derecha a izquierda. Un agregado ocupa
/// `techo(tamano/8)` ranuras.
///
/// Estaba escondida en `lang/c/codegen/agregados.rs` como `pub(super)`, y a la
/// vez **documentada como ABI** en `lang/cpp/CPP_ABI.md`. Una regla que un
/// documento llama ABI y el arbol guarda dentro de un lenguaje es una regla
/// que el segundo lenguaje copia -- y ahi empieza la divergencia.
///
/// * Un agregado de 8 bytes o menos **tambien** ocupa una ranura entera. Podria
/// caber en un registro, pero tratarlo distinto obligaria al llamante y a la
/// funcion a ponerse de acuerdo sobre el tamano, y ese es justo el desacuerdo
/// que produce basura silenciosa. Una regla, sin casos de esquina -- al
/// contrario que la clasificacion por *eightbytes* de SysV, que existe porque
/// SysV si usa registros.
pub const fn ranuras(bytes: u32) -> u32 {
    if bytes <= 8 { 1 } else { (bytes + 7) / 8 }
}

/// Cursor que coloca miembros uno detras de otro, respetando el alineado.
///
/// Vale para `struct`, para una clase de C++ y para un registro de COBOL: no
/// sabe de que lenguaje viene, y ese es el punto.
#[derive(Debug, Clone, Copy)]
pub struct Disposicion {
    off: u32,
    max_align: u32,
}

impl Disposicion {
    pub const fn nueva() -> Self {
        // El alineado minimo de un agregado vacio es 1: `struct {}` mide 0 y
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

    /// El tamano total, **redondeado al alineado del miembro mas grande**.
    ///
    /// El relleno del final no es un capricho: sin el, un array de la
    /// estructura tendria el segundo elemento mal alineado.
    pub const fn total(&self) -> u32 {
        alinear(self.off, self.max_align)
    }

    /// El alineado que exige el agregado entero.
    pub const fn alineado(&self) -> u32 { self.max_align }

    /// Los bytes ocupados **sin** el relleno del final. Para quien necesite
    /// saber donde acaba el ultimo miembro.
    pub const fn ocupado(&self) -> u32 { self.off }
}

/// La disposicion de una **union**: todos los miembros en el offset 0, y el
/// tamano es el del mas grande.
#[derive(Debug, Clone, Copy)]
pub struct DisposicionUnion {
    max: u32,
    max_align: u32,
}

impl DisposicionUnion {
    pub const fn nueva() -> Self { Self { max: 0, max_align: 1 } }

    /// Coloca un miembro de `tam` bytes. Devuelve su offset, que en una union
    /// **siempre es 0** -- se devuelve igual para que el llamante escriba el
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
    /// bytes de hueco**. Si el relleno no estuviera, el `int` empezaria en el
    /// byte 1 y una escritura de cuatro bytes cruzaria la palabra.
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

    /// El relleno del FINAL. Sin el, `struct { int n; char c; }` mediria 5 y
    /// el segundo elemento de un array empezaria en un offset impar.
    #[test]
    fn el_final_tambien_se_rellena() {
        let mut d = Disposicion::nueva();
        assert_eq!(d.coloca(4), 0); // int
        assert_eq!(d.coloca(1), 4); // char
        assert_eq!(d.ocupado(), 5);
        assert_eq!(d.total(), 8);
    }

    /// * El tope de 8. Un `char[16]` mide 16 pero **se alinea como un byte**:
    /// un array se alinea como su elemento. Sin el tope, esta estructura
    /// exigiria alineado 16 y mediria 32.
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

    /// * Un agregado pequeno ocupa una ranura ENTERA. Si los de 8 bytes o
    /// menos fueran por registro, el llamante y la funcion tendrian que
    /// ponerse de acuerdo sobre el tamano -- y ese desacuerdo produce basura
    /// silenciosa. Una regla, sin casos de esquina.
    #[test]
    fn un_agregado_pequeno_ocupa_una_ranura_entera() {
        assert_eq!(ranuras(1), 1);
        assert_eq!(ranuras(8), 1);
        assert_eq!(ranuras(9), 2);
        assert_eq!(ranuras(12), 2);
        assert_eq!(ranuras(16), 2);
        assert_eq!(ranuras(17), 3);
        // Un tipo de tamano cero sigue ocupando su sitio: cero ranuras
        // desalinearia todo lo que venga detras.
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
