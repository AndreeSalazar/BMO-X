//! **La entrada a Ring 3** -- lo que se ve cuando el userspace toma la maquina.
//!
//! === Por que existe ===
//!
//! Hasta ahora el paso de Ring 0 a Ring 3 era **invisible**: el kernel dejaba
//! de pintar, el compositor limpiaba la pantalla y aparecia un escritorio. Si
//! algo fallaba en medio, lo que quedaba era un shell -- y nadie podia decir si
//! el compositor no habia arrancado, si habia arrancado y muerto, o si estaba
//! vivo y no pintaba.
//!
//! Esta pantalla es el momento **dicho en voz alta**: quien toma la maquina,
//! que le acaban de ceder, y sobre que corre. No es adorno: cada linea es un
//! dato que, cuando falta, es exactamente lo que hay que preguntar.
//!
//! === Esta CRONOMETRADA, no contada en vueltas ===
//!
//! Ring 3 no tiene reloj en los dos syscalls... pero `RDTSC` no es privilegiada
//! y el kernel publica la frecuencia medida (`INFO_TSC_HZ`). Con eso, una
//! espera de 900 ms es de 900 ms **en esta maquina y en la siguiente**. Contar
//! vueltas del bucle habria dado una intro de dos segundos en un Ryzen y de
//! veinte en algo mas lento -- que es como se hacian las cosas cuando no habia
//! forma de saber la hora, y aqui si la hay.

use bmo_userland as bmo;

use super::*;
use super::gato;
use crate::texto::decimal;

const ENT_FONDO: u32 = 0x000A_0E17;
const ENT_TENUE: u32 = 0x0059_6B8A;


/// Espera exacta, cediendo el CPU mientras tanto -- y **cortable con una tecla**.
///
/// * Cede en el bucle a proposito: un `spin` de 900 ms en un sistema preemptivo
/// es 900 ms robados al resto de las tareas. Aqui la espera es del que mira, no
/// del que calcula.
///
/// ** Y SE PUEDE SALTAR, que es el cambio del 2026-08-07.
///
/// El arranque estaba cronometrado y el propio cronometro delato la siesta:
///
/// ```text
/// [   52ms] == BMO-X operativo ==
/// [ 1163ms] gui.bex> entrada a Ring 3 pintada
/// ```
///
/// **1.100 de los 1.205 ms hasta el escritorio eran esta espera.** El sistema
/// estaba listo en 52 ms y se quedaba mirando al techo el 91% del arranque. Y
/// el dueno lo leyo como un fallo, que es la senal de que algo va mal aunque
/// sea intencionado: si tu instrumento de medida hace que la gente sospeche de
/// la maquina, la espera es demasiado larga.
///
/// **No se borra la pantalla y no se acorta el numero.** La intro existe para
/// contestar "que le cedieron al userspace?" cuando algo falla, y eso vale
/// justo los segundos que haga falta LEERLO. Lo que se arregla es que fuera
/// obligatoria: ahora cualquier tecla la cierra. Quien necesita leerla, la lee;
/// quien no, no paga.
fn esperar_ms(ms: u64, entrada: Option<&bmo::Entrada>) {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    if hz == 0 {
        // Sin frecuencia medida no se inventa una: se sigue. Una intro que no
        // se ve es infinitamente mejor que una espera de duracion desconocida.
        return;
    }
    let objetivo = bmo::ciclos() + hz / 1000 * ms;
    while bmo::ciclos() < objetivo {
        // La tecla se consume al leerla, asi que la que salta la intro **no**
        // acaba escrita en la caja de Ejecutar. Un atajo que ademas teclea algo
        // seria un atajo que hay que deshacer.
        if let Some(e) = entrada {
            if e.tecla().is_some() {
                return;
            }
        }
        bmo::yield_screen();
    }
}

/// * Pinta EL GATO desde sus dos mascaras de 1 bit.
///
/// El fondo no se dibuja: la mascara no lo lleva, porque el fondo del splash ya
/// es negro. Solo se encienden los pixeles del trazo y los de los ojos -- 1.622
/// de los 27.360 del rectangulo, o sea que dibujarlo cuesta menos que un `rect`
/// de ese tamano.
///
/// `escala` multiplica en enteros y a proposito: interpolar un dibujo de lineas
/// de un pixel lo convierte en una mancha gris. Aqui un pixel de la mascara es
/// un cuadrado exacto, que es como se ve un logo hecho de trazos.
fn pintar_gato(p: &bmo::Pantalla, x0: u32, y0: u32, escala: u32) {
    let bit = |m: &[u8], i: usize| m[i / 8] >> (i % 8) & 1 == 1;
    for fy in 0..gato::ALTO {
        for fx in 0..gato::ANCHO {
            let i = (fy * gato::ANCHO + fx) as usize;
            // Los ojos ganan al trazo: son el unico sitio con color y es lo
            // primero que mira quien mira un gato.
            let color = if bit(&gato::OJOS, i) {
                ACENTO
            } else if bit(&gato::TRAZO, i) {
                TEXTO
            } else {
                continue;
            };
            let px = x0 + fx * escala;
            let py = y0 + fy * escala;
            if escala == 1 {
                p.punto(px, py, color);
            } else {
                p.rect(px, py, escala, escala, color);
            }
        }
    }
}

/// Una fila del informe: etiqueta a la izquierda, valor a la derecha.
fn fila(p: &bmo::Pantalla, x: u32, y: u32, etiqueta: &str, valor: &str, color: u32) {
    p.texto(x, y, etiqueta, ENT_TENUE);
    p.texto(x + 13 * bmo::GLIFO_ANCHO, y, valor, color);
}

/// **La entrada.** Se pinta entera, se lee, y se va.
///
/// La entrada y la consola son las dos capabilities que el compositor puede no
/// recibir, y sin las cuales el escritorio arranca igual pero **quieto y mudo**.
/// Que se digan aqui es lo que distingue "no funciona" de "no me la dieron".
///
/// * Recibe la `Entrada` y no un `bool`: antes era `hay_entrada: bool`, que es
/// el mismo dato con menos informacion. Con la capability delante se puede
/// ademas LEER --y por eso la espera del final se puede saltar con una tecla--,
/// y el `bool` sale de ella sin poder desincronizarse.
pub(crate) fn pintar(
    p: &bmo::Pantalla,
    entrada: Option<&bmo::Entrada>,
    hay_consola: bool,
) {
    let hay_entrada = entrada.is_some();
    p.limpiar(ENT_FONDO);

    // Una banda de acento a la izquierda, de arriba abajo. Sujeta la
    // composicion y cuesta un rectangulo.
    p.rect(0, 0, 6, p.alto, ACENTO);

    // -- * LA MAQUETA: el gato a la izquierda, el informe a la derecha --
    //
    // Antes era una columna sola pegada al margen. El logo pide dos: un dibujo
    // alto al lado de un bloque de texto se lee de un vistazo, y una columna
    // sola de veinte lineas se lee de arriba abajo o no se lee.
    //
    // La escala del gato sale de la ALTURA de la pantalla y no de un numero
    // fijo: en 1080 sale a x2 y en 720 a x1, y en las dos ocupa la misma
    // fraccion. Un `3` puesto a mano se sale por abajo en el primer monitor
    // pequeno que se enchufe.
    let escala = if p.alto >= 900 { 2 } else { 1 };
    let gato_w = gato::ANCHO * escala;

    let x = 120 + gato_w + 56;
    let mut y = p.alto / 2 - 190;

    // El gato se centra respecto al bloque de texto, no respecto a la pantalla:
    // lo que tiene que quedar alineado es lo que se mira junto.
    pintar_gato(p, 120, y + 8, escala);

    // -- El nombre, grande --
    let ancho = bmo::Pantalla::ancho_escala("BMO-X", 6);
    p.texto_escala(x, y, "BMO-X", TEXTO, 6);
    // Subrayado exacto bajo el titulo: el ancho se pregunta, no se estima.
    p.rect(x, y + 16 * 6 + 8, ancho, 3, ACENTO);
    y += 16 * 6 + 22;

    // * METAKERNEL, y no es una etiqueta bonita: es lo que hace.
    //
    // Un kernel normal falla y te deja un shell. Este guarda las ultimas cuatro
    // lineas de cada proceso (`uconsole`), y cuando el dueno de la pantalla
    // MUERE las vuelca el a mano --con la CR3 del kernel puesta, que si no es un
    // #PF recursivo-- para poder decir DONDE se rompio. No presume de no fallar:
    // presume de contarlo. De ahi el gato: se cae, se rompe algo, y sigue.
    p.texto(x, y, "BMO METAKERNEL", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 16;

    p.texto(x, y, "RING 3   -   el userspace toma la maquina", ACENTO);
    y += bmo::GLIFO_ALTO + 34;

    // -- Lo que se acaba de ceder --
    p.texto(x, y, "SE ME HA CEDIDO", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 10;

    // La pantalla: siempre esta, porque sin ella no habria nada que leer.
    let mut b = [0u8; 10];
    let mut n = decimal(p.ancho as u64, &mut b);
    p.texto(x, y, "la pantalla", ENT_TENUE);
    let mut px = p.texto_bytes(x + 13 * bmo::GLIFO_ANCHO, y, &b[..n], TEXTO);
    px = p.texto(px, y, " x ", ENT_TENUE);
    n = decimal(p.alto as u64, &mut b);
    px = p.texto_bytes(px, y, &b[..n], TEXTO);
    p.texto(px, y, "   y el kernel deja de pintar", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 6;

    if hay_entrada {
        fila(p, x, y, "la entrada", "teclado y raton son mios", TEXTO_BIEN);
    } else {
        fila(p, x, y, "la entrada", "NO: el escritorio sera mudo", TEXTO_MAL);
    }
    y += bmo::GLIFO_ALTO + 6;

    if hay_consola {
        fila(p, x, y, "una consola", "lo que yo lance escribe AQUI", TEXTO_BIEN);
    } else {
        fila(p, x, y, "una consola", "NO: los hijos escribiran en el kernel", TEXTO_MAL);
    }
    y += bmo::GLIFO_ALTO + 28;

    // -- Sobre que corre --
    p.texto(x, y, "SOBRE", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 10;

    let mut cpu = [0u8; 48];
    let ncpu = bmo::info_texto(bmo::INFO_TXT_CPU_NOMBRE, &mut cpu);
    if ncpu > 0 {
        p.texto(x, y, "cpu", ENT_TENUE);
        let mut cx = p.texto_bytes(x + 13 * bmo::GLIFO_ANCHO, y, &cpu[..ncpu], TEXTO);
        let hilos = bmo::info(bmo::INFO_CPU_HILOS);
        if hilos > 0 {
            cx = p.texto(cx, y, "   ", ENT_TENUE);
            n = decimal(hilos, &mut b);
            cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
            cx = p.texto(cx, y, " hilos", ENT_TENUE);
        }
        let hz = bmo::info(bmo::INFO_TSC_HZ);
        if hz > 0 {
            cx = p.texto(cx, y, " a ", ENT_TENUE);
            // Dos decimales de GHz sin coma flotante, igual que el resto.
            n = decimal(hz / 1_000_000_000, &mut b);
            cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
            cx = p.texto(cx, y, ".", TEXTO);
            let cent = (hz % 1_000_000_000) / 10_000_000;
            if cent < 10 {
                cx = p.texto(cx, y, "0", TEXTO);
            }
            n = decimal(cent, &mut b);
            cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
            p.texto(cx, y, " GHz medidos", ENT_TENUE);
        }
        y += bmo::GLIFO_ALTO + 6;
    }

    // ** LA RED, en la pantalla de entrada.
    //
    // Va aqui y no solo en una ventana porque es la unica linea del arranque que
    // contesta *"tengo cable?"* sin escribir nada. Y contesta las DOS mitades:
    // que tarjeta hay, y si el enlace esta arriba -- que son fallos distintos y
    // se confunden todo el tiempo.
    //
    // [!] Es la foto del arranque, no del instante: el kernel cachea la
    // identidad para que repintar no toque el BAR de la NIC. Si desenchufas el
    // cable, esta linea no cambia -- `red` en la terminal lo dice tambien.
    if bmo::info(bmo::INFO_NET_PRESENTE) != 0 {
        p.texto(x, y, "red", ENT_TENUE);
        let mut cx = x + 13 * bmo::GLIFO_ANCHO;
        let mac = bmo::info(bmo::INFO_NET_MAC);
        let mut i = 6;
        while i > 0 {
            i -= 1;
            let byte = (mac >> (i * 8)) & 0xFF;
            n = hex2(byte, &mut b);
            cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
            if i > 0 {
                cx = p.texto(cx, y, "-", ENT_TENUE);
            }
        }
        let mbit = bmo::info(bmo::INFO_NET_MEGABITS);
        if mbit > 0 {
            cx = p.texto(cx, y, "   enlace ", ENT_TENUE);
            n = decimal(mbit, &mut b);
            cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
            p.texto(cx, y, " Mbit", ENT_TENUE);
        } else {
            // El cero NO es un error: es "no hay cable", y es una respuesta.
            p.texto(cx, y, "   sin enlace", ENT_TENUE);
        }
        y += bmo::GLIFO_ALTO + 6;
    }

    // La memoria, con el numero que a esta maquina le gusta ensenar: cuanto
    // ocupa el sistema entero.
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    if total > 0 {
        p.texto(x, y, "memoria", ENT_TENUE);
        let mut cx = x + 13 * bmo::GLIFO_ANCHO;
        n = decimal(total.saturating_sub(libre) / (1024 * 1024), &mut b);
        cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
        cx = p.texto(cx, y, " MiB en uso de ", ENT_TENUE);
        n = decimal(total / (1024 * 1024 * 1024), &mut b);
        cx = p.texto_bytes(cx, y, &b[..n], TEXTO);
        p.texto(cx, y, " GiB", ENT_TENUE);
        y += bmo::GLIFO_ALTO + 6;
    }

    if bmo::info(bmo::INFO_DATOS_MONTADO) != 0 {
        fila(p, x, y, "disco", "volumen de datos montado", TEXTO_BIEN);
    } else {
        fila(p, x, y, "disco", "SIN volumen de datos", TEXTO_MAL);
    }
    y += bmo::GLIFO_ALTO + 34;

    // -- La frase, que es la tesis del proyecto --
    p.rect(x, y, 560, 1, 0x0022_3040);
    y += 14;
    // ** DECIA "TRES", Y SON DOS.
    //
    // `INVOKE` y `WAIT`. El `1` fue `CHANNEL_KICK`, se retiro, y su numero
    // **queda reservado con lapida** para que ningun binario viejo caiga en una
    // puerta nueva -- lo dice `puertas.rs:46`: *"Dos. Ver NR_CHANNEL_KICK para
    // el tercero que hubo."*
    //
    // O sea que la PRIMERA pantalla que ve cualquiera decia mal **el numero que
    // define el proyecto entero**. Y no es una errata cosmetica: "dos syscalls"
    // es la frase con la que este sistema se presenta, y un tres la debilita
    // justo donde mas fuerte es.
    //
    // [!] Es la tercera vez hoy que un texto de pantalla cuenta el estado de
    // hace meses -- como `PLAN_DOOM.md` mandando a una puerta ya cerrada y el
    // panel de ESTRATOS diciendo que no se podia escribir mientras se escribia.
    // Un numero que sale en pantalla y no sale de una constante se queda viejo
    // solo.
    p.texto(x, y, "DOS syscalls congelados.  todo lo demas son capabilities.", ENT_TENUE);
    y += bmo::GLIFO_ALTO + 4;
    p.texto(x, y, "esto no es una API prestada: es la maquina obedeciendo.", ENT_TENUE);

    // * Empujar ANTES de esperar. Sin esto la intro se pintaria en el bufer de
    // write-combining y se quedaria ahi los 1100 ms enteros -- o sea que la
    // pantalla que existe para ser leida seria justo la que no se ve.
    p.vaciar();

    // Se deja leer, y se puede saltar. Ver `esperar_ms`: es tiempo REAL, no
    // vueltas de bucle, y cualquier tecla la corta.
    p.texto(x, y + bmo::GLIFO_ALTO + 26, "una tecla para entrar", ENT_TENUE);
    p.vaciar();
    esperar_ms(1100, entrada);
}

/// Dos digitos hexadecimales, en mayusculas. Para la MAC.
///
/// Existe porque `decimal` no sirve: una MAC se lee en hexadecimal en todas
/// partes --Windows, un switch, una etiqueta pegada a la tarjeta-- y darla en
/// decimal obligaria a convertirla a mano para compararla con cualquiera de las
/// tres.
fn hex2(v: u64, b: &mut [u8; 10]) -> usize {
    const D: &[u8; 16] = b"0123456789ABCDEF";
    b[0] = D[((v >> 4) & 0xF) as usize];
    b[1] = D[(v & 0xF) as usize];
    2
}
