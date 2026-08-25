//! **BMO CRIPTO** -- el techo del plan, empezado por abajo.
//!
//! # Por que este crate existe, y por que hoy
//!
//! `PLAN_EL_PERFIL_TOTAL` tiene una escalera de ocho escalones y **siete son
//! trabajo sobre lo que ya existe**. El octavo es el unico que es un invento, y
//! es este:
//!
//! ```text
//!    SHA-256     [X] 24-08    debajo de HMAC, de HKDF, del transcript de TLS
//!    HMAC        [X] 24-08    SHA-256 dos veces, con dos rellenos
//!    HKDF        [X] 24-08    HMAC en cadena: de un secreto salen las claves
//!    X25519      [X] 24-08    curva eliptica. LA pieza dificil
//!    AES-GCM     [X] 24-08    cifrar y autenticar a la vez
//!    el AZAR     [X] 24-08    RDRAND. Sin el, todo lo de arriba no sirve
//!    ----------------------------------------------------------------------
//!    Ed25519     --           firmar. Pide SHA-512 y aritmetica de Edwards
//!    TLS 1.3     --           la maquina de estados encima de todo lo anterior
//!    X.509       --           ASN.1, fechas, cadena de confianza
//! ```
//!
//! *** SEIS DE NUEVE, y las tres que faltan no son iguales de caras. Ed25519 es
//! la que mas compra por lo que cuesta: `campo25519.rs` --la aritmetica modular
//! sobre `2^255-19`, que es la parte que asusta-- **ya esta escrita y probada**
//! para X25519. Lo que le falta a la firma es SHA-512 y la curva de Edwards
//! encima de un campo que ya existe.
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
//! Ed25519 sigue SIN ESCRIBIR --pide SHA-512 y aritmetica de Edwards-- pero
//! desde el 24-08 ya no hay un `verify` que conteste que si a una firma de
//! ceros: `examinar_ed25519` devuelve `SinFirmar` o `NoSePuedeComprobar`, y
//! ninguna de las dos se puede confundir con una firma valida.
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
//! [!] Esta cabecera decia, hasta el 25-08: *"lo que este crate NO es todavia:
//! no hay generador de numeros aleatorios (...) hasta que exista aqui solo hay
//! funciones deterministas."* **Y `pub mod azar;` estaba veinte lineas mas
//! abajo, en este mismo fichero.** El AZAR entro el mismo dia y la cabecera se
//! quedo sin actualizar.
//!
//! *** Se deja escrito porque es la clase de fallo que este arbol persigue en
//! todas partes menos en su propia prosa: **un documento que se contradice a si
//! mismo dentro del mismo fichero.** No hace falta un guardian ni otra carpeta
//! para cazarlo -- hace falta leer hasta abajo.

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
/// **AES-128 y AES-256**, solo cifrar bloques -- GCM nunca usa el inverso.
pub mod aes;
/// **AES-GCM**: cifrar y autenticar a la vez, que son dos cosas y una sola
/// llamada. [!!] Y el nonce NO SE REPITE: ver su cabecera.
pub mod gcm;
/// **El AZAR**, de `RDRAND`. Lo unico que le faltaba a todo lo de arriba para
/// servir de algo: una clave predecible es PEOR que no cifrar.
/// [!!] Y no tiene respaldo a proposito -- ver su cabecera.
pub mod azar;

pub use hmac::{expandir, extraer, hmac as hmac_sha256, iguales};
pub use sha256::Sha256;
pub use gcm::{abrir, sellar};
pub use x25519::{secreto_a_publico, secreto_compartido, x25519};
