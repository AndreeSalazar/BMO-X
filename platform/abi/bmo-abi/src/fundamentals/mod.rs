//! `fundamentals` -- los tipos que CRUZAN LA FRONTERA del ABI.
//!
//! # ** EL LISTON, CORREGIDO EL 2026-09-02
//!
//! Aqui ponia: *"Si un tipo se usa en mas del 50% del codigo BMO, vive aqui"*.
//! **Era falso**, y se midio contando consumidores fuera de este directorio:
//!
//! ```text
//!    BmoStatus                                        18   <- el unico
//!    BmoHandle                                         6
//!    BmoCap, BmoStr, BmoError, HandleKind              2 cada uno
//!    BmoString, BmoSlice, BmoBuffer, MemOrder          1
//!    BmoOption, BmoResult, BmoFormatter, BmoAllocator  0
//! ```
//!
//! **Uno de catorce pasaba el 50%.** Y un criterio escrito que no se aplica es
//! peor que ninguno: da autoridad a lo que ya esta dentro y no sabe decirle
//! que no a lo que viene.
//!
//! # El liston de verdad, que es el que estos tipos SI cumplen
//!
//! > Aqui vive lo que **cruza la frontera del ABI**: lo que viaja en un
//! > registro, lo que nombra un objeto del kernel, o lo que fija el tamano de
//! > un campo que dos lenguajes tienen que leer igual.
//!
//! Eso es una prueba que se puede hacer mirando el tipo, no un porcentaje que
//! nadie recuenta:
//!
//! ```text
//!    viaja en un registro   BmoStatus (rax/rdx al volver), BmoCap
//!    nombra un objeto       BmoHandle con su generacion, HandleKind
//!    fija un tamano         primitives, BmoStr, BmoSlice, BmoBuffer
//!    lo exige el metal      sync -- ordenes de memoria; lo usa `bmo-rt`
//! ```
//!
//! # ** Y LO QUE ESTA AQUI SIN CUMPLIRLO, dicho y no escondido
//!
//! `option`, `result`, `convert`, `error` y `fmt` **no cruzan nada todavia**:
//! son formas FFI-safe declaradas antes de tener consumidor. Se quedan --un
//! ABI puede declarar la forma antes que el motor, y `<bmo/sonido.h>` lo hace
//! a proposito-- pero **se dicen**, aqui y en su `[carril]`, fichero por
//! fichero.
//!
//! [!] Y la diferencia con `io`, que si se borro, es la que decide si algo se
//! queda: **estas formas son de ESTE sistema**. `BmoPipe` describia un SO que
//! reparte descriptores, y BMO-X no lo es. Sin estrenar se tolera; prestado de
//! otro sistema, no.
//!
//! # Las piezas
//!
//! El `(N)` es cuantos ficheros de fuera lo nombran. `(0)` no es un error: es
//! una forma declarada, y esta puesto para que se vea sin tener que contarlo.
//!
//! - [`primitives`]   -- tipos numericos (`bx_u8..u64`, `bx_i*`, `bx_f16/32/64`).
//! - [`status`]       -- `BmoStatus` 16-byte, lo que viaja en rax/rdx. **(18)**
//! - [`handle`]       -- `BmoHandle` 64-bit con generacion + ops. **(6)**
//! - [`capability`]   -- `BmoCap`, `BmoCapSet`: los bits de permiso. (2)
//! - [`string`]       -- `BmoStr`/`BmoString` (ptr+len UTF-8). (2)
//! - [`error`]        -- `BmoError` unificado de 16 bytes. (2)
//! - [`memory`]       -- `BmoSlice`, `BmoRange`, `BmoAligned`. (1)
//! - [`buffer`]       -- `BmoBuffer` descriptor de memoria compartida (32 B). (1)
//! - [`sync`]         -- atomicos y `MemOrder`. Lo unico que consume `bmo-rt`. (1)
//! - [`convert`]      -- conversiones BmoStatus <-> BmoError <-> ErrorCode. (0)
//! - [`option`]       -- `BmoOption<T>` FFI-safe. SIN ESTRENAR. (0)
//! - [`result`]       -- `BmoResult<T, E>` FFI-safe. SIN ESTRENAR. (0)
//! - [`fmt`]          -- `BmoFormatter` sobre la pila. SIN ESTRENAR. (0)
//! - [`allocator`]    -- `BmoAllocator` + `BmoGlobalAllocator`. SIN ESTRENAR. (0)
//!
//! # ** AQUI VIVIA `io`, y se borro el 2026-09-02
//!
//! Declaraba `BmoRead`, `BmoWrite`, `BmoSeek` y `BmoPipe`. **BMO-X no tiene
//! tuberias** -- el kernel no menciona `pipe` ni una vez, y el IPC son
//! endpoints y canales (ver `docs/maestro/IPC_MAESTRO.md`).
//!
//! No era una promesa a medio cumplir: era una **forma prestada de otro
//! sistema**. `BmoRead`/`BmoWrite`/`BmoSeek` son la tripa de `std::io`, y
//! `BmoPipe` es de un SO que reparte descriptores. Este no lo es.
//!
//! *** Y eso es lo que hace dano, no estar sin usar. Un ABI puede declarar
//! superficie antes de tener el motor --`<bmo/sonido.h>` lo hace a proposito,
//! *"el contrato va ANTES que el driver"*-- pero lo que declara tiene que ser
//! la forma de ESTE sistema. Un tercero que leyera `io` concluiria que BMO-X
//! ofrece tuberias.
//!
//! [!] Nadie la usaba: su unica mencion en todo el arbol estaba en el dibujo
//! ASCII de `lib.rs`. Salio contando quien usa que, no buscandola.
//!
//! Lo que hace su trabajo, cada uno con su forma:
//!
//! ```text
//!    leer y escribir un fichero   `fs/` y `<bmo/archivo.h>`
//!    hablar con otro proceso      endpoints y canales
//!    memoria sin copiar           `MEM_OP_OFRECER` + `TASK_OP_TOMAR`
//! ```
//!
//! - [`fmt`]          -- `BmoFormatter` stack-allocated (sin heap).
//! - [`sync`]         -- `BmoAtomicU32/U64/Bool`, `MemOrder`, `BmoSpinLock`.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  ROJO         el reparto de `fundamentals`, y hereda el color del
//!                        carril que manda
//! [cuesta]  PUERTA       hereda de `handle/` y `status/`: lo que sale de
//!                        aqui esta en binarios firmados
//! [riesgo]  ESPEJO UNICO
//!                        hereda las dos: hay tablas espejo del kernel, y
//!                        numeros que no se reciclan

pub mod allocator;
pub mod buffer;
pub mod capability;
pub mod convert;
pub mod error;
pub mod fmt;
pub mod handle;
pub mod memory;
pub mod option;
pub mod primitives;
pub mod result;
pub mod status;
pub mod string;
pub mod sync;
