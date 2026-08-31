//! **EN QUIEN CONFIA ESTA MAQUINA.** El ancla, y hoy esta vacia.
//!
//! [carril]  ROJO      el ancla de confianza de esta maquina
//!
//! # Por que este fichero existe, y por que estaba a punto de no existir
//!
//! El 2026-08-25 se escribio Ed25519 y se fue a cablearlo al gate del cargador.
//! Al mirar el formato aparecio esto:
//!
//! ```text
//!    Ed25519Signature = sig[64] || pubkey[32]
//! ```
//!
//! **La clave publica viaja DENTRO de la firma.** Comprobar la firma contra esa
//! clave siempre da que si, porque quien firmo eligio las dos cosas. Cualquiera
//! se genera un par, firma el binario, y mete su clave al lado.
//!
//! > Una firma que trae su propia clave demuestra que **nadie la ha tocado desde
//! > que se firmo**. No demuestra **quien la firmo**.
//!
//! *** Cablear Ed25519 sin esto habria dado un control que se pasa solo -- la
//! tercera vez en dos dias que aparece la misma forma:
//!
//! ```text
//!    C1 (24-08)   `verify_ed25519` decia SI a una firma de ceros
//!    C3 (25-08)   la firma de ceros PASABA otra vez, por matematicas
//!    aqui         la firma cuadraria... con la clave que trajo el firmante
//! ```
//!
//! # Y por que vive AQUI y no dentro del verificador
//!
//! Lo dejo escrito C1 el dia que se arreglo, y vale igual del derecho:
//!
//! > *"quien quiera permitir binarios sin firmar lo decide **arriba, en la
//! > politica, donde se ve** -- no dentro del verificador."*
//!
//! `bmo-firma` hace la aritmetica y no tiene opinion. **La opinion es este
//! fichero**, y por eso es corto, tiene nombre, y se lee de un vistazo.
//!
//! # [!] HOY ESTA VACIO, Y ESO NO ES UN HUECO: ES EL ESTADO
//!
//! Ningun `.bex` del arbol lleva firma Ed25519 --el escritor pone `sig_algo = 0`
//! en todos-- asi que no hay nada que anclar todavia. Con el ancla vacia:
//!
//! ```text
//!    un .bex de hoy (sig_algo = 0)   SoloIntegridad   -> sigue arrancando
//!    un .bex firmado por cualquiera  AutorDesconocido -> NO arranca
//! ```
//!
//! *** **Vacio significa "no confio en nadie", no "vale cualquiera".** Es la
//! unica respuesta honesta mientras no se haya decidido de quien fiarse, y hace
//! que encender la firma sea una decision con nombre en vez de un efecto
//! secundario.
//!
//! # Como se llena, el dia que haga falta
//!
//! Se pega aqui la clave **publica** de quien firma --32 bytes-- y se dice quien
//! es. La privada **no baja a la maquina**: `PLAN_SEGURIDAD.md` C3 lo decide, y
//! `bmo-cripto` lo hace cumplir dejando el firmador detras de una bandera que el
//! kernel no enciende.
//!
//! [!] Y una advertencia para ese dia: **anadir una clave aqui es conceder
//! ejecucion a todo lo que esa clave firme, para siempre.** No hay revocacion.
//! Escribir la lista de revocados antes de la primera clave seria construir la
//! puerta antes de la casa; escribirla despues de la segunda seria tarde.

/// Bytes de una clave publica de Ed25519.
pub const CLAVE: usize = 32;

/// **Las claves en las que esta maquina confia.**
///
/// El orden importa poco pero se conserva: `bmo_firma::Veredicto::Firmado`
/// devuelve el INDICE, y con el se puede decir quien firmo por su nombre en vez
/// de contestar un `si` que no distingue a nadie.
///
/// Cada entrada lleva su nombre al lado **a proposito**: treinta y dos bytes en
/// hexadecimal no se lo dicen a nadie, y una lista de claves sin nombres es una
/// lista que nadie se atreve a tocar.
pub static ANCLA: &[([u8; CLAVE], &str)] = &[
    // Todavia nadie. Ver la cabecera: vacio es una respuesta.
];

/// Solo las claves, que es lo que `bmo-firma` pide.
///
/// * Se copia a un array de tamano fijo en vez de devolver un `Vec`: en Ring 0
/// no hay a quien pedirle memoria, y el tope --ocho-- es generoso para lo que
/// esto va a tener nunca. Si algun dia se pasa, **se para en ocho y lo dice**
/// en vez de recortar en silencio.
pub fn claves(dst: &mut [[u8; CLAVE]; 8]) -> usize {
    let n = if ANCLA.len() > 8 { 8 } else { ANCLA.len() };
    for i in 0..n {
        dst[i] = ANCLA[i].0;
    }
    if ANCLA.len() > 8 {
        crate::ring0::cabina::warn(
            "confianza",
            "el ancla tiene mas claves de las que caben: se miran las 8 primeras",
            ANCLA.len() as u64,
        );
    }
    n
}

/// El nombre de la clave `i`, para poder decir QUIEN firmo.
pub fn nombre(i: usize) -> &'static str {
    match ANCLA.get(i) {
        Some((_, n)) => n,
        None => "?",
    }
}

/// **Exige esta maquina que un `.bex` venga firmado?**
///
/// Hoy `false`, y tiene que serlo: ningun `.bex` del arbol lleva firma Ed25519,
/// asi que exigirla dejaria la maquina sin arrancar nada.
///
/// * Es un `const fn` y no una constante suelta para que el dia que esto pase a
/// `true` **haya un solo sitio que cambiar**, y para que ese cambio se vea en un
/// `git diff` de una linea con este comentario al lado.
///
/// [!] Y el orden para encenderlo es: primero una clave en [`ANCLA`], despues un
/// `.bex` firmado con ella que arranque, y **al final** esto. Al reves, la
/// maquina deja de arrancar y el motivo parece del cargador.
pub const fn exige_firma() -> bool {
    false
}
