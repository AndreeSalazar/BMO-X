//! **Lo que se INTERPRETA.**
//!
//! De una linea de texto a una intencion. Aqui no se pinta nada: un modulo de
//! esta carpeta no sabe de que color es la ventana.

pub(crate) mod complete;
pub(crate) mod historial;
pub(crate) mod informes;

// -- La linea de comandos ------------------------------------------------

/// Que pidio el usuario. Se separa del bucle porque la decision "esto es un
/// comando o es una ruta" merece leerse de un vistazo.
pub(crate) enum Orden<'a> {
    Nada,
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
    NoEsLinux(&'a [u8]),
    Lanzar(&'a [u8]),
    Limpiar,
    Ayuda,
    /// Ensena o esconde la calculadora.
    Calculadora,
    /// `estratos sellar` -- **cierra una transaccion vacia. ESCRIBE EN EL DISCO.**
    ///
    /// La primera orden del escritorio que cambia el almacen. Pide dos palabras
    /// a proposito: ver el analizador.
    EstratosSellar,
    /// `perf` -- **lo que cuesta pintar**, medido.
    ///
    /// Existe para poder contestar con un numero la pregunta "hace falta una
    /// GPU?". La caja de sucio ya evita casi todo el trabajo, asi que la
    /// respuesta puede perfectamente ser que no -- y eso solo se sabe mirando.
    Pintado,
    /// `ls [ruta]` -- que hay en el disco. Antes esto no podia existir: no
    /// habia capability de directorio, asi que habia que saberse los nombres
    /// de memoria y teclearlos enteros.
    Listar(&'a [u8]),
    /// `lee <ruta>` -- ensena lo que hay DENTRO de un archivo. Es el hermano
    /// de `ls`: aquel dice que archivos hay, este los abre.
    Leer(&'a [u8]),
    /// `escribe <ruta> <texto>` -- crea un archivo con ese texto.
    ///
    /// Es la primera vez que Ring 3 GUARDA algo. Hasta ahora todo lo que
    /// aparecia en el disco lo habia puesto el anfitrion al flashear, o el
    /// kernel con su caja negra; un programa no tenia con que.
    Escribir(&'a [u8], &'a [u8]),
    /// `guarda [ruta]` -- **vuelca el historial de la salida a un `.txt`**.
    ///
    /// === Para que existe ===
    ///
    /// Depurar BMO-X era *flashear y hacerle una foto a la pantalla*. Eso vale
    /// para ver si algo arranca; no vale para comparar dos corridas, ni para
    /// leer doscientas lineas, ni para que nadie que no este delante de la
    /// maquina sepa que paso. Y una foto no se puede diferenciar contra la de
    /// ayer.
    ///
    /// Con esto la corrida deja un archivo en `datos/`, que esta en la
    /// **particion FAT32**: se enchufa el disco a un Windows y se abre con el
    /// bloc de notas. Eso convierte "cuentame que salio" en "mira el fichero".
    ///
    /// * Y por eso va a FAT32 y no a ESTRATOS, aunque ESTRATOS sea el sistema
    /// de ficheros bueno: **ningun otro sistema operativo sabe leer ESTRATOS**.
    /// Un volcado que solo BMO puede abrir no resuelve el problema que este
    /// comando existe para resolver.
    ///
    /// Sin ruta, va a [`crate::VOLCADO_POR_DEFECTO`].
    Guardar(&'a [u8]),
    /// Parece un archivo, pero no es un `.bex`. No se intenta lanzar: se dice
    /// que es y con que se abre.
    NoEsPrograma(&'a [u8]),
    /// `info` -- el informe del sistema. `cpu` y `mem` son las dos mitades.
    ///
    /// * Esto vivia SOLO en el shell de Ring 0, y no porque hiciera falta el
    /// privilegio: porque los datos estaban a su alcance. Contar RAM no ejerce
    /// ningun poder. Ahora bajan por `OP_INFO` y se pintan aqui, que es donde
    /// esta la pantalla.
    Informe,
    /// El informe del ULTIMO fallo de Ring 3, tal como lo redacto el kernel.
    Autopsia,
    Cpu,
    Memoria,
    /// **Los nucleos.** Sin argumento solo censa; con `all` o con un numero,
    /// despierta. Es la unica orden de esta caja que puede tardar casi un
    /// segundo, y por eso el mensaje va ANTES de llamar.
    Smp(&'a [u8]),
    /// **`audio`** -- le pregunta al aparato de audio como quiere las muestras.
    ///
    /// [!] Existia solo en el shell de Ring 0 y el dueno la escribio AQUI, que
    /// es donde se trabaja todos los dias. Contesto *"no es un comando ni una
    /// ruta"* y la prueba del paso 0 se quedo sin hacer. **Dos shells con dos
    /// vocabularios distintos son dos productos.**
    Audio,
    /// `reboot` -- reinicia la maquina y no vuelve.
    ///
    /// Estaba en el shell del kernel desde siempre y aqui contestaba "no lo
    /// conozco", asi que la unica forma de reiniciar era el boton de la caja.
    /// Reiniciar es tocar puertos de E/S, que Ring 3 no puede hacer: va por
    /// `OP_REINICIAR`, una operacion mas dentro de `INVOKE`.
    Reiniciar,
    /// Una palabra suelta que no parece una ruta.
    Desconocida,
}


pub(crate) fn parece_ruta(t: &[u8]) -> bool {
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
/// `run <ruta>` sigue intentandolo con lo que sea: si alguien lo escribe
/// explicitamente, la respuesta la da el gate y no esta heuristica.
pub(crate) fn parece_programa(t: &[u8]) -> bool {
    let n = t.len();
    if n < 4 {
        return false;
    }
    let cola = &t[n - 4..];
    cola[0] == b'.'
        && (cola[1] | 32) == b'b'
        && (cola[2] | 32) == b'e'
        && (cola[3] | 32) == b'x'
}

/// Parte la linea en verbo y resto.
///
/// * Acepta `run <ruta>` ADEMAS de la ruta pelada, y no por capricho: quien usa
/// esto viene del shell de Ring 0, donde se escribe `run`. Pelearse con la
/// costumbre del usuario es perder -- el que se adapta es el programa. Lo que si
/// se hace es DECIRLO cuando la palabra no es ni comando ni ruta, en vez de
/// contestar "no esta: revisa la ruta" a alguien que escribio `reboot`.
pub(crate) fn interpretar(linea: &[u8]) -> Orden<'_> {
    let linea = {
        let mut i = 0;
        while i < linea.len() && linea[i] == b' ' { i += 1; }
        &linea[i..]
    };
    if linea.is_empty() {
        return Orden::Nada;
    }
    let corte = linea.iter().position(|&c| c == b' ').unwrap_or(linea.len());
    let (verbo, resto) = linea.split_at(corte);
    let resto = {
        let mut i = 0;
        while i < resto.len() && resto[i] == b' ' { i += 1; }
        &resto[i..]
    };
    // ** LOS QUE LLEGAN DE LINUX.
    //
    // Va ANTES del despacho normal a proposito: ninguna de estas palabras es
    // una orden de BMO-X, asi que caerian en "no lo conozco" -- que es correcto
    // y no ensena nada. Que la respuesta llegue aqui cuesta un `contains` y
    // convierte un desconcierto en una explicacion.
    const DE_LINUX: [&[u8]; 14] = [
        b"sudo", b"su", b"apt", b"apt-get", b"pacman", b"yay", b"dnf", b"yum",
        b"snap", b"systemctl", b"chmod", b"chown", b"grep", b"man",
    ];
    if DE_LINUX.iter().any(|&x| x == verbo) {
        return Orden::NoEsLinux(verbo);
    }

    match verbo {
        // INGLES de primero, y es una decision del dueno: el castellano limita
        // -- no hay palabra corta para "flush", los verbos se alargan, y medio
        // mundo del sistema (los campos del hardware, los mensajes de fallo)
        // ya esta en ingles. El castellano entra cuando el sistema este
        // maduro y se pueda hacer entero, no a medias.
        //
        // Los castellanos se quedan como SINONIMOS: no estorban y ya estaban
        // escritos.
        b"run" | b"corre" | b"lanza" => {
            if resto.is_empty() { Orden::Ayuda } else { Orden::Lanzar(resto) }
        }
        b"calc" | b"calculadora" => Orden::Calculadora,
        // * El numero que decide si hace falta una GPU. Ver `Volcado`.
        b"perf" | b"pinta" => Orden::Pintado,
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
        b"sella" | b"sellar" => Orden::EstratosSellar,
        b"estratos" => {
            if resto == b"sellar" { Orden::EstratosSellar } else { Orden::Ayuda }
        }
        b"clear" | b"cls" | b"limpia" => Orden::Limpiar,
        b"ls" | b"dir" | b"lista" => Orden::Listar(resto),
        b"cat" | b"lee" => {
            if resto.is_empty() { Orden::Ayuda } else { Orden::Leer(resto) }
        }
        // `escribe <ruta> <texto>`: la ruta es la PRIMERA palabra y el texto
        // es todo lo demas, espacios incluidos. Partir por la ultima palabra
        // obligaria a escribir el texto sin espacios, que no es escribir.
        b"escribe" | b"write" => {
            let k = resto.iter().position(|&c| c == b' ');
            match k {
                Some(k) => {
                    let (ruta, texto) = resto.split_at(k);
                    let mut j = 0;
                    while j < texto.len() && texto[j] == b' ' { j += 1; }
                    Orden::Escribir(ruta, &texto[j..])
                }
                None => Orden::Ayuda,
            }
        }
        // `guarda` sin nada vuelca al fichero de siempre; con una ruta, ahi.
        // No pide texto como `escribe`: lo que guarda ya esta en la pantalla.
        b"guarda" | b"volcar" | b"dump" => Orden::Guardar(resto),
        b"info" | b"sistema" => Orden::Informe,
        // `fallo` ensena la ultima autopsia. Se guarda sola en `datos/fallos.txt`
        // en cuanto ocurre -- esto es para mirarla sin salir del escritorio.
        b"fallo" | b"fallos" | b"autopsia" => Orden::Autopsia,
        b"cpu" | b"procesador" => Orden::Cpu,
        b"mem" | b"ram" | b"memoria" => Orden::Memoria,
        b"reboot" | b"reinicia" | b"reiniciar" => Orden::Reiniciar,
        // `smp` a secas CENSA y no toca nada; `smp all` despierta a todos;
        // `smp N` despierta exactamente N. El caso sin argumento es el
        // inofensivo a proposito: ver `sys::smp_despertar`.
        b"smp" | b"nucleos" => Orden::Smp(resto),
        b"audio" | b"sonido" => Orden::Audio,
        b"help" | b"?" | b"ayuda" => Orden::Ayuda,
        _ if parece_programa(linea) => Orden::Lanzar(linea),
        // Parece un archivo pero no es un programa. Antes esto caia en
        // `Lanzar` y el kernel contestaba "sin firma no hay ejecucion" -- un
        // mensaje CORRECTO que en este sitio se lee como si el sistema pidiera
        // permisos para abrir un .txt.
        _ if parece_ruta(linea) => Orden::NoEsPrograma(linea),
        _ => Orden::Desconocida,
    }
}

