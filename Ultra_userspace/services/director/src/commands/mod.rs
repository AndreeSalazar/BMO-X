//! **Lo que se INTERPRETA.**
//!
//! De una linea de texto a una intencion. Aqui no se pinta nada: un modulo de
//! esta carpeta no sabe de que color es la ventana.

pub(crate) mod complete;
/// ** LA TERMINAL DEL DISCO: la unica caja de ordenes que ACTUA sobre el
/// almacen. Fichero propio por eso y no por tamano. Ver su cabecera.
pub(crate) mod disco;
/// ** POR DONDE EMPEZAR. La orden que faltaba, y la pidio quien lo escribio
/// todo: *"ironicamente yo como creador no se usar"*. Va por TAREAS y no por
/// ordenes -- ver su cabecera.
pub(crate) mod guia;
pub(crate) mod dispatch;
pub(crate) mod files;
pub(crate) mod shell;
pub(crate) mod system;
pub(crate) use dispatch::{dispatch, After};

pub(crate) mod history;
pub(crate) mod reports;

// -- La linea de comandos ------------------------------------------------

/// Que pidio el usuario. Se separa del bucle porque la decision "esto es un
/// comando o es una ruta" merece leerse de un vistazo.
pub(crate) enum Command<'a> {
    Nothing,
    /// Alguien escribio `sudo`, `pacman`, `apt`... **Esto NO es una distro.**
    ///
    /// Se escribio el dia que un amigo del dueno, que viene de Linux, se sento
    /// delante y dio por hecho que lo era. Y es un malentendido razonable: hay
    /// un escritorio, hay ventanas y hay una caja donde se teclea.
    ///
    /// Contestar "no lo conozco" habria sido correcto y no habria ensenado
    /// nada. Esto contesta con lo que de verdad separa a los dos sistemas --
    /// aqui no hay usuarios, ni permisos que elevar, ni paquetes que instalar:
    /// hay capabilities, y lo que no te dieron no existe para ti. Con un gato.
    NotLinux(&'a [u8]),
    Launch(&'a [u8]),
    Clear,
    Help,
    /// Ensena o esconde la calculadora.
    Calculator,
    /// `sella` escrito AQUI, donde ya no vive: la orden se mudo a la ventana de
    /// ESTRATOS (F12, tecla `S`) y esto lleva la nota con la direccion nueva.
    SealMoved,
    /// `perf` -- **lo que cuesta pintar**, medido.
    ///
    /// Existe para poder contestar con un numero la pregunta "hace falta una
    /// GPU?". La caja de sucio ya evita casi todo el trabajo, asi que la
    /// respuesta puede perfectamente ser que no -- y eso solo se sabe mirando.
    PaintCost,
    /// `ls [path]` -- que hay en el disco. Antes esto no podia existir: no
    /// habia capability de directorio, asi que habia que saberse los nombres
    /// de memoria y teclearlos enteros.
    List(&'a [u8]),
    /// `lee <path>` -- ensena lo que hay DENTRO de un archivo. Es el hermano
    /// de `ls`: aquel dice que archivos hay, este los abre.
    Read(&'a [u8]),
    /// `escribe <path> <text>` -- crea un archivo con ese texto.
    ///
    /// Es la primera vez que Ring 3 GUARDA algo. Hasta ahora todo lo que
    /// aparecia en el disco lo habia puesto el anfitrion al flashear, o el
    /// kernel con su caja negra; un programa no tenia con que.
    Write(&'a [u8], &'a [u8]),
    /// `guarda [path]` -- **vuelca el historial de la salida a un `.txt`**.
    ///
    /// === Para que existe ===
    ///
    /// Depurar BMO-X era *flashear y hacerle una foto a la pantalla*. Eso vale
    /// para ver si algo arranca; no vale para comparar dos corridas, ni para
    /// leer doscientas lineas, ni para que nadie que no este delante de la
    /// maquina sepa que paso. Y una foto no se puede diferenciar contra la de
    /// ayer.
    ///
    /// Con esto la corrida deja un archivo en `data/`, que esta en la
    /// **particion FAT32**: se enchufa el disco a un Windows y se abre con el
    /// bloc de notas. Eso convierte "cuentame que salio" en "mira el fichero".
    ///
    /// * Y por eso va a FAT32 y no a ESTRATOS, aunque ESTRATOS sea el sistema
    /// de ficheros bueno: **ningun otro sistema operativo sabe leer ESTRATOS**.
    /// Un volcado que solo BMO puede abrir no resuelve el problema que este
    /// comando existe para resolver.
    ///
    /// Sin ruta, va a [`crate::DEFAULT_DUMP`].
    Save(&'a [u8]),
    /// Parece un archivo, pero no es un `.bex`. No se intenta lanzar: se dice
    /// que es y con que se abre.
    NotAProgram(&'a [u8]),
    /// `info` -- el informe del sistema. `cpu` y `mem` son las dos mitades.
    ///
    /// * Esto vivia SOLO en el shell de Ring 0, y no porque hiciera falta el
    /// privilegio: porque los datos estaban a su alcance. Contar RAM no ejerce
    /// ningun poder. Ahora bajan por `OP_INFO` y se pintan aqui, que es donde
    /// esta la pantalla.
    Report,
    /// El informe del ULTIMO fallo de Ring 3, tal como lo redacto el kernel.
    Autopsy,
    Placa,
    Cpu,
    /// **El censo de extensiones**: que declara este silicio y que coge BMO.
    ///
    /// Vivia SOLO en el shell de Ring 0, y a ese shell no se vuelve una vez
    /// arranca el escritorio -- el rescate se niega a echar al compositor. O
    /// sea que era una tabla correcta que nadie podia mirar desde donde de
    /// verdad se trabaja. Baja por `OP_INFO` como todo lo demas: dos mascaras
    /// y los nombres, sin una segunda lista en este lado.
    Ext,
    Memoria,
    /// **El consumo, en tabla.** `cpu` y `mem` explican la maquina cada uno por
    /// su lado; esto contesta "que esta gastando ahora mismo" en una sola
    /// pantalla y con los numeros alineados, para poder comparar dos volcados.
    /// Va tambien dentro de cada `save`.
    Consumo,
    /// **Que programa tiene RAM pedida, uno por fila.** Es el instrumento de la
    /// fuga que se cerro el 14-08: un programa que muere tiene que desaparecer
    /// de esta tabla.
    Apps,
    /// **Los nucleos.** Sin argumento solo censa; con `all` o con un numero,
    /// despierta. Es la unica orden de esta caja que puede tardar casi un
    /// segundo, y por eso el mensaje va ANTES de llamar.
    Smp(&'a [u8]),
    /// **`banda`**: el ancho de banda de la memoria, por barrido.
    Banda,
    /// **`audio`** -- le pregunta al aparato de audio como quiere las muestras.
    ///
    /// [!] Existia solo en el shell de Ring 0 y el dueno la escribio AQUI, que
    /// es donde se trabaja todos los dias. Contesto *"no es un comando ni una
    /// ruta"* y la prueba del paso 0 se quedo sin hacer. **Dos shells con dos
    /// vocabularios distintos son dos productos.**
    Audio,
    /// **LA RED** -- `red`, `net`, `mac`, `link`, `link`, `frames_rx`, `phy`.
    ///
    /// ** Siete palabras y no una, porque son siete PREGUNTAS distintas y una
    /// sola respuesta gorda no sirve para depurar: cuando el cable no va, lo que
    /// hace falta es aislar *"no hay tarjeta"* de *"hay tarjeta y no hay
    /// enlace"* de *"hay enlace y no llega nada"* de *"llega y no lo estamos
    /// escuchando"*. Cada palabra corta el problema por un sitio.
    ///
    /// El argumento decide cual: `red` a secas da el informe entero.
    Net(&'a [u8]),
    /// **`disco`** -- la terminal de administracion del almacen.
    ///
    /// === Por que es una PALABRA CON ORDENES DENTRO y no seis verbos sueltos ===
    ///
    /// Porque todas hablan del mismo aparato y **una de ellas es destructiva**.
    /// `trim` suelto, entre `tramas` y `smp`, seria un verbo de cuatro letras que
    /// se puede teclear sin querer y que le dice a un SSD que olvide sectores.
    /// Con el sustantivo delante hay que nombrar al aparato antes de darle una
    /// orden, y eso ya es la mitad de una confirmacion.
    ///
    /// La otra mitad es que el verbo solo **propone**: `disco trim` ensena lo que
    /// haria y `disco trim ya` lo hace. Es lo que pide la seccion 9 de ESTRATOS
    /// --*"con lo que va a soltar listado antes de hacerlo"*-- y aqui no es
    /// cortesia: es la unica orden del escritorio que no se puede deshacer.
    ///
    /// [!] Y **ninguna acepta un LBA**. Ver `commands/disco.rs`.
    ///
    /// ** Lleva DOS trozos --la suborden y su argumento-- y no la cola en crudo.
    /// La primera version comparaba la cadena entera contra `b"trim ya"`, o sea
    /// que `disco trim  ya` con dos espacios **no era la misma orden**: caia en
    /// "no la conozco" sin decir por que. Partir por el primer espacio es lo que
    /// [`parse`] ya hace con el verbo de arriba; hacerlo una vez mas cuesta dos
    /// lineas y quita una clase entera de sorpresa.
    Disco(&'a [u8], &'a [u8]),
    /// **`guia`** -- por donde empezar, por TAREAS y no por ordenes.
    ///
    /// No es un tercer catalogo: `ayuda` lista los verbos y esto contesta *"que
    /// quiero hacer"*. Y su ultimo bloque es el que de verdad hacia falta --
    /// **lo que todavia NO se puede**, para no buscarlo media hora.
    Guia,
    /// **`estratos escribe <nombre> <texto>`** -- el primer fichero que BMO-X
    /// guarda en SU sistema de ficheros.
    ///
    /// ** El sustantivo va delante por lo mismo que en `disco`: ya hay un
    /// `escribe` y va a la FAT32. Dos ordenes con el mismo verbo y dos
    /// volumenes distintos es como se guarda algo donde no se queria.
    EstratosEscribe(&'a [u8], &'a [u8]),
    /// `reboot` -- reinicia la maquina y no vuelve.
    ///
    /// Estaba en el shell del kernel desde siempre y aqui contestaba "no lo
    /// conozco", asi que la unica forma de reiniciar era el boton de la caja.
    /// Reiniciar es tocar puertos de E/S, que Ring 3 no puede hacer: va por
    /// `OP_REINICIAR`, una operacion mas dentro de `INVOKE`.
    Reboot,
    /// Una palabra suelta que no parece una ruta.
    Unknown,
}


pub(crate) fn looks_like_path(t: &[u8]) -> bool {
    t.iter().any(|&c| c == b'/' || c == b'\\' || c == b'.')
}

/// Esto es un PROGRAMA, o sea algo que tenga sentido lanzar?
///
/// * Antes bastaba con que llevara un punto o una barra, y por eso escribir
/// `leeme.txt` a pelo intentaba EJECUTARLO. El kernel contestaba "sin firma no
/// hay ejecucion" --que es exactamente lo correcto-- y el usuario se quedaba
/// creyendo que el sistema le pedia un permiso especial para LEER un fichero
/// de texto. No se lo pedia: es que nadie le habia dicho que queria leerlo.
///
/// La conclusion de la que hay que huir es "hace falta un modo administrador".
/// Aqui no se afloja ninguna guardia: se deja de adivinar. Solo un `.bex` es
/// un programa; lo demas son datos, y a los datos se los lee.
///
/// `run <path>` sigue intentandolo con lo que sea: si alguien lo escribe
/// explicitamente, la respuesta la da el gate y no esta heuristica.
pub(crate) fn looks_like_program(t: &[u8]) -> bool {
    let n = t.len();
    if n < 4 {
        return false;
    }
    let queue = &t[n - 4..];
    queue[0] == b'.'
        && (queue[1] | 32) == b'b'
        && (queue[2] | 32) == b'e'
        && (queue[3] | 32) == b'x'
}

/// Parte la linea en verbo y resto.
///
/// * Acepta `run <path>` ADEMAS de la ruta pelada, y no por capricho: quien usa
/// esto viene del shell de Ring 0, donde se escribe `run`. Pelearse con la
/// costumbre del usuario es perder -- el que se adapta es el programa. Lo que si
/// se hace es DECIRLO cuando la palabra no es ni comando ni ruta, en vez de
/// contestar "no esta: revisa la ruta" a alguien que escribio `reboot`.
pub(crate) fn parse(line: &[u8]) -> Command<'_> {
    let line = {
        let mut i = 0;
        while i < line.len() && line[i] == b' ' { i += 1; }
        &line[i..]
    };
    if line.is_empty() {
        return Command::Nothing;
    }
    let cut = line.iter().position(|&c| c == b' ').unwrap_or(line.len());
    let (verb, rest) = line.split_at(cut);
    let rest = {
        let mut i = 0;
        while i < rest.len() && rest[i] == b' ' { i += 1; }
        &rest[i..]
    };
    // ** LOS QUE LLEGAN DE LINUX.
    //
    // Va ANTES del despacho normal a proposito: ninguna de estas palabras es
    // una orden de BMO-X, asi que caerian en "no lo conozco" -- que es correcto
    // y no ensena nada. Que la respuesta llegue aqui cuesta un `contains` y
    // convierte un desconcierto en una explicacion.
    const FROM_LINUX: [&[u8]; 14] = [
        b"sudo", b"su", b"apt", b"apt-get", b"pacman", b"yay", b"dnf", b"yum",
        b"snap", b"systemctl", b"chmod", b"chown", b"grep", b"man",
    ];
    if FROM_LINUX.iter().any(|&x| x == verb) {
        return Command::NotLinux(verb);
    }

    match verb {
        // INGLES de primero, y es una decision del dueno: el castellano limita
        // -- no hay palabra corta para "flush", los verbos se alargan, y medio
        // mundo del sistema (los campos del hardware, los mensajes de fallo)
        // ya esta en ingles. El castellano entra cuando el sistema este
        // maduro y se pueda hacer entero, no a medias.
        //
        // Los castellanos se quedan como SINONIMOS: no estorban y ya estaban
        // escritos.
        b"run" | b"corre" | b"lanza" => {
            if rest.is_empty() { Command::Help } else { Command::Launch(rest) }
        }
        b"calc" | b"calculadora" => Command::Calculator,
        // * El numero que decide si hace falta una GPU. Ver `Volcado`.
        b"perf" | b"pinta" => Command::PaintCost,
        // * `sella` -- Y ANTES ERAN DOS PALABRAS, POR UN MIEDO MAL PUESTO.
        //
        // Era `estratos sellar`, y el comentario que lo defendia decia que al
        // ser *"lo primero del sistema que escribe en el disco"* no podia ser un
        // verbo suelto. Sonaba prudente y protegia lo que no hacia falta
        // proteger: `sellar()` cierra una transaccion **SIN DATOS**. No reserva
        // un bloque, no toca un objeto, y commitea apuntando al mismo estrato
        // que ya habia. Lo peor que puede hacer un sellado accidental es
        // **subir la generacion en uno**.
        //
        // O sea que la defensa costaba descubribilidad --el dueno la busco el
        // 2026-08-13 teniendola delante y no la encontro-- a cambio de evitar un
        // dano que no existe. El dia que `sella` escriba datos DE VERDAD, la
        // proteccion que hara falta es una confirmacion que diga QUE se va a
        // escribir, no una palabra mas larga.
        //
        // El nombre sigue la gramatica de la casa, que es imperativo corto:
        // `lee`, `guarda`, `lista`, `pinta`... y ahora `sella`. Y dice lo que
        // hace: en ESTRATOS un commit **sella un estrato**.
        //
        // `estratos sellar` se queda como sinonimo: ya estaba escrito en la
        // ayuda, en dos documentos y en la cabeza del dueno.
        // ** EL SELLO SE MUDO A LA VENTANA DE ESTRATOS (F12, tecla `S`).
        //
        // Decision del dueno el 2026-08-13: *"el terminal Ctrl+Alt que se lleve
        // el sello"*. Y es la correcta -- **el verbo vive donde vive el
        // objeto**. Este terminal lanza programas y mira el sistema; sellar es
        // de ESTRATOS, y ESTRATOS tiene su propia ventana con su propio cursor
        // dentro del volumen.
        //
        // Lo que NO se hace es borrarlo y ya: quien escriba `sella` aqui --que
        // es lo que estaba escrito ayer en la linea de ayuda, en dos documentos
        // y en la cabeza del dueno-- se lleva **la direccion nueva**, no un
        // "no lo conozco". Una funcion que se muda sin dejar nota se convierte
        // en una funcion que desaparecio.
        b"sella" | b"sellar" => Command::SealMoved,
        // ** `estratos escribe <nombre> <texto>` -- EL SUSTANTIVO DELANTE,
        // igual que en `disco` y por el mismo motivo: escribe en el almacen y
        // ya hay un `escribe` que va a la FAT32. Sin el sustantivo, dos ordenes
        // con el mismo verbo irian a dos volumenes distintos.
        b"estratos" => {
            if rest == b"sellar" {
                return Command::SealMoved;
            }
            let k = rest.iter().position(|&c| c == b' ').unwrap_or(rest.len());
            let (sub, arg) = rest.split_at(k);
            let mut j = 0;
            while j < arg.len() && arg[j] == b' ' { j += 1; }
            let arg = &arg[j..];
            if sub != b"escribe" && sub != b"write" && sub != b"guarda" {
                return Command::Help;
            }
            // El nombre es la PRIMERA palabra y el texto todo lo demas,
            // espacios incluidos -- la misma regla que `escribe`.
            match arg.iter().position(|&c| c == b' ') {
                Some(k) => {
                    let (nombre, texto) = arg.split_at(k);
                    let mut j = 0;
                    while j < texto.len() && texto[j] == b' ' { j += 1; }
                    Command::EstratosEscribe(nombre, &texto[j..])
                }
                None => Command::Help,
            }
        }
        b"clear" | b"cls" | b"limpia" => Command::Clear,
        b"ls" | b"dir" | b"lista" => Command::List(rest),
        b"cat" | b"lee" => {
            if rest.is_empty() { Command::Help } else { Command::Read(rest) }
        }
        // `escribe <path> <text>`: la ruta es la PRIMERA palabra y el texto
        // es todo lo demas, espacios incluidos. Partir por la ultima palabra
        // obligaria a escribir el texto sin espacios, que no es escribir.
        b"escribe" | b"write" => {
            let k = rest.iter().position(|&c| c == b' ');
            match k {
                Some(k) => {
                    let (path, text) = rest.split_at(k);
                    let mut j = 0;
                    while j < text.len() && text[j] == b' ' { j += 1; }
                    Command::Write(path, &text[j..])
                }
                None => Command::Help,
            }
        }
        // `guarda` sin nada vuelca al fichero de siempre; con una ruta, ahi.
        // No pide texto como `escribe`: lo que guarda ya esta en la pantalla.
        //
        // ** `save` entra por peticion del dueno, y el motivo es el uso real:
        // esta es la orden que MAS se teclea --cada sesion acaba con ella-- y
        // `guarda` son seis letras en un teclado que ademas ha estado fallando.
        // Cuatro letras y sin acentos. Los nombres viejos se quedan: quitarlos
        // no ahorraria nada y rompe lo que ya esta en las notas.
        b"guarda" | b"save" | b"volcar" | b"dump" => Command::Save(rest),
        b"info" | b"sistema" => Command::Report,
        // `fallo` ensena la ultima autopsia. Se guarda sola en `data/fallos.txt`
        // en cuanto ocurre -- esto es para mirarla sin salir del escritorio.
        b"fallo" | b"fallos" | b"autopsia" => Command::Autopsy,
        // ** LA RED. `red`/`net` dan el informe entero; las demas cortan por
        // una sola pregunta, que es lo que se quiere teniendo el cable en la
        // mano. Ninguna transmite ni un byte -- son campos de INFORME.
        // ** `net` a secas INFORMA; `net rx` ARMA el receptor. La misma forma
        // que en el shell de Ring 0 --la palabra sola censa y el argumento
        // actua-- y por el mismo motivo: armar deja a un aparato escribir en la
        // memoria de esta maquina, y eso no se consigue tecleando el comando de
        // diagnostico.
        //
        // *** Y hasta el 2026-08-24 este brazo TIRABA el argumento
        // (`Command::Net(b"")`), asi que `net rx` desde el escritorio no armaba
        // nada y el panel mandaba al shell de Ring 0 -- al que el dueno no
        // vuelve. Ver `bmo::red`.
        b"red" | b"net" => Command::Net(rest),
        b"mac" => Command::Net(b"mac"),
        b"enlace" | b"link" => Command::Net(b"link"),
        b"tramas" | b"frames" => Command::Net(b"frames"),
        b"phy" => Command::Net(b"phy"),
        // ** EL DISCO. El sustantivo va delante a proposito: es la unica caja de
        // ordenes del escritorio que puede cambiar el almacen, y `trim` suelto
        // seria un verbo de cuatro letras con consecuencias que no se deshacen.
        // `almacen` entra como sinonimo porque es la palabra del diseno.
        b"disco" | b"almacen" => {
            // La suborden y su argumento, con la MISMA regla que el verbo de
            // arriba: hasta el primer espacio, y lo que sigue sin los espacios
            // de delante. Asi `disco trim ya` y `disco trim   ya` son la misma
            // orden, que es lo que cualquiera espera al teclear.
            let k = rest.iter().position(|&c| c == b' ').unwrap_or(rest.len());
            let (sub, arg) = rest.split_at(k);
            let mut j = 0;
            while j < arg.len() && arg[j] == b' ' { j += 1; }
            Command::Disco(sub, &arg[j..])
        }
        // ** Lo que la PLACA cuenta de si misma: que tablas ofrece el
        // firmware, donde vive la config de PCIe, si hay IOMMU. Contesta y no
        // concede: no cambia nada.
        b"placa" | b"firmware" => Command::Placa,
        b"cpu" | b"procesador" => Command::Cpu,
        // Los mismos dos nombres que el shell de Ring 0, para que lo que se
        // aprende en un sitio valga en el otro.
        b"ext" | b"extensiones" => Command::Ext,
        b"consumo" | b"gasto" | b"w" => Command::Consumo,
        b"apps" | b"programas" => Command::Apps,
        b"mem" | b"ram" | b"memoria" => Command::Memoria,
        b"reboot" | b"reinicia" | b"reiniciar" => Command::Reboot,
        // `smp` a secas CENSA y no toca nada; `smp all` despierta a todos;
        // `smp N` despierta exactamente N. El caso sin argumento es el
        // inofensivo a proposito: ver `sys::smp_despertar`.
        b"smp" | b"nucleos" => Command::Smp(rest),
        // ** `banda` entro el 2026-08-24 y ya existia... en el shell de Ring 0,
        // al que desde aqui NO SE VUELVE. Estaba escrita, compilada y probada, y
        // era inalcanzable desde el unico sitio donde el dueno trabaja.
        b"banda" | b"memoria" => Command::Banda,
        b"audio" | b"sonido" => Command::Audio,
        b"help" | b"?" | b"ayuda" => Command::Help,
        // La puerta del que llega. `start` porque es la palabra que se
        // teclea sin pensar cuando uno no sabe que teclear.
        b"guia" | b"empezar" | b"start" => Command::Guia,
        _ if looks_like_program(line) => Command::Launch(line),
        // Parece un archivo pero no es un programa. Antes esto caia en
        // `Launch` y el kernel contestaba "sin firma no hay ejecucion" -- un
        // mensaje CORRECTO que en este sitio se lee como si el sistema pidiera
        // permisos para abrir un .txt.
        _ if looks_like_path(line) => Command::NotAProgram(line),
        _ => Command::Unknown,
    }
}

