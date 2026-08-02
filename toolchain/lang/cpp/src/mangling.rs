//! **El mangling de BMO C++** — de un nombre del lenguaje a un símbolo único.
//!
//! ═══ Por qué hace falta, dicho sin rodeos ═══
//!
//! En cuanto existe la sobrecarga, **dos funciones distintas necesitan
//! símbolos distintos** y no hay forma de esquivarlo. `f(int)` y `f(char*)`
//! son dos funciones; el emisor sólo ve nombres. El mangling se inventó en
//! Cfront exactamente por esto, para poder sobrevivir a un enlazador de C.
//!
//! ═══ Por qué NO es el de Itanium ═══
//!
//! El *Itanium C++ ABI* (`_ZN1P5dobleEv`) existe para que objetos de
//! compiladores distintos se enlacen entre sí. **BMO no enlaza nada de nadie**:
//! no hay enlazador, no hay `.o` ajenos, no hay carga dinámica, y se compila
//! una sola unidad de traducción. La compatibilidad no compra nada.
//!
//! Lo que sí hacen falta son sus tres **propiedades**, y son las que este
//! esquema cumple:
//!
//! 1. **Determinista** — el mismo nombre da siempre el mismo símbolo.
//! 2. **Sin colisiones** — dos declaraciones distintas nunca dan el mismo
//!    símbolo, y **nada que un programa pueda escribir choca con uno generado**.
//! 3. **Reversible a ojo** — se lee sin herramienta. `_ZN1P5dobleEv` necesita
//!    `c++filt`; `P.doble#v` no.
//!
//! Hay precedente dentro de casa: BMO C ya promueve una `static` de función a
//! global llamándola `funcion.variable`, porque **el punto es ilegal en C** y
//! por tanto no puede chocar. Esto es lo mismo, con más piezas.
//!
//! ═══ ★ La lección de MSVC, que es la razón de que este fichero exista ═══
//!
//! Microsoft nunca publicó la especificación de su ABI. Clang tuvo que hacerle
//! ingeniería inversa —de ahí `MicrosoftMangle.cpp`— y el ecosistema pagó años
//! partido en dos por un documento que no se escribió.
//!
//! > **Regla, no observación: el ABI de C++ de BMO se escribe el mismo día que
//! > se implementa.** Está en `CPP_ABI.md`, al lado de este fichero, y se
//! > actualiza a la vez que el código.
//!
//! ═══ El esquema ═══
//!
//! ```text
//!   [espacio.]…[Clase.]nombre#códigos-de-parámetro
//! ```
//!
//! - El **punto** separa cualificadores. Ilegal en un identificador de C++.
//! - La **almohadilla** abre la lista de parámetros. Ilegal también.
//! - Los parámetros van separados por punto; sin parámetros, no hay nada
//!   detrás del `#`.
//!
//! | C++ | símbolo |
//! |---|---|
//! | `int f()` | `f#` |
//! | `int f(int, char)` | `f#i.c` |
//! | `int f(int*)` | `f#Pi` |
//! | `int f(int&)` | `f#Ri` |
//! | `int f(Punto)` | `f#{Punto}` |
//! | `int P::doble(int)` | `P.doble#i` |
//! | `P::P(int)` | `P.P#i` |
//! | `P::~P()` | `P.~P#` |
//! | `n::f(int)` | `n.f#i` |
//!
//! ★ **El tipo de retorno NO entra**, y es a propósito: C++ no permite
//! sobrecargar por retorno, así que meterlo generaría dos símbolos para lo que
//! el lenguaje considera **la misma función** — y una llamada no sabría a cuál
//! ir.
//!
//! El constructor es `P.P` y el destructor `P.~P` porque ninguno puede chocar:
//! dentro de la clase `P`, un miembro llamado `P` **es** el constructor (el
//! lenguaje reserva el nombre), y `~` no es legal en un identificador.

use crate::ast::TypeSpec;

/// El código de un tipo dentro de la lista de parámetros.
///
/// Minúscula con signo, MAYÚSCULA sin signo. Es la única convención que hay
/// que recordar para leer un símbolo.
pub fn codigo(t: &TypeSpec) -> String {
    match t {
        TypeSpec::Void => "v".into(),
        TypeSpec::Bool => "b".into(),
        TypeSpec::Char => "c".into(),
        TypeSpec::UnsignedChar => "C".into(),
        TypeSpec::Short => "s".into(),
        TypeSpec::UnsignedShort => "S".into(),
        TypeSpec::Int => "i".into(),
        TypeSpec::UnsignedInt => "I".into(),
        TypeSpec::Long => "l".into(),
        TypeSpec::UnsignedLong => "L".into(),
        TypeSpec::LongLong => "q".into(),
        TypeSpec::UnsignedLongLong => "Q".into(),
        TypeSpec::Float => "f".into(),
        TypeSpec::Double => "d".into(),
        TypeSpec::Ptr(t) => format!("P{}", codigo(t)),
        TypeSpec::Ref(t) => format!("R{}", codigo(t)),
        TypeSpec::Array(t, n) => format!("A{n}{}", codigo(t)),
        // Las llaves delimitan el nombre para que no se confunda con los
        // códigos de una letra: sin ellas, una clase llamada `Pi` sería
        // indistinguible de un `int*`.
        TypeSpec::ClassRef(n) => format!("{{{n}}}"),
        TypeSpec::Template(n, args) => {
            let dentro: Vec<String> = args.iter().map(codigo).collect();
            format!("{{{n}<{}>}}", dentro.join(","))
        }
        // `auto` nunca llega aquí: el parser lo resuelve antes. Si llegara,
        // un símbolo con `?` es imposible de generar por accidente y sale en
        // cualquier volcado.
        TypeSpec::Auto => "?".into(),
    }
}

/// La lista de parámetros de un símbolo, con su `#` delante.
pub fn firma(params: &[TypeSpec]) -> String {
    let codigos: Vec<String> = params.iter().map(codigo).collect();
    format!("#{}", codigos.join("."))
}

/// El símbolo de una función libre, quizá dentro de espacios de nombres.
pub fn funcion(espacios: &[String], nombre: &str, params: &[TypeSpec]) -> String {
    let mut s = String::new();
    for e in espacios { s.push_str(e); s.push('.'); }
    s.push_str(nombre);
    s.push_str(&firma(params));
    s
}

/// El símbolo de un método. `this` **no** entra en la firma: va implícito en
/// la clase, y meterlo haría que todos los métodos de una clase compartieran
/// un prefijo redundante.
pub fn metodo(espacios: &[String], clase: &str, nombre: &str, params: &[TypeSpec]) -> String {
    let mut s = String::new();
    for e in espacios { s.push_str(e); s.push('.'); }
    s.push_str(clase);
    s.push('.');
    s.push_str(nombre);
    s.push_str(&firma(params));
    s
}

/// `P::P(…)` — el constructor.
pub fn constructor(espacios: &[String], clase: &str, params: &[TypeSpec]) -> String {
    metodo(espacios, clase, clase, params)
}

/// `P::~P()` — el destructor. Nunca lleva parámetros.
///
/// ★ Y es **uno solo**. El ABI de Itanium define D0/D1/D2 —y C1/C2/C3 para
/// constructores— pero D1 y D2 difieren **sólo con bases virtuales**, que
/// están descartadas con motivo. Seis variantes se quedan en dos, y no por
/// recortar: por una decisión ya tomada por otro motivo. (D0, el que además
/// libera, aparecerá el día que existan `new`/`delete`.)
pub fn destructor(espacios: &[String], clase: &str) -> String {
    metodo(espacios, clase, &format!("~{clase}"), &[])
}

/// ¿Este nombre lo pudo escribir un programa?
///
/// Sirve de red: si alguna vez un símbolo generado saliera sin `#`, chocaría
/// con una función de C. Lo usan los tests.
pub fn es_generado(simbolo: &str) -> bool {
    simbolo.contains('#')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::TypeSpec as T;

    fn cls(n: &str) -> T { T::ClassRef(n.into()) }

    #[test]
    fn una_funcion_sin_parametros_lleva_almohadilla_igual() {
        // Sin el `#`, `f()` daría el símbolo `f` — que es exactamente lo que
        // escribiría una función de C, y chocarían.
        assert_eq!(funcion(&[], "f", &[]), "f#");
        assert!(es_generado(&funcion(&[], "f", &[])));
    }

    #[test]
    fn la_sobrecarga_da_simbolos_distintos() {
        let a = funcion(&[], "f", &[T::Int]);
        let b = funcion(&[], "f", &[T::Char]);
        let c = funcion(&[], "f", &[T::Int, T::Int]);
        assert_eq!(a, "f#i");
        assert_eq!(b, "f#c");
        assert_eq!(c, "f#i.i");
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn los_punteros_y_referencias_se_apilan() {
        assert_eq!(codigo(&T::Ptr(Box::new(T::Int))), "Pi");
        assert_eq!(codigo(&T::Ptr(Box::new(T::Ptr(Box::new(T::Char))))), "PPc");
        assert_eq!(codigo(&T::Ref(Box::new(T::Int))), "Ri");
    }

    /// ★ Sin las llaves, una clase llamada `Pi` daría el mismo código que un
    /// `int*`, y dos funciones distintas compartirían símbolo.
    #[test]
    fn una_clase_llamada_Pi_no_choca_con_un_puntero_a_int() {
        let a = funcion(&[], "f", &[cls("Pi")]);
        let b = funcion(&[], "f", &[T::Ptr(Box::new(T::Int))]);
        assert_eq!(a, "f#{Pi}");
        assert_eq!(b, "f#Pi");
        assert_ne!(a, b);
    }

    #[test]
    fn el_signo_cambia_el_codigo() {
        assert_eq!(codigo(&T::Int), "i");
        assert_eq!(codigo(&T::UnsignedInt), "I");
        assert_ne!(codigo(&T::Long), codigo(&T::UnsignedLong));
    }

    #[test]
    fn los_metodos_llevan_la_clase_delante() {
        assert_eq!(metodo(&[], "P", "doble", &[]), "P.doble#");
        assert_eq!(metodo(&[], "P", "doble", &[T::Int]), "P.doble#i");
    }

    /// Dos clases distintas con el mismo método no chocan. Es lo que hace que
    /// `A::f` y `B::f` puedan coexistir.
    #[test]
    fn el_mismo_metodo_en_dos_clases_no_choca() {
        assert_ne!(metodo(&[], "A", "f", &[]), metodo(&[], "B", "f", &[]));
    }

    #[test]
    fn constructor_y_destructor() {
        assert_eq!(constructor(&[], "P", &[]), "P.P#");
        assert_eq!(constructor(&[], "P", &[T::Int]), "P.P#i");
        assert_eq!(destructor(&[], "P"), "P.~P#");
    }

    /// El constructor no puede chocar con un método: dentro de `P`, un miembro
    /// llamado `P` **es** el constructor. El lenguaje reserva el nombre.
    #[test]
    fn el_constructor_no_choca_con_ningun_metodo() {
        assert_eq!(constructor(&[], "P", &[T::Int]), metodo(&[], "P", "P", &[T::Int]));
    }

    #[test]
    fn los_espacios_de_nombres_se_apilan_por_delante() {
        let e = vec!["n".to_string()];
        assert_eq!(funcion(&e, "f", &[T::Int]), "n.f#i");
        let e2 = vec!["a".to_string(), "b".to_string()];
        assert_eq!(funcion(&e2, "f", &[]), "a.b.f#");
        // Y `n::f` no choca con `f` a secas, que es el punto entero.
        assert_ne!(funcion(&e, "f", &[T::Int]), funcion(&[], "f", &[T::Int]));
    }

    /// ★ El tipo de RETORNO no entra. C++ no permite sobrecargar por retorno,
    /// así que meterlo generaría dos símbolos para lo que el lenguaje
    /// considera la misma función — y una llamada no sabría a cuál ir.
    #[test]
    fn el_retorno_no_entra_en_el_simbolo() {
        // `int f(int)` y `char f(int)` son la MISMA función para C++ (y un
        // error si se declaran las dos). Tienen que dar el mismo símbolo.
        assert_eq!(funcion(&[], "f", &[T::Int]), funcion(&[], "f", &[T::Int]));
    }

    /// Un símbolo generado nunca puede coincidir con algo que un programa
    /// escriba: `#`, `.` y `{}` son ilegales en un identificador de C++.
    #[test]
    fn ningun_simbolo_generado_es_escribible_a_mano() {
        for s in [
            funcion(&[], "f", &[]),
            metodo(&[], "P", "doble", &[T::Int]),
            constructor(&[], "P", &[]),
            destructor(&[], "P"),
            funcion(&["n".into()], "f", &[cls("Q")]),
        ] {
            assert!(
                s.contains('#'),
                "{s:?} no lleva `#`: podria chocar con una funcion escrita a mano",
            );
        }
    }

    #[test]
    fn los_arrays_llevan_su_tamano() {
        assert_eq!(codigo(&T::Array(Box::new(T::Int), 4)), "A4i");
        assert_ne!(
            codigo(&T::Array(Box::new(T::Int), 4)),
            codigo(&T::Array(Box::new(T::Int), 8)),
        );
    }
}
