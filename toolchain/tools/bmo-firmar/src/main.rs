//! **LA HERRAMIENTA DE FIRMAR DEL ANFITRION.** C2 de `docs/plan/EL_ORDEN.md`.
//!
//! [carril]  VERDE     corre en Windows, escribe en la consola y no toca la
//!                     maquina. Si se equivoca se ve en el sitio
//! [cuesta]  NADA      no viaja a la maquina: corre en el anfitrion
//! [riesgo]  ESPEJO    el kernel y esto leen el MISMO formato por su cuenta,
//!                     y el compilador no puede comprobar que coincidan.
//!                     Mitigado: el veredicto lo da `bmo-firma`, el crate que
//!                     ejecuta el kernel, no una copia
//!
//! # Que faltaba de verdad, y no era criptografia
//!
//! `bmo-cripto` tiene Ed25519 completo desde el 25-08. El gate del cargador
//! esta cableado desde el mismo dia --`task/admitir.rs`-- y el ancla existe,
//! con nombre propio, en `task/confianza.rs`. Todo el lado que LEE estaba hecho.
//!
//! ```text
//!    verificar una firma      HECHO      bmo-firma + bmo-cripto
//!    el gate del cargador     CABLEADO   task/admitir.rs
//!    el ancla de confianza    EXISTE, vacia a proposito
//!    *** FIRMAR               NADIE      <-- esto
//! ```
//!
//! ** Y por eso todo `.bex` sale con `sig_algo = 0`. No porque el escritor se
//! olvidara: **porque no hay quien firme**. `bmo-cripto/Cargo.toml` lo dejo
//! escrito el 25-08, con esta herramienta nombrada y todavia inexistente.
//!
//! # Por que es una herramienta aparte y no una bandera del escritor
//!
//! Porque una maquina que puede firmar tiene dentro con que falsificar lo que
//! ejecuta -- y el anfitrion del build es una maquina mas. Si `BefBuilder`
//! supiera firmar, la clave privada tendria que estar donde corre el build:
//! en el CI, en el portatil, en cualquier sitio donde alguien compile.
//!
//! *** Asi, firmar es **un acto con nombre, una orden que alguien escribe**, y
//! no un efecto secundario de compilar. Que es exactamente lo que `confianza.rs`
//! pide para el otro extremo: *"que encender la firma sea una decision con
//! nombre en vez de un efecto secundario"*.
//!
//! # Las tres ordenes
//!
//! ```text
//!    bmo-firmar generar <ruta-de-la-clave>
//!    bmo-firmar firmar  <app.bex> <ruta-de-la-clave>
//!    bmo-firmar ver     <app.bex> [ancla-en-hex ...]
//! ```
//!
//! [!] Y `ver` es la que hace util a `firmar`: contesta lo mismo que contestara
//! el Ryzen, **con el mismo codigo**, antes de flashear nada. Un fallo de firma
//! en el metal se ve como un `.bex` que no arranca y un motivo en CABINA; aqui
//! se ve en una linea y sin reiniciar.

mod bex;
mod llave;

use std::path::Path;
use std::process::ExitCode;

fn ayuda() {
    println!(
        "bmo-firmar -- la herramienta de firmar del anfitrion

  generar <ruta>              crea un par Ed25519. La privada se guarda ahi y
                              NO se imprime; sale la publica, que es la que se
                              pega en `ring0/task/confianza.rs`.
                              Se niega si la ruta esta dentro de un repo git.

  firmar <app.bex> <ruta>     comprueba los digests, firma la cadena y estampa
                              los 96 bytes en el hueco. No mueve nada mas.

  ver <app.bex> [hex ...]     dice el veredicto CON EL CRATE DEL KERNEL. Los
                              hex son el ancla; sin ellos, el ancla esta vacia,
                              que es lo que hoy tiene la maquina.

  ancla                       escupe en hex las claves que trae compiladas
                              `ring0/task/confianza.rs`, para no teclearlas:

                                  bmo-firmar ver app.bex $(bmo-firmar ancla)"
    );
}

/// **EL ANCLA QUE LLEVA EL KERNEL, EN HEX.**
///
/// # Por que esta orden existe, y es de hoy mismo
///
/// Al llenar el ancla por primera vez copie los 32 bytes a mano y **me comi
/// dos**: `0x77, 0xB9` salio `0xB7, 0x9B`. Compilaba, arrancaba, y el unico
/// sintoma habria sido un `.bex` legitimo saliendo por CABINA como *"AUTOR
/// DESCONOCIDO"* -- un mensaje que manda a mirar la firma cuando lo que estaba
/// mal era el ancla.
///
/// ** Lo caze comparando a maquina, y esta orden es esa comparacion **hecha
/// permanente**: la unica forma de teclear una clave publica es no teclearla.
///
///   > Un dato que un humano copia a mano de un sitio a otro se copia mal una
///   > vez de cada tantas. La cuenta no depende del cuidado.
///
/// [!] Lee el Rust con dos anclas de texto, no con un parser. Si `confianza.rs`
/// cambia de forma esto **falla y lo dice**, que es lo unico que no puede hacer:
/// contestar una lista incompleta.
fn ancla() -> Result<(), String> {
    const FUENTE: &str = "Ultra_kernel_x86-64/kernel/src/ring0/task/confianza.rs";
    let t = std::fs::read_to_string(FUENTE)
        .map_err(|e| format!("no se pudo leer {FUENTE}: {e} (se lee desde la raiz del repo)"))?;
    let i = t
        .find("pub static ANCLA")
        .ok_or("confianza.rs ya no declara `pub static ANCLA`: esta orden no sabe leerlo")?;
    let j = t[i..]
        .find("\n];")
        .ok_or("no encuentro el final de ANCLA")?;
    let cuerpo = &t[i..i + j];

    let mut bytes = Vec::new();
    let mut resto = cuerpo;
    while let Some(p) = resto.find("0x") {
        resto = &resto[p + 2..];
        let n: String = resto.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        // Solo los de dos digitos son bytes de clave. Cualquier otro `0x` del
        // fichero --un comentario, una mascara-- se ignora en vez de colarse.
        if n.len() == 2 {
            bytes.push(u8::from_str_radix(&n, 16).unwrap());
        }
    }
    if bytes.is_empty() {
        println!("# el ancla esta VACIA: esta maquina no confia en nadie");
        return Ok(());
    }
    if bytes.len() % 32 != 0 {
        return Err(format!(
            "el ancla suma {} bytes, que no es multiplo de 32: o esta truncada o \
             este lector se equivoca. No se contesta una lista a medias",
            bytes.len()
        ));
    }
    for k in bytes.chunks(32) {
        println!("{}", llave::en_hex(k));
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(|s| s.as_str()) {
        Some("generar") if args.len() == 2 => generar(Path::new(&args[1])),
        Some("firmar") if args.len() == 3 => firmar(Path::new(&args[1]), Path::new(&args[2])),
        Some("ver") if args.len() >= 2 => ver(Path::new(&args[1]), &args[2..]),
        Some("ancla") if args.len() == 1 => ancla(),
        _ => {
            ayuda();
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bmo-firmar: {e}");
            ExitCode::FAILURE
        }
    }
}

fn generar(destino: &Path) -> Result<(), String> {
    let publica = llave::generar(destino)?;
    println!("clave nueva en {}", destino.display());
    println!();
    println!("La PRIVADA se ha quedado ahi y no se imprime. La PUBLICA es esta:");
    println!();
    println!("    {}", llave::en_hex(&publica));
    println!();
    println!("Para que esta maquina confie en ella, en `confianza.rs::ANCLA`:");
    println!();
    print!("    ([");
    for (i, x) in publica.iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        print!("0x{x:02X}");
    }
    println!("], \"...\"),");
    println!();
    println!("[!] Anadir una clave al ancla es conceder ejecucion a todo lo que");
    println!("    esa clave firme, PARA SIEMPRE. No hay revocacion.");
    Ok(())
}

fn firmar(ruta: &Path, clave: &Path) -> Result<(), String> {
    let mut b = std::fs::read(ruta).map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
    let f = bex::Bex::abrir(&b)?;

    // == 1. NO SE FIRMA LO QUE NO SE HA COMPROBADO =======================
    //
    // Una firma sobre un binario corrupto no lo arregla: lo acredita. Y el
    // resultado es peor que no firmar, porque el sistema pasa a tener una
    // razon para ejecutarlo.
    f.comprobar(&b)?;
    println!("los {} digests cuadran con los bytes", f.cuantos);

    if f.algo(&b) != 0 {
        // Refirmar no es un error --se cambia de clave, caduca la anterior--
        // pero es una cosa distinta de firmar y se dice.
        println!("[!] este .bex YA venia firmado: se sustituye la firma");
    }

    let (secreta, publica) = llave::leer(clave)?;
    let cadena = f.cadena(&b);
    let sig = bmo_cripto::ed25519::firmar(&secreta, &cadena);
    f.estampar(&mut b, &sig, &publica)?;
    std::fs::write(ruta, &b).map_err(|e| format!("no se pudo escribir: {e}"))?;

    println!("cadena  {}", llave::en_hex(&cadena));
    println!("firmado por {}", llave::en_hex(&publica));

    // == 2. Y SE RELEE CON EL CRATE DEL KERNEL ===========================
    //
    // No con una comprobacion de aqui. Lo que decida `bmo-firma` es literalmente
    // lo que decidira `task/admitir.rs` en el Ryzen sobre estos mismos bytes.
    let f2 = bex::Bex::abrir(&b)?;
    let v = bmo_firma::examinar(f2.seccion(&b), &f2.cadena(&b), &[publica]);
    match v {
        bmo_firma::Veredicto::Firmado { .. } => {
            println!("comprobado con bmo-firma: FIRMADO");
            Ok(())
        }
        otro => Err(format!(
            "acabo de firmarlo y el verificador dice \"{}\" -- no se toca el \
             fichero hasta entender esto",
            otro.motivo()
        )),
    }
}

fn ver(ruta: &Path, ancla_hex: &[String]) -> Result<(), String> {
    let b = std::fs::read(ruta).map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
    let f = bex::Bex::abrir(&b)?;
    println!("seccion Signature en 0x{:X}, {} bytes", f.sec_off, f.sec_len);
    println!("digests   {}", f.cuantos);
    println!("sig_algo  {}", f.algo(&b));

    // == *** LAS DOS PUERTAS, Y NINGUNA SOBRA (2026-09-10) ================
    //
    // La primera version de esto escribia el veredicto de la firma y ya. Y al
    // probarlo con un byte del codigo cambiado contesto:
    //
    // ```text
    //    integridad: la seccion 0 NO cuadra con su digest
    //    veredicto:  Firmado -> ARRANCA
    // ```
    //
    // ** Las dos lineas son CIERTAS y juntas mienten. La firma **no cubre los
    // bytes: cubre los digests**. Cambiar el codigo no toca la cadena, asi que
    // la firma sigue cuadrando -- lo que caza ese byte es la otra puerta, el
    // hash por seccion que el kernel comprueba al aterrizar cada una
    // (`admitir.rs`, `Cierre::Cuadra`).
    //
    // ```text
    //    un byte del codigo cambiado    lo caza EL DIGEST de esa seccion
    //    un digest cambiado             lo caza LA CADENA, o sea la firma
    //    una seccion quitada o anadida  lo caza LA CADENA: sobra o falta un digest
    //    -> la firma sin los digests no prueba nada de los bytes
    //    -> los digests sin la firma los puede recalcular cualquiera
    // ```
    //
    // *** Asi que el kernel pasa LAS DOS, y esta herramienta tiene que contestar
    // lo mismo que el kernel. Una herramienta que da una respuesta mejor que la
    // maquina es peor que no tenerla: se usa para decidir, y decide de mas.
    let integro = match f.comprobar(&b) {
        Ok(()) => {
            println!("integridad: los digests cuadran");
            true
        }
        Err(e) => {
            println!("integridad: {e}");
            println!("            [!] la firma NO cubre esto -- cubre los digests, y");
            println!("            este byte lo caza el hash de su seccion al aterrizar");
            false
        }
    };

    let mut ancla: Vec<[u8; 32]> = Vec::new();
    for h in ancla_hex {
        let mut k = [0u8; 32];
        if h.trim().len() != 64 {
            return Err(format!("\"{h}\" no son 64 caracteres hexadecimales"));
        }
        for i in 0..32 {
            k[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16)
                .map_err(|_| format!("\"{h}\" no es hexadecimal"))?;
        }
        ancla.push(k);
    }

    let v = bmo_firma::examinar(f.seccion(&b), &f.cadena(&b), &ancla);
    println!();
    println!("veredicto: {v:?}");
    println!("           {}", v.motivo());
    println!();
    // Las dos politicas, porque la diferencia entre ellas ES la decision que
    // queda pendiente en `confianza.rs::exige_firma()`. Y las dos van con `Y`
    // sobre la integridad: ver el comentario de las dos puertas, arriba.
    let dice = |exige: bool| -> &'static str {
        if !integro {
            "NO arranca (lo para el digest, no la firma)"
        } else if v.permite_ejecutar(exige) {
            "ARRANCA"
        } else {
            "NO arranca"
        }
    };
    println!("  con exige_firma = false (lo de hoy):  {}", dice(false));
    println!("  con exige_firma = true:               {}", dice(true));
    Ok(())
}
