//! **La resolucion de sobrecarga de BMO C++** -- cual de las firmas con el
//! mismo nombre recibe una llamada, o por que ninguna.
//!
//! Salio de `parser.rs` el 2026-09-17, entera y sin cambiar una linea de su
//! logica, cuando le crecio la conversion de derivada a base y el trinquete
//! de L6a le dijo que no al fichero de al lado. Es una pregunta cerrada --
//! tipos que entran, una firma o un error que sale-- y por eso es un corte
//! limpio: solo lee `clases` y `err` del parser.

use super::*;

/// Lo bien que encaja un argumento en un parametro. **Menos es mejor.**
///
/// === Como lo hace GCC, y que se le quita ===
///
/// `gcc/cp/call.cc` --uno de los ficheros mas grandes del frontend de C++, y
/// sorprende que lo sea-- construye para cada argumento una *secuencia de
/// conversion implicita* con hasta tres eslabones (lvalue, promocion,
/// cualificacion) y luego ordena secuencias parcialmente. Eso es lo que hace
/// falta para resolver contra plantillas, conversiones definidas por el
/// usuario y ADL.
///
/// BMO no tiene ninguna de las tres, asi que el orden colapsa a **tres
/// escalones** que se comparan sumando. Es lo que `MAESTROS.md` fijo como
/// alcance: *ranking minimo -- exacto > promocion > conversion*.
#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub(super) enum Encaje {
    Exacto,
    /// `char`/`short` -> `int`, `float` -> `double`. No pierde informacion.
    Promocion,
    /// Cualquier aritmetico a cualquier aritmetico. **Puede perder**, y por eso
    /// es el ultimo escalon: si hay una alternativa mejor, gana la otra.
    Conversion,
}

impl Parser {
    // -- Resolucion de sobrecarga ------------------------------------

    /// Es un tipo con el que se puede hacer aritmetica?
    pub(super) fn es_numero(t: &TypeSpec) -> bool {
        use TypeSpec as T;
        matches!(t, T::Bool | T::Char | T::UnsignedChar | T::Short | T::UnsignedShort
            | T::Int | T::UnsignedInt | T::Long | T::UnsignedLong
            | T::LongLong | T::UnsignedLongLong | T::Float | T::Double)
    }

    /// Lo bien que un argumento de tipo `dado` encaja en un parametro `quiere`.
    pub(super) fn encaje(&self, dado: &TypeSpec, quiere: &TypeSpec) -> Option<Encaje> {
        use TypeSpec as T;
        if dado == quiere { return Some(Encaje::Exacto); }
        // ** DERIVED TO BASE (2026-09-17). `saldo(Cuenta*)` called with an
        // `Ahorro*` was "no version accepts those types" -- while `Cuenta *c =
        // &ahorro;` already worked. The first real C++ program that reached
        // the disk tripped on it. Single inheritance puts the base FIRST
        // (fields and vptr), so the address does not change: this is only
        // typing. And it ranks as a CONVERSION, as in the standard
        // ([over.ics.rank]): an exact overload for the derived class wins.
        let clases = match (dado, quiere) {
            (T::Ptr(d), T::Ptr(b)) => match (&**d, &**b) {
                (T::ClassRef(dn), T::ClassRef(bn)) => Some((dn, bn)),
                _ => None,
            },
            (T::ClassRef(dn), T::Ref(b)) => match &**b {
                T::ClassRef(bn) => Some((dn, bn)),
                _ => None,
            },
            _ => None,
        };
        if let Some((dn, bn)) = clases {
            if self.deriva_de(dn, bn) {
                return Some(Encaje::Conversion);
            }
        }
        // Una referencia se ata al valor: `f(int&)` acepta un `int`. Encaja
        // exacto porque no hay conversion ninguna -- solo se pasa la direccion.
        if let T::Ref(d) = quiere {
            if &**d == dado { return Some(Encaje::Exacto); }
        }
        // Un array decae a puntero a su elemento, que es lo que C hace en toda
        // llamada. Sin esto, `f(char*)` no aceptaria un `char[8]`.
        if let (T::Array(e, _), T::Ptr(p)) = (dado, quiere) {
            if e == p { return Some(Encaje::Exacto); }
        }
        if !Self::es_numero(dado) || !Self::es_numero(quiere) { return None; }
        // La promocion entera y la de coma flotante: NO pierden informacion.
        let promociona = matches!(
            (dado, quiere),
            (T::Char | T::UnsignedChar | T::Short | T::UnsignedShort | T::Bool, T::Int)
            | (T::Float, T::Double)
        );
        Some(if promociona { Encaje::Promocion } else { Encaje::Conversion })
    }

    /// `derivada` hereda, directa o indirectamente, de `base`? Walks the single
    /// inheritance chain; a chain longer than the number of classes is a cycle
    /// the class parser should have refused, and answers `false`.
    pub(super) fn deriva_de(&self, derivada: &str, base: &str) -> bool {
        let mut actual = self.clases.get(derivada).and_then(|c| c.base.clone());
        for _ in 0..=self.clases.len() {
            match actual {
                Some(ref n) if n == base => return true,
                Some(ref n) => actual = self.clases.get(n).and_then(|c| c.base.clone()),
                None => return false,
            }
        }
        false
    }

    /// Elige la firma que mejor encaja, o dice por que no puede.
    ///
    /// El criterio es la **suma** de los escalones de cada argumento, y el
    /// empate es un error con los dos candidatos escritos. Una ambiguedad que
    /// se resolviera sola --eligiendo "el primero", por ejemplo-- haria que
    /// anadir una sobrecarga cambiara a que funcion va una llamada existente,
    /// en silencio.
    pub(super) fn resolver<'f>(&self, que: &str, firmas: &'f [Firma], args: &[TypeSpec])
        -> Result<&'f Firma, CppError>
    {
        let mut mejor: Option<(u32, &Firma)> = None;
        let mut empate = false;
        let mut hubo_aridad = false;

        for f in firmas {
            if f.params.len() != args.len() { continue; }
            hubo_aridad = true;
            let mut coste = 0u32;
            let mut vale = true;
            for (a, p) in args.iter().zip(f.params.iter()) {
                match self.encaje(a, p) {
                    Some(e) => coste += e as u32,
                    None => { vale = false; break; }
                }
            }
            if !vale { continue; }
            match mejor {
                None => mejor = Some((coste, f)),
                Some((c, _)) if coste < c => { mejor = Some((coste, f)); empate = false; }
                Some((c, _)) if coste == c => empate = true,
                _ => {}
            }
        }

        if empate {
            let opciones: Vec<String> = firmas.iter()
                .filter(|f| f.params.len() == args.len())
                .map(|f| f.simbolo.clone()).collect();
            return Err(self.err(format!(
                "la llamada a `{que}` es ambigua entre {}: ninguna encaja mejor que la otra",
                opciones.join(" y "))));
        }
        match mejor {
            Some((_, f)) => Ok(f),
            None if hubo_aridad => Err(self.err(format!(
                "ninguna version de `{que}` acepta esos tipos de argumento"))),
            None => Err(self.err(format!(
                "`{que}` no tiene ninguna version con {} argumento(s)", args.len()))),
        }
    }
}
