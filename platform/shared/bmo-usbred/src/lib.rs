//! **BMO USBRED** -- la red por USB de un movil: que ES cada interfaz, y RNDIS.
//!
//! generacion: nieto -- clases y mensajes; no sabe de controladores ni de DMA
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan el portero del kernel y el banco
//!
//! # S3b de `docs/plan/PLAN_CLOUD_LOCAL.md` (2026-09-14)
//!
//! Eddi: *"continua con el USB en mi BMO-X para que sepa"*, con su HONOR X7a
//! enchufado. Un movil en **anclaje por USB** se presenta como una tarjeta de
//! red USB, y lo hace de una de dos formas:
//!
//! ```text
//!    RNDIS   el de Microsoft. Casi todos los Android lo usan por defecto.
//!            Control por mensajes encapsulados + tramas con cabecera de 44
//!    NCM     el estandar USB (CDC). Algunos Android recientes. Tramas en
//!            bloques NTB con su tabla
//! ```
//!
//! Hoy BMO-X NO tiene driver para ninguna: su pila USB no sabe transferencias
//! BULK, que es por donde van las tramas. Lo que SI puede hacer ya es **decir que
//! llego** en vez de un numero en hexadecimal, y tener probado en el anfitrion
//! lo que el driver va a escribir y leer. Eso es este crate.
//!
//! [!] Los numeros de clase son del USB-IF y los de RNDIS de la especificacion de
//! Microsoft ([MS-RNDIS]). Se escriben una vez aqui para que el kernel no tenga
//! una segunda copia que pueda discrepar.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod clase;
/// TODO NEGADO por defecto: la categoria de cada aparato y lo que se le deja.
pub mod politica;
pub mod rndis;
