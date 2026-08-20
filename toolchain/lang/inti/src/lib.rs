//! # INTI -- el lenguaje de BMO-X
//!
//! *"Es como lo hizo C pero para Unix, pero BMO-X tendra su INTI."*
//!
//! Frontend: de texto a arbol. El **porque** de cada decision esta en
//! `docs/maestro/INTI_MAESTRO.md`; **lo que se escribe**, en
//! `toolchain/lang/inti/GRAMATICA.md`; y **como se decidio partir el
//! compilador**, en `ARQUITECTURA.md`, que esta al lado de este fichero.
//!
//! ## Los modulos, y por que son estos
//!
//! El corte no es por fases del compilador --eso saldria igual en cualquier
//! libro--, sino por **lo que cada pieza tiene que poder decir sin nombrar a
//! las demas**:
//!
//! ```text
//!    aviso      el mensaje de cuatro partes y los codigos estables.
//!               No sabe que existe INTI. Se prueba solo.
//!
//!    palabras   el vocabulario, leido de `tables/lang/inti/palabras.toml`.
//!               Es lo que hace que el idioma sea una columna y no un fork.
//!
//!    lexico     de bytes a piezas. No conoce la gramatica.
//!      pieza      los datos, sin logica: los lee todo el mundo
//!      sangria    el margen, que es lo unico con estado del barrido
//! ```
//!
//! OJO: **Lo que este crate NO enlaza todavia**: `bmo-abi`, `bmo-lower` y
//! `bmo-verify`, que es lo que enlazan los otros cuatro frontends. F1 no emite
//! bytes -- entra texto y sale un arbol. Atar el frontend a la forma del
//! emisor antes de tener nada que emitir es el orden que este proyecto evita.
//!
//! ## Lo que hay hoy
//!
//! ```text
//!    F1a  lexico completo                      <- esto
//!    F1b  arbol + sintaxis (de piezas a arbol)
//!    F2   INTI LLANO a `.bex` nativo
//! ```

pub mod arbol;
pub mod aviso;
pub mod lexico;
pub mod palabras;
pub mod sintaxis;

pub use aviso::{Aviso, Cosecha, Sitio};
pub use lexico::{Clase, Pieza, Signo};
pub use palabras::{Simbolo, Vocabulario};
pub use arbol::{Modulo, Perfil};

/// Barre un fuente con el vocabulario que traiga el sistema.
///
/// El vocabulario se busca en las raices de `bmo-mods` (`$BMO_MODS` -> `mods/`
/// -> `tables/`) y, si no aparece en ninguna, se usa el que viaja dentro. Un
/// compilador que no arranca porque falta un fichero de datos es peor que uno
/// que arranca con lo que traia -- pero **cual de las dos cosas paso se puede
/// preguntar**, con `palabras::Vocabulario::cargar`.
pub fn barrer(fuente: &str) -> Cosecha<Vec<Pieza>> {
    let raices = bmo_mods::Roots::find();
    let (vocab, _origen) = Vocabulario::cargar(&raices);
    match vocab {
        Ok(v) => lexico::barrer(fuente, &v),
        // Solo pasa si alguien rompio la tabla incrustada, y entonces el
        // compilador no tiene idioma con el que hablar.
        Err(e) => panic!("palabras.toml esta roto y ni el respaldo carga: {}", e),
    }
}

/// Lee un fuente entero: barrido + gramatica.
///
/// Los avisos de las dos fases salen **juntos y en orden**. Es la razon de que
/// `Cosecha` no sea un `Result`: si el barrido encuentra tres cosas y la
/// gramatica dos, el que escribe quiere ver las cinco.
pub fn leer(fuente: &str) -> Cosecha<Modulo> {
    let raices = bmo_mods::Roots::find();
    let (vocab, _) = Vocabulario::cargar(&raices);
    let v = match vocab {
        Ok(v) => v,
        Err(e) => panic!("palabras.toml esta roto y ni el respaldo carga: {}", e),
    };
    let piezas = lexico::barrer(fuente, &v);
    let mut arbol = sintaxis::leer(&piezas.valor, &v);
    let mut avisos = piezas.avisos;
    avisos.append(&mut arbol.avisos);
    Cosecha::con(arbol.valor, avisos)
}
