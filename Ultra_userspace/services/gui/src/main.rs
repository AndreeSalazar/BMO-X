//! **El compositor de BMO.** El proceso Ring 3 que es dueno de la pantalla.
//!
//! ## La caja
//!
//! No hay terminal. Habia uno planeado --`apps/terminal`, doce lineas de
//! esqueleto-- y se ha quitado, porque un terminal de verdad es una pila entera:
//! scrollback, PTY, senales, un interprete, edicion de linea, historial. Nada de
//! eso hace falta para lo unico que hoy se quiere hacer desde la pantalla, que
//! es **arrancar un programa**.
//!
//! Asi que lo que hay es una caja de una linea, como el `Win+R` de Windows.
//! Escribes una ruta, pulsas Enter, y el `.bex` corre. Es la forma mas pequena
//! de "terminal" que sigue siendo util, y no arrastra nada de lo otro.
//!
//! * Y no es una API prestada de nadie: `Win+R` tampoco lo es alli. Es UI del
//! shell, y por debajo acaba llamando a lo mismo que llamaria cualquiera. Aqui
//! por debajo hay `OP_EJECUTAR` sobre `CURRENT_TASK`, que es una operacion mas
//! en una tabla -- **el ABI de dos syscalls no se toca para esto**.
//!
//! ## Quien manda sobre el teclado
//!
//! Reclamar `KIND_INPUT` ahora cede el teclado ademas del raton, y eso tiene
//! consecuencia al otro lado: mientras este proceso viva, el shell de Ring 0 no
//! lee el teclado fisico. No es un reparto --los dos drenarian la misma cola y
//! se robarian letras-- es una cesion. El cable serie sigue siendo del kernel,
//! que es lo que hace falta cuando esto se rompa.
//!
//! ## La tira de medida sigue
//!
//! Los seis parches de color siguen ahi abajo porque la pregunta que hacen
//! sigue abierta: en la primera foto en hardware la geometria salio exacta pero
//! los colores mucho mas claros de lo que dice el codigo. Hasta que una foto lo
//! zanje, se quedan --
//!
//! - si el parche `0x00FF0000` sale ROJO, el formato es XRGB como creemos;
//! - si sale AZUL, los canales estan al reves (BGR) y hay que voltearlos;
//! - si `0x00202020` sale gris medio en vez de casi negro, no es orden de
//!   canales: algo toca la intensidad (el panel, o el propio GOP).

#![no_std]
#![no_main]

use bmo_userland as bmo;

// -- El reparto ----------------------------------------------------------
//
// Eran 2308 lineas en un solo fichero: colores, geometria, la rejilla de
// salida, el historial, la calculadora, los informes del sistema, el
// interprete de comandos y el bucle de fotograma. Todo junto, compartiendo
// constantes y variables locales.
//
// Dos carpetas, y la frontera es la que importa:
//
//   scene/     lo que se PINTA      -- no sabe que es un comando
//   commands/  lo que se INTERPRETA -- no sabe de que color es la ventana
//
// `main.rs` se queda con lo unico que necesita a las dos: el bucle.
//
// ** LOS NOMBRES SON INGLESES DESDE EL 2026-08-14, y la frontera de ese
// cambio es la comilla: un identificador es un CONTRATO --puede tener gemelo
// en `bmo-userland`, en `bmo-input` o en el `.bex`-- y una cadena es SALIDA,
// que se queda en espanol porque es lo que se lee en pantalla.
//
// Lo que NO se toco, a proposito:
//
//   `p.texto`, `p.ancho`, `p.alto`, `Pantalla`, `Entrada`, `Consola`   userland
//   `Foco::es_para`, `poner_modo`, `nombre_largo`                      bmo-input
//   `bmo_rtc::escribir`, `bmo::estratos::tipo`, `e.tecla`              sus crates
//   `cabina.rs`, `gato.rs`                                             NOMBRES PROPIOS
//
// Renombrar un contrato solo en este lado no da un error: da DOS nombres para
// una cosa, y nada caza la deriva hasta que alguien lee el campo equivocado.
mod scene;
mod commands;
mod text;
mod watch;

use scene::calc::{paint_calc, Calc, CalcPad};
use scene::cursor::SaveUnder;
use scene::output::{paint_output, Output, INK_GOOD, INK_ECHO, INK_ERR, INK_PLAIN};
use scene::*;
use commands::complete::{complete, file_error_reason};
use commands::history::History;
use commands::reports::{report_autopsy, report_cpu, report_memory, report_system};
use commands::*;
use text::{decimal, is_dot_entry};
use watch::{watch_run, Run};


// -- El programa ---------------------------------------------------------

/// Cada cuantas vueltas del bucle parpadea el cursor de escritura.
///
/// Se cuenta en fotogramas y no en tiempo porque aqui no hay reloj: los tres
/// syscalls no incluyen "que hora es". Es un parpadeo que depende de la
/// velocidad de la maquina, y para decir "aqui se escribe" eso basta.
const BLINK: u32 = 12_000;

/// Donde va el volcado cuando nadie dice otra cosa.
///
/// * En `data/`, que vive en la particion **FAT32** -- la misma que se enchufa
/// a un Windows y se abre con el bloc de notas. Ese es el motivo entero de que
/// esto exista: hasta hoy, saber que habia hecho una corrida de BMO-X era
/// hacerle una foto a la pantalla. Una foto no se compara con la de ayer, no se
/// busca dentro, y no se le puede ensenar a nadie que no este delante.
///
/// **No va a ESTRATOS aunque ESTRATOS sea el sistema de ficheros bueno**, y no
/// es una concesion: ningun otro sistema operativo sabe leerlo. Un volcado que
/// solo BMO puede abrir no resuelve el problema para el que se escribio.
///
/// `SALIDA.TXT` es 8.3 -- el driver FAT32 del kernel se niega a recortar.
pub(crate) const DEFAULT_DUMP: &[u8] = b"datos/salida.txt";

/// Monta `data/<programa>.txt` a partir de la ruta que se lanzo.
///
/// === Por que un archivo POR PROGRAMA y no siempre el mismo ===
///
/// Porque la forma de trabajar es *"corre los doce ejemplos y mirame el
/// disco"*. Con un `output.txt` unico, correr los doce deja **uno**: el del
/// ultimo. Con el nombre del programa dentro, deja doce, y se pueden comparar
/// entre si y con los de ayer.
///
/// De `cobol/10/maestro.bex` sale `data/maestro.txt`: se coge lo que hay tras
/// la ultima barra y se corta en el punto. **Ocho letras como mucho**, porque
/// el driver FAT32 del kernel se niega a recortar y un nombre largo no crearia
/// el archivo -- fallaria al cerrar, en silencio, que es justo lo que se acaba
/// de arreglar.
fn dump_name(target: &[u8], dst: &mut [u8; 32]) -> usize {
    // El verbo `run` delante, si lo lleva: `run cobol/1/hola.bex`.
    let path = match target.iter().rposition(|&c| c == b' ') {
        Some(i) => &target[i + 1..],
        None => target,
    };
    let cut = path.iter().rposition(|&c| c == b'/' || c == b'\\');
    let base = match cut {
        Some(i) => &path[i + 1..],
        None => path,
    };
    let stem = match base.iter().position(|&c| c == b'.') {
        Some(i) => &base[..i],
        None => base,
    };
    // ** LA CARPETA VA DELANTE, y esto no es adorno.
    //
    // Con solo el nombre del programa, `cobol/8/cierre.bex` y `ada/cierre.bex`
    // escribian **los dos en `data/cierre.txt`**: el segundo se comia al
    // primero sin decir nada. Correr la escalera entera y perder un resultado
    // por el camino es justo lo que este volcado existe para impedir.
    //
    // Con la carpeta delante quedan `8cierre` y `adacierr`, y de paso el
    // numero del nivel se lee en el nombre: `2banco`, `10maestr`, `9comisio`.
    let folder: &[u8] = match cut {
        Some(i) => {
            let top = &path[..i];
            match top.iter().rposition(|&c| c == b'/' || c == b'\\') {
                Some(j) => &top[j + 1..],
                None => top,
            }
        }
        None => b"",
    };
    let mut n = 0usize;
    for &b in b"datos/" {
        dst[n] = b;
        n += 1;
    }
    // Ocho letras y ni una mas: el driver FAT32 del kernel **se niega a
    // recortar**, asi que un nombre largo no fallaria al crear -- fallaria al
    // cerrar, que es donde ya sabemos que duele.
    let start = n;
    for &b in folder.iter().chain(stem.iter()) {
        if n - start >= 8 {
            break;
        }
        dst[n] = b;
        n += 1;
    }
    if n == start {
        // Ni carpeta ni nombre: mejor un archivo con un nombre soso que
        // ninguno.
        for &b in b"salida" {
            dst[n] = b;
            n += 1;
        }
    }
    for &b in b".txt" {
        dst[n] = b;
        n += 1;
    }
    n
}

/// Escribe las filas `[from..=to]` del historial en un archivo de texto.
///
/// Devuelve `Ok(bytes)` o el motivo. **Nada llega al disco hasta `close`**, y
/// por eso el resultado de `close` es el que se mira: es el unico que sabe si
/// el archivo existe de verdad. Que se guarde encima de uno anterior es
/// deliberado -- un volcado que fallara la segunda vez obligaria a inventar
/// nombres, y un `salida1.txt`, `salida2.txt`... es exactamente el desorden que
/// este archivo viene a evitar.
/// ** GUARDA LA AUTOPSIA SOLA, en cuanto el kernel mata una tarea.
///
/// El kernel redacta el informe y lo deja en RAM; esto lo saca a disco. La
/// division es a proposito y esta explicada en `ring0/core/autopsia.rs`:
/// escribir un fichero DENTRO de un manejador de faults es entrar en el driver
/// de disco, que puede ser justo lo que acaba de caerse.
///
/// * Se llama con el numero de fallos que se vieron la ultima vez. Si no ha
/// cambiado no hace NADA -- ni abre el fichero, ni lee un renglon. Es una
/// comparacion de enteros por vuelta del bucle, que es lo que permite que esto
/// viva en el camino de cada fotograma sin costar nada.
///
/// Se ANADE al fichero abriendolo entero cada vez y reescribiendo los cuatro
/// que el kernel guarda: `Archivo::create` trunca, y llevar un cursor entre
/// arranques seria estado que hay que sincronizar. Cuatro informes de ocho
/// renglones son dos kilobytes; reescribirlos es mas barato que acordarse.
fn save_autopsies(vistos: &mut u64) -> bool {
    let total = bmo::autopsia_total();
    if total == *vistos {
        return false;
    }
    *vistos = total;
    let Ok(a) = bmo::Archivo::create(b"datos/fallos.txt") else {
        // Sin fichero no se pierde el informe: sigue en el kernel y `fallo` lo
        // ensena. Se contesta `true` igual porque el fallo SI ocurrio, que es
        // lo que el que mira la pantalla tiene que saber.
        return true;
    };
    let how_many = bmo::autopsia_disponibles();
    let mut buf = [0u8; 96];
    for i in 0..how_many {
        let rows = bmo::autopsia_renglones(i);
        for f in 0..rows {
            let n = bmo::autopsia_linea(i, f, &mut buf);
            a.write(&buf[..n]);
            // `\r\n` por lo mismo que `dump_output`: esto se abre en Windows
            // para mandarlo, y el Notepad viejo junta los saltos de Unix.
            a.write(b"\r\n");
        }
        a.write(b"\r\n");
    }
    a.close();
    true
}

fn dump_output(output: &Output, path: &[u8], from: usize, to: usize) -> Result<usize, u32> {
    let a = bmo::Archivo::create(path)?;
    let mut bytes = 0usize;
    for f in from..=to {
        let line = output.line(f);
        bytes += a.write(line);
        // `\r\n` y no `\n`: esto lo va a abrir el bloc de notas de Windows, y
        // el Notepad viejo ensena un archivo con saltos de Unix como una sola
        // linea kilometrica. Aqui el destinatario manda sobre la elegancia.
        bytes += a.write(b"\r\n");
    }
    if a.close() {
        Ok(bytes)
    } else {
        // El kernel no dice el motivo -- se queda en la CABINA (F11). Lo que si
        // se sabe con certeza es que en el disco NO hay nada, y eso es lo que
        // el que mira la pantalla necesita saber.
        Err(0)
    }
}

/// **Devuelve la caja de Ejecutar a la pantalla** tras haberla tapado.
///
/// === Por que es una funcion y no tres lineas ===
///
/// Porque eran tres lineas **trece veces**, y no identicas: unas llevaban
/// `erase_window` delante, otras `top_before` detras, y en alguna faltaba
/// `output.dirty`. Esa ultima variacion no da un error de compilacion -- da una
/// rejilla de salida que se queda en blanco hasta que algo *no relacionado*
/// vuelve a ensuciarla, y entonces se busca el fallo en el terminal cuando
/// estaba en el gestor de ventanas.
///
/// Trece copias de una secuencia no son un estilo: son doce oportunidades de
/// que una se quede atras. Aqui solo se puede olvidar en un sitio.
///
/// El `top_before` se queda FUERA a proposito: eso es el orden de las
/// ventanas, otra pregunta. Meterlo dentro haria que esta funcion mintiera
/// sobre lo que hace.
fn uncover(
    p: &bmo::Pantalla,
    run_box: &RunBox,
    visible: bool,
    output: &mut Output,
    repaint_field: &mut bool,
) {
    if !visible {
        return;
    }
    paint_run_box(p, run_box);
    *repaint_field = true;
    // La rejilla se marca sucia y NO se pinta aqui: pintarla ahora la dibujaria
    // por debajo de una ventana que a lo mejor sigue encima. Quien decide eso es
    // el bloque de pintado, que sabe quien esta arriba.
    output.dirty = true;
}

/// * Este `.bex` DECLARA que quiere la pantalla?
///
/// Se lee la cabecera BEF del archivo antes de lanzarlo: `flags` esta en el
/// offset 8 y el bit 10 es `BefFlags::WANTS_SCREEN`, que **pone el compilador**
/// al ver que el programa invoca `BMO_OP_PANTALLA_RECLAMAR`.
///
/// # Por que esto es lo que hacia falta, y `presta` no
///
/// `presta <path>` funcionaba y era el diseno equivocado: ponia la POLITICA en
/// los dedos del usuario, que tenia que saberse de memoria que programas son
/// graficos. Con la bandera, **el compositor decide** -- que es su trabajo, y la
/// razon de que exista un compositor.
///
/// Tres capas, cada una con lo suyo: el kernel arbitra (un dueno, `release`), el
/// BEF declara, y aqui se manda. La misma separacion que un planificador de GPU:
/// el hardware no sabe que es importante, el planificador si.
///
/// # Doce bytes que cuestan una lectura entera, y hay que decirlo
///
/// Se leen doce bytes, pero `Archivo::leer_de` **se trae el archivo COMPLETO** al
/// abrirlo (por eso una lectura posterior no puede fallar a mitad). Asi que cada
/// `run` toca el disco dos veces: una aqui y otra al lanzar.
///
/// Para un `.bex` de 7 KB desde un SATA no se nota, y se acepta a cambio de que
/// la politica viva en el sitio correcto. **La forma barata seria que el kernel
/// devolviera la bandera desde `EJECUTAR`**, que ya tiene el binario en la mano
/// -- queda anotado como lo que es: una optimizacion pendiente, no un misterio.
///
/// No se valida el binario: de eso ya se encarga el gate de admision del kernel,
/// que es quien tiene autoridad para rechazarlo. Aqui un archivo raro o ilegible
/// contesta `false` y sigue el camino normal -- **la duda se resuelve NO
/// prestando**, que es el lado seguro.
fn wants_screen(path: &[u8]) -> bool {
    let Ok(f) = bmo::Archivo::leer_de(path) else { return false };
    let mut cab = [0u8; 12];
    if f.read(&mut cab) < 12 {
        return false;
    }
    // El magic se comprueba antes de creerse los flags: doce bytes de un `.txt`
    // tambien tienen un bit 10.
    let magic = u32::from_le_bytes([cab[0], cab[1], cab[2], cab[3]]);
    if magic != bmo_abi_magic() {
        return false;
    }
    let flags = u32::from_le_bytes([cab[8], cab[9], cab[10], cab[11]]);
    flags & (1 << 10) != 0
}

/// El magic de un BEF: los cuatro bytes `BEF1`.
///
/// Escrito aqui y no importado de `bmo-abi` por el mismo motivo que el kernel lo
/// lee a mano en `bex.rs`: el compositor es `no_std` sin `alloc` y no enlaza esa
/// crate. Se construye desde el literal --`from_le_bytes(*b"BEF1")`-- y no como un
/// hexadecimal a mano: un numero copiado se equivoca de orden de bytes en
/// silencio, y las cuatro letras no.
const fn bmo_abi_magic() -> u32 {
    u32::from_le_bytes(*b"BEF1")
}

/// * PRESTAR LA PANTALLA a un programa y recuperarla cuando muera.
///
/// Consume la `Pantalla` y devuelve otra: entre medias **este proceso no tiene
/// pantalla**, y que el tipo lo refleje es lo que impide pintar en un puntero ya
/// desmapeado. Ver `bmo::Pantalla::release`.
///
/// # Por que se PREGUNTA quien la tiene, en vez de intentar tomarla
///
/// La tentacion es un bucle de `Pantalla::claim()` hasta que salga. No sirve:
/// justo despues de soltarla **esta libre**, asi que el primer intento acierta y
/// se la quitamos al programa antes de que llegue a pedirla. Reclamar para
/// averiguar si esta libre te la deja puesta.
///
/// De ahi `INFO_PANTALLA_DUENO`, que contesta el `pid` del dueno (o `0`) sin
/// tocar nada.
///
/// # Y por que no vale `has_child()`
///
/// Porque contesta *"el hijo ha escrito en la consola"*, no *"el hijo esta
/// vivo"* -- lo dice el vigilante de la corrida en este mismo archivo. `ray.bex`
/// dibuja durante minutos sin imprimir una letra, asi que esperarlo por ahi
/// habria vuelto en el primer fotograma. Lo que si es exacto es la propiedad de
/// la pantalla: el kernel la libera en `fb::process_died`, o sea que el dueno
/// volviendo a `0` **es** el programa terminando.
///
/// # Las dos fases, y el tope de la primera
///
/// 1. Esperar a que la TOME, con tope de 500 ms. Si no la toma, no la queria --
///    un `presta ls` no puede colgar el escritorio para siempre.
/// 2. Esperar a que la SUELTE, sin tope: aqui si se sabe que hay alguien
///    dentro, y un juego puede durar lo que quiera.
fn lend_screen(
    p: bmo::Pantalla,
    input: Option<bmo::Entrada>,
    target: &[u8],
    consola: u64,
) -> Option<(bmo::Pantalla, Option<bmo::Entrada>)> {
    // ** SE PRESTAN LAS DOS, Y ESTO ES UN ARREGLO EN METAL.
    //
    // La primera version presto solo la PANTALLA. En el Ryzen, `ray.bex` pinto
    // cielo y suelo -- y se quedo dentro para siempre, porque el escritorio se
    // habia quedado la ENTRADA y el raycaster no podia leer su propio ESC. La
    // maquina sin teclado y sin forma de volver.
    //
    // **Ceder la pantalla sin ceder la entrada no es prestar: es dejar a alguien
    // pintando en una habitacion cerrada.**
    //
    // La entrada se suelta DESPUES de la pantalla y se recupera ANTES, o sea en
    // orden inverso: si algo falla en medio, el escritorio prefiere quedarse sin
    // teclado un momento que sin pantalla.
    let mut had_input = false;
    if let Some(e) = input {
        had_input = true;
        e.release();
    }
    let recover = move || {
        let p = bmo::Pantalla::claim()?;
        let e = if had_input { bmo::Entrada::claim() } else { None };
        Some((p, e))
    };
    if !p.release() {
        // No eramos el dueno: raro, pero no se sigue a ciegas. Se intenta
        // recuperar y punto.
        return recover();
    }
    if bmo::ejecutar_en(target, consola).is_err() {
        // El programa no arranco, asi que nadie va a tomar la pantalla: se
        // recupera YA en vez de esperar los 500 ms de la fase 1.
        return recover();
    }
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    let mut took_it = false;
    if hz > 0 {
        let limit = bmo::ciclos() + hz / 2; // 500 ms, cronometrados
        while bmo::ciclos() < limit {
            if bmo::info(bmo::INFO_PANTALLA_DUENO) != 0 {
                took_it = true;
                break;
            }
            bmo::yield_screen();
        }
    }
    if took_it {
        while bmo::info(bmo::INFO_PANTALLA_DUENO) != 0 {
            bmo::yield_screen();
        }
    }
    recover()
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // El aviso va ANTES de reclamar: en cuanto la cesion se consuma, el kernel
    // deja de dibujar y nada de lo que se imprima despues llega al panel.
    bmo::consola("reclamo pantalla y entrada\n");

    let Some(mut p) = bmo::Pantalla::claim() else {
        bmo::consola("sin pantalla que reclamar\n");
        bmo::salir()
    };

    // -- * EL DOBLE BUFER --
    //
    // Se pide ANTES de pintar nada, que es cuando la RAM esta menos
    // fragmentada: el bloque tiene que ser contiguo en fisico y son ~8 MB.
    //
    // Y se dice en los dos casos. Que no haya doble bufer **no impide arrancar**
    // --se dibuja en el panel, como siempre--, pero cambia dos cosas que se notan:
    // vuelve el riesgo de tearing y el cursor tiene que poner una barrera antes
    // de leer. Un escritorio que se degrada en silencio es un escritorio del que
    // no se puede diagnosticar nada.
    if p.activar_doble_bufer() {
        bmo::consola("doble bufer: pintando fuera de la pantalla\n");
    } else {
        bmo::consola("SIN doble bufer: no hubo bloque, pinto directo al panel\n");
    }
    // La entrada es opcional a proposito: sin ella hay escritorio, solo que
    // quieto y mudo. Un compositor que se niega a arrancar porque falta un
    // periferico es un compositor que no arranca el dia que el periferico falla.
    // `mut` porque `presta` la SUELTA y la vuelve a reclamar: la capability se
    // va y vuelve, asi que el binding tiene que poder cambiar.
    let mut input = bmo::Entrada::claim();

    // La consola de este terminal. Desde aqui, todo lo que lance escribe en
    // ESTE anillo y no en el panel del kernel -- que es lo unico que separaba
    // una caja de lanzar de un terminal de verdad.
    let child_console = bmo::Consola::create();

    let run_box = RunBox::new(p.ancho, p.alto);

    // -- LA ENTRADA A RING 3 --
    //
    // Antes de dibujar nada del escritorio, decir lo que acaba de pasar: el
    // userspace tiene la maquina. Hasta hoy este paso era invisible y por eso
    // un compositor muerto y un compositor que no pinta se veian igual -- un
    // shell donde debia haber un escritorio.
    //
    // Y lleva las dos capabilities OPCIONALES escritas en la cara, que es lo
    // que distingue "no funciona" de "no me la dieron".
    // * Y la espera del final se puede SALTAR con una tecla, por eso va la
    // capability y no un `bool`: 1.100 de los 1.205 ms hasta el escritorio eran
    // esa espera, y el dueno la leyo como un fallo mirando el cronometro del
    // klog. Tenia razon en sospechar.
    scene::splash::paint(&p, input.as_ref(), child_console.is_some());
    bmo::consola("entrada a Ring 3 pintada\n");

    // -- El escritorio --
    //
    // * Aqui vivian los SEIS PARCHES DE MEDIDA y el PULSOMETRO del raton, y se
    // han quitado el 2026-08-04. No eran decoracion: los parches contestaban
    // "el orden de canales es el que creo?" y la barra contestaba "llegan
    // informes del raton?". **Las dos preguntas estan contestadas** -- los
    // colores salen bien desde hace semanas y el puntero se mueve donde se
    // mueve la mano, o sea que el propio cursor ES el pulsometro.
    //
    // Un instrumento que ya no mide nada deja de ser un instrumento y pasa a
    // ser ruido: seis cuadrados de colores puros y una barra en mitad del
    // escritorio son lo que hacia que esto pareciera un panel de pruebas y no
    // una maquina. Si algun dia hay que volver a medir el formato del
    // framebuffer, el `git log` tiene los valores exactos con su porque.
    paint_background(&p);
    // ** LOS ICONOS, y se leen UNA VEZ.
    //
    // Recorrer `apps\` y sacarle el icono a cada `.bex` son varias lecturas de
    // disco por app, y ninguna cambia mientras la maquina esta encendida. Un
    // escritorio que releyera el directorio por fotograma haria E/S sesenta
    // veces por segundo para ensenar exactamente lo mismo.
    //
    // Va JUSTO DESPUES del fondo y antes de todo lo demas: los iconos son lo de
    // mas atras que se pinta, igual que en cualquier escritorio.
    let launcher = scene::launcher::Launcher::new();
    scene::launcher::paint(&p, &launcher);
    p.rect(16, 13, 14, 14, ACCENT);
    p.texto(38, 14, "BMO-X", INK);
    // Las fichas se pintan en el bucle: dependen de que este abierto y de
    // quien tenga el foco, y las dos cosas cambian.
    let mut taskbar_dirty = true;
    let mut taskbar_state_before = (false, 0u8, false, false);

    // Lo que SI era informacion y no instrumento: si la entrada no se pudo
    // reclamar hay que decirlo, y ahora se dice con palabras en la barra en vez
    // de con el color de un marco. Un rojo sin texto obliga a saberse el
    // codigo de colores.
    if input.is_none() {
        // El aviso se coloca por su LARGO REAL y no por un numero a ojo: son
        // cuarenta letras, y con un hueco puesto a mano de treinta y cuatro se
        // saldria por la derecha justo el dia que haga falta leerlo.
        const WARN: &str = "SIN ENTRADA: teclado y raton son de otro";
        let width = bmo::Pantalla::ancho_escala(WARN, 1);
        p.texto(p.ancho.saturating_sub(width + 16), 14, WARN, INK_BAD);
    }

    paint_run_box(&p, &run_box);
    let mut path = [0u8; PATH_MAX];
    let mut n = 0usize;
    let mut output = Output::new();
    let mut history = History::new();
    // Posicion del cursor DENTRO de la linea. Sin esto solo se puede escribir
    // al final y borrar desde el final: equivocarte en la tercera letra de una
    // ruta larga obliga a borrarlo todo hasta ahi.
    let mut cur = 0usize;
    // Portapapeles. Ctrl+C copia la linea entera, Ctrl+V la pega donde este el
    // cursor. Ctrl+ARRIBA / Ctrl+ABAJO hacen lo mismo con las flechas.
    let mut clipboard = [0u8; PATH_MAX];
    let mut clipboard_n = 0usize;
    let mut calc = Calc::new();
    let calc_pad = CalcPad::new(&run_box);
    // Teclas que se mete el propio escritorio, no el teclado. Hoy solo las pone
    // el lanzador al pulsar un icono; se drenan al principio del fotograma
    // siguiente. Ver el bucle de teclas.
    let mut injected = [0u8; 32];
    let mut ni = 0usize;

    // Flanco del boton del raton: un clic es una BAJADA, no "el boton esta
    // pulsado". Sin esto, mantener pulsado teclearia cien veces por segundo.
    let mut button_before = false;
    // Mientras el motor no conteste, su salida NO va a la rejilla: es el
    // resultado, no un mensaje. Se acumula aparte.
    let mut resp = [0u8; 24];
    let mut resp_n = 0usize;
    if child_console.is_none() {
        output.text(b"sin consola: la salida de los programas ira al panel del kernel\n");
    }
    paint_field(&p, &run_box, &path[..n], cur, true);
    paint_output(&p, &run_box, &output);
    if input.is_some() {
        paint_status(&p, &run_box, "listo", INK_DIM);
    } else {
        // Decirlo, y decir por que. Una caja que no responde y no explica nada
        // es peor que no tener caja.
        paint_status(&p, &run_box, "sin teclado: la entrada no se pudo reclamar", INK_BAD);
    }

    bmo::consola("escritorio pintado\n");

    // -- El bucle de vida --
    //
    // No termina: si saliera, `revoke_all` devolveria la pantalla y el kernel
    // repintaria su panel encima. Un escritorio es un proceso que VIVE -- y de
    // paso esto ejerce el cambio de contexto miles de veces por segundo, que es
    // justo el camino que costo una foto de madrugada.
    let (mut ax, mut ay) = (u32::MAX, u32::MAX);
    let mut frames = 0u32;
    let mut caret = true;
    // Vueltas desde la ultima tecla. Se reinicia al escribir para que el
    // cursor este SIEMPRE encendido mientras se teclea.
    let mut since_key: u32 = 0;
    // -- El atajo: un TOQUE de Ctrl+Alt --
    //
    // Se dispara al SOLTAR, y solo si no llego ningun caracter mientras
    // estaban pulsados. No es una floritura: en la distribucion espanola
    // `Ctrl+Alt` **es** `AltGr` --lo que produce `@`, `#`, `[`, `]`, `\`, `|`
    // y `EUR`-- asi que disparar al pulsarlos romperia escribir todos esos
    // caracteres. Con el toque, `Ctrl+Alt` a secas invoca la ventana y
    // `Ctrl+Alt+2` sigue dando `@`.
    let mut combo_before = false;
    let mut key_during_combo = false;
    let mut visible = true;
    // -- La consola de DATOS (F12) --
    //
    // Una tecla de funcion no produce caracter en NINGUNA distribucion, asi que
    // no puede chocar con escribir. Es lo unico que importa en un atajo del
    // sistema, y es lo que `Ctrl+Alt` no puede ofrecer: en espanol ES AltGr.
    let mut data_win = scene::data::DataWindow::new(&p);
    // * ABIERTA no es lo mismo que ARRIBA. Abierta es "existe y esta dibujada";
    // arriba es "es la que tapa a la otra". Se separan porque aqui no hay
    // recorte: las ventanas se pintan enteras una encima de otra, y la ultima
    // que se pinta gana. Sin la distincion, Alt+Tab podria dejar el teclado en
    // Ejecutar con Datos delante -- escribiendo en una linea que no se ve, que
    // es el mismo fallo de antes al reves.
    let mut data_open = false;

    // -- CABINA (F11): lo que el kernel ve, CON severidad --
    //
    // Lo que dice Ring 0, leido desde aqui. **No es "ir a Ring 0"**: este
    // proceso sigue en Ring 3 con sus capabilities contadas y lo unico que hace
    // es preguntar (`TASK_OP_CABINA_*`). Ver `scene::cabina`.
    //
    // Y F11 en vez de un comando por una razon de hoy: **no hace falta teclear
    // nada para abrirla**. Cuando lo que falla es el campo donde se escribe, un
    // diagnostico que exige escribir un comando no sirve de nada.
    let mut cabina_win = scene::cabina::CabinaWindow::new(&p);
    let mut cabina_open = false;

    // -- F7 y F8: las vitales, cada una en SU ventana --
    //
    // No son un comando de la caja de Ejecutar a proposito: `info` es una FOTO
    // que se queda en el historial, y esto es una VISTA que se repinta. Un
    // numero que cambia dentro de un historial empuja hacia arriba lo que
    // estabas leyendo. Ver la cabecera de `scene::vitals`.
    let mut cpu_win = scene::vitals::VitalsWindow::new(&p, scene::vitals::Which::Cpu);
    let mut cpu_open = false;
    let mut mem_win = scene::vitals::VitalsWindow::new(&p, scene::vitals::Which::Memoria);
    let mut mem_open = false;
    // Cuantas lineas hacia atras empieza la ventana. RePag/AvPag la mueven, que
    // es lo que permite llegar al PRINCIPIO del arranque -- donde estan las
    // respuestas de por que algo no arranco.
    
    // Que familia de modulos deja pasar la ventana del kernel. `0` = todas.
    // Vive aqui y no dentro de `klog.rs` por lo mismo que el desplazamiento:
    // es estado de la SESION, y el modulo que pinta no debe recordar nada.
    

    // -- La ventana del SONIDO (F10) --
    //
    // * El aparato se toma AL ABRIR y se devuelve AL CERRAR, y esa es la
    // decision de diseno de toda la ventana. `KIND_AUDIO` es exclusivo: si el
    // escritorio lo reclamara al arrancar --como hace con la pantalla y la
    // entrada-- ningun programa lanzado desde aqui podria volver a sonar, y el
    // sintoma seria `c/musica.bex` diciendo "lo tiene otro proceso" para
    // siempre. Ya paso con la pantalla y costo escribir `PANTALLA_SOLTAR`
    // despues, con el fallo delante. Ver `scene::sound`.
    let mut sound_win = scene::sound::SoundWindow::new(&p);
    let mut sound_open = false;
    // El handle, mientras la ventana esta abierta. `None` cuando esta cerrada o
    // cuando otro proceso tiene el aparato -- que son dos cosas distintas y la
    // ventana las dice distinto.
    let mut sound_cap: Option<bmo::Sonido> = None;
    let mut sound_devices = 0u64;
    // Estado de la SESION, no del modulo que pinta: igual que el desplazamiento
    // y el filtro del klog. El volumen sobrevive a cerrar y abrir la ventana.
    let mut sound_volume = 80u8;
    let mut sound_pressed: Option<usize> = None;

    /// Que tecla de la calculadora tiene el puntero encima, si alguna.
    ///
    /// Se lleva como estado porque el realce solo se repinta **cuando cambia**:
    /// repintar la calculadora entera en cada fotograma que el raton se mueva
    /// un pixel serian veinte rectangulos y veinte glifos por vuelta para
    /// ensenar exactamente lo mismo.
    let mut calc_hover: Option<u8> = None;

    // -- El FOCO --
    //
    // Quien recibe las teclas cuando hay mas de una ventana. La politica vive
    // en `bmo_input::focus` y se prueba ALLI (12 tests); aqui solo se le
    // pregunta y se pinta lo que decidio.
    //
    // Hacia falta ya: hasta ahora F12 se atendia arriba del todo y **todo lo
    // demas caia en Ejecutar** aunque Datos estuviera abierta. Con una tercera
    // ventana, chocan.
    const W_RUN: u8 = 0;
    const W_DATA: u8 = 1;
    const W_CABINA: u8 = 2;
    /// F7 -- el CPU. Ver `scene::vitals`.
    const W_CPU: u8 = 3;
    /// F8 -- la memoria, con QUIEN se la esta comiendo.
    const W_MEM: u8 = 4;
    const W_SOUND: u8 = 3;
    let mut focus = bmo_input::Foco::nuevo();
    focus.open(W_RUN);
    let mut alt_before = false;
    let mut switcher_painted = false;
    // Quien tapaba a quien en la vuelta anterior, para pintar solo cuando
    // cambia. `data_open && focus.es_para(W_DATA)` es la cuenta entera:
    // **la que tiene el teclado es la que se ve**.
    let mut top_before = W_RUN;

    // Lo que hay DEBAJO del cursor del raton. Ver `scene::cursor::SaveUnder`: se
    // quita al principio del fotograma y se pone al final, y en medio se pinta.
    let mut save_under = SaveUnder::new();
    // Estado anterior de los botones EN PANTALLA, para no repintar el testigo
    // del pulsometro sesenta veces por segundo con el mismo color.

    // -- * EL VIGILANTE DE LA CORRIDA --
    //
    // Cuando se lanza un programa se apunta aqui donde empieza su salida; en
    // cuanto muere, lo que escribio se vuelca solo a `data/output.txt`.
    //
    // === Por que hace falta el `visto` ===
    //
    // `ejecutar_en` vuelve en cuanto el hijo arranca, y **`has_child()` puede
    // contestar `false` en el fotograma siguiente sin que el programa haya
    // terminado**: todavia no se ha puesto a escribir en la consola. Sin la
    // bandera, cada lanzamiento volcaria un archivo vacio en el acto y luego
    // no volcaria el de verdad.
    //
    // Con ella, el volcado solo ocurre en el flanco `alive_one -> muerto`, que es lo
    // unico que significa "termino".
    // `Run` y su vigilante viven en `watch.rs`: es el unico bloque de
    // esta funcion que toca solo TRES variables del estado, asi que es el unico
    // que se puede sacar sin arrastrar media firma. Ver la cabecera del modulo.
    let mut run: Option<Run> = None;

    // Cuantos fallos de Ring 3 se habian visto. Empieza en el total actual y no
    // en cero: los de antes de arrancar el escritorio ya se guardaron.
    let mut faults_seen = bmo::autopsia_total();

    // -- ** LAS APPS EN SU CAJA --
    //
    // Una app pide memoria, dibuja ahi y **se la ofrece** al que la lanzo. Desde
    // aqui se toma una vez y se pega dentro de un marco cada vez que su
    // secuencia sube. La pantalla no cambia de dueno ni una vez -- que es todo
    // lo que separa esto de `lend_screen`, el camino de al lado, que le
    // entrega el aparato al hijo y deja el escritorio sin existir mientras dure.
    //
    // Ver `scene::surface`. Nace vacia: no hay ventana hasta que una app
    // ofrezca, y las que no ofrezcan siguen yendo por el camino de siempre.
    let mut table = scene::surface::Table::new();
    // Los rectangulos que dejan las ventanas cuya app murio, para devolverselos
    // al escritorio. Se declara fuera del bucle porque es un buzon, no un
    // estado: se llena y se vacia dentro de la misma vuelta.
    let mut dead_boxes = [(0u32, 0u32, 0u32, 0u32); scene::surface::MAX];

    loop {
        // -- Termino el programa que se lanzo? Entonces, a guardarlo --
        //
        // 71 lineas que estaban aqui dentro. Se fueron ENTERAS a
        // `watch.rs`, sin tocar una coma de su logica.
        watch_run(&mut run, &child_console, &mut output);

        // ** MURIO ALGO? Entonces la autopsia ya esta escrita, y se guarda.
        //
        // Comparar un entero por vuelta cuesta nada; leer el informe solo pasa
        // cuando de verdad hubo un fallo. Y avisar en la barra importa tanto
        // como guardarlo: un fichero que nadie sabe que existe es un fichero
        // que no se manda.
        if save_autopsies(&mut faults_seen) {
            paint_status(
                &p,
                &run_box,
                "fallo de Ring 3 guardado en datos/fallos.txt -- escribe `fallo`",
                INK_BAD,
            );
        }

        frames = frames.wrapping_add(1);
        let mut repaint_field = false;

        // -- * ALGUIEN OFRECE UNA SUPERFICIE? --
        //
        // Se pregunta al kernel una vez por vuelta y casi siempre dice que no.
        // Es el precio de no tener que avisar: una app ofrece cuando le viene
        // bien --puede ser en su primer fotograma o en el mil-- y el DIRECTOR se
        // entera **mirando**, no porque nadie le mande un mensaje. Una operacion
        // que ya existia y ninguna cola nueva.
        let mut born = false;
        if table.collect(&p) {
            born = true;
        }
        // Y las que se quedaron sin dueno. Va ANTES de pintar nada: la ventana
        // de una app muerta tiene que desaparecer en el mismo fotograma en que
        // se sabe, no en el siguiente.
        let dead = table.reap_dead(&mut dead_boxes);

        // -- Va a pintar algo este fotograma? --
        //
        // Hay que saberlo ANTES de pintar, porque el cursor del raton se quita
        // al principio y se pone al final: hacerlo en todos los fotogramas
        // dejaria el puntero ausente la mitad del tiempo y en pantalla se veria
        // palido y parpadeante. Y como leer una tecla la CONSUME, "hay
        // teclas?" obliga a tenerlas ya en la mano.
        //
        // Lo que se lee aqui no se interpreta aqui: esto solo recoge.
        // ** Y las superficies cuentan aqui, no donde se pintan. Si un fotograma
        // en el que solo cambio una app no se contara como "va a pintar", el
        // cursor del raton no se quitaria antes de componer -- la app dibujaria
        // encima y el puntero desapareceria bajo su ventana.
        let mut will_paint = output.dirty
            || since_key + 1 >= BLINK
            || born
            || dead > 0
            || table.has_new();

        if let Some(e) = input.as_ref() {
            // -- El atajo, ANTES de leer teclas --
            let m = e.modificadores();
            let ctrl = m & bmo::MOD_CTRL != 0;
            let combo = ctrl && m & bmo::MOD_ALT != 0;
            // * Alt SOLO, sin Ctrl. La distincion no es cosmetica: `Ctrl+Alt`
            // **es AltGr** en espanol, y ya tiene dueno (invocar la ventana).
            // El driver ademas da el Alt DERECHO como `SC_ALTGR` con codigo
            // propio, asi que `MOD_ALT` es el izquierdo -- el de Alt+Tab de toda
            // la vida.
            let alt_alone = m & bmo::MOD_ALT != 0 && !ctrl;

            // El tope no descarta: lo que no quepa se queda en el anillo del
            // kernel y llega en el fotograma siguiente. Drenar sin tope y tirar
            // el sobrante seria perder letras justo cuando se escribe rapido.
            // ** EL INVARIANTE DEL CAMPO: el cursor NUNCA pasa del texto.
            //
            // `cur <= n` lo dan por hecho las tres teclas que borran, y las tres
            // restan de `n`. Romperlo una vez --un camino que pone `n = 0` y se
            // olvida de `cur`-- deja una mina que no explota hasta que alguien
            // pulsa retroceso, y entonces `n` se desborda por abajo y el
            // escritorio entero se cae con un `usize::MAX`. Paso en el Ryzen el
            // 2026-08-09, y el camino que lo rompio era de ese mismo dia.
            //
            // Se restaura AQUI, una vez por vuelta y en un solo sitio, en vez de
            // ir persiguiendo cada `n = 0` del fichero. Cuesta una comparacion
            // por fotograma y **quita la clase entera de fallo**: cualquier
            // camino futuro que se olvide de `cur` queda corregido antes de que
            // nadie pueda teclear.
            cur = cur.min(n);

            let mut keys = [0u8; 64];
            let mut nt = 0usize;
            // ** LO QUE INYECTA EL LANZADOR va DELANTE de lo que llega del
            // teclado, y por eso entra aqui y no en otro sitio.
            //
            // Pulsar un icono es exactamente **teclear su ruta y dar Enter**, y
            // eso es lo que hace: el clic rellena el campo y mete un `\n` por
            // esta puerta. Asi el camino de lanzar sigue siendo UNO -- con su
            // consola, con la pantalla prestada, con el eco en la salida y con
            // el vigilante que recoge lo que el hijo imprima.
            //
            // La alternativa era llamar a `lend_screen` desde el clic, y
            // eso habria sido un segundo camino de lanzar programas con las
            // mismas cinco cosas que recordar. El dia que uno de los dos se
            // arregle, el otro se queda roto y nadie se entera.
            for k in 0..ni.min(keys.len()) {
                keys[nt] = injected[k];
                nt += 1;
            }
            ni = 0;
            while nt < keys.len() {
                match e.tecla() {
                    Some(c) => {
                        keys[nt] = c;
                        nt += 1;
                    }
                    None => break,
                }
            }
            let pos = e.puntero();
            let wheel = e.rueda();

            will_paint |= nt > 0
                || wheel != 0
                || pos.x != ax
                || pos.y != ay
                || (pos.botones != 0) != button_before
                || alt_alone != alt_before
                || combo != combo_before;

            // A partir de aqui se PINTA, asi que el cursor se aparta.
            if will_paint {
                save_under.lift(&p);
            }

            // -- Alt+Tab: el conmutador --
            //
            // La pila se reordena al SOLTAR, no en cada Tab: eso es lo que hace
            // que pulsarlo dos veces te devuelva a donde estabas. Ver
            // `bmo_input::focus`.
            // ** La guarda es `switcher_painted`, NO `focus.conmutando()`.
            //
            // Eran dos estados distintos gobernando la misma cosa: uno dice
            // *que hay dibujado en la pantalla* y el otro *que cree la politica
            // de foco*. Mientras coincidan, bien; el dia que no --y en el Ryzen
            // no coincidieron-- el conmutador se queda pintado para siempre,
            // porque el unico que sabia borrarlo estaba esperando permiso del
            // que no lo pinto.
            //
            // Lo que hay que borrar lo decide quien lo pinto. `soltar_conmutador`
            // se llama igual: pedirle a la politica que se suelte no puede
            // depender de que ella misma diga que estaba conmutando.
            if !alt_alone && alt_before && switcher_painted {
                focus.soltar_conmutador();
                let (bx, by, ba, bh) = scene::switcher::area(&p, focus.abiertas());
                for fy in 0..bh {
                    for fx in 0..ba {
                        let (x, y) = (bx + fx, by + fy);
                        p.punto(x, y, scene_color(&run_box, visible, x, y, p.alto));
                    }
                }
                switcher_painted = false;
                // Lo que tapaba vuelve a pintarse entero, **de abajo arriba**:
                // es el unico orden que deja la pantalla como estaba. Y quien
                // va arriba lo acaba de decidir el Alt que se solto.
                //
                // * Con tres ventanas esto se escribe como lo que es: pintar
                // TODAS las abiertas, y la que tiene el foco la ULTIMA. La
                // version de dos ventanas enumeraba los casos a mano, y con
                // tres eso son seis ramas que dicen una sola regla.
                let top_now = if mem_open && focus.es_para(W_MEM) {
                    W_MEM
                } else if cpu_open && focus.es_para(W_CPU) {
                    W_CPU
                } else if sound_open && focus.es_para(W_SOUND) {
                    W_SOUND
                } else if cabina_open && focus.es_para(W_CABINA) {
                    W_CABINA
                } else if data_open && focus.es_para(W_DATA) {
                    W_DATA
                } else {
                    W_RUN
                };
                let mut paint_one = |v: u8, repintar: &mut bool, sal: &mut scene::output::Output| {
                    match v {
                        W_CABINA if cabina_open => {
                            scene::cabina::paint(&p, &cabina_win)
                        }
                        W_DATA if data_open => scene::data::paint(&p, &data_win),
                        // Las vitales son VISTAS: se repintan cada vez que les
                        // toca turno, que es lo que las diferencia de `info`.
                        W_CPU if cpu_open => scene::vitals::paint(&p, &cpu_win),
                        W_MEM if mem_open => scene::vitals::paint(&p, &mem_win),
                        W_SOUND if sound_open => scene::sound::paint(
                            &p,
                            &sound_win,
                            sound_cap.is_some(),
                            sound_devices,
                            sound_volume,
                            sound_pressed,
                        ),
                        W_RUN => uncover(&p, &run_box, visible, sal, repintar),
                        _ => {}
                    }
                };
                for v in [W_RUN, W_DATA, W_CABINA, W_SOUND] {
                    if v != top_now {
                        paint_one(v, &mut repaint_field, &mut output);
                    }
                }
                paint_one(top_now, &mut repaint_field, &mut output);
                top_before = top_now;
            }
            alt_before = alt_alone;
            if combo && !combo_before {
                key_during_combo = false;
            }
            if !combo && combo_before && !key_during_combo {
                visible = !visible;
                if visible {
                    // Esconderla y volver a invocarla es cerrarla y abrirla
                    // para el foco. Sin esto, Alt+Tab llevaria el teclado a una
                    // ventana que no esta en la pantalla: escribirias en algo
                    // invisible, que es la peor forma de perder una linea.
                    focus.open(W_RUN);
                    uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                    paint_status(&p, &run_box, "listo", INK_DIM);
                } else {
                    focus.close(W_RUN);
                    erase_box(&p, &run_box);
                }
            }
            combo_before = combo;

            // -- Teclado --
            //
            // Se atienden TODAS las de la vuelta, no una por fotograma:
            // escribiendo rapido llegan varias entre vuelta y vuelta, y
            // quedarse con una seria perder letras de forma que pareceria un
            // teclado malo. Ya estan recogidas arriba.
            for &c in &keys[..nt] {
                // Tab con Alto pulsado NO llega a ninguna ventana: es del
                // conmutador. Shift lo recorre al reves.
                if alt_alone && c == 0x09 {
                    if m & bmo::MOD_SHIFT != 0 {
                        focus.conmutar_atras();
                    } else {
                        focus.conmutar();
                    }
                    scene::switcher::paint(
                        &p,
                        focus.lista(),
                        focus.pointed_index(),
                        focus.modo().name(),
                    );
                    switcher_painted = true;
                    continue;
                }
                // -- Alt+M: cambiar el MODO del foco --
                //
                // Sin una tecla, los tres modos son decoracion: `Fijo` y
                // `Puntero` existirian sin forma de llegar a ellos. Va con Alt
                // por lo mismo que el Tab --`Alt` solo no produce caracter en
                // ninguna distribucion, `Ctrl+Alt` SI (es AltGr)-- y se anuncia
                // en la propia ventanita, que es donde se lee el modo.
                if alt_alone && (c == b'm' || c == b'M') {
                    focus.poner_modo(focus.modo().next());
                    if switcher_painted {
                        scene::switcher::paint(
                            &p,
                            focus.lista(),
                            focus.pointed_index(),
                            focus.modo().name(),
                        );
                    } else if visible {
                        // Cambiarlo sin el conmutador abierto tambien tiene que
                        // verse: un modo que cambia en silencio se descubre
                        // cuando el teclado ya se fue a otra ventana.
                        paint_status(&p, &run_box, focus.modo().nombre_largo(), ACCENT);
                    }
                    continue;
                }
                // -- ** ALT+FLECHAS: MOVER Y ENCAJAR SIN SOLTAR EL TECLADO --
                //
                // Alt+Tab ya elegia ventana y no podia hacer nada con ella. Esto
                // cierra el gesto: se elige con Tab y se coloca con las flechas,
                // sin que la mano salga del teclado.
                //
                // * **A secas mueve; con Shift encaja** -- media pantalla a los
                // lados, el panel entero arriba, y abajo deshace el maximizado.
                // Es lo que hace Windows con la tecla de la ventanita, y se
                // copia el reparto a proposito: un atajo de colocar ventanas que
                // no es el que ya tienes en los dedos se usa una vez.
                //
                // Va con `Alt` por lo mismo que el Tab y la M, y esta escrito
                // dos lineas mas arriba: `Alt` solo no produce caracter en
                // ninguna distribucion y `Ctrl+Alt` SI, porque es AltGr.
                //
                // [!] Se atiende ANTES que las flechas de las ventanas, y por eso
                // no les quita nada: sin `Alt` esto no entra, y las flechas de
                // Datos y el volumen de Sonido siguen llegando enteras.
                if alt_alone && (0x80..=0x83).contains(&c) {
                    use scene::chrome::Heading;
                    let heading = match c {
                        0x80 => Heading::Up,
                        0x81 => Heading::Down,
                        0x82 => Heading::Left,
                        _ => Heading::Right,
                    };
                    let fit = m & bmo::MOD_SHIFT != 0;
                    let mut moved = false;
                    // -- ** SE MUEVE LA SENALADA, NO LA QUE TIENE EL FOCO --
                    //
                    // `focus.actual()` parece lo obvio y es justo lo que no vale:
                    // **no cambia mientras conmutas**, a proposito --lo dice su
                    // propia documentacion-- porque una letra escrita a mitad de
                    // un Alt+Tab no puede caer en una ventana que todavia no has
                    // elegido.
                    //
                    // Pero estas flechas se pulsan CON EL ALT PULSADO, que es
                    // exactamente "a mitad de un Alt+Tab". Con `actual()`, elegir
                    // CABINA con Tab y darle a la flecha moveria la ventana
                    // ANTERIOR -- se veria moverse la que no es, que es peor que
                    // no moverse nada.
                    //
                    // `pointed_at()` contesta las dos situaciones con una regla:
                    // conmutando es la resaltada, y sin conmutar es la que ya
                    // tiene el foco. La que se mueve es **la que estas mirando en
                    // la ventanita**, y eso se puede explicar en una frase.
                    match focus.pointed_at() {
                        Some(W_DATA) if data_open && !data_win.chrome.minimized => {
                            let (vx, vy, va, vl) = (
                                data_win.x(), data_win.y(),
                                data_win.width(), data_win.height(),
                            );
                            let cambio = if fit {
                                data_win.chrome.snap(&p, heading)
                            } else {
                                data_win.chrome.push(&p, heading)
                            };
                            if cambio {
                                erase_window(&p, &run_box, vx, vy, va, vl, visible);
                                uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                                // Encajar CAMBIA el tamano, asi que las cajas del
                                // grafo hay que recolocarlas: sin esto la ventana
                                // mide una cosa y su contenido sigue midiendo otra.
                                data_win.relayout();
                                scene::data::paint(&p, &data_win);
                                top_before = W_DATA;
                                moved = true;
                            }
                        }
                        Some(W_CABINA) if cabina_open && !cabina_win.chrome.minimized => {
                            let (vx, vy, va, vl) = (
                                cabina_win.chrome.x, cabina_win.chrome.y,
                                cabina_win.chrome.width, cabina_win.chrome.height,
                            );
                            let cambio = if fit {
                                cabina_win.chrome.snap(&p, heading)
                            } else {
                                cabina_win.chrome.push(&p, heading)
                            };
                            if cambio {
                                erase_window(&p, &run_box, vx, vy, va, vl, visible);
                                uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                                scene::cabina::paint(&p, &cabina_win);
                                top_before = W_CABINA;
                                moved = true;
                            }
                        }
                        Some(W_SOUND) if sound_open && !sound_win.chrome.minimized => {
                            let (vx, vy, va, vl) = (
                                sound_win.chrome.x, sound_win.chrome.y,
                                sound_win.chrome.width, sound_win.chrome.height,
                            );
                            let cambio = if fit {
                                sound_win.chrome.snap(&p, heading)
                            } else {
                                sound_win.chrome.push(&p, heading)
                            };
                            if cambio {
                                erase_window(&p, &run_box, vx, vy, va, vl, visible);
                                uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                                scene::sound::paint(
                                    &p, &sound_win, sound_cap.is_some(),
                                    sound_devices, sound_volume, sound_pressed,
                                );
                                top_before = W_SOUND;
                                moved = true;
                            }
                        }
                        // Ejecutar no se mueve --es el escritorio, no una
                        // ventana-- y sin foco no hay a quien mover. En los dos
                        // casos la tecla se come igual: dejarla pasar mandaria un
                        // Alt+flecha a la linea de comandos.
                        _ => {}
                    }
                    // La ventana se acaba de pintar ENCIMA del conmutador, que
                    // esta en el centro. Sin esto, mover tapa la ventanita que
                    // dice cual estas moviendo -- y a la segunda flecha ya no
                    // sabes en cual estas. Al soltar Alt se repinta todo de abajo
                    // arriba, asi que el destrozo se repara solo; lo que hay que
                    // arreglar es lo que se ve MIENTRAS.
                    if moved && switcher_painted {
                        scene::switcher::paint(
                            &p,
                            focus.lista(),
                            focus.pointed_index(),
                            focus.modo().name(),
                        );
                    }
                    continue;
                }
                // Cualquier tecla durante el combo lo convierte en AltGr y
                // cancela el toque: el usuario estaba escribiendo, no llamando.
                if combo {
                    key_during_combo = true;
                }

                // -- F12 es del SISTEMA, no de una ventana --
                //
                // Se atiende ANTES de preguntar por el foco, y tiene que ser
                // asi: un atajo que solo funciona si ya estas en la ventana que
                // abre no sirve para abrirla -- y peor, no sirve para cerrarla,
                // porque para entonces el foco ya es suyo.
                //
                // ESC cierra la de arriba, que es lo que hace ESC en todas
                // partes. En Ejecutar ESC sigue borrando la linea: son dos
                // ventanas distintas y cada una contesta lo suyo.
                let toggle_data = if c == 0x94 {
                    Some(!data_open)
                } else if c == 0x1B && data_open && focus.es_para(W_DATA) {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_data {
                    data_open = open;
                    if open {
                        // Abrir es decirselo al foco y ya: en modo `Fijo` la
                        // ventana aparece y NO se lleva el teclado, y quien
                        // decide eso es la politica, no esta tecla.
                        focus.open(W_DATA);
                        scene::data::paint(&p, &data_win);
                        top_before = if focus.es_para(W_DATA) { W_DATA } else { W_RUN };
                        // En `Fijo` se ha pintado encima de una caja que sigue
                        // teniendo el teclado: hay que devolverla arriba.
                        if top_before == W_RUN {
                            uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        }
                    } else {
                        // Al cerrarla hay que devolver el fondo Y repintar
                        // lo que tapaba: la caja de Ejecutar esta debajo.
                        focus.close(W_DATA);
                        erase_window(
                            &p, &run_box, data_win.x(), data_win.y(),
                            data_win.width(), data_win.height(), visible,
                        );
                        top_before = W_RUN;
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                    }
                    continue;
                }

                // -- F7 y F8: las vitales --
                //
                // Calcadas de F11 y por los mismos motivos: se atienden ANTES
                // de preguntar por el foco, porque un atajo que solo funciona
                // si ya estas dentro de la ventana no sirve para abrirla.
                //
                // ESC cierra la que este abierta. Si las dos lo estan, cierra
                // primero la de memoria -- que es la que se abre encima.
                let toggle_cpu = if c == 0x8F {
                    Some(!cpu_open)
                } else if c == 0x1B && cpu_open && !mem_open {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_cpu {
                    cpu_open = open;
                    if open {
                        focus.open(W_CPU);
                        scene::vitals::paint(&p, &cpu_win);
                    } else {
                        focus.close(W_CPU);
                        erase_window(
                            &p, &run_box, cpu_win.chrome.x, cpu_win.chrome.y,
                            cpu_win.chrome.width, cpu_win.chrome.height, visible,
                        );
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                    }
                    continue;
                }
                let toggle_mem = if c == 0x90 {
                    Some(!mem_open)
                } else if c == 0x1B && mem_open {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_mem {
                    mem_open = open;
                    if open {
                        focus.open(W_MEM);
                        scene::vitals::paint(&p, &mem_win);
                    } else {
                        focus.close(W_MEM);
                        erase_window(
                            &p, &run_box, mem_win.chrome.x, mem_win.chrome.y,
                            mem_win.chrome.width, mem_win.chrome.height, visible,
                        );
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                    }
                    continue;
                }

                // -- F11: la consola del KERNEL --
                //
                // Calcada de F12 y por los mismos motivos: se atiende ANTES de
                // preguntar por el foco, porque un atajo que solo funciona si ya
                // estas dentro de la ventana no sirve para abrirla.
                let toggle_klog = if c == 0x93 {
                    Some(!cabina_open)
                } else if c == 0x1B && cabina_open {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_klog {
                    cabina_open = open;
                    if open {
                        // Se abre SIEMPRE por lo ultimo, que es lo que se quiere
                        // ver el 90% de las veces. Para ir al arranque estan
                        // RePag/AvPag.
                        cabina_win.from = 0;
                        focus.open(W_CABINA);
                        scene::cabina::paint(&p, &cabina_win);
                        top_before = if focus.es_para(W_CABINA) { W_CABINA } else { W_RUN };
                        if top_before == W_RUN {
                            uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        }
                    } else {
                        focus.close(W_CABINA);
                        erase_window(
                            &p, &run_box, cabina_win.chrome.x, cabina_win.chrome.y,
                            cabina_win.chrome.width, cabina_win.chrome.height, visible,
                        );
                        top_before = W_RUN;
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        // Si Datos estaba abierta debajo, vuelve a verse.
                        if data_open {
                            scene::data::paint(&p, &data_win);
                        }
                    }
                    continue;
                }

                // -- F10: la ventana del SONIDO --
                //
                // Calcada de F11, y con una diferencia que no es cosmetica:
                // aqui abrir y cerrar **toman y devuelven un aparato**, no solo
                // pintan. Por eso el orden importa en los dos sentidos --
                // reclamar antes de pintar (para que la ventana ensene lo que
                // de verdad hay) y CALLAR antes de soltar (un tono que sigue
                // sonando despues de devolver el aparato es del sistema, y el
                // sistema no pidio ese tono).
                let toggle_sound = if c == 0x92 {
                    Some(!sound_open)
                } else if c == 0x1B && sound_open && focus.es_para(W_SOUND) {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_sound {
                    sound_open = open;
                    if open {
                        // Puede fallar, y entonces la ventana lo DICE en vez de
                        // pintar un volumen que no manda sobre nada.
                        sound_cap = bmo::Sonido::claim();
                        sound_devices = match &sound_cap {
                            Some(s) => {
                                s.volumen(sound_volume);
                                s.aparatos()
                            }
                            None => 0,
                        };
                        sound_pressed = None;
                        focus.open(W_SOUND);
                        scene::sound::paint(
                            &p, &sound_win, sound_cap.is_some(),
                            sound_devices, sound_volume, sound_pressed,
                        );
                        top_before = if focus.es_para(W_SOUND) { W_SOUND } else { W_RUN };
                        if top_before == W_RUN {
                            uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        }
                    } else {
                        // * DEVOLVER EL APARATO. Esto es lo que impide que el
                        // escritorio deje mudos a todos los programas que lanza.
                        if let Some(s) = sound_cap.take() {
                            s.callar();
                            s.release();
                        }
                        focus.close(W_SOUND);
                        erase_window(
                            &p, &run_box, sound_win.chrome.x, sound_win.chrome.y,
                            sound_win.chrome.width, sound_win.chrome.height, visible,
                        );
                        top_before = W_RUN;
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        // Si habia ventanas debajo, vuelven a verse.
                        if data_open {
                            scene::data::paint(&p, &data_win);
                        }
                        if cabina_open {
                            scene::cabina::paint(&p, &cabina_win);
                        }
                    }
                    continue;
                }

                // Las teclas de la ventana del sonido. **Solo con el foco
                // AQUI**: con el foco en Ejecutar, una `z` es una letra que el
                // dueno esta escribiendo, y robarsela para un atajo seria el
                // peor intercambio posible. Es la misma regla que la `f` del
                // klog.
                if sound_open && focus.es_para(W_SOUND) {
                    if let Some(s) = &sound_cap {
                        // Flechas: el volumen, de diez en diez.
                        //
                        // * `KEY_LEFT` es 0x82 y `KEY_RIGHT` 0x83 -- ver
                        // `ring0/dev/keyboard.rs`. Esto se escribio con 0x83 y
                        // 0x84, y **0x84 es INICIO**: la flecha izquierda no
                        // habria bajado el volumen y la tecla Inicio lo habria
                        // subido. No da error, da un control que obedece a la
                        // tecla equivocada.
                        if c == 0x82 || c == 0x83 {
                            sound_volume = if c == 0x83 {
                                (sound_volume + 10).min(100)
                            } else {
                                sound_volume.saturating_sub(10)
                            };
                            s.volumen(sound_volume);
                            scene::sound::paint(
                                &p, &sound_win, true, sound_devices,
                                sound_volume, sound_pressed,
                            );
                            continue;
                        }
                        // Z..M: una octava. Se pinta la tecla ANTES de pitar
                        // porque `pitar` bloquea el nucleo mientras suena: al
                        // reves, la tecla se veria encendida cuando ya callo.
                        let min = c.to_ascii_lowercase();
                        if let Some(i) = scene::sound::NOTES.iter().position(|n| n.0 == min) {
                            sound_pressed = Some(i);
                            scene::sound::paint(
                                &p, &sound_win, true, sound_devices,
                                sound_volume, sound_pressed,
                            );
                            s.pitar(scene::sound::NOTES[i].1, 160);
                            sound_pressed = None;
                            scene::sound::paint(
                                &p, &sound_win, true, sound_devices,
                                sound_volume, sound_pressed,
                            );
                            continue;
                        }
                        // P: la frase. La misma que toca `c/musica.bex`, para
                        // que la ventana y el programa suenen igual -- si no,
                        // no se sabria cual de los dos esta mal.
                        if min == b'p' {
                            for (hz, ms) in [
                                (440u32, 170u32), (523, 170), (659, 240),
                                (587, 170), (523, 170), (659, 300),
                            ] {
                                s.pitar(hz, ms);
                                s.pitar(0, 30);
                            }
                            continue;
                        }
                    }
                }

                // RePag/AvPag dentro de la consola del kernel: recorrer el log.
                //
                // ** MIENTRAS CABINA ESTA ABIERTA, ESTAS TRES TECLAS SON SUYAS
                // -- RePag, AvPag y `G`-- y no se le piden al foco.
                //
                // Antes se exigia `focus.es_para(W_CABINA)`, y el 2026-08-09 eso
                // dio una ventana que **prometia en su pie algo que no hacia**:
                // el dueno abrio CABINA con F11, la vio ocupando la pantalla, y
                // RePag no movio nada. No era un fallo del scroll: era la
                // politica funcionando. **Abrir no es enfocar** --y no debe
                // serlo, porque robar el teclado a quien esta escribiendo es
                // mucho peor-- pero el compositor la PINTA encima igualmente,
                // asi que lo que se ve y lo que manda dejaban de coincidir.
                //
                // La regla que queda: **las teclas de ESCRITURA son del foco;
                // las de NAVEGACION, de la ventana que estas mirando.** Una
                // letra sigue cayendo en Ejecutar; un RePag mueve lo que se ve.
                // Se paga que no se pueda recorrer el historial de Ejecutar con
                // CABINA delante -- y eso no se pierde, porque debajo de CABINA
                // no se ve.
                // -- F: cambiar el filtro de la ventana del kernel --
                //
                // Solo con el foco AQUI: con el foco en Ejecutar, una `f` es una
                // letra que el dueno esta escribiendo, y robarsela para un atajo
                // seria el peor intercambio posible.
                //
                // Se reinicia el desplazamiento al cambiar: lo que se estaba
                // mirando en la lista vieja no senala nada en la nueva, y dejar
                // el numero puesto haria que la ventana pareciera vacia.
                // G: subir el listero de GRAVEDAD. Cinco escalones y vuelta.
                //
                // Es `G` y no `F` porque ya no filtra por FAMILIA de modulo
                // --eso lo hacia el klog, adivinando por el prefijo de la
                // linea-- sino por la severidad que CABINA lleva de verdad.
                if cabina_open && (c == b'g' || c == b'G') {
                    cabina_win.minima = (cabina_win.minima + 1) % 5;
                    cabina_win.from = 0;
                    scene::cabina::paint(&p, &cabina_win);
                    continue;
                }
                // ** A: SOLO LO QUE HIZO LA ULTIMA ACCION.
                //
                // Lo pidio el dueno asi: *"que lea en tiempo real que hace el
                // puntero, y al escribir doom.bex y ejecutar, que lo filtre --
                // para no quedarse en que falla sino poder verificar todo"*.
                //
                // `G` contesta *"que fue grave"* y mezcla lo de esta accion con
                // lo de las diez anteriores. `A` contesta la pregunta que uno se
                // hace de verdad delante de la pantalla: **todo lo que produjo
                // esa pulsacion**, lo bueno y lo malo, en orden y sin nada de
                // antes. El kernel ya lo agrupaba; faltaba leerlo.
                if cabina_open && (c == b'a' || c == b'A') {
                    cabina_win.last_only = !cabina_win.last_only;
                    cabina_win.from = 0;
                    scene::cabina::paint(&p, &cabina_win);
                    continue;
                }
                if cabina_open && (c == 0x87 || c == 0x88) {
                    let any = bmo::cabina_disponibles();
                    if c == 0x87 {
                        // Hacia atras en el tiempo, sin pasarse del principio.
                        cabina_win.from = (cabina_win.from + 6).min(any.saturating_sub(1));
                    } else {
                        cabina_win.from = cabina_win.from.saturating_sub(6);
                    }
                    scene::cabina::paint(&p, &cabina_win);
                    continue;
                }

                // -- * La consola de DATOS: cambiar de vista y recorrer el arbol --
                //
                // Va aqui, junto al bloque del klog y por el mismo motivo: son
                // teclas DE ESTA VENTANA. Con Datos delante, las flechas no
                // tienen nada que ver con el historial de comandos de Ejecutar,
                // y hasta hoy iban alli -- se navegaba una ventana tapada.
                if data_open && focus.es_para(W_DATA) {
                    use scene::data::{Seal, View};
                    let mut served = true;
                    match c {
                        // TAB: numeros <-> nodos. Es la misma tecla que cambia de
                        // pestana en todas partes.
                        b'\t' => {
                            data_win.view = match data_win.view {
                                View::Numbers => {
                                    // Al entrar en el arbol se empieza por la
                                    // raiz. Conservar el sitio de la ultima vez
                                    // ensenaria un directorio que ya no se sabe
                                    // cual es.
                                    bmo::estratos::a_la_raiz();
                                    data_win.to_top();
                                    View::Nodes
                                }
                                // ** DE NODOS A CARPETAS **SIN TOCAR EL CURSOR**.
                                //
                                // Y eso es lo que hace que se sientan una sola
                                // cosa: estas en `/cobol/10` mirando el grafo,
                                // pulsas TAB y estas en `/cobol/10` mirando la
                                // lista. Volver a la raiz aqui --como se hace al
                                // ENTRAR desde numeros-- convertiria las dos
                                // pestanas en dos programas.
                                View::Nodes => View::Folders,
                                View::Folders => View::Numbers,
                            };
                            data_win.seal = Seal::Idle;
                        }
                        _ if data_win.view == View::Numbers => served = false,
                        // ARRIBA / ABAJO por la lista de hijos.
                        // Al cambiar de caja se borra la verificacion: es de
                        // UN archivo, y un `CUADRA` viejo bajo el nombre de
                        // otro es peor que no decir nada.
                        0x80 => { data_win.move_sel(-1, bmo::estratos::hijos() as usize); data_win.verified = None; }
                        0x81 => { data_win.move_sel(1, bmo::estratos::hijos() as usize); data_win.verified = None; }
                        0x87 => data_win.move_sel(-5, bmo::estratos::hijos() as usize),
                        0x88 => data_win.move_sel(5, bmo::estratos::hijos() as usize),
                        // ENTRAR / DERECHA: bajar al hijo senalado. `entrar`
                        // dice que no si es un archivo, y entonces no pasa nada
                        // -- que es lo correcto: un archivo no tiene dentro.
                        b'\r' | b'\n' | 0x83 => {
                            if bmo::estratos::entrar(data_win.sel as u64) {
                                data_win.to_top();
                                data_win.verified = None;
                            }
                        }
                        // RETROCESO / IZQUIERDA: subir al padre.
                        0x08 | 0x82 => {
                            if bmo::estratos::subir() {
                                data_win.to_top();
                                data_win.verified = None;
                            }
                        }
                        // * V: COMPROBAR LA FIRMA del nodo senalado.
                        //
                        // Se pide a mano y no se calcula al pintar: lee el
                        // archivo entero y le hace el BLAKE3, y hacer eso
                        // sesenta veces por segundo convertiria este panel en
                        // un martillo sobre el disco.
                        b'v' | b'V' => {
                            data_win.verified =
                                Some(bmo::estratos::verificar(data_win.sel as u64));
                            data_win.seal = Seal::Idle;
                        }
                        // * S: SELLAR, en dos tiempos. Ver `data::Seal`.
                        //
                        // Se mudo aqui desde el terminal principal porque el
                        // verbo vive donde vive el objeto: sellar es de
                        // ESTRATOS, y esta es la ventana de ESTRATOS. Y va en
                        // dos tiempos porque una tecla suelta que escribe en el
                        // disco, en una ventana donde se pulsan flechas, seria
                        // peor que las dos palabras que se quitaron.
                        b's' | b'S' => {
                            data_win.seal = match data_win.seal {
                                Seal::Asking => match bmo::estratos_sellar() {
                                    0 => Seal::Failed,
                                    g => Seal::Done(g),
                                },
                                _ => Seal::Asking,
                            };
                        }
                        _ => {
                            // Cualquier otra tecla CANCELA la pregunta. Es la
                            // salida que hace que preguntar sea barato: si te
                            // arrepientes, sigue navegando y ya esta.
                            data_win.seal = Seal::Idle;
                            served = false;
                        }
                    }
                    if served {
                        scene::data::paint(&p, &data_win);
                        continue;
                    }
                }

                // -- * DE QUIEN es esta tecla? --
                //
                // La pregunta que faltaba, y la razon de que exista
                // `bmo_input::focus`. Hasta ahora TODA tecla se editaba en la
                // linea de Ejecutar aunque la consola de datos estuviera
                // encima: escribias en una ventana tapada, sin verlo. Con una
                // tercera, chocan.
                //
                // Ninguna abierta --todas escondidas-- tampoco es "Ejecutar por
                // defecto": las teclas se descartan y vuelven al invocarla.
                if !focus.es_para(W_RUN) {
                    continue;
                }
                debug_assert!(visible, "el foco de una ventana escondida es un bug");
                // Cualquier tecla enciende el cursor y reinicia el parpadeo.
                caret = true;
                since_key = 0;
                repaint_field = true;
                match c {
                    b'\r' | b'\n' => {
                        // Eco SIEMPRE, tambien de lo que no se entiende: un
                        // terminal que se traga lo que escribiste deja al
                        // usuario sin saber que llego.
                        // El eco lleva un punto medio (0xB7) y no `>`. El `>`
                        // es la marca de Unix y este sistema no es Unix; el
                        // punto medio separa igual de bien y no arrastra la
                        // convencion de otro. Esta en la tabla de extras del
                        // font, asi que se dibuja sin tocar nada mas.
                        // El eco en su tinta y la respuesta en la normal: al
                        // mirar la rejilla, los comandos son las anclas y todo
                        // lo de debajo es lo que contestaron.
                        output.with_ink(INK_ECHO);
                        output.byte(0xB7);
                        output.byte(b' ');
                        output.text(&path[..n]);
                        output.byte(b'\n');
                        output.with_ink(INK_PLAIN);

                        // Hay un programa vivo escuchando en esta consola?
                        // Entonces la linea NO es un comando: es SUYA. Es lo
                        // que hace cualquier shell, y sin esto un `ACCEPT` de
                        // COBOL no puede recibir nada nunca -- el terminal se
                        // come la respuesta y contesta "no lo conozco".
                        //
                        // La calculadora se excluye a proposito: mientras
                        // espera al motor, ese hijo es SUYO y ya recibio sus
                        // tres lineas. Colar una mas ahi le cambiaria la
                        // cuenta a alguien que no la pidio.
                        let from_child = !calc.waiting
                            && child_console.as_ref().map(|cc| cc.has_child()).unwrap_or(false);

                        if from_child {
                            if let Some(cc) = child_console.as_ref() {
                                cc.write(&path[..n]);
                                // El salto va aparte y SIEMPRE: `read_line`
                                // espera a verlo para dar la linea por
                                // cerrada. Sin el, el programa sigue
                                // esperando algo que ya escribiste.
                                cc.write(b"\n");
                            }
                            paint_status(&p, &run_box, "para el programa", INK_DIM);
                            n = 0;
                            cur = 0;
                            repaint_field = true;
                            continue;
                        }

                        // Al historial va lo que es un COMANDO. Un importe
                        // tecleado para un `ACCEPT` es un dato, y mezclarlo
                        // con las rutas ensucia la flecha arriba justo cuando
                        // hace falta repetir el comando de verdad.
                        history.push(&path[..n]);
                        match parse(&path[..n]) {
                            Command::Nothing => {
                                paint_status(&p, &run_box, "escribe algo", INK_DIM);
                            }
                            // ** ESTO NO ES UNA DISTRO, y se dice con un gato.
                            //
                            // Un amigo del dueno, que viene de Linux, se sento
                            // delante y dio por hecho que lo era. Es un
                            // malentendido razonable --hay escritorio, ventanas
                            // y una caja donde teclear-- y "no lo conozco"
                            // habria sido correcto sin ensenar nada.
                            //
                            // Asi que la respuesta cuenta lo que de verdad
                            // separa a los dos sistemas: aqui no hay usuarios
                            // que elevar ni paquetes que instalar. Hay
                            // capabilities, y lo que no te dieron no existe
                            // para ti. Se rie del malentendido, nunca de quien
                            // lo tuvo.
                            Command::NotLinux(verb) => {
                                output.text(b"    n_n_n
");
                                output.text(b"   ( -.- )   ~nya. eso aqui no se dice.
");
                                output.text(b"   ( u u )   esto NO es Linux, es BMO-X.
");
                                output.text(b"    ^^ ^^    no hay root que pedir:
");
                                output.text(b"             o te dieron la capability, o no existe.
");
                                output.text(b"
");
                                let hint: &[u8] = match verb {
                                    b"sudo" | b"su" => {
                                        b"  aqui nadie ELEVA permisos: un proceso nace con lo que le
  concedieron, y no hay forma de pedir mas.
"
                                    }
                                    b"apt" | b"apt-get" | b"pacman" | b"yay" | b"dnf" | b"yum"
                                    | b"snap" => {
                                        b"  no hay repositorios. Los programas se compilan aqui, con el
  toolchain propio, y salen en .bex.
"
                                    }
                                    b"systemctl" => {
                                        b"  no hay demonios. Un servicio es un proceso de Ring 3 con su
  capability, y se lanza con `run`.
"
                                    }
                                    b"chmod" | b"chown" => {
                                        b"  no hay bits de permiso ni duenos. El permiso ES el handle:
  sin el, el objeto no se puede ni nombrar.
"
                                    }
                                    b"man" => b"  prueba `ayuda`. Es mas corta y cabe en la pantalla.
",
                                    b"grep" => b"  prueba `cat` y la rueda. O F11, que filtra por gravedad.
",
                                    _ => b"  prueba `ayuda` para ver lo que SI hay.
",
                                };
                                output.text(hint);
                                paint_status(&p, &run_box, "esto no es Linux :3", INK_DIM);
                                repaint_field = true;
                            }
                            Command::List(dir_path) => {
                                match bmo::Directorio::open(dir_path) {
                                    Ok(d) => {
                                        let mut count = 0u32;
                                        // Tope por si un directorio enorme se
                                        // comiera el fotograma entero.
                                        while count < 256 {
                                            let e = match d.next() {
                                                Some(e) => e,
                                                None => break,
                                            };
                                            let mut nom = [0u8; 12];
                                            let length = e.legible(&mut nom);
                                            // `.` y `..` no se ensenan: aqui
                                            // no hay carpeta actual a la que
                                            // volver, asi que son ruido.
                                            if is_dot_entry(&nom[..length]) { continue; }
                                            output.text(b"  ");
                                            output.text(&nom[..length]);
                                            // Alinear la columna del tamano.
                                            let mut k = length;
                                            while k < 14 { output.byte(b' '); k += 1; }
                                            if e.es_dir {
                                                output.text(b"<DIR>");
                                            } else {
                                                let mut d10 = [0u8; 10];
                                                let n10 = decimal(e.bytes as u64, &mut d10);
                                                output.text(&d10[..n10]);
                                            }
                                            output.byte(b'\n');
                                            count += 1;
                                        }
                                        if count == 0 {
                                            output.text(b"  (vacio)
");
                                        }
                                        paint_status(&p, &run_box, "listo", INK_DIM);
                                    }
                                    // * El MOTIVO, no un "no pude" para todo.
                                    //
                                    // Esto tiraba el codigo con `Err(_)` y
                                    // decia siempre "no puedo abrir esa
                                    // carpeta". Cuando la tabla de directorios
                                    // del kernel se lleno, eso fue una mentira
                                    // exacta: la carpeta estaba ahi, lo que no
                                    // habia era ranura. Y mando a buscar el
                                    // fallo al disco, que estaba perfecto.
                                    //
                                    // Un error que no distingue sus causas es
                                    // un error que manda a mirar donde no es.
                                    Err(cod) => {
                                        // 25 = sin hueco, 26 = no esta. Ver
                                        // `ring0/obj/directorio.rs`.
                                        let (line, estado): (&[u8], &str) = if cod == 25 {
                                            (
                                                b"  no queda slot de directorio en el kernel.\n",
                                                "sin ranura libre",
                                            )
                                        } else {
                                            (
                                                b"  no puedo open esa carpeta.\n",
                                                "carpeta no encontrada",
                                            )
                                        };
                                        output.with_ink(INK_ERR);
                                        output.text(line);
                                        output.with_ink(INK_PLAIN);
                                        paint_status(&p, &run_box, estado, INK_BAD);
                                    }
                                }
                                n = 0;
                            }
                            // -- Leer un archivo --
                            //
                            // El hermano de `ls`: aquel dice QUE hay, este
                            // ensena lo de DENTRO. Es la primera vez que un
                            // programa de Ring 3 abre un archivo del disco.
                            Command::Read(file_path) => {
                                match bmo::Archivo::leer_de(file_path) {
                                    Ok(a) => {
                                        let mut chunk = [0u8; 256];
                                        let mut total = 0usize;
                                        // El ultimo byte se guarda segun pasa:
                                        // reconstruirlo al final obligaria a
                                        // saber en que trozo cayo, y el buffer
                                        // ya se ha reutilizado.
                                        let mut last = 0u8;
                                        // De 256 en 256 y con tope: un archivo
                                        // que no sea texto llenaria la rejilla
                                        // de basura y se comeria el fotograma.
                                        loop {
                                            let n = a.read(&mut chunk);
                                            if n == 0 { break; }
                                            output.text(&chunk[..n]);
                                            last = chunk[n - 1];
                                            total += n;
                                            if total >= 2048 {
                                                output.text(b"\n  ...(cortado)\n");
                                                last = b'\n';
                                                break;
                                            }
                                        }
                                        if total == 0 {
                                            output.text(b"  (vacio)\n");
                                        } else if last != b'\n' {
                                            // Sin esto, el proximo mensaje se
                                            // pega al final del archivo.
                                            output.byte(b'\n');
                                        }
                                        a.close();
                                        paint_status(&p, &run_box, "listo", INK_DIM);
                                    }
                                    Err(e) => {
                                        output.with_ink(INK_ERR);
                                        output.text(b"  ");
                                        output.text(file_error_reason(e));
                                        output.byte(b'\n');
                                        output.with_ink(INK_PLAIN);
                                        paint_status(&p, &run_box, "no se pudo leer", INK_BAD);
                                    }
                                }
                                n = 0;
                            }
                            // -- Escribir un archivo --
                            //
                            // Lo que NUNCA habia pasado: un programa de Ring 3
                            // dejando algo en el disco. Hasta hoy todo lo que
                            // habia ahi lo puso el anfitrion al flashear o el
                            // kernel con su caja negra.
                            Command::Write(file_path, text) => {
                                match bmo::Archivo::create(file_path) {
                                    Ok(a) => {
                                        let placed = a.write(text);
                                        // El salto final: un archivo de texto
                                        // sin el ultimo salto es el clasico
                                        // que descuadra al siguiente que lo lee.
                                        a.write(b"\n");
                                        // * Aqui es donde llega al disco. Antes
                                        // de esto no hay nada escrito.
                                        if a.close() {
                                            output.text(b"  guardado: ");
                                            let mut d10 = [0u8; 10];
                                            let n10 = decimal(placed as u64 + 1, &mut d10);
                                            output.text(&d10[..n10]);
                                            output.text(b" bytes\n");
                                            paint_status(&p, &run_box, "guardado", INK_OK);
                                        } else {
                                            output.text(b"  no se guardo nada.\n");
                                            paint_status(&p, &run_box, "no se pudo guardar", INK_BAD);
                                        }
                                    }
                                    Err(e) => {
                                        output.with_ink(INK_ERR);
                                        output.text(b"  ");
                                        output.text(file_error_reason(e));
                                        output.byte(b'\n');
                                        output.with_ink(INK_PLAIN);
                                        paint_status(&p, &run_box, "no se pudo crear", INK_BAD);
                                    }
                                }
                                n = 0;
                            }
                            // -- Volcar el historial a un .txt --
                            //
                            // El hermano manual del volcado automatico: aquel
                            // guarda lo de UNA corrida, este guarda todo lo que
                            // quede en el historial, que es lo que hace falta
                            // cuando lo interesante son tres comandos juntos.
                            Command::Save(arg) => {
                                let dest = if arg.is_empty() { DEFAULT_DUMP } else { arg };
                                // El rango se toma ANTES de escribir nada:
                                // los mensajes de abajo son de esta orden, no
                                // de lo que se estaba guardando, y colarlos
                                // dentro haria que el archivo hablara de si
                                // mismo.
                                let (from, to) = output.all_rows();
                                match dump_output(&output, dest, from, to) {
                                    Ok(bytes) => {
                                        output.with_ink(INK_GOOD);
                                        output.text(b"  guardado en ");
                                        output.text(dest);
                                        output.text(b": ");
                                        let mut d = [0u8; 10];
                                        let k = decimal(bytes as u64, &mut d);
                                        output.text(&d[..k]);
                                        output.text(b" bytes, ");
                                        let k = decimal((to - from + 1) as u64, &mut d);
                                        output.text(&d[..k]);
                                        output.text(b" lineas\n");
                                        output.with_ink(INK_PLAIN);
                                        paint_status(&p, &run_box, "volcado", INK_OK);
                                    }
                                    Err(0) => {
                                        output.with_ink(INK_ERR);
                                        output.text(b"  no se guardo nada. el motivo esta en F11.\n");
                                        output.with_ink(INK_PLAIN);
                                        paint_status(&p, &run_box, "no se pudo guardar", INK_BAD);
                                    }
                                    Err(e) => {
                                        output.with_ink(INK_ERR);
                                        output.text(b"  ");
                                        output.text(file_error_reason(e));
                                        output.byte(b'\n');
                                        output.with_ink(INK_PLAIN);
                                        paint_status(&p, &run_box, "no se pudo crear", INK_BAD);
                                    }
                                }
                                n = 0;
                            }
                            // * `sella` YA NO VIVE AQUI, y esto lo dice.
                            //
                            // La orden se mudo a la ventana de ESTRATOS porque
                            // el verbo vive donde vive el objeto. Borrarla y
                            // contestar "no lo conozco" habria sido correcto y
                            // cruel: estaba escrita en la linea de ayuda de
                            // ayer, en dos documentos y en la costumbre del
                            // dueno. **Una funcion que se muda sin dejar nota se
                            // convierte en una funcion que desaparecio.**
                            Command::SealMoved => {
                                output.text(b"  sellar se mudo a la ventana de ESTRATOS.
");
                                output.with_ink(INK_GOOD);
                                output.text(b"  F12  ->  TAB  ->  tecla S
");
                                output.with_ink(INK_PLAIN);
                                output.text(b"  ahi se ve el volumen mientras se sella, que es
");
                                output.text(b"  donde tiene sentido: la generacion sube delante.
");
                                paint_status(&p, &run_box, "esta en F12", INK);
                                n = 0;
                            }
                            // * `perf` -- el numero antes que la tarjeta.
                            //
                            // Se pinta ANTES de leer los contadores no: se leen
                            // aqui y se imprimen, y el fotograma que los pinta
                            // sumara el suyo. Da igual: lo que interesa es el
                            // orden de magnitud y el peor caso, no un digito.
                            Command::PaintCost => {
                                let v = p.volcado();
                                output.text(b"  pintado\n");
                                output.text(b"    modo        ");
                                output.text(match v.modo {
                                    bmo::Volcador::Ninguno => b"directo al panel (SIN doble bufer)\n" as &[u8],
                                    bmo::Volcador::Directo => b"doble bufer, volcado por CPU\n",
                                });
                                output.text(b"    fotogramas  ");
                                let mut d = [0u8; 10];
                                let k = decimal(v.fotogramas, &mut d);
                                output.text(&d[..k]);
                                output.text(b"   con algo que mover\n");
                                if v.fotogramas > 0 {
                                    output.text(b"    medio      ");
                                    let k = decimal(v.bytes / v.fotogramas / 1024, &mut d);
                                    output.text(&d[..k]);
                                    output.text(b" KiB por fotograma\n");
                                    // El PEOR caso va aparte y a proposito: un
                                    // tiron se nota y una media buena lo tapa.
                                    output.text(b"    peor       ");
                                    let k = decimal(v.peor / 1024, &mut d);
                                    output.text(&d[..k]);
                                    output.text(b" KiB en un fotograma\n");
                                    output.text(b"    total      ");
                                    // ** Y CUANTAS CAJAS tenia ese peor
                                    // fotograma. Con la caja unica de antes
                                    // esto seria SIEMPRE 1 y el `worst` la
                                    // pantalla entera; si aqui sale 2 o 3 con
                                    // un peor pequeno, el troceado trabaja.
                                    output.text(b"    cajas      ");
                                    let k = decimal(v.cajas as u64, &mut d);
                                    output.text(&d[..k]);
                                    output.text(b"
");
                                    let k = decimal(v.bytes / 1024 / 1024, &mut d);
                                    output.text(&d[..k]);
                                    output.text(b" MiB movidos desde el arranque\n");
                                }
                                output.with_ink(INK_ECHO);
                                output.text(b"    la caja de sucio ya recorta esto: una GPU solo\n");
                                output.text(b"    compra algo si estos numeros son grandes.\n");
                                output.with_ink(INK_PLAIN);
                                paint_status(&p, &run_box, "listo", INK_DIM);
                                n = 0;
                            }
                            Command::Calculator => {
                                calc.visible = !calc.visible;
                                if calc.visible {
                                    paint_calc(&p, &calc_pad, &calc, calc_hover);
                                    output.text(b"  calculadora: la cara en Rust, el calculo en COBOL
");
                                } else {
                                    // Devolver esa zona a la escena.
                                    for f in 0..calc_pad.height {
                                        for co in 0..calc_pad.width {
                                            let (px, py) = (calc_pad.x + co, calc_pad.y + f);
                                            p.punto(px, py, scene_color(&run_box, visible, px, py, p.alto));
                                        }
                                    }
                                }
                                paint_status(&p, &run_box, "listo", INK_DIM);
                                n = 0;
                                cur = 0;
                            }
                            Command::Clear => {
                                output.clear();
                                paint_status(&p, &run_box, "listo", INK_DIM);
                                n = 0;
                            }
                            // ** `audio` -- paso 0 de docs/AUDIO_MAESTRO.md.
                            //
                            // La orden existia SOLO en el shell de Ring 0 y el
                            // dueno la escribio aqui, que es donde se trabaja.
                            // Contesto "no es un comando ni una ruta" y la
                            // prueba se quedo sin hacer. Dos shells con dos
                            // vocabularios distintos son dos productos.
                            // ** LA RED -- siete campos que hasta hoy no
                            // cruzaban a Ring 3. Mismo criterio que `audio`: la
                            // orden existia SOLO en el shell de Ring 0, y dos
                            // shells con dos vocabularios son dos productos.
                            Command::Net(what) => {
                                commands::reports::report_net(&mut output, what);
                            }
                            Command::Audio => {
                                let had_any = bmo::audio_censo();
                                if had_any {
                                    output.with_ink(INK_GOOD);
                                    output.text(b"  aparato de reproduccion HALLADO\n");
                                    output.with_ink(INK_PLAIN);
                                    output.text(b"  los ocho numeros estan en F11 (canales, bits, frecuencias)\n");
                                    output.text(b"  comparalos con lo que dice Windows del mismo audifono\n");
                                } else {
                                    output.with_ink(INK_ERR);
                                    output.text(b"  ningun aparato de reproduccion en los puertos libres\n");
                                    output.with_ink(INK_PLAIN);
                                    // La distincion que decide el siguiente paso, y por eso
                                    // se dice aqui y no solo en CABINA.
                                    output.text(b"  F11 dice CUANTOS puertos se miraron: si es 0, el fallo\n");
                                    output.text(b"  es del censo; si es >0, el aparato no es UAC1\n");
                                }
                                paint_status(&p, &run_box, "audio", INK_DIM);
                                n = 0;
                            }
                            Command::Help => {
                                output.text(b"  <ruta>       lanza un .bex   (cobol/banco.bex)\n");
                                output.text(b"  run <ruta>   lo mismo, como en el shell de Ring 0\n");
                                // Va JUSTO detras de `run` porque es su hermana,
                                // y con la consecuencia delante: lo que sorprende
                                // no es que lance, es que el escritorio se vaya.
                                output.text(b"  presta <ruta>  se lo lanza CON LA PANTALLA: el\n");
                                output.text(b"               escritorio se aparta y vuelve cuando\n");
                                output.text(b"               el programa termina  (c/ray.bex)\n");
                                output.text(b"  cat <ruta>   ensena lo que hay dentro\n");
                                output.text(b"  write <ruta> <texto>     lo guarda\n");
                                output.text(b"  guarda [ruta]  vuelca esta salida a un .txt\n");
                                output.text(b"               (por defecto datos/salida.txt, y cada\n");
                                output.text(b"                programa que corre lo deja solo ahi)\n");
                                output.text(b"  clear / cls  limpia esta salida\n");
                                output.text(b"  TAB          completa   Ctrl+A/E inicio/fin\n");
                                output.text(b"  Ctrl+K corta al final    Ctrl+W borra palabra\n");
                                output.text(b"  Ctrl+U borra linea       Ctrl+L limpia\n");
                                output.text(b"  info         RAM, CPU, tareas y disco\n");
                                output.text(b"  cpu / mem    solo esa parte del informe\n");
                                output.text(b"  perf         lo que cuesta pintar, medido\n");
                                output.text(b"  estratos sellar   ESCRIBE EN EL DISCO (commit vacio)\n");
                                output.text(b"  help         esto\n");
                                output.text(b"  reboot       reinicia la maquina\n");
                                output.text(b"  Ctrl+Alt     esconde o invoca esta ventana\n");
                                paint_status(&p, &run_box, "listo", INK_DIM);
                                n = 0;
                            }
                            // Ni se intenta lanzar. Se dice lo que es y con
                            // que se abre -- un mensaje sobre la FIRMA aqui
                            // manda a buscar un permiso que no hace falta.
                            Command::NotAProgram(r) => {
                                output.with_ink(INK_ERR);
                                output.text(b"  eso no es un programa (solo .bex se lanza).\n");
                                output.text(b"  para verlo:  cat ");
                                output.text(r);
                                output.byte(b'\n');
                                output.with_ink(INK_PLAIN);
                                paint_status(&p, &run_box, "no es un programa: prueba lee", INK_DIM);
                                n = 0;
                            }
                            Command::Autopsy => {
                                report_autopsy(&mut output);
                                paint_status(&p, &run_box, "ultimo fallo de Ring 3", INK_DIM);
                                n = 0;
                            }
                            Command::Report => {
                                report_system(&mut output);
                                paint_status(&p, &run_box, "informe del sistema", INK_DIM);
                                n = 0;
                            }
                            Command::Cpu => {
                                report_cpu(&mut output);
                                paint_status(&p, &run_box, "procesador", INK_DIM);
                                n = 0;
                            }
                            Command::Memoria => {
                                report_memory(&mut output);
                                paint_status(&p, &run_box, "memoria", INK_DIM);
                                n = 0;
                            }
                            // * El aviso va ANTES y se VUELCA antes, porque la
                            // llamada bloquea hasta un segundo entero mientras
                            // el kernel manda INIT+SIPI a cada nucleo. Un
                            // mensaje escrito despues de volver no explica
                            // nada: para entonces la espera ya paso, y lo que
                            // el dueno habria visto es un escritorio congelado
                            // sin motivo.
                            Command::Smp(arg) => {
                                // * El CONTROL, y el reparto de quien decide:
                                // aqui solo se traduce lo que el dueno escribio
                                // a un numero. `smp` a secas censa y no toca
                                // nada -- que sea el caso por defecto es la
                                // diferencia entre un mando y un boton.
                                // Los dos mandos que no son un numero: parar y
                                // medir. Se resuelven aqui y salen, porque no
                                // comparten NADA con el camino de despertar.
                                // `stop` y `test` son los nombres que el dueno
                                // pidio; `parar` y `prueba` siguen valiendo. Un
                                // alias cuesta cuatro bytes y evita el unico
                                // fallo de una orden bien escrita: no acordarse
                                // de como se llamaba.
                                if arg == b"parar" || arg == b"para" || arg == b"stop" {
                                    bmo::smp_parar();
                                    output.text(b"  obreros parados (vuelven a hlt)\n");
                                    // ** Y LO QUE VA A PASAR DESPUES, DICHO AQUI.
                                    //
                                    // El dueno escribio `smp stop`, luego `smp`,
                                    // y leyo `12 de 12`. Las dos lineas eran
                                    // ciertas y juntas decian una mentira. Lo
                                    // que faltaba no era un numero distinto:
                                    // era avisar de que ese numero cuenta otra
                                    // cosa.
                                    output.text(b"  [!] seguiran contando como \"en pie\": encendidos, no trabajando\n");
                                    output.text(b"      `smp all` los vuelve a poner a trabajar\n");
                                    paint_output(&p, &run_box, &output);
                                    paint_status(&p, &run_box, "smp", INK_DIM);
                                    n = 0;
                                    cur = 0;
                                    continue;
                                }
                                if arg == b"prueba" || arg == b"bench" || arg == b"test" {
                                    output.text(b"  midiendo reparto (esto tarda)...\n");
                                    paint_output(&p, &run_box, &output);
                                    p.volcar();
                                    let x100 = bmo::smp_prueba();
                                    let mut b = [0u8; 10];
                                    output.with_ink(if x100 >= 150 { INK_GOOD } else { INK_ERR });
                                    output.text(b"  aceleracion: ");
                                    let k = decimal(x100 / 100, &mut b);
                                    output.text(&b[..k]);
                                    output.text(b".");
                                    // Los dos decimales, con su cero delante:
                                    // "8.4" y "8.04" no son el mismo numero.
                                    if x100 % 100 < 10 {
                                        output.text(b"0");
                                    }
                                    let k = decimal(x100 % 100, &mut b);
                                    output.text(&b[..k]);
                                    output.text(b"x   (F11 trae los ticks)\n");
                                    output.with_ink(INK_PLAIN);
                                    if x100 == 0 {
                                        output.text(b"  0 = falto una parte: el numero no vale\n");
                                    }
                                    paint_status(&p, &run_box, "smp", INK_DIM);
                                    n = 0;
                                    cur = 0;
                                    continue;
                                }
                                let how_many = if arg.is_empty() {
                                    0
                                } else if arg == b"all" || arg == b"todos" {
                                    u32::MAX
                                } else {
                                    let mut v = 0u32;
                                    let mut ok = false;
                                    for &b in arg {
                                        if b >= b'0' && b <= b'9' {
                                            v = v.saturating_mul(10) + (b - b'0') as u32;
                                            ok = true;
                                        } else {
                                            ok = false;
                                            break;
                                        }
                                    }
                                    // Un argumento que no se entiende NO se
                                    // interpreta como "todos": eso convertiria
                                    // un dedazo en once INIT+SIPI.
                                    if ok { v } else { 0 }
                                };
                                if how_many == 0 {
                                    output.text(b"  censando (no se despierta a nadie)\n");
                                } else {
                                    output.text(b"  despertando nucleos (esto tarda)...\n");
                                }
                                paint_output(&p, &run_box, &output);
                                p.volcar();
                                let (alive, expected, stopped) = bmo::smp_censo(how_many);
                                output.with_ink(if alive == expected {
                                    INK_GOOD
                                } else {
                                    INK_ERR
                                });
                                output.text(b"  nucleos en pie: ");
                                let mut b = [0u8; 10];
                                let k = decimal((alive + 1) as u64, &mut b);
                                output.text(&b[..k]);
                                output.text(b" de ");
                                let k = decimal((expected + 1) as u64, &mut b);
                                output.text(&b[..k]);
                                output.text(b"   (F11 lo cuenta entero)\n");
                                output.with_ink(INK_PLAIN);
                                // ** LA MITAD QUE FALTABA DEL CENSO.
                                //
                                // "En pie" cuenta nucleos que contestaron al
                                // SIPI, y ese numero no baja al pararlos --
                                // correctamente: salir del reset no es trabajar.
                                // Pero leido solo, dice que `smp stop` no hizo
                                // nada. Ahora se dicen las dos cosas.
                                if stopped {
                                    output.with_ink(INK_ERR);
                                    output.text(b"  [!] pero estan PARADOS: en pie no es trabajando\n");
                                    output.with_ink(INK_PLAIN);
                                    output.text(b"      `smp all` los vuelve a poner a trabajar\n");
                                }
                                // La guia va donde se necesita: justo despues
                                // de censar, que es cuando uno se pregunta
                                // "y ahora como los enciendo?". Un atajo que
                                // solo vive en la documentacion no existe.
                                // ** LAS CINCO, NO SOLO DOS (2026-08-11).
                                //
                                // Aqui se decian `smp all` y `smp N` y se
                                // callaban `prueba` y `parar`, que son las dos
                                // unicas que HACEN algo interesante. El dueno
                                // lo dijo con todas las letras: *"el smp no me
                                // salen mensajes de recomendacion"*.
                                //
                                // Son cinco lineas y caben. Una orden con
                                // subordenes que no las dice obliga a buscar en
                                // `help`, y a `help` se va cuando uno ya se
                                // rindio.
                                if how_many == 0 {
                                    output.text(b"  smp all      despierta todos    smp 3   solo tres\n");
                                    output.text(b"  smp test     reparte una cuenta y mide la aceleracion\n");
                                    output.text(b"  smp stop     los duerme. [!] sin IPI NO vuelven\n");
                                    output.text(b"  F11 dice en que esta cada nucleo y cual gira en vacio\n");
                                }
                                paint_status(&p, &run_box, "smp", INK_DIM);
                                n = 0;
                                cur = 0;
                            }
                            // Se pinta ANTES de pedirlo: la llamada no vuelve,
                            // asi que un mensaje despues no lo veria nadie. Y
                            // que quede escrito distingue "reinicio pedido" de
                            // "se colgo" en la foto. `Pantalla` escribe directo
                            // al framebuffer, asi que al volver de `text` ya
                            // esta en el cristal: no hay nada que vaciar.
                            Command::Reboot => {
                                output.text(b"  reiniciando...\n");
                                paint_status(&p, &run_box, "reiniciando", INK_DIM);
                                bmo::reiniciar();
                            }
                            Command::Unknown => {
                                // El mensaje honesto. Antes se contestaba "no
                                // esta: revisa la ruta" a quien escribia
                                // `reboot`, y eso manda a buscar un archivo que
                                // nunca existio en vez de decir la verdad.
                                output.text(b"  no es un comando ni una ruta. escribe 'help'.\n");
                                paint_status(&p, &run_box, "no lo conozco: prueba help", INK_BAD);
                                n = 0;
                            }
                            Command::Launch(target) => {
                                let cap = child_console.as_ref().map(|c| c.cap).unwrap_or(0);
                                // ** `run` DECIDE SOLO.
                                //
                                // Si el `.bex` declara `WANTS_SCREEN` --bandera
                                // que pone el COMPILADOR al ver que el programa
                                // reclama la pantalla-- el escritorio se aparta
                                // sin que nadie tenga que pedirlo.
                                //
                                // Esto es lo que `presta` deberia haber sido
                                // desde el principio: la politica en el
                                // compositor, no en los dedos del usuario.
                                // `presta` sigue existiendo para forzarlo a
                                // mano, pero ya no hace falta saberselo.
                                if wants_screen(target) {
                                    match lend_screen(p, input.take(), target, cap) {
                                        Some((new, ent)) => {
                                            p = new;
                                            input = ent;
                                            // ** EL ESCRITORIO ENTERO, NO UN RELLENO PLANO.
                                            //
                                            // Aqui habia un `p.clear(BG)` y nada mas. O
                                            // sea que al volver de prestar la pantalla el
                                            // escritorio se quedaba **sin degradado, sin barra
                                            // y sin iconos**: fondo liso, la caja de Ejecutar
                                            // flotando, y nada mas. Es exactamente lo que salio
                                            // en la foto del 2026-08-11 cuando DOOM no arranco,
                                            // y se leyo como *"el escritorio se bugeo"*.
                                            //
                                            // No estaba bugeado: **estaba a medio pintar**, y
                                            // el que faltaba por pintar era todo menos una
                                            // ventana.
                                            //
                                            // [!] Y este camino se recorre tambien --sobre todo--
                                            // cuando el programa **NO** arranca: `lend_screen`
                                            // recupera y vuelve por aqui. O sea que el aspecto
                                            // del escritorio despues de un lanzamiento FALLIDO
                                            // depende enteramente de estas lineas. Es el
                                            // camino de error, que es el que nadie prueba a
                                            // mano (patron 29).
                                            scene::paint_background(&p);
                                            scene::launcher::paint(&p, &launcher);
                                            p.rect(16, 13, 14, 14, ACCENT);
                                            p.texto(38, 14, "BMO-X", INK);
                                            taskbar_dirty = true;
                                            paint_run_box(&p, &run_box);
                                            paint_field(&p, &run_box, &path[..n], cur, true);
                                            paint_output(&p, &run_box, &output);
                                            paint_status(&p, &run_box, "pantalla devuelta", INK_OK);
                                            p.vaciar();
                                            repaint_field = true;
                                        }
                                        None => {
                                            bmo::consola(
                                                "no pude recuperar la pantalla tras prestarla
",
                                            );
                                            bmo::salir()
                                        }
                                    }
                                    n = 0;
                                    continue;
                                }
                                match bmo::ejecutar_en(target, cap) {
                                    Ok(_) => {
                                        paint_status(&p, &run_box, "lanzado", INK_OK);
                                        // * Se apunta DONDE empieza esta
                                        // corrida. El volcado no puede hacerse
                                        // aqui: `ejecutar_en` vuelve en cuanto
                                        // el hijo arranca y todavia no ha
                                        // escrito ni una letra. Lo que se
                                        // guarda es la marca, y el volcado
                                        // ocurre cuando el hijo MUERE -- ver el
                                        // vigilante del bucle principal.
                                        // `-1` para que el ECO entre en el
                                        // volcado. El archivo se sobreescribe
                                        // en cada corrida, asi que sin la
                                        // linea del comando dentro no hay
                                        // forma de saber QUE lo produjo -- y un
                                        // volcado anonimo es la mitad de un
                                        // volcado.
                                        let mut dest = [0u8; 32];
                                        let dest_n = dump_name(target, &mut dest);
                                        run = Some(Run {
                                            mark: output.mark().saturating_sub(1),
                                            waits: 0,
                                            dest,
                                            dest_n,
                                        });
                                        // El campo se vacia al lanzar, como el
                                        // Win+R: la caja esta para el SIGUIENTE
                                        // programa, no para admirar el anterior.
                                        n = 0;
                                    }
                                    // [!] Este codigo tapa DOS causas: que el
                                    // archivo no este, y que este pero no se
                                    // pueda cargar --por ejemplo si pasa de
                                    // `MAX_BEX`, 1 MiB--. Le paso al dueno con
                                    // `c/read.bex`, que SALIA EN `ls` y aqui
                                    // decia que no estaba.
                                    //
                                    // Separarlas de verdad es tocar el ABI. Lo
                                    // que se hace ya es mandar a mirar donde el
                                    // kernel SI cuenta el motivo entero.
                                    Err(bmo::ERROR_NOT_THERE) => paint_status(
                                        &p,
                                        &run_box,
                                        "no se pudo cargar: F11 dice por que",
                                        INK_BAD,
                                    ),
                                    Err(bmo::ERROR_GATE) => paint_status(
                                        &p,
                                        &run_box,
                                        "rechazado: la firma no cuadra",
                                        INK_BAD,
                                    ),
                                    Err(bmo::ERROR_BUSY) => {
                                        paint_status(&p, &run_box, "no hay hueco ahora mismo", INK_BAD)
                                    }
                                    Err(_) => {
                                        paint_status(&p, &run_box, "no paso la admision", INK_BAD)
                                    }
                                }
                            }
                        }
                        // El cursor detras de la linea, SIEMPRE. Las ramas que
                        // vacian el campo ponian `n = 0` y dejaban `cur` donde
                        // estaba: la tecla siguiente se escribia en `path[cur]`
                        // --fuera de lo que se dibuja-- y el campo ensenaba los
                        // bytes VIEJOS del comando anterior. Escribir `2` tras
                        // `run apps/calc.bex` mostraba una `r`. Las ramas de
                        // error conservan la ruta a proposito para poder
                        // corregirla, y ahi `cur` no se mueve: por eso es un
                        // `min` y no un cero.
                        cur = cur.min(n);
                        repaint_field = true;
                    }
                    // TAB: completar.
                    b'\t' => {
                        let antes = n;
                        n = complete(&mut path, n, &mut output);
                        cur = n;
                        if n == antes {
                            paint_status(&p, &run_box, "nada que completar", INK_DIM);
                        }
                        repaint_field = true;
                    }
                    // Retroceso.
                    //
                    // ** LA GUARDA ES `cur > 0 && n > 0`, Y LE FALTABA LA
                    // SEGUNDA MITAD. Panico en el Ryzen el 2026-08-09:
                    //
                    //     range end index 18446744073709551615
                    //     out of range for slice of length ...
                    //     en services\gui\src\main.rs:2834
                    //
                    // Esa linea es `paint_field(..., &path[..n], ...)`, y el
                    // indice es `usize::MAX`: **`n` se desbordo por abajo**.
                    // Este `n -= 1` estaba guardado por `cur > 0` -- que es la
                    // condicion del OTRO contador. Con `cur > 0` y `n == 0`, la
                    // resta da la vuelta y el siguiente repintado revienta.
                    //
                    // ** Y para llegar ahi hacia falta romper `cur <= n`, que es
                    // el invariante de este campo. Lo rompio el camino nuevo del
                    // lanzador: pulsar el icono deja `n = cur = 17`, el `run` se
                    // lanza, **falla la admision**, y en ese camino de fallo `n`
                    // vuelve a 0 sin que `cur` le acompane. Un retroceso
                    // despues, la maquina se lleva el escritorio por delante.
                    //
                    // Se arregla en los dos sitios: aqui la guarda correcta, y
                    // arriba el invariante restaurado en cada vuelta -- que es
                    // lo que impide que el proximo camino nuevo lo vuelva a
                    // romper sin que nadie se entere.
                    0x08 | 0x7F => {
                        if cur > 0 && n > 0 {
                            let mut k = cur;
                            while k < n {
                                path[k - 1] = path[k];
                                k += 1;
                            }
                            cur -= 1;
                            n -= 1;
                            repaint_field = true;
                        }
                    }
                    // Escape: borrar la linea entera, igual que en el Win+R.
                    0x1B => {
                        n = 0;
                        cur = 0;
                        paint_status(&p, &run_box, "listo", INK_DIM);
                        repaint_field = true;
                    }
                    // -- El portapapeles --
                    //
                    // Ctrl+C copia la linea entera; Ctrl+V la pega donde este
                    // el cursor. No es un lujo: la mitad de lo que se teclea en
                    // un terminal es una variacion de lo anterior, y sin copiar
                    // hay que reescribirlo todo.
                    //
                    // Ctrl+C para copiar y no para interrumpir, que es lo que
                    // significa en Unix. Aqui no hay senales que mandar, y el
                    // dedo que ya sabe Ctrl+C sabe copiar -- no interrumpir.
                    0x03 => {
                        clipboard_n = n;
                        clipboard[..n].copy_from_slice(&path[..n]);
                        paint_status(&p, &run_box, "copiado", INK_DIM);
                    }
                    0x16 => {
                        if clipboard_n > 0 && n + clipboard_n <= PATH_MAX {
                            // Hueco del tamano del pegado, y meterlo.
                            let mut k = n;
                            while k > cur {
                                path[k + clipboard_n - 1] = path[k - 1];
                                k -= 1;
                            }
                            path[cur..cur + clipboard_n].copy_from_slice(&clipboard[..clipboard_n]);
                            cur += clipboard_n;
                            n += clipboard_n;
                            repaint_field = true;
                        }
                    }
                    // Ctrl+U -- borra la linea. Ctrl+L -- borra la salida.
                    // Los mismos que el shell de Ring 0, porque los dedos ya
                    // los tienen y un atajo que cambia entre dos ventanas del
                    // mismo sistema es peor que no tenerlo.
                    0x15 => {
                        n = 0;
                        cur = 0;
                        repaint_field = true;
                    }
                    0x0C => {
                        output.clear();
                        repaint_field = true;
                    }
                    // FLECHA ARRIBA / ABAJO -- el historial. Llegan por la misma
                    // cola que las letras, con bytes del rango C1 (0x80..0x9F)
                    // que no tienen glifo: el driver los eligio justo para que
                    // no puedan confundirse con texto.
                    // Ctrl+ARRIBA copia, Ctrl+ABAJO pega. Lo mismo que
                    // Ctrl+C / Ctrl+V, con las flechas -- porque los dedos que
                    // ya andan por el historial no tienen que irse a buscar
                    // otra tecla para copiar lo que acaban de recuperar.
                    0x80 if ctrl => {
                        clipboard_n = n;
                        clipboard[..n].copy_from_slice(&path[..n]);
                        paint_status(&p, &run_box, "copiado", INK_DIM);
                    }
                    0x81 if ctrl => {
                        if clipboard_n > 0 && n + clipboard_n <= PATH_MAX {
                            let mut k = n;
                            while k > cur {
                                path[k + clipboard_n - 1] = path[k - 1];
                                k -= 1;
                            }
                            path[cur..cur + clipboard_n].copy_from_slice(&clipboard[..clipboard_n]);
                            cur += clipboard_n;
                            n += clipboard_n;
                            repaint_field = true;
                        }
                    }
                    0x80 => {
                        if let Some(k) = history.back(&mut path) {
                            n = k;
                            cur = k;
                            repaint_field = true;
                        }
                    }
                    0x81 => {
                        if let Some(k) = history.forward(&mut path) {
                            n = k;
                            cur = k;
                            repaint_field = true;
                        }
                    }
                    // IZQUIERDA / DERECHA -- mover el cursor.
                    0x82 => {
                        if cur > 0 { cur -= 1; repaint_field = true; }
                    }
                    0x83 => {
                        if cur < n { cur += 1; repaint_field = true; }
                    }
                    // INICIO / FIN.
                    0x84 => { cur = 0; repaint_field = true; }
                    0x85 => { cur = n; repaint_field = true; }
                    // -- Los atajos de edicion de linea --
                    //
                    // Los de toda la vida en una consola: Ctrl+A al principio,
                    // Ctrl+E al final, Ctrl+K corta hasta el final, Ctrl+W
                    // borra la palabra de atras. Van ADEMAS de Inicio/Fin, que
                    // ya estaban: los dedos que vienen de un terminal buscan
                    // estos, y los que vienen de Windows buscan aquellos.
                    // Atender a los dos cuesta cuatro lineas.
                    0x01 => { cur = 0; repaint_field = true; }
                    0x05 => { cur = n; repaint_field = true; }
                    // Ctrl+K: tirar lo que hay del cursor al final.
                    0x0B => {
                        n = cur;
                        repaint_field = true;
                    }
                    // Ctrl+W: borrar la palabra de atras. Primero se comen los
                    // espacios y luego las letras, que es lo que espera
                    // cualquiera que lo haya usado -- si no, borrar tras un
                    // espacio no haria nada.
                    0x17 => {
                        // `cur - k` con `cur` pasado de `n` daria un `removed`
                        // enorme y el `n -= removed` de abajo se desbordaria
                        // igual que el retroceso. El invariante de arriba ya lo
                        // impide; la guarda se queda porque esta resta no tiene
                        // por que fiarse de que alguien lo mantenga.
                        let limit = cur.min(n);
                        let mut k = limit;
                        while k > 0 && path[k - 1] == b' ' { k -= 1; }
                        while k > 0 && path[k - 1] != b' ' { k -= 1; }
                        let removed = limit - k;
                        if removed > 0 {
                            let mut i = limit;
                            while i < n {
                                path[i - removed] = path[i];
                                i += 1;
                            }
                            n -= removed;
                            cur = k;
                            repaint_field = true;
                        }
                    }
                    // SUPRIMIR -- borra HACIA ADELANTE, al reves que el
                    // retroceso. Son dos teclas porque son dos intenciones.
                    0x86 => {
                        if cur < n {
                            let mut k = cur + 1;
                            while k < n { path[k - 1] = path[k]; k += 1; }
                            n -= 1;
                            repaint_field = true;
                        }
                    }
                    // * PgUp / PgDn -- el historial de la salida.
                    //
                    // Estaban ignoradas "explicitamente", que era honesto pero
                    // inutil: lo que salia por arriba se perdia para siempre, y
                    // en una maquina donde depurar es fotografiar la pantalla,
                    // perder la salida de un batch cuesta un arranque entero.
                    // Ahora suben y bajan la ventana sobre 200 filas guardadas.
                    0x87 => {
                        output.scroll_view(OUT_ROWS as i32 - 1);
                    }
                    0x88 => {
                        output.scroll_view(-(OUT_ROWS as i32 - 1));
                    }
                    // * F12 (0x94) NO esta aqui: se atiende arriba, antes de
                    // preguntar por el foco, porque es del sistema y no de esta
                    // ventana. Ver la conmutacion de la consola de datos.
                    //
                    // El resto de navegacion se ignora, pero EXPLICITAMENTE:
                    // dejarlas caer al comodin las dibujaria como basura.
                    0x89..=0x9F => {}
                    // Todo lo demas imprimible, incluido el Latin-1 alto: la
                    // `n` llega como 0xF1 y la fuente la tiene.
                    c if c >= 0x20 => {
                        if n < PATH_MAX {
                            // Hueco en el cursor y meter ahi: escribir en
                            // medio de una linea es lo normal, no un caso raro.
                            let mut k = n;
                            while k > cur {
                                path[k] = path[k - 1];
                                k -= 1;
                            }
                            path[cur] = c;
                            cur += 1;
                            n += 1;
                            repaint_field = true;
                        }
                    }
                    _ => {}
                }
            }

            // -- Raton --
            // La rueda, primero: mueve el historial de la salida. Es lo que
            // pidio Eddi --"ver y scrollear"-- y funciona con la rueda o con
            // PgUp/PgDn, porque un teclado siempre hay.
            // * La rueda se atiende MAS ABAJO, cuando ya se sabe sobre que
            // ventana esta el puntero. Antes se atendia aqui y siempre movia el
            // historial de salida: con la consola del kernel abierta y encima,
            // girar la rueda desplazaba una rejilla que ni siquiera se veia.
            //
            // Ver `under_pointer`.
            // -- Los botones de la calculadora --
            let button = pos.botones != 0;

            // -- ** UN CLIC EN UN ICONO: dar clic y ya --------------------
            //
            // Se rellena el campo con `run <path>` y se inyecta un Enter. No es
            // un atajo perezoso: es la afirmacion de que **pulsar un icono y
            // teclear su nombre son la misma cosa**, y por eso comparten camino
            // entero -- consola, prestamo de pantalla, eco y vigilante.
            //
            // Solo con la caja Ejecutar DELANTE, y esa condicion no es
            // cosmetica: si hay una ventana encima, el clic es de esa ventana.
            // Un escritorio que lanza programas a traves de lo que hay dibujado
            // encima es un escritorio en el que no se puede confiar al pulsar.
            if button
                && !button_before
                && !calc.visible
                && !data_open
                && !cabina_open
                && !sound_open
            {
                if let Some(i) = launcher.app_at(&p, pos.x, pos.y) {
                    if let Some(app) = launcher.app(i) {
                        let r = app.path();
                        // `run ` + la ruta. Si no cupiera se deja como estaba:
                        // media ruta lanzaria otra cosa, y eso es peor que no
                        // lanzar nada.
                        if 4 + r.len() <= path.len() {
                            path[..4].copy_from_slice(b"run ");
                            path[4..4 + r.len()].copy_from_slice(r);
                            n = 4 + r.len();
                            cur = n;
                            repaint_field = true;
                            if ni < injected.len() {
                                injected[ni] = b'\n';
                                ni += 1;
                            }
                        }
                    }
                }
            }

            if calc.visible && button && !button_before && !calc.waiting {
                if let Some(t) = calc_pad.key_at(pos.x, pos.y) {
                    match t {
                        b'C' => calc.clear(),
                        b'+' => calc.operator(1),
                        b'-' => calc.operator(2),
                        b'*' => calc.operator(3),
                        b'/' => calc.operator(4),
                        b'=' => {
                            if calc.op != 0 && calc.saved_n > 0 && calc.n > 0 {
                                // Lanzar el MOTOR y darle los tres datos por su
                                // consola. Aqui es donde la cara deja de saber
                                // de aritmetica y empieza a saber COBOL.
                                let cap = child_console.as_ref().map(|c| c.cap).unwrap_or(0);
                                if bmo::ejecutar_en(b"cobol/calcgui.bex", cap).is_ok() {
                                    if let Some(cc) = child_console.as_ref() {
                                        cc.write(&calc.saved_path[..calc.saved_n]);
                                        cc.write(b"\n");
                                        cc.write(&[b'0' + calc.op]);
                                        cc.write(b"\n");
                                        cc.write(&calc.input[..calc.n]);
                                        cc.write(b"\n");
                                    }
                                    calc.waiting = true;
                                    resp_n = 0;
                                } else {
                                    paint_status(&p, &run_box, "falta cobol/calcgui.bex", INK_BAD);
                                }
                            }
                        }
                        d => calc.feed(d),
                    }
                    paint_calc(&p, &calc_pad, &calc, calc_hover);
                }
            }
            // -- El raton tambien manda en el foco --
            //
            // Sin esto, dos de los tres modos son decoracion: `click-to-focus`
            // no existiria y `focus-follows-mouse` no tendria quien le dijera
            // por donde va el puntero.
            //
            // * El orden de estas dos preguntas ES el Z-order: Datos se pinta
            // ENCIMA de Ejecutar, asi que se pregunta primero, y un clic en la
            // zona compartida es de la de arriba. `bmo_input::focus` no sabe que
            // ventana tapa a cual y no tiene por que: eso lo sabe el que pinta.
            // * Con TRES ventanas, el orden de las preguntas deja de caber en
            // un `if/else` escrito a mano por pares. Se pregunta primero por la
            // que esta ARRIBA --sea cual sea-- y despues por las demas: un clic
            // en la zona compartida es siempre de la de encima, y eso es una
            // regla, no una lista de casos.
            let at = |v: u8| match v {
                W_DATA => data_open && data_win.contains(pos.x, pos.y),
                W_CABINA => cabina_open && cabina_win.chrome.contains(pos.x, pos.y),
                W_SOUND => sound_open && sound_win.chrome.contains(pos.x, pos.y),
                _ => visible && run_box.contains(pos.x, pos.y),
            };
            let under_pointer = if at(top_before) {
                Some(top_before)
            } else {
                [W_SOUND, W_CABINA, W_DATA, W_RUN]
                    .into_iter()
                    .find(|&v| v != top_before && at(v))
            };
            // -- * LA RUEDA VA A LA VENTANA QUE HAY DEBAJO --
            //
            // Es lo que hace cualquier sistema y lo que la mano espera sin
            // pensarlo: se gira donde se mira. Antes iba SIEMPRE al historial
            // de salida, asi que con la consola del kernel delante la rueda
            // movia una rejilla tapada -- el gesto no hacia nada visible y
            // parecia que la rueda no funcionaba.
            //
            // Sin ventana debajo no se hace nada, y eso tambien es una
            // decision: mandar el giro a la ventana con el foco cuando el
            // puntero esta en el escritorio mueve cosas que no se estan
            // mirando.
            if wheel != 0 {
                match under_pointer {
                    Some(W_CABINA) => {
                        // Positivo es hacia arriba, y en un log "arriba" es
                        // hacia ATRAS en el tiempo: el desplazamiento cuenta
                        // lineas hacia el pasado, asi que suma.
                        let any = bmo::cabina_disponibles();
                        let step = (wheel * 3) as i64;
                        let new = cabina_win.from as i64 + step;
                        cabina_win.from =
                            new.clamp(0, any.saturating_sub(1) as i64) as u64;
                        scene::cabina::paint(&p, &cabina_win);
                    }
                    Some(W_RUN) => {
                        // Tres filas por muesca: una sola se queda corta y una
                        // pagina entera se pasa. Es el paso de un terminal.
                        output.scroll_view(wheel * 3);
                    }
                    // La rueda sobre el arbol de nodos mueve la seleccion. En la
                    // pestana de numeros no hay nada que desplazar: cabe entera.
                    Some(W_DATA) if data_win.view == scene::data::View::Nodes => {
                        // Girar hacia arriba sube por la lista: `wheel` positivo
                        // es hacia arriba y la seleccion de arriba es la menor.
                        let how_many = bmo::estratos::hijos() as usize;
                        data_win.move_sel(-wheel, how_many);
                        scene::data::paint(&p, &data_win);
                    }
                    _ => {}
                }
            }

            // -- El realce de la calculadora --
            //
            // Solo cuando CAMBIA la tecla senalada, y solo si la calculadora se
            // ve y no esta tapada. Al salir de ella el realce se apaga, que es
            // la mitad que se olvida siempre: un boton que se queda encendido
            // cuando ya no lo senalas miente sobre donde esta el raton.
            let hover_now = if calc.visible && top_before == W_RUN {
                calc_pad.key_at(pos.x, pos.y)
            } else {
                None
            };
            if hover_now != calc_hover {
                calc_hover = hover_now;
                if calc.visible {
                    paint_calc(&p, &calc_pad, &calc, calc_hover);
                }
            }

            if let Some(v) = under_pointer {
                // Pasar por encima: solo hace algo en modo `Puntero`, y la
                // guarda esta DENTRO de la politica -- aqui solo se cuenta lo
                // que pasa, no se decide lo que significa.
                if pos.x != ax || pos.y != ay {
                    focus.puntero_en(v);
                }
                // Un clic lo pide en CUALQUIER modo, incluido `Fijo`: lo que
                // ese modo impide es que una ventana se lo tome sin que nadie
                // se lo pida, no que tu se lo des.
                if button && !button_before {
                    focus.clic_en(v);
                }
            }

            // -- * EL RATON SOBRE LA VENTANA DE DATOS --
            //
            // Tres gestos que comparten estructura: los BOTONES de la barra,
            // ARRASTRAR por el asa y ESTIRAR por la esquina. Quien decide cual
            // es el marco, no esto: aqui solo se le cuenta lo que paso.
            if data_open && !data_win.chrome.minimized {
                use scene::chrome::Button;

                // El realce de los botones. Solo cuando CAMBIA -- repintarlo
                // cada fotograma serian 1.700 pixeles de memoria de video sin
                // cache para dejarlo igual, y ademas pisaria el cursor.
                let hover_now = data_win.chrome.button_at(pos.x, pos.y);
                if hover_now != data_win.chrome.hover {
                    data_win.chrome.hover = hover_now;
                    scene::data::paint(&p, &data_win);
                    top_before = W_DATA;
                }

                if button && !button_before {
                    // Un boton se dispara al PULSAR y no al soltar. Es lo que
                    // hace todo el mundo, y con `close` importa: soltar fuera
                    // para arrepentirse no funciona en ningun escritorio, asi
                    // que fingirlo aqui seria inventarse una costumbre.
                    match data_win.chrome.button_at(pos.x, pos.y) {
                        Some(Button::Close) => {
                            data_open = false;
                            focus.close(W_DATA);
                            erase_window(
                                &p, &run_box, data_win.x(), data_win.y(),
                                data_win.width(), data_win.height(), visible,
                            );
                            top_before = W_RUN;
                            uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        }
                        Some(Button::Minimize) => {
                            // Minimizar NO es cerrar: la ventana sigue abierta
                            // y conserva su sitio, su tamano y lo que estuviera
                            // mirando. Se va a su ficha de la barra.
                            let (vx, vy, va, vl) = (
                                data_win.x(), data_win.y(),
                                data_win.width(), data_win.height(),
                            );
                            data_win.chrome.minimized = true;
                            focus.close(W_DATA);
                            erase_window(&p, &run_box, vx, vy, va, vl, visible);
                            top_before = W_RUN;
                            uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                            taskbar_dirty = true;
                        }
                        Some(Button::Maximize) => {
                            let (vx, vy, va, vl) = data_win.chrome.toggle_maximized(&p);
                            // Al restaurar, el hueco que deja hay que
                            // devolverselo al escritorio; al maximizar no sobra
                            // nada, pero borrar el rectangulo viejo entero
                            // cubre los dos casos con una sola regla.
                            erase_window(&p, &run_box, vx, vy, va, vl, visible);
                            uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                            data_win.relayout();
                            scene::data::paint(&p, &data_win);
                            top_before = W_DATA;
                        }
                        None => {
                            // -- * CLIC DENTRO DEL GRAFO --
                            //
                            // El gesto que faltaba: hasta ahora el raton solo
                            // servia para mover la ventana, y una ventana llena
                            // de cajas en la que no se puede pulsar ninguna es
                            // una ventana que parece interactiva y no lo es.
                            let how_many = bmo::estratos::hijos() as usize;
                            match data_win.box_at(pos.x, pos.y, how_many) {
                                // La caja del PADRE: sube un nivel. Es el gesto
                                // que la mano busca sola cuando ya has bajado.
                                Some(i) if i == usize::MAX => {
                                    if bmo::estratos::subir() {
                                        data_win.to_top();
                                        data_win.verified = None;
                                        scene::data::paint(&p, &data_win);
                                        top_before = W_DATA;
                                    }
                                }
                                Some(i) => {
                                    data_win.sel = i;
                                    // El resultado de una verificacion es de UN
                                    // archivo: al cambiar de caja se borra. Si
                                    // no, un `CUADRA` viejo se quedaria debajo
                                    // del nombre de otro.
                                    data_win.verified = None;
                                    // * Ctrl+clic BAJA de una vez, sin tener que
                                    // senalar y pulsar ENTRAR. El clic a secas
                                    // solo senala, porque senalar tiene que
                                    // poder hacerse sin miedo a moverte de sitio.
                                    if ctrl && bmo::estratos::entrar(i as u64) {
                                        data_win.to_top();
                                    }
                                    scene::data::paint(&p, &data_win);
                                    top_before = W_DATA;
                                }
                                None => {
                                    data_win.chrome.grab(pos.x, pos.y);
                                }
                            }
                        }
                    }
                }

                if !button && data_win.chrome.grabbed() {
                    data_win.chrome.release();
                } else if button && data_win.chrome.grabbed() {
                    // El sitio VIEJO hay que borrarlo antes de mover. Si no, la
                    // ventana deja un rastro de copias de si misma: aqui no hay
                    // recorte ni compositor que repinte lo de debajo solo.
                    //
                    // Al ESTIRAR pasa lo mismo pero solo al encoger; borrar el
                    // rectangulo viejo entero cubre los dos casos con una regla
                    // en vez de con dos.
                    let (vx, vy, va, vl) = (
                        data_win.x(), data_win.y(),
                        data_win.width(), data_win.height(),
                    );
                    if data_win.chrome.follow_pointer(&p, pos.x, pos.y) {
                        erase_window(&p, &run_box, vx, vy, va, vl, visible);
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        data_win.relayout();
                        scene::data::paint(&p, &data_win);
                        top_before = W_DATA;
                    }
                }
            }

            // -- ** EL ARRASTRE DE LAS OTRAS DOS VENTANAS --
            //
            // CABINA y Sonido nacieron con `Chrome` --que trae `grab`,
            // `follow_pointer` y `release`-- y **nadie las llamaba**. El
            // resultado en el Ryzen: dos ventanas con barra de titulo, con sus
            // tres botones pintados, y clavadas en el sitio.
            //
            // Es el patron 24 de `BITACORA.md` calcado: la politica escrita y
            // sin lector. Alli fue `es_para` del foco, que existia entera con
            // tests y no se llamaba ni una vez; aqui es el arrastre. Dar el
            // mecanismo NO es cablearlo, y la unica forma de notar la
            // diferencia es ejecutandolo -- por eso salio en metal y no antes.
            //
            // * Y el cableado nacio ANIDADO dentro del `if data_open`, que
            // es el mismo patron una vuelta mas: el lector existia y solo se le
            // daba corriente cuando ESTRATOS estaba abierta y sin minimizar.
            // Con Datos cerrada, las dos ventanas volvian a estar clavadas. El
            // arrastre de una ventana no depende de otra ventana, asi que va
            // aqui, al nivel de las demas.
            if cabina_open && !cabina_win.chrome.minimized {
                if button && !cabina_win.chrome.grabbed() && focus.es_para(W_CABINA)
                    && cabina_win.chrome.on_the_grip(pos.x, pos.y)
                {
                    cabina_win.chrome.grab(pos.x, pos.y);
                } else if !button && cabina_win.chrome.grabbed() {
                    cabina_win.chrome.release();
                } else if button && cabina_win.chrome.grabbed() {
                    // El sitio VIEJO se borra antes de mover: aqui no hay
                    // compositor que repinte lo de debajo, asi que sin esto
                    // la ventana deja un rastro de copias de si misma.
                    let (vx, vy, va, vl) = (
                        cabina_win.chrome.x, cabina_win.chrome.y,
                        cabina_win.chrome.width, cabina_win.chrome.height,
                    );
                    if cabina_win.chrome.follow_pointer(&p, pos.x, pos.y) {
                        erase_window(&p, &run_box, vx, vy, va, vl, visible);
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        scene::cabina::paint(&p, &cabina_win);
                        top_before = W_CABINA;
                    }
                }
            }

            if sound_open && !sound_win.chrome.minimized {
                if button && !sound_win.chrome.grabbed() && focus.es_para(W_SOUND)
                    && sound_win.chrome.on_the_grip(pos.x, pos.y)
                {
                    sound_win.chrome.grab(pos.x, pos.y);
                } else if !button && sound_win.chrome.grabbed() {
                    sound_win.chrome.release();
                } else if button && sound_win.chrome.grabbed() {
                    let (vx, vy, va, vl) = (
                        sound_win.chrome.x, sound_win.chrome.y,
                        sound_win.chrome.width, sound_win.chrome.height,
                    );
                    if sound_win.chrome.follow_pointer(&p, pos.x, pos.y) {
                        erase_window(&p, &run_box, vx, vy, va, vl, visible);
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        scene::sound::paint(
                            &p, &sound_win, sound_cap.is_some(),
                            sound_devices, sound_volume, sound_pressed,
                        );
                        top_before = W_SOUND;
                    }
                }
            }

            // -- ** EL RATON SOBRE UNA CAJA DE APP --
            //
            // Los mismos tres gestos que las ventanas del sistema, y por eso son
            // ocho lineas: el marco ya sabe hacerlos. **Este es el cobro del
            // `chrome.rs`** -- se escribio para que la cuarta ventana saliera
            // gratis, y la cuarta ventana resulta ser un programa entero.
            //
            // Va DESPUES de las ventanas del sistema y antes de las fichas: una
            // app en su caja esta por delante de ellas, asi que su clic manda.
            {
                use scene::chrome::Button;

                if button && !button_before {
                    if let Some(i) = table.at(pos.x, pos.y) {
                        // El realce se pone aunque no se pulse: si no, los tres
                        // botones de una app serian los unicos del escritorio
                        // que no se encienden al pasar por encima.
                        let gesture = table.get_mut(i).and_then(|s| s.chrome.button_at(pos.x, pos.y));
                        match gesture {
                            // ** CERRAR NO MATA A LA APP: le quita la caja.
                            //
                            // Matar un proceso ajeno por ser el DIRECTOR seria
                            // `root` con otro nombre, en el sistema cuya primera
                            // clausula dice que la autoridad no se hereda. Matar
                            // se hara con el handle que devolvio LANZARLA --
                            // paso 3 del plan-- y no desde aqui.
                            Some(Button::Close) => {
                                if let Some((vx, vy, va, vl)) = table.close(i) {
                                    erase_window(&p, &run_box, vx, vy, va, vl, visible);
                                    uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                                    for s in table.iter_mut() {
                                        s.repaint_all();
                                    }
                                }
                            }
                            Some(Button::Minimize) => {
                                if let Some(s) = table.get_mut(i) {
                                    let (vx, vy, va, vl) =
                                        (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
                                    s.chrome.minimized = true;
                                    erase_window(&p, &run_box, vx, vy, va, vl, visible);
                                    uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                                }
                            }
                            // ** PANTALLA COMPLETA = QUE NO SE DIBUJE EL BORDE.
                            //
                            // Y aqui todavia no: maximizar da el hueco entero
                            // bajo la barra, que es lo que hacen las demas. Lo
                            // que NO pasa --ni pasara-- es entregarle el
                            // aparato: se sigue componiendo, asi que Alt+Tab
                            // sigue y `Ctrl+Alt+ESC` sigue. Un juego colgado se
                            // cierra con el teclado y no con el boton de reset.
                            Some(Button::Maximize) => {
                                if let Some(s) = table.get_mut(i) {
                                    let (vx, vy, va, vl) = s.chrome.toggle_maximized(&p);
                                    erase_window(&p, &run_box, vx, vy, va, vl, visible);
                                    uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                                    s.repaint_all();
                                }
                            }
                            None => {
                                if let Some(s) = table.get_mut(i) {
                                    s.chrome.grab(pos.x, pos.y);
                                }
                            }
                        }
                    }
                }

                // Arrastrar y estirar. El sitio VIEJO se borra antes de mover:
                // aqui no hay nadie que repinte lo de debajo, asi que sin esto
                // la ventana deja un rastro de copias de si misma.
                for i in 0..scene::surface::MAX {
                    let Some(s) = table.get_mut(i) else { continue };
                    if !s.chrome.grabbed() {
                        continue;
                    }
                    if !button {
                        s.chrome.release();
                        continue;
                    }
                    let (vx, vy, va, vl) = (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
                    if s.chrome.follow_pointer(&p, pos.x, pos.y) {
                        s.repaint_all();
                        erase_window(&p, &run_box, vx, vy, va, vl, visible);
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                    }
                }
            }

            // -- Clic en una FICHA de la barra: traer esa ventana --
            //
            // Es la mitad que hace que minimizar signifique algo. Sin esto, el
            // boton de minimizar seria uno de "desaparece para siempre".
            // * Una ficha hace SIEMPRE lo mismo: **trae su ventana y le da el
            // foco**, este minimizada, escondida o simplemente detras.
            //
            // La primera version solo actuaba `si estaba minimized` o `si
            // estaba escondida`, y por eso pulsar la ficha de una ventana que
            // ya se veia no hacia nada. En el Ryzen eso se lee como *"la barra
            // se olvida de mis clics"*, y con razon: un control que a veces
            // responde y a veces no es peor que uno que no esta.
            if button && !button_before && pos.y < TASKBAR_H {
                if let Some(i) = scene::chip_at(pos.x, pos.y, 2) {
                    if i == 1 && data_open {
                        // Estaba minimizada o no, da igual: acaba visible,
                        // encajada, con el foco y delante.
                        data_win.chrome.minimized = false;
                        data_win.chrome.fit(&p);
                        focus.open(W_DATA);
                        focus.clic_en(W_DATA);
                        data_win.relayout();
                        scene::data::paint(&p, &data_win);
                        top_before = W_DATA;
                        taskbar_dirty = true;
                    } else if i == 0 {
                        if !visible {
                            visible = true;
                        }
                        focus.open(W_RUN);
                        focus.clic_en(W_RUN);
                        uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                        top_before = W_RUN;
                        taskbar_dirty = true;
                    }
                }
            }
            button_before = button;

            // -- * El foco arrastra el Z-order --
            //
            // Levantar una ventana no da el teclado --eso es mezclar dos cosas y
            // es el error clasico de un gestor de ventanas--, pero **al reves si
            // vale**: la que tiene el teclado tiene que verse. Aqui no hay
            // recorte, asi que "verse" es pintarse la ultima.
            //
            // Sin esto, Alt+Tab a Ejecutar con Datos delante dejaria el teclado
            // en una linea tapada: escribirias sin ver nada. Es exactamente el
            // fallo que se acaba de arreglar, del reves.
            let top = if cabina_open && focus.es_para(W_CABINA) {
                W_CABINA
            } else if data_open && focus.es_para(W_DATA) {
                W_DATA
            } else {
                W_RUN
            };
            if top != top_before {
                match top {
                    W_CABINA => scene::cabina::paint(&p, &cabina_win),
                    W_DATA => scene::data::paint(&p, &data_win),
                    // Sin guarda de `visible`: `uncover` ya no hace nada si
                    // la caja esta escondida, y una guarda repetida es una que
                    // puede quedarse desincronizada de la funcion.
                    _ => uncover(&p, &run_box, visible, &mut output, &mut repaint_field),
                }
                top_before = top;
            }

            // El cursor ya no se borra aqui: se pone al final del fotograma y
            // se quita al principio del siguiente, con lo que habia debajo
            // guardado. Aqui solo se apunta donde esta.
            ax = pos.x;
            ay = pos.y;

            // * Aqui se pintaban el PULSOMETRO y el testigo de botones. Fuera
            // el 2026-08-04, con los seis parches de medida: contestaban
            // "llegan informes del raton?" y esa pregunta la contesta ya el
            // propio puntero moviendose. Ver la nota del escritorio.
        }

        // -- Drenar la salida de los hijos --
        //
        // Con tope por fotograma. Un programa que escupe sin parar podria
        // quedarse con el bucle entero y congelar el cursor: es preferible que
        // la salida vaya un poco por detras a que el escritorio deje de
        // responder. Lo que no se lea ahora sigue en el anillo del kernel.
        if let Some(c) = child_console.as_ref() {
            let mut buf = [0u8; 8];
            let mut frames = 0;
            while frames < 64 {
                let read_bytes = c.read(&mut buf);
                if read_bytes == 0 {
                    break;
                }
                if calc.waiting {
                    // Todo lo que escriba el motor es la respuesta: el
                    // programa no imprime prompts a proposito.
                    for &b in &buf[..read_bytes] {
                        if b == b'\n' {
                            if resp_n > 0 {
                                calc.input = [0; 20];
                                let k = resp_n.min(calc.input.len());
                                calc.input[..k].copy_from_slice(&resp[..k]);
                                calc.n = k;
                                calc.saved_n = 0;
                                calc.op = 0;
                                calc.waiting = false;
                                // * El cursor SE APARTA antes de pintar aqui.
                                //
                                // Este es el unico pintado del bucle que no
                                // dispara la ENTRADA: lo dispara el HIJO al
                                // contestar. Asi que puede caer en un fotograma
                                // con `will_paint` en falso -- o sea con el
                                // puntero todavia en pantalla y lo que hay
                                // debajo ya guardado. Pintar encima **caduca**
                                // ese guardado, y el `lift` de la vuelta
                                // siguiente devolveria los pixeles viejos
                                // encima del resultado recien escrito: un
                                // rectangulo fantasma sobre la calculadora.
                                //
                                // `lift` es idempotente --si no esta puesto no
                                // hace nada--, asi que llamarlo aqui no cuesta
                                // nada en los fotogramas que ya lo apartaron.
                                save_under.lift(&p);
                                paint_calc(&p, &calc_pad, &calc, calc_hover);
                            }
                        } else if resp_n < resp.len() && b >= 0x20 {
                            resp[resp_n] = b;
                            resp_n += 1;
                        }
                    }
                } else {
                    output.text(&buf[..read_bytes]);
                }
                frames += 1;
            }
        }
        // * Y solo en un fotograma que haya apartado el cursor. Un hijo que
        // escribe no es motivo suficiente: pintar aqui dejaria el puntero
        // enterrado bajo la rejilla y, al quitarlo, devolveria pixeles viejos
        // encima de lo recien escrito. `dirty` se queda puesto y la vuelta
        // siguiente ya empieza sabiendo que hay que pintar.
        if output.dirty && will_paint {
            // Se pinta solo si se ve; el contenido sigue acumulandose oculto,
            // asi que al invocar la ventana esta todo lo que paso mientras.
            //
            // * Y NO si la consola de datos esta ARRIBA. Sin este guardia, el
            // fotograma siguiente repintaria la rejilla POR DEBAJO y la
            // dibujaria encima de la ventana de datos, dejandola a trozos. La
            // salida no se pierde: `dirty` se queda puesto y se pinta entera
            // cuando esta ventana vuelva a estar arriba.
            //
            // Y es ARRIBA, no ABIERTA: con Datos abierta pero detras, la
            // rejilla se ve y tiene que seguir escribiendose.
            if visible && top_before != W_DATA && !switcher_painted {
                paint_output(&p, &run_box, &output);
                output.dirty = false;
            } else if !visible {
                output.dirty = false;
            }
        }

        // -- Las FICHAS de la barra --
        //
        // Se repintan solo cuando algo cambia de estado. Son la lista de lo que
        // hay abierto, y la unica forma de volver a una ventana minimizada.
        //
        // * Lo que las ensucia se calcula AQUI, comparando el estado con el del
        // fotograma anterior, en vez de poner `taskbar_dirty = true` en los seis
        // sitios que cambian algo. Un `sucio` que hay que acordarse de poner es
        // un `sucio` que un dia no se pone, y entonces la barra ensena un
        // estado viejo sin que nada falle -- el peor tipo de fallo de interfaz.
        let taskbar_state = (visible, top_before, data_open, data_win.chrome.minimized);
        if taskbar_state != taskbar_state_before {
            taskbar_state_before = taskbar_state;
            taskbar_dirty = true;
        }
        if taskbar_dirty && will_paint {
            scene::paint_chip(&p, 0, "Ejecutar", ACCENT, visible && top_before == W_RUN, !visible);
            if data_open {
                scene::paint_chip(
                    &p, 1, "ESTRATOS", 0x0034_D399,
                    top_before == W_DATA, data_win.chrome.minimized,
                );
            } else {
                // Cerrada: su hueco vuelve al color de la barra. Una ficha que
                // se queda tras cerrar la ventana promete algo que ya no esta.
                let (fx, fy, fw, fh) = scene::chip_box(1);
                p.rect(fx, fy, fw, fh, TASKBAR);
            }
            taskbar_dirty = false;
        }

        // El parpadeo del cursor de escritura. Solo repinta cuando cambia de
        // estado -- repintar el campo cada vuelta seria reescribir la ruta
        // miles de veces por segundo para que se vea igual.
        //
        // * El contador se REINICIA con cada tecla (ver el manejador). Antes
        // era `frames % BLINK`, un reloj que corria solo: si te ponias a
        // escribir justo cuando tocaba apagarlo, el cursor desaparecia a mitad
        // de la palabra y no volvia hasta la siguiente vuelta entera. Un
        // cursor que se esconde mientras escribes es lo contrario de lo que
        // un cursor existe para decir.
        since_key = since_key.wrapping_add(1);
        if since_key >= BLINK {
            since_key = 0;
            caret = !caret;
            repaint_field = true;
        }
        if repaint_field
            && will_paint
            && visible
            && top_before != W_DATA
            && !switcher_painted
        {
            paint_field(&p, &run_box, &path[..n], cur, caret);
        }

        // * UNA sola vez, al cerrar el primer fotograma entero. Con esto, las
        // ultimas palabras que guarda el kernel dicen DONDE murio sin tener que
        // adivinarlo:
        //
        //   "reclamo pantalla y entrada"  -> murio en el arranque o en la intro
        //   "escritorio pintado"          -> murio sin cerrar el primer cuadro
        //   "primer fotograma completo"   -> murio ya en el bucle
        //
        // Tres mensajes que ya existian mas este, y el diagnostico deja de ser
        // una teoria. Cuesta una linea en el log y se dice una vez en la vida
        // del proceso.
        // -- ** LAS APPS, COMPUESTAS --
        //
        // Va **al final** y por el mismo motivo que el cursor va detras: lo que
        // se pinta al final es lo que queda encima. Una app en su caja esta por
        // delante de las ventanas del sistema, y el unico que se le pone encima
        // es el puntero del raton.
        //
        // El hueco de las que murieron se devuelve ANTES de componer las vivas:
        // borrar despues taparia a una ventana que si esta.
        if will_paint {
            for &(vx, vy, va, vl) in dead_boxes[..dead].iter() {
                erase_window(&p, &run_box, vx, vy, va, vl, visible);
            }
            if dead > 0 {
                uncover(&p, &run_box, visible, &mut output, &mut repaint_field);
                // Lo que quedara debajo de la que se fue tiene que volver a
                // pintarse: `erase_window` devuelve el FONDO, no las ventanas.
                for s in table.iter_mut() {
                    s.repaint_all();
                }
            }
            table.compose(&p);
        }

        if frames == 1 {
            bmo::consola("primer fotograma completo\n");
        }

        // -- ** LAS VITALES SE REPINTAN SOLAS, y eso es lo que las hace vistas
        //
        // Aqui, al final del fotograma y ANTES del cursor: lo que se pinte
        // despues del cursor le come el save-under.
        //
        // Cada 15 vueltas y no cada una, por dos razones que van juntas:
        //
        //   * Un panel de ~500x400 repintado a 60 fps son megabytes por segundo
        //     de volcado por unos numeros que cambian despacio. Es justo el
        //     derroche que el troceado por regiones acaba de quitar en otro
        //     sitio.
        //   * Y los numeros del CPU son MEDIDAS POR DIFERENCIA: con intervalos
        //     de 16 ms la ventana es tan corta que el resultado tiembla. A 15
        //     fotogramas son ~250 ms, que es donde un vatio se lee quieto.
        //
        // O sea que refrescar mas no daria mas informacion: daria la misma
        // temblando.
        if (cpu_open || mem_open) && frames % 15 == 0 {
            if cpu_open {
                scene::vitals::paint(&p, &cpu_win);
            }
            if mem_open {
                scene::vitals::paint(&p, &mem_win);
            }
        }

        // -- El cursor del raton, ENCIMA de todo y lo ultimo --
        //
        // Aqui ya no queda nada por pintar en este fotograma, asi que lo que se
        // guarda debajo es lo definitivo. Ponerlo antes obligaria a que cada
        // ventana supiera esquivarlo -- que es justo lo que no se puede pedir a
        // una ventana que todavia no existe.
        if ax != u32::MAX {
            // * QUE ESTA DICIENDO EL PUNTERO.
            //
            // Se decide aqui, al final del fotograma, porque es aqui donde ya
            // se sabe todo lo que paso en el: que ventana quedo arriba, donde
            // acabo el raton y si la calculadora esta abierta.
            //
            // El orden de las preguntas es el Z-order otra vez: lo que esta
            // encima manda. Un boton de la calculadora tapado por la consola
            // del kernel no puede pedir la mano -- senalaria algo que no se
            // puede pulsar, que es peor que no senalar nada.
            let shape = if calc.visible
                && top_before == W_RUN
                && calc_pad.key_at(ax, ay).is_some()
            {
                scene::cursor::Shape::Hand
            } else if visible && top_before == W_RUN && run_box.on_field(ax, ay) {
                scene::cursor::Shape::Beam
            } else {
                scene::cursor::Shape::Arrow
            };
            save_under.place(&p, ax, ay, shape);
        }

        // * Y ahora EMPUJARLO a la pantalla.
        //
        // El framebuffer esta mapeado en write-combining: el CPU acumula las
        // escrituras y las suelta cuando el bufer se llena. Sin esta linea, lo
        // pintado en este fotograma se queda esperando a que alguien escriba
        // mas -- y el sintoma es exactamente el que aparecio en el Ryzen:
        // teclear no pintaba nada hasta que se movia el raton, porque mover el
        // raton era lo que llenaba el bufer.
        //
        // Una instruccion, una vez por fotograma, al final de todo. Ver
        // `Pantalla::vaciar`.
        p.vaciar();

        bmo::yield_screen();
    }
}

/// Un panico aqui no puede tumbar nada mas que a este proceso: lo dice y sale
/// por la puerta normal. El kernel revoca sus capabilities --incluidas la
/// pantalla y la entrada-- y sigue vivo.
///
/// * **Y DICE DONDE.** Esto era `_info` --el guion bajo delata el bug-- y
/// escribia "panico en el compositor" y nada mas. El escritorio se moria al
/// arrancar, dejaba la maquina en el shell de Ring 0, y el unico que sabia el
/// archivo y la linea era este manejador, que los tiraba.
///
/// Se escribe en la consola del KERNEL a proposito: cuando esto corre, la
/// pantalla puede estar reclamada por nosotros y a medio pintar, asi que el
/// unico sitio donde el mensaje sobrevive es el panel del kernel -- que es
/// justo donde se queda la maquina cuando el escritorio no arranca.
/// Un buffer de pila que sabe recibir un `write!`. Es lo minimo para poder
/// pedirle a `PanicInfo` su MENSAJE, que es texto formateado y no una `&str`.
struct Line {
    buf: [u8; 192],
    n: usize,
}

impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            if self.n >= self.buf.len() {
                break; // se trunca; media linea dice mas que ninguna
            }
            self.buf[self.n] = b;
            self.n += 1;
        }
        Ok(())
    }
}

#[panic_handler]
fn panic_report(info: &core::panic::PanicInfo) -> ! {
    bmo::consola("panico en el compositor\n");
    // ** EL MENSAJE, y no solo el sitio.
    //
    // Esto decia archivo y linea, y con eso el 2026-08-09 se supo que el
    // escritorio moria en `main.rs:2744` -- que es un `&path[..n]`. Pero
    // saber la LINEA de un corte de rebanada no dice **cual era el numero**,
    // y sin el numero hay que deducir por que `n` valdria mas de 128
    // leyendo los veinte sitios que lo tocan. Se leyeron: ninguno puede.
    //
    // El mensaje de Rust lo trae dentro: *"range end index 131 out of range
    // for slice of length 128"*. Una linea que convierte una tarde de
    // lectura en un vistazo -- y cuesta un buffer de pila de 192 bytes que
    // solo existe cuando el proceso ya se esta muriendo.
    {
        use core::fmt::Write;
        let mut r = Line { buf: [0u8; 192], n: 0 };
        let _ = write!(r, "{}", info.message());
        if r.n > 0 {
            if let Ok(s) = core::str::from_utf8(&r.buf[..r.n]) {
                bmo::consola("  ");
                bmo::consola(s);
                bmo::consola("\n");
            }
        }
    }
    if let Some(l) = info.location() {
        bmo::consola("  en ");
        bmo::consola(l.file());
        bmo::consola(":");
        // El numero a mano: aqui no hay `format!` ni asignador.
        let mut buf = [0u8; 12];
        let mut n = 0usize;
        let mut v = l.line();
        if v == 0 {
            buf[0] = b'0';
            n = 1;
        } else {
            let mut d = [0u8; 12];
            let mut k = 0;
            while v > 0 {
                d[k] = b'0' + (v % 10) as u8;
                v /= 10;
                k += 1;
            }
            while k > 0 {
                k -= 1;
                buf[n] = d[k];
                n += 1;
            }
        }
        if let Ok(s) = core::str::from_utf8(&buf[..n]) {
            bmo::consola(s);
        }
        bmo::consola("\n");
    }
    bmo::salir()
}

