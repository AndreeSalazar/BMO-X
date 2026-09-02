//! `fundamentals` -- tipos que TODO el codigo del BMO ABI necesita.
//!
//! Si un tipo se usa en mas del 50% del codigo BMO, vive aqui.
//!
//! - [`primitives`]   -- tipos numericos (`bx_u8..u64`, `bx_i*`, `bx_f16/32/64`).
//! - [`status`]       -- `BmoStatus` 16-byte (sustituye `HRESULT`/`errno`).
//! - [`handle`]       -- `BmoHandle` 64-bit con generacion + ops (sustituye `HANDLE`/`fd`).
//! - [`capability`]   -- `BmoCap`, `BmoCapSet` (sustituye permisos Unix/ACL).
//! - [`option`]       -- `BmoOption<T>` FFI-safe (sustituye punteros nullable).
//! - [`result`]       -- `BmoResult<T, E>` FFI-safe (errores inline sin TLS).
//! - [`error`]        -- `BmoError` unificado de 16 bytes.
//! - [`convert`]      -- conversiones BmoStatus <-> BmoError <-> ErrorCode.
//! - [`string`]       -- `BmoStr`/`BmoString` (ptr+len UTF-8).
//! - [`memory`]       -- `BmoSlice`, `BmoRange`, `BmoAligned`.
//! - [`buffer`]       -- `BmoBuffer` descriptor de memoria compartida (32 B).
//! - [`allocator`]    -- `BmoAllocator` trait + `BmoGlobalAllocator`.
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
