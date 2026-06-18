//! `compat` — interop con el C ABI heredado (Win64 / SysV AMD64).
//!
//! Solo necesario cuando se llama código C externo (drivers binarios,
//! bibliotecas de terceros, shim L4 para apps Windows). El código nativo
//! BMO no debería entrar aquí jamás.
//!
//! Costo de cada thunk: ~5 ns en el 5600X (un par de movs + un call).

pub mod thunks;

