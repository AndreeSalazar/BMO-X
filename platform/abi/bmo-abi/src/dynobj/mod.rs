//! `dynobj` -- LA FORMA DE UN OBJETO CON VIDA PROPIA, como contrato.
//!
//! # ** DE QUIEN ES ESTO, Y POR QUE CAMBIO (2026-08-23)
//!
//! Aqui ponia *"this is the first piece of `docs/maestro/PYTHON_MAESTRO.md`"*, y
//! eso dejo de ser verdad. Decision de Eddi:
//!
//! > *"es ironico que intente renovar Python para mi sistema, pero no hay
//! > sentido. INTI toma su lugar."*
//!
//! **Este es el modelo de objetos de INTI**, y sus dos primeros inquilinos son
//! `lista` y `texto`. No se renombra a `inti` por el mismo motivo por el que no
//! se llamo `python`: lo que vive aqui es una REPRESENTACION, no la semantica de
//! ningun lenguaje.
//!
//! ```text
//!    lo que vive aqui   la cabecera, el bit de inmortal, los numeros de ranura
//!    lo que NO          que SIGNIFICA `a + b` cuando `a` es texto y `b` es lista
//! ```
//!
//! Es la misma linea que separa `bmo_lower::packed` --que guarda BCD porque
//! empaquetar es una representacion, y `COMP-3` de COBOL, el `Decimal` del Annex
//! F de Ada y el `FIXED DECIMAL` de PL/I piden los mismos nibbles-- de la
//! `PICTURE`, que es de COBOL y se queda en COBOL.
//!
//! ** Y esa linea es lo que hace que este fichero **no haya tenido que cambiar**
//! al cambiar de dueno. Un contrato escrito sobre bytes sobrevive al lenguaje
//! que lo pidio primero; uno escrito sobre semantica, no.
//!
//! # Por que no vive dentro de `runtime/`
//!
//! `runtime/` ya existe aqui con `TypeRegistry`, `VTableStore` y `LangBridge`,
//! pero es un registro de INTERFACES entre lenguajes: otro trabajo.
//!
//! ** Y no podria servir para esto de todas formas: `VTableEntry` es un
//! `Option<extern "C" fn()>`, un puntero crudo a funcion. La seccion siguiente
//! dice por que un puntero crudo no puede aparecer en un objeto que se presta.
//!
//! # ** THE CONSTRAINT THAT SHAPES EVERYTHING HERE
//!
//! The point of this contract is that the immutable part of a program -- code,
//! types, constant pool -- is **lent between processes** with `MEM_OP_OFRECER`
//! instead of copied. Two facts about BMO make that demand a specific shape:
//!
//! 1. **`loan::take` maps at an address decided by the SLOT.** The same lent
//!    page lands at a *different* virtual address in each process. Therefore a
//!    shared object **cannot hold a pointer to another shared object**. It holds
//!    an INDEX. This is the same "offsets, not pointers" rule already written
//!    for the bytecode section, and here it is not a preference: a pointer is
//!    simply wrong.
//!
//! 2. **A lent page can be READ-ONLY.** So a reference count in it must not be
//!    written -- and "not written" is stronger than "wasted work": the store
//!    would fault. That is why [`header::may_write`] exists as its own question,
//!    separate from [`header::retain`].
//!
//! ** Las dos son decisiones que NO SE PUEDEN METER DESPUES, y hay una factura
//! ajena que lo demuestra: CPython aprendio la segunda tarde. Los objetos
//! inmortales (PEP 683) llegaron en la 3.12, anos despues de fijar la cabecera,
//! y costaron un ciclo de version entero -- porque hasta entonces cada hijo de
//! un `fork()` ensuciaba todas las paginas compartidas **solo por LEER**.
//!
//! *** Eso se cita como EVIDENCIA y no como herencia. Lo que se copia no es el
//! diseno de nadie: es el escarmiento. Y es la razon entera de que este modulo
//! se escribiera **antes que una sola linea de lo que lo usa**.
//!
//! # What is here and what is not
//!
//! The rule agreed for this work is *the CONTRACT may be complete, the
//! IMPLEMENTATION is the seed* -- the same way `SectionKind::Resources = 0x0B`
//! sat declared and empty from the day BEF was designed until something needed
//! it.
//!
//! ```text
//!    header    los dieciseis bytes con los que empieza todo objeto
//!    slots     las operaciones numeradas de un tipo, como los `TASK_OP_*`
//!    lista     la primera instancia: `lista de T`
//!    texto     la segunda: cadena UTF-8 inmutable
//!    tabla     la tercera, y la primera con un ALGORITMO dentro
//! ```
//!
//! Not here yet, and each has its reason:
//!
//! - **The type table entry** (the BEF `Tipos` section format). Next increment;
//!   it needs the slot numbers below to be settled first.
//! - **The allocator.** It is Ring 3 code over `KIND_MEMORIA`, not a contract.
//! - **Anything that runs.** This crate is tested on the host and holds no
//!   behaviour: it is bytes and numbers.

pub mod header;
/// La forma de una `lista de T` en memoria: la primera instancia del contrato
/// de `header`, y lo que INTI le anade.
pub mod lista;
pub mod slots;
pub mod tabla;
pub mod texto;

pub use header::{DynHeader, DynVarHeader};
