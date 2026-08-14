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
mod desktop;
mod scene;
mod commands;
mod text;
mod watch;

use scene::calc::paint_calc;
use scene::output::{paint_output, Output, INK_GOOD, INK_ECHO, INK_ERR, INK_PLAIN};
use scene::*;
use commands::complete::{complete, file_error_reason};
use commands::reports::{report_autopsy, report_cpu, report_memory, report_system};
use commands::*;
use text::{decimal, is_dot_entry};
use desktop::{W_CABINA, W_CPU, W_DATA, W_MEM, W_RUN, W_SOUND, BLINK};
use watch::{watch_run, Run};


// -- El programa ---------------------------------------------------------


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
    // Reclamar la maquina, decir lo que paso y pintar el primer cuadro son 310
    // lineas que ocurren UNA vez, y vivian en el mismo ambito que las 52
    // variables de un bucle que no termina. Ahora son `desktop::boot`.
    //
    // `p` e `input` vuelven como bindings sueltos y NO como campos: `lend_screen`
    // se los lleva POR VALOR y los devuelve, asi que tienen que poder moverse.
    // Ver la cabecera de `desktop/mod.rs`.
    let (mut p, mut input, mut dsk) = desktop::boot();
    loop {
        // -- Termino el programa que se lanzo? Entonces, a guardarlo --
        //
        // 71 lineas que estaban aqui dentro. Se fueron ENTERAS a
        // `watch.rs`, sin tocar una coma de su logica.
        watch_run(&mut dsk.out.run, &dsk.out.console, &mut dsk.out.grid);

        // ** MURIO ALGO? Entonces la autopsia ya esta escrita, y se guarda.
        //
        // Comparar un entero por vuelta cuesta nada; leer el informe solo pasa
        // cuando de verdad hubo un fallo. Y avisar en la barra importa tanto
        // como guardarlo: un fichero que nadie sabe que existe es un fichero
        // que no se manda.
        if save_autopsies(&mut dsk.out.faults_seen) {
            paint_status(
                &p,
                &dsk.run_box,
                "fallo de Ring 3 guardado en datos/fallos.txt -- escribe `fallo`",
                INK_BAD,
            );
        }

        dsk.tick.frames = dsk.tick.frames.wrapping_add(1);
        dsk.tick.repaint_field = false;

        // -- * ALGUIEN OFRECE UNA SUPERFICIE? --
        //
        // Se pregunta al kernel una vez por vuelta y casi siempre dice que no.
        // Es el precio de no tener que avisar: una app ofrece cuando le viene
        // bien --puede ser en su primer fotograma o en el mil-- y el DIRECTOR se
        // entera **mirando**, no porque nadie le mande un mensaje. Una operacion
        // que ya existia y ninguna cola nueva.
        let mut born = false;
        if dsk.table.collect(&p) {
            born = true;
        }
        // Y las que se quedaron sin dueno. Va ANTES de pintar nada: la ventana
        // de una app muerta tiene que desaparecer en el mismo fotograma en que
        // se sabe, no en el siguiente.
        let dead = dsk.table.reap_dead(&mut dsk.tick.dead_boxes);

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
        dsk.tick.will_paint = dsk.out.grid.dirty
            || dsk.field.since_key + 1 >= BLINK
            || born
            || dead > 0
            || dsk.table.has_new();

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
            dsk.field.cur = dsk.field.cur.min(dsk.field.n);

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
            for k in 0..dsk.field.ni.min(keys.len()) {
                keys[nt] = dsk.field.injected[k];
                nt += 1;
            }
            dsk.field.ni = 0;
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

            dsk.tick.will_paint |= nt > 0
                || wheel != 0
                || pos.x != dsk.tick.ax
                || pos.y != dsk.tick.ay
                || (pos.botones != 0) != dsk.tick.button_before
                || alt_alone != dsk.win.alt_before
                || combo != dsk.tick.combo_before;

            // A partir de aqui se PINTA, asi que el cursor se aparta.
            if dsk.tick.will_paint {
                dsk.save_under.lift(&p);
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
            if !alt_alone && dsk.win.alt_before && dsk.win.switcher_painted {
                dsk.win.focus.soltar_conmutador();
                let (bx, by, ba, bh) = scene::switcher::area(&p, dsk.win.focus.abiertas());
                for fy in 0..bh {
                    for fx in 0..ba {
                        let (x, y) = (bx + fx, by + fy);
                        p.punto(x, y, scene_color(&dsk.run_box, dsk.win.visible, x, y, p.alto));
                    }
                }
                dsk.win.switcher_painted = false;
                // Lo que tapaba vuelve a pintarse entero, **de abajo arriba**:
                // es el unico orden que deja la pantalla como estaba. Y quien
                // va arriba lo acaba de decidir el Alt que se solto.
                //
                // * Con tres ventanas esto se escribe como lo que es: pintar
                // TODAS las abiertas, y la que tiene el foco la ULTIMA. La
                // version de dos ventanas enumeraba los casos a mano, y con
                // tres eso son seis ramas que dicen una sola regla.
                let top_now = if dsk.win.mem_open && dsk.win.focus.es_para(W_MEM) {
                    W_MEM
                } else if dsk.win.cpu_open && dsk.win.focus.es_para(W_CPU) {
                    W_CPU
                } else if dsk.win.sound_open && dsk.win.focus.es_para(W_SOUND) {
                    W_SOUND
                } else if dsk.win.cabina_open && dsk.win.focus.es_para(W_CABINA) {
                    W_CABINA
                } else if dsk.win.data_open && dsk.win.focus.es_para(W_DATA) {
                    W_DATA
                } else {
                    W_RUN
                };
                let mut paint_one = |v: u8, repintar: &mut bool, sal: &mut scene::output::Output| {
                    match v {
                        W_CABINA if dsk.win.cabina_open => {
                            scene::cabina::paint(&p, &dsk.win.cabina)
                        }
                        W_DATA if dsk.win.data_open => scene::data::paint(&p, &dsk.win.data),
                        // Las vitales son VISTAS: se repintan cada vez que les
                        // toca turno, que es lo que las diferencia de `info`.
                        W_CPU if dsk.win.cpu_open => scene::vitals::paint(&p, &dsk.win.cpu),
                        W_MEM if dsk.win.mem_open => scene::vitals::paint(&p, &dsk.win.mem),
                        W_SOUND if dsk.win.sound_open => scene::sound::paint(
                            &p,
                            &dsk.win.sound,
                            dsk.snd.cap.is_some(),
                            dsk.snd.devices,
                            dsk.snd.volume,
                            dsk.snd.pressed,
                        ),
                        W_RUN => uncover(&p, &dsk.run_box, dsk.win.visible, sal, repintar),
                        _ => {}
                    }
                };
                for v in [W_RUN, W_DATA, W_CABINA, W_SOUND] {
                    if v != top_now {
                        paint_one(v, &mut dsk.tick.repaint_field, &mut dsk.out.grid);
                    }
                }
                paint_one(top_now, &mut dsk.tick.repaint_field, &mut dsk.out.grid);
                dsk.win.top_before = top_now;
            }
            dsk.win.alt_before = alt_alone;
            if combo && !dsk.tick.combo_before {
                dsk.tick.key_during_combo = false;
            }
            if !combo && dsk.tick.combo_before && !dsk.tick.key_during_combo {
                dsk.win.visible = !dsk.win.visible;
                if dsk.win.visible {
                    // Esconderla y volver a invocarla es cerrarla y abrirla
                    // para el foco. Sin esto, Alt+Tab llevaria el teclado a una
                    // ventana que no esta en la pantalla: escribirias en algo
                    // invisible, que es la peor forma de perder una linea.
                    dsk.win.focus.open(W_RUN);
                    uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    paint_status(&p, &dsk.run_box, "listo", INK_DIM);
                } else {
                    dsk.win.focus.close(W_RUN);
                    erase_box(&p, &dsk.run_box);
                }
            }
            dsk.tick.combo_before = combo;

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
                        dsk.win.focus.conmutar_atras();
                    } else {
                        dsk.win.focus.conmutar();
                    }
                    scene::switcher::paint(
                        &p,
                        dsk.win.focus.lista(),
                        dsk.win.focus.pointed_index(),
                        dsk.win.focus.modo().name(),
                    );
                    dsk.win.switcher_painted = true;
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
                    dsk.win.focus.poner_modo(dsk.win.focus.modo().next());
                    if dsk.win.switcher_painted {
                        scene::switcher::paint(
                            &p,
                            dsk.win.focus.lista(),
                            dsk.win.focus.pointed_index(),
                            dsk.win.focus.modo().name(),
                        );
                    } else if dsk.win.visible {
                        // Cambiarlo sin el conmutador abierto tambien tiene que
                        // verse: un modo que cambia en silencio se descubre
                        // cuando el teclado ya se fue a otra ventana.
                        paint_status(&p, &dsk.run_box, dsk.win.focus.modo().nombre_largo(), ACCENT);
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
                    match dsk.win.focus.pointed_at() {
                        Some(W_DATA) if dsk.win.data_open && !dsk.win.data.chrome.minimized => {
                            let (vx, vy, va, vl) = (
                                dsk.win.data.x(), dsk.win.data.y(),
                                dsk.win.data.width(), dsk.win.data.height(),
                            );
                            let cambio = if fit {
                                dsk.win.data.chrome.snap(&p, heading)
                            } else {
                                dsk.win.data.chrome.push(&p, heading)
                            };
                            if cambio {
                                erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                                uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                                // Encajar CAMBIA el tamano, asi que las cajas del
                                // grafo hay que recolocarlas: sin esto la ventana
                                // mide una cosa y su contenido sigue midiendo otra.
                                dsk.win.data.relayout();
                                scene::data::paint(&p, &dsk.win.data);
                                dsk.win.top_before = W_DATA;
                                moved = true;
                            }
                        }
                        Some(W_CABINA) if dsk.win.cabina_open && !dsk.win.cabina.chrome.minimized => {
                            let (vx, vy, va, vl) = (
                                dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
                                dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height,
                            );
                            let cambio = if fit {
                                dsk.win.cabina.chrome.snap(&p, heading)
                            } else {
                                dsk.win.cabina.chrome.push(&p, heading)
                            };
                            if cambio {
                                erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                                uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                                scene::cabina::paint(&p, &dsk.win.cabina);
                                dsk.win.top_before = W_CABINA;
                                moved = true;
                            }
                        }
                        Some(W_SOUND) if dsk.win.sound_open && !dsk.win.sound.chrome.minimized => {
                            let (vx, vy, va, vl) = (
                                dsk.win.sound.chrome.x, dsk.win.sound.chrome.y,
                                dsk.win.sound.chrome.width, dsk.win.sound.chrome.height,
                            );
                            let cambio = if fit {
                                dsk.win.sound.chrome.snap(&p, heading)
                            } else {
                                dsk.win.sound.chrome.push(&p, heading)
                            };
                            if cambio {
                                erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                                uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                                scene::sound::paint(
                                    &p, &dsk.win.sound, dsk.snd.cap.is_some(),
                                    dsk.snd.devices, dsk.snd.volume, dsk.snd.pressed,
                                );
                                dsk.win.top_before = W_SOUND;
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
                    if moved && dsk.win.switcher_painted {
                        scene::switcher::paint(
                            &p,
                            dsk.win.focus.lista(),
                            dsk.win.focus.pointed_index(),
                            dsk.win.focus.modo().name(),
                        );
                    }
                    continue;
                }
                // Cualquier tecla durante el combo lo convierte en AltGr y
                // cancela el toque: el usuario estaba escribiendo, no llamando.
                if combo {
                    dsk.tick.key_during_combo = true;
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
                    Some(!dsk.win.data_open)
                } else if c == 0x1B && dsk.win.data_open && dsk.win.focus.es_para(W_DATA) {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_data {
                    dsk.win.data_open = open;
                    if open {
                        // Abrir es decirselo al foco y ya: en modo `Fijo` la
                        // ventana aparece y NO se lleva el teclado, y quien
                        // decide eso es la politica, no esta tecla.
                        dsk.win.focus.open(W_DATA);
                        scene::data::paint(&p, &dsk.win.data);
                        dsk.win.top_before = if dsk.win.focus.es_para(W_DATA) { W_DATA } else { W_RUN };
                        // En `Fijo` se ha pintado encima de una caja que sigue
                        // teniendo el teclado: hay que devolverla arriba.
                        if dsk.win.top_before == W_RUN {
                            uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                        }
                    } else {
                        // Al cerrarla hay que devolver el fondo Y repintar
                        // lo que tapaba: la caja de Ejecutar esta debajo.
                        dsk.win.focus.close(W_DATA);
                        erase_window(
                            &p, &dsk.run_box, dsk.win.data.x(), dsk.win.data.y(),
                            dsk.win.data.width(), dsk.win.data.height(), dsk.win.visible,
                        );
                        dsk.win.top_before = W_RUN;
                        uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
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
                    Some(!dsk.win.cpu_open)
                } else if c == 0x1B && dsk.win.cpu_open && !dsk.win.mem_open {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_cpu {
                    dsk.win.cpu_open = open;
                    if open {
                        dsk.win.focus.open(W_CPU);
                        scene::vitals::paint(&p, &dsk.win.cpu);
                    } else {
                        dsk.win.focus.close(W_CPU);
                        erase_window(
                            &p, &dsk.run_box, dsk.win.cpu.chrome.x, dsk.win.cpu.chrome.y,
                            dsk.win.cpu.chrome.width, dsk.win.cpu.chrome.height, dsk.win.visible,
                        );
                        uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    }
                    continue;
                }
                let toggle_mem = if c == 0x90 {
                    Some(!dsk.win.mem_open)
                } else if c == 0x1B && dsk.win.mem_open {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_mem {
                    dsk.win.mem_open = open;
                    if open {
                        dsk.win.focus.open(W_MEM);
                        scene::vitals::paint(&p, &dsk.win.mem);
                    } else {
                        dsk.win.focus.close(W_MEM);
                        erase_window(
                            &p, &dsk.run_box, dsk.win.mem.chrome.x, dsk.win.mem.chrome.y,
                            dsk.win.mem.chrome.width, dsk.win.mem.chrome.height, dsk.win.visible,
                        );
                        uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    }
                    continue;
                }

                // -- F11: la consola del KERNEL --
                //
                // Calcada de F12 y por los mismos motivos: se atiende ANTES de
                // preguntar por el foco, porque un atajo que solo funciona si ya
                // estas dentro de la ventana no sirve para abrirla.
                let toggle_klog = if c == 0x93 {
                    Some(!dsk.win.cabina_open)
                } else if c == 0x1B && dsk.win.cabina_open {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_klog {
                    dsk.win.cabina_open = open;
                    if open {
                        // Se abre SIEMPRE por lo ultimo, que es lo que se quiere
                        // ver el 90% de las veces. Para ir al arranque estan
                        // RePag/AvPag.
                        dsk.win.cabina.from = 0;
                        dsk.win.focus.open(W_CABINA);
                        scene::cabina::paint(&p, &dsk.win.cabina);
                        dsk.win.top_before = if dsk.win.focus.es_para(W_CABINA) { W_CABINA } else { W_RUN };
                        if dsk.win.top_before == W_RUN {
                            uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                        }
                    } else {
                        dsk.win.focus.close(W_CABINA);
                        erase_window(
                            &p, &dsk.run_box, dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
                            dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height, dsk.win.visible,
                        );
                        dsk.win.top_before = W_RUN;
                        uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                        // Si Datos estaba abierta debajo, vuelve a verse.
                        if dsk.win.data_open {
                            scene::data::paint(&p, &dsk.win.data);
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
                    Some(!dsk.win.sound_open)
                } else if c == 0x1B && dsk.win.sound_open && dsk.win.focus.es_para(W_SOUND) {
                    Some(false)
                } else {
                    None
                };
                if let Some(open) = toggle_sound {
                    dsk.win.sound_open = open;
                    if open {
                        // Puede fallar, y entonces la ventana lo DICE en vez de
                        // pintar un volumen que no manda sobre nada.
                        dsk.snd.cap = bmo::Sonido::claim();
                        dsk.snd.devices = match &dsk.snd.cap {
                            Some(s) => {
                                s.volumen(dsk.snd.volume);
                                s.aparatos()
                            }
                            None => 0,
                        };
                        dsk.snd.pressed = None;
                        dsk.win.focus.open(W_SOUND);
                        scene::sound::paint(
                            &p, &dsk.win.sound, dsk.snd.cap.is_some(),
                            dsk.snd.devices, dsk.snd.volume, dsk.snd.pressed,
                        );
                        dsk.win.top_before = if dsk.win.focus.es_para(W_SOUND) { W_SOUND } else { W_RUN };
                        if dsk.win.top_before == W_RUN {
                            uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                        }
                    } else {
                        // * DEVOLVER EL APARATO. Esto es lo que impide que el
                        // escritorio deje mudos a todos los programas que lanza.
                        if let Some(s) = dsk.snd.cap.take() {
                            s.callar();
                            s.release();
                        }
                        dsk.win.focus.close(W_SOUND);
                        erase_window(
                            &p, &dsk.run_box, dsk.win.sound.chrome.x, dsk.win.sound.chrome.y,
                            dsk.win.sound.chrome.width, dsk.win.sound.chrome.height, dsk.win.visible,
                        );
                        dsk.win.top_before = W_RUN;
                        uncover(&p, &dsk.run_box, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                        // Si habia ventanas debajo, vuelven a verse.
                        if dsk.win.data_open {
                            scene::data::paint(&p, &dsk.win.data);
                        }
                        if dsk.win.cabina_open {
                            scene::cabina::paint(&p, &dsk.win.cabina);
                        }
                    }
                    continue;
                }

                // Las teclas de la ventana del sonido. **Solo con el foco
                // AQUI**: con el foco en Ejecutar, una `z` es una letra que el
                // dueno esta escribiendo, y robarsela para un atajo seria el
                // peor intercambio posible. Es la misma regla que la `f` del
                // klog.
                if dsk.win.sound_open && dsk.win.focus.es_para(W_SOUND) {
                    if let Some(s) = &dsk.snd.cap {
                        // Flechas: el volumen, de diez en diez.
                        //
                        // * `KEY_LEFT` es 0x82 y `KEY_RIGHT` 0x83 -- ver
                        // `ring0/dev/keyboard.rs`. Esto se escribio con 0x83 y
                        // 0x84, y **0x84 es INICIO**: la flecha izquierda no
                        // habria bajado el volumen y la tecla Inicio lo habria
                        // subido. No da error, da un control que obedece a la
                        // tecla equivocada.
                        if c == 0x82 || c == 0x83 {
                            dsk.snd.volume = if c == 0x83 {
                                (dsk.snd.volume + 10).min(100)
                            } else {
                                dsk.snd.volume.saturating_sub(10)
                            };
                            s.volumen(dsk.snd.volume);
                            scene::sound::paint(
                                &p, &dsk.win.sound, true, dsk.snd.devices,
                                dsk.snd.volume, dsk.snd.pressed,
                            );
                            continue;
                        }
                        // Z..M: una octava. Se pinta la tecla ANTES de pitar
                        // porque `pitar` bloquea el nucleo mientras suena: al
                        // reves, la tecla se veria encendida cuando ya callo.
                        let min = c.to_ascii_lowercase();
                        if let Some(i) = scene::sound::NOTES.iter().position(|note| note.0 == min) {
                            dsk.snd.pressed = Some(i);
                            scene::sound::paint(
                                &p, &dsk.win.sound, true, dsk.snd.devices,
                                dsk.snd.volume, dsk.snd.pressed,
                            );
                            s.pitar(scene::sound::NOTES[i].1, 160);
                            dsk.snd.pressed = None;
                            scene::sound::paint(
                                &p, &dsk.win.sound, true, dsk.snd.devices,
                                dsk.snd.volume, dsk.snd.pressed,
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
                if dsk.win.cabina_open && (c == b'g' || c == b'G') {
                    dsk.win.cabina.minima = (dsk.win.cabina.minima + 1) % 5;
                    dsk.win.cabina.from = 0;
                    scene::cabina::paint(&p, &dsk.win.cabina);
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
                if dsk.win.cabina_open && (c == b'a' || c == b'A') {
                    dsk.win.cabina.last_only = !dsk.win.cabina.last_only;
                    dsk.win.cabina.from = 0;
                    scene::cabina::paint(&p, &dsk.win.cabina);
                    continue;
                }
                if dsk.win.cabina_open && (c == 0x87 || c == 0x88) {
                    let any = bmo::cabina_disponibles();
                    if c == 0x87 {
                        // Hacia atras en el tiempo, sin pasarse del principio.
                        dsk.win.cabina.from = (dsk.win.cabina.from + 6).min(any.saturating_sub(1));
                    } else {
                        dsk.win.cabina.from = dsk.win.cabina.from.saturating_sub(6);
                    }
                    scene::cabina::paint(&p, &dsk.win.cabina);
                    continue;
                }

                // -- * La consola de DATOS: cambiar de vista y recorrer el arbol --
                //
                // Va aqui, junto al bloque del klog y por el mismo motivo: son
                // teclas DE ESTA VENTANA. Con Datos delante, las flechas no
                // tienen nada que ver con el historial de comandos de Ejecutar,
                // y hasta hoy iban alli -- se navegaba una ventana tapada.
                if dsk.win.data_open && dsk.win.focus.es_para(W_DATA) {
                    use scene::data::{Seal, View};
                    let mut served = true;
                    match c {
                        // TAB: numeros <-> nodos. Es la misma tecla que cambia de
                        // pestana en todas partes.
                        b'\t' => {
                            dsk.win.data.view = match dsk.win.data.view {
                                View::Numbers => {
                                    // Al entrar en el arbol se empieza por la
                                    // raiz. Conservar el sitio de la ultima vez
                                    // ensenaria un directorio que ya no se sabe
                                    // cual es.
                                    bmo::estratos::a_la_raiz();
                                    dsk.win.data.to_top();
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
                            dsk.win.data.seal = Seal::Idle;
                        }
                        _ if dsk.win.data.view == View::Numbers => served = false,
                        // ARRIBA / ABAJO por la lista de hijos.
                        // Al cambiar de caja se borra la verificacion: es de
                        // UN archivo, y un `CUADRA` viejo bajo el nombre de
                        // otro es peor que no decir nada.
                        0x80 => { dsk.win.data.move_sel(-1, bmo::estratos::hijos() as usize); dsk.win.data.verified = None; }
                        0x81 => { dsk.win.data.move_sel(1, bmo::estratos::hijos() as usize); dsk.win.data.verified = None; }
                        0x87 => dsk.win.data.move_sel(-5, bmo::estratos::hijos() as usize),
                        0x88 => dsk.win.data.move_sel(5, bmo::estratos::hijos() as usize),
                        // ENTRAR / DERECHA: bajar al hijo senalado. `entrar`
                        // dice que no si es un archivo, y entonces no pasa nada
                        // -- que es lo correcto: un archivo no tiene dentro.
                        b'\r' | b'\n' | 0x83 => {
                            if bmo::estratos::entrar(dsk.win.data.sel as u64) {
                                dsk.win.data.to_top();
                                dsk.win.data.verified = None;
                            }
                        }
                        // RETROCESO / IZQUIERDA: subir al padre.
                        0x08 | 0x82 => {
                            if bmo::estratos::subir() {
                                dsk.win.data.to_top();
                                dsk.win.data.verified = None;
                            }
                        }
                        // * V: COMPROBAR LA FIRMA del nodo senalado.
                        //
                        // Se pide a mano y no se calcula al pintar: lee el
                        // archivo entero y le hace el BLAKE3, y hacer eso
                        // sesenta veces por segundo convertiria este panel en
                        // un martillo sobre el disco.
                        b'v' | b'V' => {
                            dsk.win.data.verified =
                                Some(bmo::estratos::verificar(dsk.win.data.sel as u64));
                            dsk.win.data.seal = Seal::Idle;
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
                            dsk.win.data.seal = match dsk.win.data.seal {
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
                            dsk.win.data.seal = Seal::Idle;
                            served = false;
                        }
                    }
                    if served {
                        scene::data::paint(&p, &dsk.win.data);
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
                if !dsk.win.focus.es_para(W_RUN) {
                    continue;
                }
                debug_assert!(dsk.win.visible, "el foco de una ventana escondida es un bug");
                // Cualquier tecla enciende el cursor y reinicia el parpadeo.
                dsk.field.caret = true;
                dsk.field.since_key = 0;
                dsk.tick.repaint_field = true;
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
                        dsk.out.grid.with_ink(INK_ECHO);
                        dsk.out.grid.byte(0xB7);
                        dsk.out.grid.byte(b' ');
                        dsk.out.grid.text(dsk.field.line());
                        dsk.out.grid.byte(b'\n');
                        dsk.out.grid.with_ink(INK_PLAIN);

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
                        let from_child = !dsk.calc.waiting
                            && dsk.out.console.as_ref().map(|cc| cc.has_child()).unwrap_or(false);

                        if from_child {
                            if let Some(cc) = dsk.out.console.as_ref() {
                                cc.write(dsk.field.line());
                                // El salto va aparte y SIEMPRE: `read_line`
                                // espera a verlo para dar la linea por
                                // cerrada. Sin el, el programa sigue
                                // esperando algo que ya escribiste.
                                cc.write(b"\n");
                            }
                            paint_status(&p, &dsk.run_box, "para el programa", INK_DIM);
                            dsk.field.n = 0;
                            dsk.field.cur = 0;
                            dsk.tick.repaint_field = true;
                            continue;
                        }

                        // Al historial va lo que es un COMANDO. Un importe
                        // tecleado para un `ACCEPT` es un dato, y mezclarlo
                        // con las rutas ensucia la flecha arriba justo cuando
                        // hace falta repetir el comando de verdad.
                        dsk.field.history.push(&dsk.field.path[..dsk.field.n]);
                        match parse(dsk.field.line()) {
                            Command::Nothing => {
                                paint_status(&p, &dsk.run_box, "escribe algo", INK_DIM);
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
                                dsk.out.grid.text(b"    n_n_n
");
                                dsk.out.grid.text(b"   ( -.- )   ~nya. eso aqui no se dice.
");
                                dsk.out.grid.text(b"   ( u u )   esto NO es Linux, es BMO-X.
");
                                dsk.out.grid.text(b"    ^^ ^^    no hay root que pedir:
");
                                dsk.out.grid.text(b"             o te dieron la capability, o no existe.
");
                                dsk.out.grid.text(b"
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
                                dsk.out.grid.text(hint);
                                paint_status(&p, &dsk.run_box, "esto no es Linux :3", INK_DIM);
                                dsk.tick.repaint_field = true;
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
                                            dsk.out.grid.text(b"  ");
                                            dsk.out.grid.text(&nom[..length]);
                                            // Alinear la columna del tamano.
                                            let mut k = length;
                                            while k < 14 { dsk.out.grid.byte(b' '); k += 1; }
                                            if e.es_dir {
                                                dsk.out.grid.text(b"<DIR>");
                                            } else {
                                                let mut d10 = [0u8; 10];
                                                let n10 = decimal(e.bytes as u64, &mut d10);
                                                dsk.out.grid.text(&d10[..n10]);
                                            }
                                            dsk.out.grid.byte(b'\n');
                                            count += 1;
                                        }
                                        if count == 0 {
                                            dsk.out.grid.text(b"  (vacio)
");
                                        }
                                        paint_status(&p, &dsk.run_box, "listo", INK_DIM);
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
                                        dsk.out.grid.with_ink(INK_ERR);
                                        dsk.out.grid.text(line);
                                        dsk.out.grid.with_ink(INK_PLAIN);
                                        paint_status(&p, &dsk.run_box, estado, INK_BAD);
                                    }
                                }
                                dsk.field.n = 0;
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
                                            let got = a.read(&mut chunk);
                                            if got == 0 { break; }
                                            dsk.out.grid.text(&chunk[..got]);
                                            last = chunk[dsk.field.n - 1];
                                            total += dsk.field.n;
                                            if total >= 2048 {
                                                dsk.out.grid.text(b"\n  ...(cortado)\n");
                                                last = b'\n';
                                                break;
                                            }
                                        }
                                        if total == 0 {
                                            dsk.out.grid.text(b"  (vacio)\n");
                                        } else if last != b'\n' {
                                            // Sin esto, el proximo mensaje se
                                            // pega al final del archivo.
                                            dsk.out.grid.byte(b'\n');
                                        }
                                        a.close();
                                        paint_status(&p, &dsk.run_box, "listo", INK_DIM);
                                    }
                                    Err(e) => {
                                        dsk.out.grid.with_ink(INK_ERR);
                                        dsk.out.grid.text(b"  ");
                                        dsk.out.grid.text(file_error_reason(e));
                                        dsk.out.grid.byte(b'\n');
                                        dsk.out.grid.with_ink(INK_PLAIN);
                                        paint_status(&p, &dsk.run_box, "no se pudo leer", INK_BAD);
                                    }
                                }
                                dsk.field.n = 0;
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
                                            dsk.out.grid.text(b"  guardado: ");
                                            let mut d10 = [0u8; 10];
                                            let n10 = decimal(placed as u64 + 1, &mut d10);
                                            dsk.out.grid.text(&d10[..n10]);
                                            dsk.out.grid.text(b" bytes\n");
                                            paint_status(&p, &dsk.run_box, "guardado", INK_OK);
                                        } else {
                                            dsk.out.grid.text(b"  no se guardo nada.\n");
                                            paint_status(&p, &dsk.run_box, "no se pudo guardar", INK_BAD);
                                        }
                                    }
                                    Err(e) => {
                                        dsk.out.grid.with_ink(INK_ERR);
                                        dsk.out.grid.text(b"  ");
                                        dsk.out.grid.text(file_error_reason(e));
                                        dsk.out.grid.byte(b'\n');
                                        dsk.out.grid.with_ink(INK_PLAIN);
                                        paint_status(&p, &dsk.run_box, "no se pudo crear", INK_BAD);
                                    }
                                }
                                dsk.field.n = 0;
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
                                let (from, to) = dsk.out.grid.all_rows();
                                match dump_output(&dsk.out.grid, dest, from, to) {
                                    Ok(bytes) => {
                                        dsk.out.grid.with_ink(INK_GOOD);
                                        dsk.out.grid.text(b"  guardado en ");
                                        dsk.out.grid.text(dest);
                                        dsk.out.grid.text(b": ");
                                        let mut d = [0u8; 10];
                                        let k = decimal(bytes as u64, &mut d);
                                        dsk.out.grid.text(&d[..k]);
                                        dsk.out.grid.text(b" bytes, ");
                                        let k = decimal((to - from + 1) as u64, &mut d);
                                        dsk.out.grid.text(&d[..k]);
                                        dsk.out.grid.text(b" lineas\n");
                                        dsk.out.grid.with_ink(INK_PLAIN);
                                        paint_status(&p, &dsk.run_box, "volcado", INK_OK);
                                    }
                                    Err(0) => {
                                        dsk.out.grid.with_ink(INK_ERR);
                                        dsk.out.grid.text(b"  no se guardo nada. el motivo esta en F11.\n");
                                        dsk.out.grid.with_ink(INK_PLAIN);
                                        paint_status(&p, &dsk.run_box, "no se pudo guardar", INK_BAD);
                                    }
                                    Err(e) => {
                                        dsk.out.grid.with_ink(INK_ERR);
                                        dsk.out.grid.text(b"  ");
                                        dsk.out.grid.text(file_error_reason(e));
                                        dsk.out.grid.byte(b'\n');
                                        dsk.out.grid.with_ink(INK_PLAIN);
                                        paint_status(&p, &dsk.run_box, "no se pudo crear", INK_BAD);
                                    }
                                }
                                dsk.field.n = 0;
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
                                dsk.out.grid.text(b"  sellar se mudo a la ventana de ESTRATOS.
");
                                dsk.out.grid.with_ink(INK_GOOD);
                                dsk.out.grid.text(b"  F12  ->  TAB  ->  tecla S
");
                                dsk.out.grid.with_ink(INK_PLAIN);
                                dsk.out.grid.text(b"  ahi se ve el volumen mientras se sella, que es
");
                                dsk.out.grid.text(b"  donde tiene sentido: la generacion sube delante.
");
                                paint_status(&p, &dsk.run_box, "esta en F12", INK);
                                dsk.field.n = 0;
                            }
                            // * `perf` -- el numero antes que la tarjeta.
                            //
                            // Se pinta ANTES de leer los contadores no: se leen
                            // aqui y se imprimen, y el fotograma que los pinta
                            // sumara el suyo. Da igual: lo que interesa es el
                            // orden de magnitud y el peor caso, no un digito.
                            Command::PaintCost => {
                                let v = p.volcado();
                                dsk.out.grid.text(b"  pintado\n");
                                dsk.out.grid.text(b"    modo        ");
                                dsk.out.grid.text(match v.modo {
                                    bmo::Volcador::Ninguno => b"directo al panel (SIN doble bufer)\n" as &[u8],
                                    bmo::Volcador::Directo => b"doble bufer, volcado por CPU\n",
                                });
                                dsk.out.grid.text(b"    fotogramas  ");
                                let mut d = [0u8; 10];
                                let k = decimal(v.fotogramas, &mut d);
                                dsk.out.grid.text(&d[..k]);
                                dsk.out.grid.text(b"   con algo que mover\n");
                                if v.fotogramas > 0 {
                                    dsk.out.grid.text(b"    medio      ");
                                    let k = decimal(v.bytes / v.fotogramas / 1024, &mut d);
                                    dsk.out.grid.text(&d[..k]);
                                    dsk.out.grid.text(b" KiB por fotograma\n");
                                    // El PEOR caso va aparte y a proposito: un
                                    // tiron se nota y una media buena lo tapa.
                                    dsk.out.grid.text(b"    peor       ");
                                    let k = decimal(v.peor / 1024, &mut d);
                                    dsk.out.grid.text(&d[..k]);
                                    dsk.out.grid.text(b" KiB en un fotograma\n");
                                    dsk.out.grid.text(b"    total      ");
                                    // ** Y CUANTAS CAJAS tenia ese peor
                                    // fotograma. Con la caja unica de antes
                                    // esto seria SIEMPRE 1 y el `worst` la
                                    // pantalla entera; si aqui sale 2 o 3 con
                                    // un peor pequeno, el troceado trabaja.
                                    dsk.out.grid.text(b"    cajas      ");
                                    let k = decimal(v.cajas as u64, &mut d);
                                    dsk.out.grid.text(&d[..k]);
                                    dsk.out.grid.text(b"
");
                                    let k = decimal(v.bytes / 1024 / 1024, &mut d);
                                    dsk.out.grid.text(&d[..k]);
                                    dsk.out.grid.text(b" MiB movidos desde el arranque\n");
                                }
                                dsk.out.grid.with_ink(INK_ECHO);
                                dsk.out.grid.text(b"    la caja de sucio ya recorta esto: una GPU solo\n");
                                dsk.out.grid.text(b"    compra algo si estos numeros son grandes.\n");
                                dsk.out.grid.with_ink(INK_PLAIN);
                                paint_status(&p, &dsk.run_box, "listo", INK_DIM);
                                dsk.field.n = 0;
                            }
                            Command::Calculator => {
                                dsk.calc.visible = !dsk.calc.visible;
                                if dsk.calc.visible {
                                    paint_calc(&p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
                                    dsk.out.grid.text(b"  calculadora: la cara en Rust, el calculo en COBOL
");
                                } else {
                                    // Devolver esa zona a la escena.
                                    for f in 0..dsk.calc_pad.height {
                                        for co in 0..dsk.calc_pad.width {
                                            let (px, py) = (dsk.calc_pad.x + co, dsk.calc_pad.y + f);
                                            p.punto(px, py, scene_color(&dsk.run_box, dsk.win.visible, px, py, p.alto));
                                        }
                                    }
                                }
                                paint_status(&p, &dsk.run_box, "listo", INK_DIM);
                                dsk.field.n = 0;
                                dsk.field.cur = 0;
                            }
                            Command::Clear => {
                                dsk.out.grid.clear();
                                paint_status(&p, &dsk.run_box, "listo", INK_DIM);
                                dsk.field.n = 0;
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
                                commands::reports::report_net(&mut dsk.out.grid, what);
                            }
                            Command::Audio => {
                                let had_any = bmo::audio_censo();
                                if had_any {
                                    dsk.out.grid.with_ink(INK_GOOD);
                                    dsk.out.grid.text(b"  aparato de reproduccion HALLADO\n");
                                    dsk.out.grid.with_ink(INK_PLAIN);
                                    dsk.out.grid.text(b"  los ocho numeros estan en F11 (canales, bits, frecuencias)\n");
                                    dsk.out.grid.text(b"  comparalos con lo que dice Windows del mismo audifono\n");
                                } else {
                                    dsk.out.grid.with_ink(INK_ERR);
                                    dsk.out.grid.text(b"  ningun aparato de reproduccion en los puertos libres\n");
                                    dsk.out.grid.with_ink(INK_PLAIN);
                                    // La distincion que decide el siguiente paso, y por eso
                                    // se dice aqui y no solo en CABINA.
                                    dsk.out.grid.text(b"  F11 dice CUANTOS puertos se miraron: si es 0, el fallo\n");
                                    dsk.out.grid.text(b"  es del censo; si es >0, el aparato no es UAC1\n");
                                }
                                paint_status(&p, &dsk.run_box, "audio", INK_DIM);
                                dsk.field.n = 0;
                            }
                            Command::Help => {
                                dsk.out.grid.text(b"  <ruta>       lanza un .bex   (cobol/banco.bex)\n");
                                dsk.out.grid.text(b"  run <ruta>   lo mismo, como en el shell de Ring 0\n");
                                // Va JUSTO detras de `run` porque es su hermana,
                                // y con la consecuencia delante: lo que sorprende
                                // no es que lance, es que el escritorio se vaya.
                                dsk.out.grid.text(b"  presta <ruta>  se lo lanza CON LA PANTALLA: el\n");
                                dsk.out.grid.text(b"               escritorio se aparta y vuelve cuando\n");
                                dsk.out.grid.text(b"               el programa termina  (c/ray.bex)\n");
                                dsk.out.grid.text(b"  cat <ruta>   ensena lo que hay dentro\n");
                                dsk.out.grid.text(b"  write <ruta> <texto>     lo guarda\n");
                                dsk.out.grid.text(b"  guarda [ruta]  vuelca esta salida a un .txt\n");
                                dsk.out.grid.text(b"               (por defecto datos/salida.txt, y cada\n");
                                dsk.out.grid.text(b"                programa que corre lo deja solo ahi)\n");
                                dsk.out.grid.text(b"  clear / cls  limpia esta salida\n");
                                dsk.out.grid.text(b"  TAB          completa   Ctrl+A/E inicio/fin\n");
                                dsk.out.grid.text(b"  Ctrl+K corta al final    Ctrl+W borra palabra\n");
                                dsk.out.grid.text(b"  Ctrl+U borra linea       Ctrl+L limpia\n");
                                dsk.out.grid.text(b"  info         RAM, CPU, tareas y disco\n");
                                dsk.out.grid.text(b"  cpu / mem    solo esa parte del informe\n");
                                dsk.out.grid.text(b"  perf         lo que cuesta pintar, medido\n");
                                dsk.out.grid.text(b"  estratos sellar   ESCRIBE EN EL DISCO (commit vacio)\n");
                                dsk.out.grid.text(b"  help         esto\n");
                                dsk.out.grid.text(b"  reboot       reinicia la maquina\n");
                                dsk.out.grid.text(b"  Ctrl+Alt     esconde o invoca esta ventana\n");
                                paint_status(&p, &dsk.run_box, "listo", INK_DIM);
                                dsk.field.n = 0;
                            }
                            // Ni se intenta lanzar. Se dice lo que es y con
                            // que se abre -- un mensaje sobre la FIRMA aqui
                            // manda a buscar un permiso que no hace falta.
                            Command::NotAProgram(r) => {
                                dsk.out.grid.with_ink(INK_ERR);
                                dsk.out.grid.text(b"  eso no es un programa (solo .bex se lanza).\n");
                                dsk.out.grid.text(b"  para verlo:  cat ");
                                dsk.out.grid.text(r);
                                dsk.out.grid.byte(b'\n');
                                dsk.out.grid.with_ink(INK_PLAIN);
                                paint_status(&p, &dsk.run_box, "no es un programa: prueba lee", INK_DIM);
                                dsk.field.n = 0;
                            }
                            Command::Autopsy => {
                                report_autopsy(&mut dsk.out.grid);
                                paint_status(&p, &dsk.run_box, "ultimo fallo de Ring 3", INK_DIM);
                                dsk.field.n = 0;
                            }
                            Command::Report => {
                                report_system(&mut dsk.out.grid);
                                paint_status(&p, &dsk.run_box, "informe del sistema", INK_DIM);
                                dsk.field.n = 0;
                            }
                            Command::Cpu => {
                                report_cpu(&mut dsk.out.grid);
                                paint_status(&p, &dsk.run_box, "procesador", INK_DIM);
                                dsk.field.n = 0;
                            }
                            Command::Memoria => {
                                report_memory(&mut dsk.out.grid);
                                paint_status(&p, &dsk.run_box, "memoria", INK_DIM);
                                dsk.field.n = 0;
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
                                    dsk.out.grid.text(b"  obreros parados (vuelven a hlt)\n");
                                    // ** Y LO QUE VA A PASAR DESPUES, DICHO AQUI.
                                    //
                                    // El dueno escribio `smp stop`, luego `smp`,
                                    // y leyo `12 de 12`. Las dos lineas eran
                                    // ciertas y juntas decian una mentira. Lo
                                    // que faltaba no era un numero distinto:
                                    // era avisar de que ese numero cuenta otra
                                    // cosa.
                                    dsk.out.grid.text(b"  [!] seguiran contando como \"en pie\": encendidos, no trabajando\n");
                                    dsk.out.grid.text(b"      `smp all` los vuelve a poner a trabajar\n");
                                    paint_output(&p, &dsk.run_box, &dsk.out.grid);
                                    paint_status(&p, &dsk.run_box, "smp", INK_DIM);
                                    dsk.field.n = 0;
                                    dsk.field.cur = 0;
                                    continue;
                                }
                                if arg == b"prueba" || arg == b"bench" || arg == b"test" {
                                    dsk.out.grid.text(b"  midiendo reparto (esto tarda)...\n");
                                    paint_output(&p, &dsk.run_box, &dsk.out.grid);
                                    p.volcar();
                                    let x100 = bmo::smp_prueba();
                                    let mut b = [0u8; 10];
                                    dsk.out.grid.with_ink(if x100 >= 150 { INK_GOOD } else { INK_ERR });
                                    dsk.out.grid.text(b"  aceleracion: ");
                                    let k = decimal(x100 / 100, &mut b);
                                    dsk.out.grid.text(&b[..k]);
                                    dsk.out.grid.text(b".");
                                    // Los dos decimales, con su cero delante:
                                    // "8.4" y "8.04" no son el mismo numero.
                                    if x100 % 100 < 10 {
                                        dsk.out.grid.text(b"0");
                                    }
                                    let k = decimal(x100 % 100, &mut b);
                                    dsk.out.grid.text(&b[..k]);
                                    dsk.out.grid.text(b"x   (F11 trae los ticks)\n");
                                    dsk.out.grid.with_ink(INK_PLAIN);
                                    if x100 == 0 {
                                        dsk.out.grid.text(b"  0 = falto una parte: el numero no vale\n");
                                    }
                                    paint_status(&p, &dsk.run_box, "smp", INK_DIM);
                                    dsk.field.n = 0;
                                    dsk.field.cur = 0;
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
                                    dsk.out.grid.text(b"  censando (no se despierta a nadie)\n");
                                } else {
                                    dsk.out.grid.text(b"  despertando nucleos (esto tarda)...\n");
                                }
                                paint_output(&p, &dsk.run_box, &dsk.out.grid);
                                p.volcar();
                                let (alive, expected, stopped) = bmo::smp_censo(how_many);
                                dsk.out.grid.with_ink(if alive == expected {
                                    INK_GOOD
                                } else {
                                    INK_ERR
                                });
                                dsk.out.grid.text(b"  nucleos en pie: ");
                                let mut b = [0u8; 10];
                                let k = decimal((alive + 1) as u64, &mut b);
                                dsk.out.grid.text(&b[..k]);
                                dsk.out.grid.text(b" de ");
                                let k = decimal((expected + 1) as u64, &mut b);
                                dsk.out.grid.text(&b[..k]);
                                dsk.out.grid.text(b"   (F11 lo cuenta entero)\n");
                                dsk.out.grid.with_ink(INK_PLAIN);
                                // ** LA MITAD QUE FALTABA DEL CENSO.
                                //
                                // "En pie" cuenta nucleos que contestaron al
                                // SIPI, y ese numero no baja al pararlos --
                                // correctamente: salir del reset no es trabajar.
                                // Pero leido solo, dice que `smp stop` no hizo
                                // nada. Ahora se dicen las dos cosas.
                                if stopped {
                                    dsk.out.grid.with_ink(INK_ERR);
                                    dsk.out.grid.text(b"  [!] pero estan PARADOS: en pie no es trabajando\n");
                                    dsk.out.grid.with_ink(INK_PLAIN);
                                    dsk.out.grid.text(b"      `smp all` los vuelve a poner a trabajar\n");
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
                                    dsk.out.grid.text(b"  smp all      despierta todos    smp 3   solo tres\n");
                                    dsk.out.grid.text(b"  smp test     reparte una cuenta y mide la aceleracion\n");
                                    dsk.out.grid.text(b"  smp stop     los duerme. [!] sin IPI NO vuelven\n");
                                    dsk.out.grid.text(b"  F11 dice en que esta cada nucleo y cual gira en vacio\n");
                                }
                                paint_status(&p, &dsk.run_box, "smp", INK_DIM);
                                dsk.field.n = 0;
                                dsk.field.cur = 0;
                            }
                            // Se pinta ANTES de pedirlo: la llamada no vuelve,
                            // asi que un mensaje despues no lo veria nadie. Y
                            // que quede escrito distingue "reinicio pedido" de
                            // "se colgo" en la foto. `Pantalla` escribe directo
                            // al framebuffer, asi que al volver de `text` ya
                            // esta en el cristal: no hay nada que vaciar.
                            Command::Reboot => {
                                dsk.out.grid.text(b"  reiniciando...\n");
                                paint_status(&p, &dsk.run_box, "reiniciando", INK_DIM);
                                bmo::reiniciar();
                            }
                            Command::Unknown => {
                                // El mensaje honesto. Antes se contestaba "no
                                // esta: revisa la ruta" a quien escribia
                                // `reboot`, y eso manda a buscar un archivo que
                                // nunca existio en vez de decir la verdad.
                                dsk.out.grid.text(b"  no es un comando ni una ruta. escribe 'help'.\n");
                                paint_status(&p, &dsk.run_box, "no lo conozco: prueba help", INK_BAD);
                                dsk.field.n = 0;
                            }
                            Command::Launch(target) => {
                                let cap = dsk.out.console.as_ref().map(|c| c.cap).unwrap_or(0);
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
                                            scene::launcher::paint(&p, &dsk.launcher);
                                            p.rect(16, 13, 14, 14, ACCENT);
                                            p.texto(38, 14, "BMO-X", INK);
                                            dsk.win.taskbar_dirty = true;
                                            paint_run_box(&p, &dsk.run_box);
                                            paint_field(&p, &dsk.run_box, dsk.field.line(), dsk.field.cur, true);
                                            paint_output(&p, &dsk.run_box, &dsk.out.grid);
                                            paint_status(&p, &dsk.run_box, "pantalla devuelta", INK_OK);
                                            p.vaciar();
                                            dsk.tick.repaint_field = true;
                                        }
                                        None => {
                                            bmo::consola(
                                                "no pude recuperar la pantalla tras prestarla
",
                                            );
                                            bmo::salir()
                                        }
                                    }
                                    dsk.field.n = 0;
                                    continue;
                                }
                                match bmo::ejecutar_en(target, cap) {
                                    Ok(_) => {
                                        paint_status(&p, &dsk.run_box, "lanzado", INK_OK);
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
                                        dsk.out.run = Some(Run {
                                            mark: dsk.out.grid.mark().saturating_sub(1),
                                            waits: 0,
                                            dest,
                                            dest_n,
                                        });
                                        // El campo se vacia al lanzar, como el
                                        // Win+R: la caja esta para el SIGUIENTE
                                        // programa, no para admirar el anterior.
                                        dsk.field.n = 0;
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
                                        &dsk.run_box,
                                        "no se pudo cargar: F11 dice por que",
                                        INK_BAD,
                                    ),
                                    Err(bmo::ERROR_GATE) => paint_status(
                                        &p,
                                        &dsk.run_box,
                                        "rechazado: la firma no cuadra",
                                        INK_BAD,
                                    ),
                                    Err(bmo::ERROR_BUSY) => {
                                        paint_status(&p, &dsk.run_box, "no hay hueco ahora mismo", INK_BAD)
                                    }
                                    Err(_) => {
                                        paint_status(&p, &dsk.run_box, "no paso la admision", INK_BAD)
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
                        dsk.field.cur = dsk.field.cur.min(dsk.field.n);
                        dsk.tick.repaint_field = true;
                    }
                    // TAB: completar.
                    b'\t' => {
                        let antes = dsk.field.n;
                        dsk.field.n = complete(&mut dsk.field.path, dsk.field.n, &mut dsk.out.grid);
                        dsk.field.cur = dsk.field.n;
                        if dsk.field.n == antes {
                            paint_status(&p, &dsk.run_box, "nada que completar", INK_DIM);
                        }
                        dsk.tick.repaint_field = true;
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
                    // Esa linea es `paint_field(..., &path[..dsk.field.n], ...)`, y el
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
                        if dsk.field.cur > 0 && dsk.field.n > 0 {
                            let mut k = dsk.field.cur;
                            while k < dsk.field.n {
                                dsk.field.path[k - 1] = dsk.field.path[k];
                                k += 1;
                            }
                            dsk.field.cur -= 1;
                            dsk.field.n -= 1;
                            dsk.tick.repaint_field = true;
                        }
                    }
                    // Escape: borrar la linea entera, igual que en el Win+R.
                    0x1B => {
                        dsk.field.n = 0;
                        dsk.field.cur = 0;
                        paint_status(&p, &dsk.run_box, "listo", INK_DIM);
                        dsk.tick.repaint_field = true;
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
                        dsk.field.clipboard_n = dsk.field.n;
                        let upto = dsk.field.n;
                        let (src, dst) = (&dsk.field.path[..upto], &mut dsk.field.clipboard[..upto]);
                        dst.copy_from_slice(src);
                        paint_status(&p, &dsk.run_box, "copiado", INK_DIM);
                    }
                    0x16 => {
                        if dsk.field.clipboard_n > 0 && dsk.field.n + dsk.field.clipboard_n <= PATH_MAX {
                            // Hueco del tamano del pegado, y meterlo.
                            let mut k = dsk.field.n;
                            while k > dsk.field.cur {
                                dsk.field.path[k + dsk.field.clipboard_n - 1] = dsk.field.path[k - 1];
                                k -= 1;
                            }
                            dsk.field.path[dsk.field.cur..dsk.field.cur + dsk.field.clipboard_n].copy_from_slice(&dsk.field.clipboard[..dsk.field.clipboard_n]);
                            dsk.field.cur += dsk.field.clipboard_n;
                            dsk.field.n += dsk.field.clipboard_n;
                            dsk.tick.repaint_field = true;
                        }
                    }
                    // Ctrl+U -- borra la linea. Ctrl+L -- borra la salida.
                    // Los mismos que el shell de Ring 0, porque los dedos ya
                    // los tienen y un atajo que cambia entre dos ventanas del
                    // mismo sistema es peor que no tenerlo.
                    0x15 => {
                        dsk.field.n = 0;
                        dsk.field.cur = 0;
                        dsk.tick.repaint_field = true;
                    }
                    0x0C => {
                        dsk.out.grid.clear();
                        dsk.tick.repaint_field = true;
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
                        dsk.field.clipboard_n = dsk.field.n;
                        let upto = dsk.field.n;
                        let (src, dst) = (&dsk.field.path[..upto], &mut dsk.field.clipboard[..upto]);
                        dst.copy_from_slice(src);
                        paint_status(&p, &dsk.run_box, "copiado", INK_DIM);
                    }
                    0x81 if ctrl => {
                        if dsk.field.clipboard_n > 0 && dsk.field.n + dsk.field.clipboard_n <= PATH_MAX {
                            let mut k = dsk.field.n;
                            while k > dsk.field.cur {
                                dsk.field.path[k + dsk.field.clipboard_n - 1] = dsk.field.path[k - 1];
                                k -= 1;
                            }
                            dsk.field.path[dsk.field.cur..dsk.field.cur + dsk.field.clipboard_n].copy_from_slice(&dsk.field.clipboard[..dsk.field.clipboard_n]);
                            dsk.field.cur += dsk.field.clipboard_n;
                            dsk.field.n += dsk.field.clipboard_n;
                            dsk.tick.repaint_field = true;
                        }
                    }
                    0x80 => {
                        if let Some(k) = dsk.field.history.back(&mut dsk.field.path) {
                            dsk.field.n = k;
                            dsk.field.cur = k;
                            dsk.tick.repaint_field = true;
                        }
                    }
                    0x81 => {
                        if let Some(k) = dsk.field.history.forward(&mut dsk.field.path) {
                            dsk.field.n = k;
                            dsk.field.cur = k;
                            dsk.tick.repaint_field = true;
                        }
                    }
                    // IZQUIERDA / DERECHA -- mover el cursor.
                    0x82 => {
                        if dsk.field.cur > 0 { dsk.field.cur -= 1; dsk.tick.repaint_field = true; }
                    }
                    0x83 => {
                        if dsk.field.cur < dsk.field.n { dsk.field.cur += 1; dsk.tick.repaint_field = true; }
                    }
                    // INICIO / FIN.
                    0x84 => { dsk.field.cur = 0; dsk.tick.repaint_field = true; }
                    0x85 => { dsk.field.cur = dsk.field.n; dsk.tick.repaint_field = true; }
                    // -- Los atajos de edicion de linea --
                    //
                    // Los de toda la vida en una consola: Ctrl+A al principio,
                    // Ctrl+E al final, Ctrl+K corta hasta el final, Ctrl+W
                    // borra la palabra de atras. Van ADEMAS de Inicio/Fin, que
                    // ya estaban: los dedos que vienen de un terminal buscan
                    // estos, y los que vienen de Windows buscan aquellos.
                    // Atender a los dos cuesta cuatro lineas.
                    0x01 => { dsk.field.cur = 0; dsk.tick.repaint_field = true; }
                    0x05 => { dsk.field.cur = dsk.field.n; dsk.tick.repaint_field = true; }
                    // Ctrl+K: tirar lo que hay del cursor al final.
                    0x0B => {
                        dsk.field.n = dsk.field.cur;
                        dsk.tick.repaint_field = true;
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
                        let limit = dsk.field.cur.min(dsk.field.n);
                        let mut k = limit;
                        while k > 0 && dsk.field.path[k - 1] == b' ' { k -= 1; }
                        while k > 0 && dsk.field.path[k - 1] != b' ' { k -= 1; }
                        let removed = limit - k;
                        if removed > 0 {
                            let mut i = limit;
                            while i < dsk.field.n {
                                dsk.field.path[i - removed] = dsk.field.path[i];
                                i += 1;
                            }
                            dsk.field.n -= removed;
                            dsk.field.cur = k;
                            dsk.tick.repaint_field = true;
                        }
                    }
                    // SUPRIMIR -- borra HACIA ADELANTE, al reves que el
                    // retroceso. Son dos teclas porque son dos intenciones.
                    0x86 => {
                        if dsk.field.cur < dsk.field.n {
                            let mut k = dsk.field.cur + 1;
                            while k < dsk.field.n { dsk.field.path[k - 1] = dsk.field.path[k]; k += 1; }
                            dsk.field.n -= 1;
                            dsk.tick.repaint_field = true;
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
                        dsk.out.grid.scroll_view(OUT_ROWS as i32 - 1);
                    }
                    0x88 => {
                        dsk.out.grid.scroll_view(-(OUT_ROWS as i32 - 1));
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
                        if dsk.field.n < PATH_MAX {
                            // Hueco en el cursor y meter ahi: escribir en
                            // medio de una linea es lo normal, no un caso raro.
                            let mut k = dsk.field.n;
                            while k > dsk.field.cur {
                                dsk.field.path[k] = dsk.field.path[k - 1];
                                k -= 1;
                            }
                            dsk.field.path[dsk.field.cur] = c;
                            dsk.field.cur += 1;
                            dsk.field.n += 1;
                            dsk.tick.repaint_field = true;
                        }
                    }
                    _ => {}
                }
            }
            desktop::mouse::on_pointer(&mut dsk, &p, pos, wheel, ctrl);
        }

        desktop::paint::compose(&mut dsk, &p, dead);

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
    // escritorio moria en `main.rs:2744` -- que es un `&path[..dsk.field.n]`. Pero
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

