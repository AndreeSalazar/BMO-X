//! **LA CLAVE, Y LOS DOS SITIOS DONDE NO PUEDE ESTAR.**
//!
//! [carril]  VERDE     falla en la consola, antes de escribir nada
//! [cuesta]  NADA      no viaja a la maquina: corre en el anfitrion
//! [riesgo]  UNICO     no hay revocacion. Una privada que se escape no se
//!                     cancela: se cambia el ancla y se refirma todo
//!
//! # Las dos negativas que este modulo existe para decir
//!
//! ```text
//!    1. dentro de un repositorio git   ->  NO, y dice por que
//!    2. encima de una clave que ya hay ->  NO, y dice por que
//! ```
//!
//! **La 1 es la que importa.** Una clave privada dentro de un arbol con `.git`
//! no se filtra el dia que alguien la sube: se filtra el dia que alguien hace
//! `git add -A` sin mirar, y entonces ya esta en el historial, que no se borra.
//! Un `.gitignore` seria una peticion; esto es una negativa.
//!
//! ** Y la comprobacion es por ANCESTROS, no por el directorio de al lado: se
//! sube desde el destino buscando un `.git`, que es exactamente lo que hace git
//! para decidir si un fichero le pertenece. Preguntarlo de otra forma seria
//! preguntar una cosa parecida.
//!
//! *** La 2 no es paranoia de fichero. Sobreescribir una clave de firma en
//! silencio deja **todo lo firmado antes sin nadie que lo avale**, y el sintoma
//! aparece en el arranque siguiente como *"autor desconocido"* en binarios que
//! no se han tocado. Un fallo cuya causa es de hace tres dias y cuyo mensaje
//! apunta al binario de hoy.
//!
//! # Que NO hace este modulo
//!
//! No cifra la clave con contrasena. Y se dice aqui porque callarlo seria peor
//! que no hacerlo: hoy la proteccion de la privada es **el sistema de ficheros
//! del anfitrion y que no este en el repo**, ni mas ni menos. Cifrarla pedia
//! una derivacion de contrasena (Argon2 o scrypt) que este arbol no tiene, y
//! traerla de fuera rompe la regla de `bmo-cripto`: sin dependencias, para que
//! quien audite pueda leer las lineas.

use std::path::Path;

/// La cabecera del fichero de clave. Se comprueba al leer: un fichero que no
/// empieza asi no es una clave de esta casa, y tratarlo como tal seria firmar
/// con 32 bytes de cualquier cosa.
const MARCA: &str = "BMO-X clave de firma v1";

/// Sube por los ancestros buscando un `.git`. `Some(ruta)` = esta dentro.
fn dentro_de_un_repo(destino: &Path) -> Option<String> {
    let abs = std::fs::canonicalize(destino.parent().unwrap_or(Path::new(".")))
        .unwrap_or_else(|_| destino.to_path_buf());
    let mut d: Option<&Path> = Some(abs.as_path());
    while let Some(p) = d {
        if p.join(".git").exists() {
            return Some(p.display().to_string());
        }
        d = p.parent();
    }
    None
}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

fn de_hex(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// **Genera un par y lo guarda.** Devuelve la publica, que es lo unico que sale
/// por la consola: la privada se escribe al fichero y no se imprime nunca.
pub fn generar(destino: &Path) -> Result<[u8; 32], String> {
    if let Some(repo) = dentro_de_un_repo(destino) {
        return Err(format!(
            "ese sitio esta DENTRO del repositorio {repo}\n\
             \n\
             Una clave privada ahi no se filtra el dia que alguien la sube: se\n\
             filtra el dia que alguien hace `git add -A` sin mirar, y el\n\
             historial de git no se borra.\n\
             \n\
             Elige una ruta fuera del arbol."
        ));
    }
    if destino.exists() {
        return Err(format!(
            "ya hay una clave en {}\n\
             \n\
             No se sobreescribe. Todo lo que firmo esa clave se quedaria sin\n\
             nadie que lo avale, y el sintoma saldria en el arranque siguiente\n\
             como \"autor desconocido\" en binarios que nadie ha tocado.\n\
             \n\
             Si de verdad quieres una clave nueva, mueve esa a mano primero.",
            destino.display()
        ));
    }

    // El azar sale de RDRAND con su prueba de salud -- `bmo-cripto::azar`, que
    // sabe decir que no. Un generador que no puede fallar es un generador que
    // no comprueba.
    let secreta = bmo_cripto::azar::clave()
        .map_err(|_| "el CPU no dio azar sano: RDRAND fallo o dio repetidos".to_string())?;
    let publica = bmo_cripto::ed25519::publica_de(&secreta);

    let texto = format!(
        "{MARCA}\n\
         # La PRIVADA no baja a la maquina. Ver task/confianza.rs.\n\
         privada: {}\n\
         publica: {}\n",
        hex(&secreta),
        hex(&publica)
    );
    std::fs::write(destino, texto).map_err(|e| format!("no se pudo escribir: {e}"))?;
    Ok(publica)
}

/// Lee el par de un fichero de clave.
pub fn leer(origen: &Path) -> Result<([u8; 32], [u8; 32]), String> {
    let t = std::fs::read_to_string(origen)
        .map_err(|e| format!("no se pudo leer {}: {e}", origen.display()))?;
    if !t.starts_with(MARCA) {
        return Err(format!(
            "{} no empieza por \"{MARCA}\": no es una clave de esta casa",
            origen.display()
        ));
    }
    let mut secreta = None;
    for l in t.lines() {
        if let Some(v) = l.strip_prefix("privada:") {
            secreta = de_hex(v);
        }
    }
    let secreta = secreta.ok_or("el fichero no trae una linea `privada:` de 64 hex")?;
    // ** La publica se DERIVA, no se lee, aunque este escrita al lado.
    //
    // Si se leyera, un fichero con una publica que no corresponde a su privada
    // produciria una firma perfecta con la clave equivocada al lado -- y eso da
    // `NoCuadra` en el arranque, que manda a mirar el binario cuando el problema
    // esta en el fichero de clave. La linea `publica:` del fichero es para leerla
    // un humano, no esta herramienta.
    let publica = bmo_cripto::ed25519::publica_de(&secreta);
    Ok((secreta, publica))
}

/// Hexadecimal, para escribirlo por pantalla.
pub fn en_hex(b: &[u8]) -> String {
    hex(b)
}
