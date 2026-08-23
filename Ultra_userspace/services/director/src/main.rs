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
mod simbolos;
mod commands;
mod text;
mod watch;

use scene::output::{paint_output, Output};
use scene::*;
use desktop::BLINK;
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
    let mut anotacion = [0u8; 128];
    for i in 0..how_many {
        let rows = bmo::autopsia_renglones(i);
        // ** LA RUTA DEL BINARIO DEL MUERTO, sacada del propio informe.
        //
        // El renglon `programa` trae el nombre; con el se abre el `.bex` y se
        // le lee la tabla de simbolos. Se busca UNA vez por informe y no por
        // renglon: la tabla es la misma para todos.
        //
        // Si el programa no aparece, o su binario no esta donde se busca, o no
        // trae tabla, esto queda en `None` y el informe sale **exactamente como
        // antes**. Resolver nombres es una mejora, no un requisito.
        let mut rutas = [0u8; 64];
        let mut ruta: Option<(usize, usize)> = None;
        for f in 0..rows {
            let n = bmo::autopsia_linea(i, f, &mut buf);
            if let Some((ini, fin)) = simbolos::programa_de(&buf[..n]) {
                let nombre = &buf[ini..fin];
                let mut copia = [0u8; 64];
                let ln = nombre.len().min(copia.len());
                copia[..ln].copy_from_slice(&nombre[..ln]);
                let tramos = simbolos::rutas_probables(&copia[..ln], &mut rutas);
                // La primera que exista gana. `apps/` antes que `sys/` porque es
                // de donde se lanza casi todo.
                for (a0, a1) in tramos {
                    if bmo::Archivo::leer_de(&rutas[a0..a1]).is_ok() {
                        ruta = Some((a0, a1));
                        break;
                    }
                }
                break;
            }
        }

        for f in 0..rows {
            let n = bmo::autopsia_linea(i, f, &mut buf);
            a.write(&buf[..n]);
            // `\r\n` por lo mismo que `dump_output`: esto se abre en Windows
            // para mandarlo, y el Notepad viejo junta los saltos de Unix.
            a.write(b"\r\n");
            // ** Y DEBAJO, LOS NOMBRES -- en su propia linea.
            //
            // No sustituye a la del kernel a proposito: aquella es la PRUEBA y
            // esta es una INTERPRETACION que sale de un fichero del disco. Si
            // el `.bex` guardado no fuera el que se ejecuto, las dos lineas no
            // cuadrarian -- y eso es informacion. Machacando la original no
            // quedaria nada con que discrepar.
            if let Some((r0, r1)) = ruta {
                let m = simbolos::anotar(&buf[..n], &rutas[r0..r1], &mut anotacion);
                if m > 0 {
                    a.write(&anotacion[..m]);
                    a.write(b"\r\n");
                }
            }
        }
        a.write(b"\r\n");
    }
    a.close();
    true
}

pub(crate) fn dump_output(output: &Output, path: &[u8], from: usize, to: usize) -> Result<usize, u32> {
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
pub(crate) fn uncover(
    p: &bmo::Pantalla,
    run_box: &RunBox,
    launcher: &scene::launcher::Launcher,
    visible: bool,
    output: &mut Output,
    repaint_field: &mut bool,
) {
    // ** LA REJILLA DE ICONOS, Y VA ANTES DEL `return`.
    //
    // El fallo que arregla: `scene_color` --que se llama a si misma "el modelo
    // entero del escritorio"-- sabe de la barra, de su marca, de la caja de
    // Ejecutar y del degradado. NO SABE QUE EXISTEN LOS ICONOS. Y todo lo que
    // restaura fondo le pregunta a ella, asi que arrastrar la terminal por
    // encima de la rejilla se comia los iconos POR FRANJAS --una por evento de
    // raton-- y no volvian hasta reiniciar.
    //
    // La funcion que hacia falta llevaba escrita desde el principio
    // (`launcher::area`, con el comentario "para que quien repinte el fondo
    // sepa que tiene que volver a pintar esto encima") y **no la llamaba
    // nadie**: era el `warning: function 'area' is never used` de cada build.
    //
    // Va aqui y no en los treinta sitios que borran fondo, por lo mismo que
    // dice el parrafo de arriba: aqui solo se puede olvidar en un sitio.
    //
    // [!] Y ANTES del `if !visible`: la caja de Ejecutar puede estar escondida
    // y aun asi haberse borrado fondo encima de la rejilla --cerrar Datos,
    // Sonido o CABINA sobre ella--. Con el `return` delante, esos tres casos se
    // quedaban sin arreglar y el fallo habria parecido a medias arreglado, que
    // es peor que no tocarlo.
    //
    // Se repinta entera aunque lo borrado no la tocara. Cuesta unos 6.000
    // tests de pixel; lo que acaba de pasar por delante fueron 325.000
    // escrituras pixel a pixel, asi que no es donde esta el gasto. Recortarlo
    // al area danada es el trabajo del `.maqueta` -- ver PLAN_LA_CARA_VIAJA.
    //
    // ** Y NO se repinta si la caja la tapa entera, y eso no es ahorro: es
    // CORRECCION. La rejilla va por debajo de las ventanas, asi que pintarla
    // bajo una que la cubre le dibujaria los iconos ENCIMA. Aqui se sabe de una
    // --la de Ejecutar, que se repinta cuatro lineas mas abajo-- y para eso
    // sirve `launcher::area`.
    //
    // [!] Queda un hueco conocido: si es la ventana de Datos o la de Sonido la
    // que tapa la rejilla, esto le pinta los iconos encima hasta que esa
    // ventana se repinte. Es el mismo agujero que ya tenia `paint_run_box`
    // aqui, y no se tapa con otro `if`: se tapa cuando el mueble del escritorio
    // sea una lista de pintado que se pueda reproducir RECORTADA.
    let (gx, gy, gw, gh) = scene::launcher::area(p, launcher);
    let tapada = visible
        && gw > 0
        && run_box.x <= gx
        && run_box.y <= gy
        && run_box.x + run_box.w() >= gx + gw
        && run_box.y + run_box.h() >= gy + gh;
    if !tapada {
        scene::launcher::paint(p, launcher);
    }

    if !visible {
        return;
    }
    // La caja va DESPUES: esta por encima de la rejilla, y pintarla al reves
    // dejaria los iconos encima de la ventana.
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
/// * Apartarse DORMIDO, que no es lo mismo que apartarse cediendo.
///
/// `yield_screen()` devuelve el turno pero deja la tarea **LISTA**: el
/// planificador se la encuentra otra vez en la ronda siguiente, y una tarea que
/// no tiene nada que hacer sigue costando dos cambios de contexto por vuelta.
/// `wait(0, _, plazo)` la deja **BLOQUEADA** -- fuera de la lista de listos-- y
/// el temporizador la despierta al vencer el plazo.
///
/// # Por que 20 ms, y no mas ni menos
///
/// Es el unico numero de esta funcion que se elige por comodidad y no por una
/// medida, asi que conviene decir a que se renuncia: **el escritorio tarda como
/// mucho 20 ms de mas** en darse cuenta de que el programa tomo la pantalla o
/// de que se murio. Un ojo no ve 20 ms. A cambio, mientras el otro juega, este
/// se despierta 50 veces por segundo en vez de en cada ronda del planificador.
///
/// [!] El plazo **no puede ser cero**: `wait` con `timeout_ns = 0` y esperable
/// `0` calcula `deadline = 0`, y un bloqueo sin plazo y sin clave no lo
/// despierta nadie. Seria colgar el escritorio para siempre.
fn dormir_un_rato() {
    const VEINTE_MS_EN_NS: u64 = 20_000_000;
    bmo::wait(0, 0, VEINTE_MS_EN_NS);
}

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
    // ** FASE 1: SE ESPERA A QUE ESTE VIVO, NO A QUE SEA RAPIDO.
    //
    // Aqui habia un cronometro de **500 ms**, y ese numero es la razon de que
    // DOOM no se pudiera lanzar desde el escritorio en todo el 14-08.
    //
    // Un programa grafico no reclama la pantalla al arrancar: la reclama cuando
    // llega a su `I_InitGraphics`, y antes de eso hace su trabajo. DOOM lee un
    // WAD de 4 MiB por AHCI (que sondea con el CPU parado), arma texturas y
    // sprites en `R_Init` y calcula el SHA-1 del WAD entero en `D_CheckNetGame`.
    // **Medido en metal: tarda unos DIEZ SEGUNDOS**, veinte veces el plazo.
    //
    // Y lo que pasaba al vencer el plazo no era esperar de mas: era lo
    // contrario. El escritorio decidia *"no la queria"*, **volvia a reclamar la
    // pantalla**, y cuando DOOM por fin la pedia se encontraba con que ya tenia
    // dueno -> `DOOM: no hay pantalla (la tiene otro proceso)`. O sea que el
    // unico camino que sabe devolver la pantalla al escritorio era justo el que
    // no se podia usar, y habia que lanzar desde el shell de Ring 0 -- donde no
    // hay nadie que la recupere y se acaba en el panel del kernel.
    //
    // ** El arreglo no es un plazo mas largo: es **dejar de medir el tiempo y
    // medir lo que de verdad importa**. Aqui se llega solo si el `.bex` DECLARO
    // `WANTS_SCREEN`, o sea que la pregunta *"la queria?"* ya esta contestada
    // por el binario. Lo unico que puede acabar la espera es que la tome o que
    // se muera, y las dos se saben preguntando:
    //
    //     INFO_PANTALLA_DUENO != 0        la tomo
    //     INFO_TAREAS_TOTAL   < antes     se murio sin llegar a pedirla
    //
    // [!] El tope se queda, pero cambia de oficio: ya no decide nada, es el
    // cinturon para el programa que ni la toma ni se muere (un cuelgue antes de
    // `claim`). Treinta segundos es holgado para el arranque mas lento que
    // existe hoy y sigue siendo finito, que es lo que impide que un programa
    // colgado se lleve el escritorio por delante.
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    let mut took_it = false;
    let vivas_antes = bmo::info(bmo::INFO_TAREAS_TOTAL);
    if hz > 0 {
        let limit = bmo::ciclos() + hz * 30;
        while bmo::ciclos() < limit {
            if bmo::info(bmo::INFO_PANTALLA_DUENO) != 0 {
                took_it = true;
                break;
            }
            // Se murio antes de pedirla: no hay nada que esperar, y esperarlo
            // seria dejar el escritorio negro treinta segundos por un programa
            // que ya no existe.
            if bmo::info(bmo::INFO_TAREAS_TOTAL) < vivas_antes {
                break;
            }
            dormir_un_rato();
        }
    }
    if took_it {
        // ** FASE 2: MIENTRAS EL OTRO JUEGA, ESTE SE APARTA DE VERDAD.
        //
        // Este bucle dura **lo que dure el programa** -- una partida entera de
        // DOOM. Y hasta hoy era `yield_screen()` a pelo, o sea que el
        // escritorio se quedaba en la lista de LISTOS dando vueltas: pide el
        // dueno de la pantalla, cede, y el planificador se lo vuelve a
        // encontrar en la siguiente ronda. No quema el quantum --ceder es lo
        // correcto-- pero **sigue siendo una tarea despierta que no tiene nada
        // que hacer**, y cada vuelta son dos syscalls y dos cambios de
        // contexto que le salen del turno al que si esta trabajando.
        //
        // Con `wait` la tarea queda **BLOQUEADA**: sale de la lista de listos,
        // el planificador ni la mira, y el temporizador la devuelve al vencer
        // el plazo (`scheduler::on_timer` barre los `wait_deadline`). Eso es
        // lo que hace que la frase de la casa --"un juego de un solo hilo
        // tiene el nucleo entero por construccion"-- sea cierta tambien
        // cuando lo lanza el escritorio y no el shell.
        while bmo::info(bmo::INFO_PANTALLA_DUENO) != 0 {
            dormir_un_rato();
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
    // Quien tenia el foco la vuelta pasada. Sirve para no repetir la misma
    // orden al kernel sesenta veces por segundo: el turno largo se pide
    // cuando el foco CAMBIA, no mientras siga donde estaba.
    let mut foco_antes: Option<desktop::Ventana> = None;
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

        dsk.tick.pulse();
        dsk.tick.repaint_field = false;

        // -- * ALGUIEN OFRECE UNA SUPERFICIE? --
        //
        // Se pregunta al kernel una vez por vuelta y casi siempre dice que no.
        // Es el precio de no tener que avisar: una app ofrece cuando le viene
        // bien --puede ser en su primer fotograma o en el mil-- y el DIRECTOR se
        // entera **mirando**, no porque nadie le mande un mensaje. Una operacion
        // que ya existia y ninguna cola nueva.
        let mut born = false;
        if let Some(hueco) = dsk.table.collect(&p) {
            born = true;
            // ** Y SE LE DICE AL FOCO QUE EXISTE. Hasta hoy una app tenia
            // caja pero no nombre: `bmo_input::foco` habla en ids y ninguno
            // era suyo, asi que Alt+Tab pasaba de largo por encima de una
            // ventana que se estaba viendo.
            dsk.win.focus.open(desktop::Ventana::App(hueco as u8));
        }
        // Y las que se quedaron sin dueno. Va ANTES de pintar nada: la ventana
        // de una app muerta tiene que desaparecer en el mismo fotograma en que
        // se sabe, no en el siguiente.
        let dead = dsk.table.reap_dead(&mut dsk.tick.dead_boxes);

        // ** EL FOCO SE CONVIERTE EN TURNO, Y EN UN SOLO SITIO.
        //
        // Aqui y no en el clic: por el clic no pasa Alt+Tab, y tener la
        // misma regla en dos sitios es como se acaba con dos reglas que no
        // dicen lo mismo. Una vez por vuelta, y solo cuando CAMBIA -- si no,
        // serian dos cruces de puerta por fotograma para repetir lo que ya
        // era verdad.
        //
        // No sube prioridad: alarga el turno. La app de delante corriendo
        // por encima del DIRECTOR seria la primera en dejar de refrescarse.
        let foco_ahora = dsk.win.focus.actual();
        if foco_ahora != foco_antes {
            foco_antes = foco_ahora;
            if let Some(desktop::Ventana::App(i)) = foco_ahora {
                if let Some(tid) = dsk.table.get_mut(i as usize).map(|s| s.tid) {
                    if let Some(h) = bmo::Hijo::por_tid(tid) {
                        h.delante(true);
                    }
                }
            }
        }

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

        // -- LA ENTRADA, en dos mitades que no se pueden mezclar --
        //
        // ** RECOGER toma prestada la capability; INTERPRETAR no. Acotar ese
        // prestamo a `gather` es lo que deja `input` libre para que
        // `lend_screen` se lo lleve POR VALOR mas abajo -- y de paso es lo que
        // partia en dos los "90 nombres libres" que tenia este bloque.
        let gathered = input.as_ref().map(|e| desktop::keys::gather(&mut dsk, e));
        if let Some(g) = gathered {
            // ** SON FLANCOS, no estados, y la diferencia importa.
            //
            // `(botones != 0) != button_before` pinta cuando el boton CAMBIA;
            // `botones != 0` a secas repintaria cada fotograma que se tenga
            // pulsado. Y sin los dos ultimos, soltar Alt no cuenta como motivo
            // para pintar -- que es justo el fotograma en el que hay que BORRAR
            // el conmutador de Alt+Tab.
            dsk.tick.will_paint |= g.nt > 0
                || g.wheel != 0
                || g.pos.x != dsk.tick.ax
                || g.pos.y != dsk.tick.ay
                || (g.pos.botones != 0) != dsk.tick.button_before
                || g.alt_alone != dsk.win.alt_before
                || g.combo != dsk.tick.combo_before;
            if dsk.tick.will_paint {
                dsk.save_under.lift(&p);
            }

            desktop::keys::edges(&mut dsk, &p, &g);

            // ** Y AQUI `run`, que es lo unico que el teclado NO puede hacer.
            //
            // El editor decide y devuelve la ruta; ejecutarla exige la pantalla
            // y la entrada POR VALOR, y esas dos son de `_start`.
            if let Some((buf, tn)) = desktop::keys::dispatch(&mut dsk, &p, &g) {
                let target = &buf[..tn];
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
                } else {
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
                dsk.field.cur = dsk.field.cur.min(dsk.field.n);
                dsk.tick.repaint_field = true;
            }

            desktop::mouse::on_pointer(&mut dsk, &p, g.pos, g.wheel, g.ctrl);
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

