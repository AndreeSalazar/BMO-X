//! **BMO CRIPTO** -- el techo del plan, empezado por abajo.
//!
//! # Por que este crate existe, y por que hoy
//!
//! `PLAN_EL_PERFIL_TOTAL` tiene una escalera de ocho escalones y **siete son
//! trabajo sobre lo que ya existe**. El octavo es el unico que es un invento, y
//! es este:
//!
//! ```text
//!    SHA-256     [X] hoy      debajo de HMAC, de HKDF, del transcript de TLS
//!    HMAC        [X] hoy      SHA-256 dos veces, con dos rellenos
//!    HKDF        [X] hoy      HMAC en cadena: de un secreto salen las claves
//!    X25519      [X] hoy      curva eliptica. LA pieza dificil
//!    AES-GCM     --           o ChaCha20-Poly1305
//!    TLS 1.3     --           la maquina de estados encima de todo lo anterior
//!    X.509       --           ASN.1, fechas, cadena de confianza
//! ```
//!
//! ## *** Y ABRE DOS PUERTAS CON LA MISMA LLAVE
//!
//! Esto no es "lo que hace falta para navegar". Es tambien lo que hace falta
//! para que BMO-X **pueda firmar lo que ejecuta**:
//!
//! ```text
//!    HTTPS            pide curva eliptica + hash
//!    firmar un `.bex` pide curva eliptica + hash     <- LA MISMA
//! ```
//!
//! Hoy `verify_ed25519` **dice que si a una firma de ceros**, y no lo llama
//! nadie todavia. Se creian dos deudas y es una: **pagarla una vez cobra dos.**
//!
//! # La regla que gobierna todo lo que entre aqui
//!
//! *** **Una criptografia mal escrita no falla: funciona y no protege.** No hay
//! ninguna otra parte del sistema donde eso sea verdad de esta forma -- un
//! driver roto no arranca, un compilador roto no compila, y un AES roto cifra
//! perfectamente y no guarda nada.
//!
//! De ahi salen las tres reglas de este crate:
//!
//! 1. **Nada entra sin sus vectores oficiales.** Si el algoritmo no tiene
//!    respuesta publicada contra la que comparar, no entra todavia.
//! 2. **Lo que lleva secreto dentro se dice.** Un hash no lo lleva; un HMAC si,
//!    y un AES tambien -- y eso cambia si hay que preocuparse por el TIEMPO que
//!    tarda. Cada pieza lo declara en su pagina.
//! 3. **Se escribe, no se trae.** El hash es software y la ley 24 dejaria traer
//!    una crate; se escribe igual, porque el dia que esto firme un `.bex` quien
//!    lo audite tiene que poder leer las lineas. **Una cadena de confianza que
//!    empieza en un `Cargo.toml` no es una cadena de confianza.**
//!
//! [!] Y lo que este crate NO es todavia: no hay generador de numeros
//! aleatorios. Sin el no hay claves, y **una clave predecible es peor que no
//! cifrar** -- porque parece que si. Ese es su propio problema y su propia
//! pagina, y hasta que exista aqui solo hay funciones deterministas.

#![cfg_attr(not(test), no_std)]

#[cfg(test)]
extern crate alloc;

pub mod sha256;
/// **HMAC-SHA256 y HKDF.** El primer sitio del arbol donde el TIEMPO que tarda
/// algo es parte de si es correcto. Ver su cabecera.
pub mod hmac;
/// **La aritmetica de X25519**: numeros modulo `2^255 - 19`. Solo cuentas.
pub mod campo25519;
/// **X25519**: dos maquinas que nunca se han visto acaban con el mismo secreto,
/// y quien escuchaba el cable no lo tiene.
pub mod x25519;

pub use hmac::{expandir, extraer, hmac as hmac_sha256, iguales};
pub use sha256::Sha256;
pub use x25519::{secreto_a_publico, secreto_compartido, x25519};
