//! **`cabina` -- la caja negra del kernel, LEIDA DESDE EL ESCRITORIO.**
//!
//! # Por que esto no existia, y es la tercera vez que pasa lo mismo
//!
//! CABINA lleva meses viendolo todo, con severidad y capa, y **desde el
//! escritorio no habia forma de leerla**. La orden existe en el shell de Ring 0
//! --`session.rs`, y hasta con F8-- y desde aqui **a Ring 0 no se vuelve**.
//!
//! ```text
//!    Ring 0   `cabina`    VUELCA a disco (CABINA.LOG)
//!    Ring 0   `fallo`     PINTA la ultima autopsia
//!    aqui     `autopsia`  pinta el ultimo fallo de Ring 3  <- OTRA cosa
//!    aqui     `cabina`    no existia. **Este fichero**
//! ```
//!
//! *** Y LA FONTANERIA ESTABA ENTERA. `OP_CABINA_INFO`, `OP_CABINA_TEXTO`, los
//! nueve campos y las cinco severidades llevaban en `userland` desde que se
//! escribieron, con sus envoltorios (`cabina_total`, `cabina_campo`,
//! `cabina_texto`...). Lo unico que faltaba era **la orden**.
//!
//! Es el caso de `banda`, escrito cuatro lineas mas abajo en el despachador:
//!
//! > *"estaba escrita, compilada y probada, y era inalcanzable desde el unico
//! > sitio donde el dueno trabaja."*
//!
//! ## Lo que lo destapo, y es lo que hace que valga la pena
//!
//! El 2026-08-25 se cablearon dos caminos para que el careo de la topologia
//! saliera de Ring 0 -- despues de que el escritorio pintara `27 fisicos / 54
//! logicos` en un 6/12 sin que nada chistara. Uno llegaba y el otro no:
//!
//! ```text
//!    los BITS de duda   llegan     INFO_CPU_TOPOLOGIA_DUDA, y `cpu` los pinta
//!    el DETALLE         NO llegaba a que testigo, con que valor
//! ```
//!
//! Con esto llega el detalle. `cpu` dice **que** dudar; esto dice **quien lo
//! dijo y cuanto**.
//!
//! ## ** NO CONCEDE NADA
//!
//! Ni una de estas llamadas escribe -- lo dice el propio `userland`: *"ver y
//! poder son cosas separadas, y esta es la mitad de mirar."*

use bmo_userland as bmo;

use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

use super::reports::section;

/// Cuantos eventos se pintan si no se pide otra cosa.
///
/// El anillo son 48 y caben; el tope existe para que `cabina` **quepa en una
/// pantalla** sin tener que desplazarse, que es como se lee un arranque de un
/// vistazo. Con `cabina todo` salen los 48.
const POR_DEFECTO: u64 = 20;

/// La tinta de cada severidad.
///
/// [!] `TRACE` va en la misma tinta apagada que `INFO` **a proposito**: si el
/// ruido se pintara igual que lo importante, el color dejaria de decir nada --
/// que es lo que le pasa a un aviso que sale siempre.
fn tinta(sev: u64) -> u8 {
    match sev {
        bmo::SEV_FAULT | bmo::SEV_PANIC => INK_ERR,
        bmo::SEV_WARNING => INK_GOOD,
        _ => INK_ECHO,
    }
}

/// Las tres letras que dicen la gravedad sin gastar una columna de mas.
fn marca(sev: u64) -> &'static [u8] {
    match sev {
        bmo::SEV_PANIC => b"!!!",
        bmo::SEV_FAULT => b"[X]",
        bmo::SEV_WARNING => b"[!]",
        bmo::SEV_TRACE => b" . ",
        _ => b"   ",
    }
}

/// **`cabina [N | todo | fallos]`** -- el anillo de eventos, el mas reciente
/// arriba.
///
/// # Los tres modos, y por que son estos
///
/// ```text
///    cabina          los ultimos 20. Lo que se teclea al arrancar
///    cabina todo     los 48 del anillo
///    cabina fallos   solo WARNING y peores  <- la que se usa cuando algo fallo
/// ```
///
/// ** `fallos` no es un lujo: en un arranque normal el anillo va lleno de
/// `INFO`, y **la linea que importa es la unica ambar entre veinte verdes**. Sin
/// el filtro hay que encontrarla a ojo justo el dia que uno tiene prisa.
pub(crate) fn report_cabina(s: &mut Output, arg: &[u8]) {
    section(s, b"cabina");

    let hay = bmo::cabina_disponibles();
    let total = bmo::cabina_total();
    let perdidos = bmo::cabina_perdidos();

    if hay == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"    el anillo esta vacio: el kernel no ha apuntado nada\n");
        s.with_ink(INK_PLAIN);
        return;
    }

    // ** LOS PERDIDOS VAN ARRIBA, ANTES DE LA PRIMERA LINEA.
    //
    // Lo dice `userland`: *"un anillo que dio la vuelta y no lo dice hace creer
    // que el arranque empezo donde empieza el primero que sobrevive."* Si esto
    // fuera al final, se leeria despues de haber sacado la conclusion.
    s.with_ink(INK_ECHO);
    s.text(b"    ");
    s.dec(hay);
    s.text(b" en el anillo de ");
    s.dec(total);
    if perdidos > 0 {
        s.text(b"   [!] ");
        s.dec(perdidos);
        s.text(b" se cayeron: esto NO es el principio del arranque");
    }
    s.byte(b'\n');
    s.with_ink(INK_PLAIN);

    let solo_fallos = arg == b"fallos" || arg == b"fallo";
    let cuantos = if arg == b"todo" || arg == b"all" || solo_fallos {
        hay
    } else {
        let pedido = numero(arg).unwrap_or(POR_DEFECTO);
        if pedido < hay {
            pedido
        } else {
            hay
        }
    };

    let mut modulo = [0u8; 24];
    let mut mensaje = [0u8; 96];
    let mut pintados = 0u64;

    for n in 0..cuantos {
        // `None` = ese evento no existe, que NO es lo mismo que un campo a cero.
        // Se corta en vez de seguir: un hueco en medio del anillo significa que
        // alguien lo esta escribiendo mientras se lee, y pintar lo de despues
        // seria pintar el pasado debajo del presente.
        let Some(sev) = bmo::cabina_campo(bmo::CABINA_SEVERIDAD, n) else {
            break;
        };
        if solo_fallos && sev < bmo::SEV_WARNING {
            continue;
        }
        let valor = bmo::cabina_campo(bmo::CABINA_VALOR, n).unwrap_or(0);
        let nm = bmo::cabina_texto(n, bmo::CABINA_TXT_MODULO, &mut modulo);
        let nx = bmo::cabina_texto(n, bmo::CABINA_TXT_MENSAJE, &mut mensaje);

        s.with_ink(tinta(sev));
        s.text(b"  ");
        s.text(marca(sev));
        s.byte(b' ');
        // El modulo a ancho fijo: es lo que hace que dos volcados se puedan
        // comparar poniendo uno debajo del otro, igual que en `consumo`.
        s.text(&modulo[..nm]);
        for _ in nm..10 {
            s.byte(b' ');
        }
        s.text(&mensaje[..nx]);
        // ** El valor solo si lo hay. Un `=0` detras de cada linea entrena a no
        // leer el numero, y entonces el dia que el numero importe tampoco se lee.
        if valor != 0 {
            s.text(b" =");
            s.dec(valor);
        }
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
        pintados += 1;
    }

    if solo_fallos && pintados == 0 {
        s.with_ink(INK_GOOD);
        s.text(b"    ni un aviso ni un fallo en todo el anillo\n");
        s.with_ink(INK_PLAIN);
    }
}

/// Un numero decimal, o `None` si no lo es.
///
/// [!] Devuelve `None` para una palabra desconocida en vez de tratarla como
/// cero: `cabina xyz` tiene que ensenar lo de siempre, no un anillo vacio.
fn numero(arg: &[u8]) -> Option<u64> {
    if arg.is_empty() {
        return None;
    }
    let mut v = 0u64;
    for &c in arg {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((c - b'0') as u64)?;
    }
    Some(v)
}
