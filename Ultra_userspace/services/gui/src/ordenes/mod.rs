//! **Lo que se INTERPRETA.**
//!
//! De una linea de texto a una intencion. Aqui no se pinta nada: un modulo de
//! esta carpeta no sabe de que color es la ventana.

pub(crate) mod completar;
pub(crate) mod historial;
pub(crate) mod informes;

// ── La línea de comandos ────────────────────────────────────────────────

/// Qué pidió el usuario. Se separa del bucle porque la decisión "esto es un
/// comando o es una ruta" merece leerse de un vistazo.
pub(crate) enum Orden<'a> {
    Nada,
    Lanzar(&'a [u8]),
    Limpiar,
    Ayuda,
    /// Ensena o esconde la calculadora.
    Calculadora,
    /// `ls [ruta]` — qué hay en el disco. Antes esto no podía existir: no
    /// había capability de directorio, así que había que saberse los nombres
    /// de memoria y teclearlos enteros.
    Listar(&'a [u8]),
    /// `lee <ruta>` — enseña lo que hay DENTRO de un archivo. Es el hermano
    /// de `ls`: aquel dice qué archivos hay, éste los abre.
    Leer(&'a [u8]),
    /// `escribe <ruta> <texto>` — crea un archivo con ese texto.
    ///
    /// Es la primera vez que Ring 3 GUARDA algo. Hasta ahora todo lo que
    /// aparecía en el disco lo había puesto el anfitrión al flashear, o el
    /// kernel con su caja negra; un programa no tenía con qué.
    Escribir(&'a [u8], &'a [u8]),
    /// Parece un archivo, pero no es un `.bex`. No se intenta lanzar: se dice
    /// qué es y con qué se abre.
    NoEsPrograma(&'a [u8]),
    /// `info` — el informe del sistema. `cpu` y `mem` son las dos mitades.
    ///
    /// ★ Esto vivía SOLO en el shell de Ring 0, y no porque hiciera falta el
    /// privilegio: porque los datos estaban a su alcance. Contar RAM no ejerce
    /// ningún poder. Ahora bajan por `OP_INFO` y se pintan aquí, que es donde
    /// está la pantalla.
    Informe,
    Cpu,
    Memoria,
    /// `reboot` — reinicia la máquina y no vuelve.
    ///
    /// Estaba en el shell del kernel desde siempre y aquí contestaba "no lo
    /// conozco", así que la única forma de reiniciar era el botón de la caja.
    /// Reiniciar es tocar puertos de E/S, que Ring 3 no puede hacer: va por
    /// `OP_REINICIAR`, una operación más dentro de `INVOKE`.
    Reiniciar,
    /// Una palabra suelta que no parece una ruta.
    Desconocida,
}


pub(crate) fn parece_ruta(t: &[u8]) -> bool {
    t.iter().any(|&c| c == b'/' || c == b'\\' || c == b'.')
}

/// ¿Esto es un PROGRAMA, o sea algo que tenga sentido lanzar?
///
/// ★ Antes bastaba con que llevara un punto o una barra, y por eso escribir
/// `leeme.txt` a pelo intentaba EJECUTARLO. El kernel contestaba "sin firma no
/// hay ejecucion" —que es exactamente lo correcto— y el usuario se quedaba
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

/// Parte la línea en verbo y resto.
///
/// ★ Acepta `run <ruta>` ADEMÁS de la ruta pelada, y no por capricho: quien usa
/// esto viene del shell de Ring 0, donde se escribe `run`. Pelearse con la
/// costumbre del usuario es perder — el que se adapta es el programa. Lo que sí
/// se hace es DECIRLO cuando la palabra no es ni comando ni ruta, en vez de
/// contestar "no esta: revisa la ruta" a alguien que escribió `reboot`.
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
    match verbo {
        // INGLES de primero, y es una decision del dueno: el castellano limita
        // — no hay palabra corta para "flush", los verbos se alargan, y medio
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
        b"info" | b"sistema" => Orden::Informe,
        b"cpu" | b"procesador" => Orden::Cpu,
        b"mem" | b"ram" | b"memoria" => Orden::Memoria,
        b"reboot" | b"reinicia" | b"reiniciar" => Orden::Reiniciar,
        b"help" | b"?" | b"ayuda" => Orden::Ayuda,
        _ if parece_programa(linea) => Orden::Lanzar(linea),
        // Parece un archivo pero no es un programa. Antes esto caia en
        // `Lanzar` y el kernel contestaba "sin firma no hay ejecucion" — un
        // mensaje CORRECTO que en este sitio se lee como si el sistema pidiera
        // permisos para abrir un .txt.
        _ if parece_ruta(linea) => Orden::NoEsPrograma(linea),
        _ => Orden::Desconocida,
    }
}

