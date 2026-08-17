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
//!     let offset = d.coloca(tamano_de(m), alineado_de_tipo(m));
//!     // ...guardar (m, offset) donde le convenga al llamante
//! }
//! let total = d.total();
//! ```
//!
//! [!] **Son DOS numeros y no uno.** El alineado de un miembro no se puede
//! deducir de su tamano en cuanto el miembro es un array o un agregado, y
//! deducirlo era la version anterior de esto. Ver [`Disposicion::coloca`].

/// El alineado de un miembro **ESCALAR**, dado su tamano.
///
/// Es el tamano, tapado a 8 y con minimo 1. Para un `char`, un `short`, un
/// `int`, un `long` o un puntero el tamano Y el alineado son el mismo numero,
/// asi que aqui basta con uno.
///
/// # [!] Esto NO vale para un array ni para un agregado
///
/// Y esa confusion costo la disposicion entera de DOOM. Ver
/// [`Disposicion::coloca`]: el alineado de un array es el de su ELEMENTO y el
/// de un struct es el suyo propio, y ninguno de los dos se puede deducir del
/// tamano total. Quien coloque un miembro que no sea escalar tiene que
/// calcular su alineado y pasarlo.
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
/// vez **documentada como ABI** en `toolchain/lang/cpp/CPP_ABI.md`. Una regla que un
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

    /// Coloca un miembro de `tam` bytes con alineado `alineado`, y devuelve
    /// **su offset**.
    ///
    /// # ** Por que el alineado es un ARGUMENTO y no se deduce del tamano
    ///
    /// Porque deducirlo del tamano es lo que estaba escrito antes --`coloca`
    /// llamaba a [`alineado_de`] con el tamano del miembro-- y **es falso para
    /// todo lo que no sea un escalar**:
    ///
    /// | miembro | tamano | alineado deducido | el de verdad |
    /// |---|---|---|---|
    /// | `char name[8]` | 8 | 8 | **1** |
    /// | `short sidenum[2]` | 4 | 4 | **2** |
    /// | `mappatch_t patches[1]` | 10 | 8 | **2** |
    ///
    /// Un array se alinea como su ELEMENTO y un agregado como el mas exigente
    /// de sus miembros. El tamano total no lo dice: `char[8]` y `long` miden
    /// los dos ocho bytes y no se alinean igual.
    ///
    /// ## Lo que costo, con nombre y fichero
    ///
    /// DOOM lee sus structs **directamente de los bytes del WAD**: en
    /// `r_data.c` hace `(maptexture_t *)(maptex + offset)` y lee los campos.
    /// Con el alineado deducido del tamano, esa estructura ponia `patches` en
    /// el byte **24** y el disco lo tiene en el **22**. Y no era un caso
    /// aislado: `maplinedef_t` media 16 en vez de 14 --o sea que a partir del
    /// SEGUNDO linedef del nivel todo se leia corrido--, `mapsidedef_t` ponia
    /// las texturas en el 8 en vez del 4, y `mapsector_t` igual.
    ///
    /// * Que las estructuras de DOOM salgan exactas con el alineado natural no
    /// es suerte: estan disenadas asi, y por eso su `PACKEDATTR` puede quedarse
    /// vacio sin que nada cambie. Un compilador que las coloca bien no necesita
    /// entender `__attribute__((packed))`.
    pub fn coloca(&mut self, tam: u32, alineado: u32) -> u32 {
        let a = if alineado < 1 { 1 } else { alineado };
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

    /// Coloca un miembro de `tam` bytes con alineado `alineado`. Devuelve su
    /// offset, que en una union **siempre es 0** -- se devuelve igual para que
    /// el llamante escriba el mismo bucle que con un struct.
    pub fn coloca(&mut self, tam: u32, alineado: u32) -> u32 {
        if tam > self.max { self.max = tam; }
        let a = if alineado < 1 { 1 } else { alineado };
        if a > self.max_align { self.max_align = a; }
        0
    }

    pub const fn total(&self) -> u32 { self.max }
    pub const fn alineado(&self) -> u32 { self.max_align }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un escalar se coloca con su tamano como alineado, que es el caso comun.
    fn escalar(d: &mut Disposicion, tam: u32) -> u32 {
        d.coloca(tam, alineado_de(tam))
    }

    /// El caso que decide todo: un `char` seguido de un `int` deja **tres
    /// bytes de hueco**. Si el relleno no estuviera, el `int` empezaria en el
    /// byte 1 y una escritura de cuatro bytes cruzaria la palabra.
    #[test]
    fn char_luego_int_deja_hueco() {
        let mut d = Disposicion::nueva();
        assert_eq!(escalar(&mut d, 1), 0); // char c
        assert_eq!(escalar(&mut d, 4), 4); // int n
        assert_eq!(d.total(), 8);
        assert_eq!(d.alineado(), 4);
    }

    #[test]
    fn todo_del_mismo_tamano_va_pegado() {
        let mut d = Disposicion::nueva();
        assert_eq!(escalar(&mut d, 4), 0);
        assert_eq!(escalar(&mut d, 4), 4);
        assert_eq!(escalar(&mut d, 4), 8);
        assert_eq!(d.total(), 12);
    }

    /// El relleno del FINAL. Sin el, `struct { int n; char c; }` mediria 5 y
    /// el segundo elemento de un array empezaria en un offset impar.
    #[test]
    fn el_final_tambien_se_rellena() {
        let mut d = Disposicion::nueva();
        assert_eq!(escalar(&mut d, 4), 0); // int
        assert_eq!(escalar(&mut d, 1), 4); // char
        assert_eq!(d.ocupado(), 5);
        assert_eq!(d.total(), 8);
    }

    /// ** Un array se alinea como su ELEMENTO, y eso no se deduce del
    /// tamano.** `char[16]` mide 16 y se alinea a 1; un `long` mide 8 y se
    /// alinea a 8. La version anterior de `coloca` deducia el alineado del
    /// tamano y por eso ponia el `char[16]` en el byte 8.
    #[test]
    fn un_array_se_alinea_como_su_elemento() {
        let mut d = Disposicion::nueva();
        assert_eq!(escalar(&mut d, 1), 0); // char c
        assert_eq!(d.coloca(16, 1), 1); // char t[16] -- PEGADO, alineado 1
        assert_eq!(d.total(), 17);
        assert_eq!(d.alineado(), 1);
    }

    #[test]
    fn el_agregado_vacio_mide_cero() {
        let d = Disposicion::nueva();
        assert_eq!(d.total(), 0);
        assert_eq!(d.alineado(), 1);
    }

    // == LAS ESTRUCTURAS DE DISCO DE DOOM =============================
    //
    // No son ejemplos inventados: son los structs que `r_data.c` y `p_setup.c`
    // castean **encima de los bytes crudos del WAD**. El numero de la derecha
    // es el del formato de fichero, que lleva fijo desde 1993 y no negocia.
    //
    // Por eso valen como test de un ABI: es la unica disposicion que existe
    // ademas de la que calcula el compilador, y estan obligadas a coincidir.

    /// `mappatch_t` -- cinco shorts, 10 bytes en disco.
    fn mappatch() -> Disposicion {
        let mut d = Disposicion::nueva();
        for _ in 0..5 {
            escalar(&mut d, 2);
        }
        d
    }

    #[test]
    fn maptexture_t_pone_los_parches_en_el_22() {
        let mp = mappatch();
        assert_eq!(mp.total(), 10, "mappatch_t mide 10 en disco");
        assert_eq!(mp.alineado(), 2, "y se alinea a 2: son todo shorts");

        let mut d = Disposicion::nueva();
        assert_eq!(d.coloca(8, 1), 0, "char name[8]");
        assert_eq!(escalar(&mut d, 4), 8, "int masked");
        assert_eq!(escalar(&mut d, 2), 12, "short width");
        assert_eq!(escalar(&mut d, 2), 14, "short height");
        assert_eq!(escalar(&mut d, 4), 16, "int obsolete");
        assert_eq!(escalar(&mut d, 2), 20, "short patchcount");
        // ** LA CASILLA QUE MATABA A `R_InitTextures`: con el alineado deducido
        // del tamano (10 -> 8) esto caia en el 24.
        assert_eq!(
            d.coloca(mp.total(), mp.alineado()),
            22,
            "mappatch_t patches[1] va en el 22, no en el 24"
        );
    }

    /// `maplinedef_t` -- **14 bytes**, y el tamano importa tanto como los
    /// offsets: `p_setup.c` recorre el lump como un array, asi que un byte de
    /// mas en el total corre TODOS los linedefs a partir del segundo.
    #[test]
    fn maplinedef_t_mide_catorce_y_no_dieciseis() {
        let mut d = Disposicion::nueva();
        for _ in 0..5 {
            escalar(&mut d, 2); // v1, v2, flags, special, tag
        }
        assert_eq!(d.coloca(4, 2), 10, "short sidenum[2] va en el 10");
        assert_eq!(d.total(), 14, "y el registro entero mide 14");
    }

    /// `mapsidedef_t` -- las tres texturas son `char[8]` y van PEGADAS al
    /// segundo short, en el 4.
    #[test]
    fn mapsidedef_t_pega_las_texturas_en_el_cuatro() {
        let mut d = Disposicion::nueva();
        assert_eq!(escalar(&mut d, 2), 0, "short textureoffset");
        assert_eq!(escalar(&mut d, 2), 2, "short rowoffset");
        assert_eq!(d.coloca(8, 1), 4, "char toptexture[8]");
        assert_eq!(d.coloca(8, 1), 12, "char bottomtexture[8]");
        assert_eq!(d.coloca(8, 1), 20, "char midtexture[8]");
        assert_eq!(escalar(&mut d, 2), 28, "short sector");
        assert_eq!(d.total(), 30);
    }

    /// `mapnode_t` -- el nodo del BSP. Aqui los offsets salian bien por
    /// casualidad y **el total no**: 32 en vez de 28, o sea el arbol entero
    /// leido corrido a partir del segundo nodo.
    #[test]
    fn mapnode_t_mide_veintiocho() {
        let mut d = Disposicion::nueva();
        for _ in 0..4 {
            escalar(&mut d, 2); // x, y, dx, dy
        }
        assert_eq!(d.coloca(16, 2), 8, "short bbox[2][4]");
        assert_eq!(d.coloca(4, 2), 24, "unsigned short children[2]");
        assert_eq!(d.total(), 28);
    }

    #[test]
    fn la_union_solapa_todo_en_cero() {
        let mut u = DisposicionUnion::nueva();
        assert_eq!(u.coloca(4, 4), 0);
        assert_eq!(u.coloca(1, 1), 0);
        assert_eq!(u.coloca(8, 8), 0);
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
