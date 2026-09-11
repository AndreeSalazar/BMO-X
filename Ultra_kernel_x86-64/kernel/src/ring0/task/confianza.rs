//! **EN QUIEN CONFIA ESTA MAQUINA.** El ancla, y hoy esta vacia.
//!
//! [carril]  ROJO      el ancla de confianza de esta maquina
//! [consumo] NADA      corre cuando alguien lanza o admite una tarea
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
//! # ESTUVO VACIO DIECISEIS DIAS, Y ESE VACIO ERA EL ESTADO
//!
//! Del 25-08 al 10-09 aqui no habia ninguna clave, y no era un hueco por hacer:
//! **no habia nada que anclar**. Ningun `.bex` llevaba firma Ed25519 porque
//! nadie sabia firmar -- el escritor pone `sig_algo = 0` y no existia la
//! herramienta del anfitrion que lo cambiara.
//!
//! *** Vacio significaba **"no confio en nadie"**, no "vale cualquiera". Y esa
//! era la unica respuesta honesta mientras no se hubiera decidido de quien
//! fiarse.
//!
//! # *** Y EL 2026-09-10 SE LLENO, PORQUE YA HAY QUIEN FIRME
//!
//! `toolchain/tools/bmo-firmar` es la herramienta que `bmo-cripto/Cargo.toml`
//! llevaba nombrando desde el 25-08 como *"la que todavia no existe"*. Genera
//! el par, se niega a guardar la privada dentro de un repositorio git, y
//! estampa la firma en un `.bex` **ya construido**, sin mover un byte de lo
//! demas.
//!
//! La privada de la clave de abajo vive fuera de este arbol y **no baja a la
//! maquina**. `bmo-cripto` lo hace cumplir dejando el firmador detras de una
//! bandera que el kernel no enciende y que solo enciende esa herramienta.
//!
//! # [!] Y LO QUE CAMBIA HOY PARA UN `.bex` ES NADA. A PROPOSITO
//!
//! ```text
//!    un .bex sin firmar (sig_algo = 0)  SoloIntegridad    -> arranca, como ayer
//!    firmado con LA clave de abajo      Firmado           -> arranca, y DICE QUIEN
//!    firmado por cualquier otro         AutorDesconocido  -> NO arranca
//! ```
//!
//! ** OJO A LA TERCERA FILA, que sorprende: un `.bex` firmado por una clave que
//! no esta aqui **no arranca aunque `exige_firma()` sea `false`**. La bandera
//! decide si se admite lo NO firmado; no degrada una firma desconocida a "sin
//! firma". O sea que firmar algo antes de anclar su clave lo deja sin arrancar,
//! y por eso el orden de mas abajo es el que es.
//!
//!   > Una firma que no se reconoce es peor noticia que ninguna firma: alguien
//!   > se molesto en decir quien era, y no es nadie de aqui.
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
    // *** LA PRIMERA CLAVE DE ESTA MAQUINA, 2026-09-10.
    //
    // Generada por `bmo-firmar generar`, con azar de RDRAND y su prueba de
    // salud. La privada esta FUERA de este arbol y no ha pasado por ningun
    // commit: la herramienta se niega a escribirla dentro de un repositorio
    // git, subiendo por los ancestros hasta encontrar un `.git`.
    //
    // [!] Esto concede ejecucion a TODO lo que esa clave firme, para siempre.
    // No hay revocacion, y escribir la lista de revocados antes de la segunda
    // clave seria construir la puerta antes de la casa.
    (
        [
            0x8C, 0x64, 0x13, 0x09, 0xFA, 0x1B, 0xC4, 0x9B,
            0x77, 0xB9, 0xA2, 0xD8, 0xC5, 0x53, 0x8A, 0x75,
            0xBC, 0x85, 0x3B, 0xF9, 0xE7, 0xAF, 0x36, 0xCB,
            0x13, 0xD1, 0x40, 0xF4, 0x08, 0x96, 0x42, 0x1E,
        ],
        "Eddi -- el anfitrion",
    ),
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
