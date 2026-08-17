//! **`disco`: la terminal de administracion del almacen.**
//!
//! === Por que esto existe, y por que aqui ===
//!
//! Porque BMO-X no es Linux, no es Windows y no es un Mac: no hay `fstrim`, no
//! hay `hdparm`, no hay un `/dev` donde apuntar una herramienta ajena. Lo que
//! el sistema sepa hacer con su disco **tiene que poder pedirse desde donde vive
//! el dueno**, que es este escritorio -- al shell de Ring 0 no se vuelve una vez
//! el compositor reclama la entrada, y una orden que solo existe alli es codigo
//! que su dueno no puede usar. Ya paso con `smp`, con `ext` y con `audio`.
//!
//! === La regla de esta caja: PROPONER y luego obedecer ===
//!
//! La seccion 9 de ESTRATOS lo pide con todas las letras -- *"un mando manual
//! con lo que va a soltar listado antes de hacerlo"*. Por eso:
//!
//! ```text
//!   disco trim       ENSENA la propuesta y NO manda nada
//!   disco trim ya    la manda
//! ```
//!
//! No es una confirmacion de cortesia: recortar es **destructivo**, y una orden
//! que se ejecuta en el momento en que se teclea no deja sitio para leerla.
//!
//! === Lo que esta caja NO tiene, y no es un olvido ===
//!
//! Una forma de decir **donde**. Ni `disco trim <lba>` ni nada que se le
//! parezca: el rango lo calcula el kernel y lo comprueba contra la ventana de
//! escritura. Un recorte apuntable desde el teclado seria un borrado a distancia
//! con formulario, y en esta maquina el vecino de particion es el arranque.

use bmo_userland as bmo;

use super::reports::{label, report_disco, section};
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};
use crate::paint_output;

/// Sectores que cubre un bloque de payload: 64 descriptores de 65.535 sectores.
///
/// ** Es del FORMATO de `DATA SET MANAGEMENT`, no del disco ni de esta ventana:
/// cuantos bloques caben en una orden lo dice el aparato
/// (`INFO_DISCO_TRIM_BLOQUES`, la palabra 105) y se pregunta. Multiplicar los
/// dos da el numero REAL de ordenes, no un techo inventado en este lado.
const SECTORES_POR_BLOQUE_DE_PAYLOAD: u64 = 64 * 65_535;

/// El cuadro entero: que aparato es, cuanto queda y que se le ha devuelto.
pub(crate) fn cuadro(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    let s = &mut dsk.out.grid;
    section(s, b"disco");
    report_disco(s);
    espacio(s);
    devuelto(s);
    ordenes(s);
    paint_status(p, &dsk.run_box, "disco", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **Cuanto queda en el volumen**, que es la pregunta previa a cualquier otra.
///
/// Los numeros son los de `bmo_estratos::espacio` y los umbrales tambien: aqui
/// no se decide donde cae el ambar. La cuenta es **una resta** porque ESTRATOS
/// reserva con un puntero que solo avanza -- ni mapa de bits ni fragmentacion.
fn espacio(s: &mut Output) {
    section(s, b"espacio del volumen");
    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        label(s, b"volumen");
        s.with_ink(INK_ERR);
        s.text(b"ningun ESTRATOS montado: no hay espacio del que hablar\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let usados = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);
    let libres = bloques.saturating_sub(usados);

    label(s, b"volumen");
    s.with_ink(INK_GOOD);
    s.text(b"montado");
    s.with_ink(INK_PLAIN);
    s.text(b"   generacion ");
    s.dec(bmo::info(bmo::INFO_ES_GENERACION));
    // La identidad va pegada: un volumen clonado se monta y se lee igual, y la
    // diferencia es que NO tiene ventana de escritura. Sin esta linea, "montado"
    // se lee como "listo para todo".
    if bmo::info(bmo::INFO_ES_IDENTIDAD) == 0 {
        s.with_ink(INK_ERR);
        s.text(b"   NO nacio en este disco (clonado?)");
        s.with_ink(INK_PLAIN);
    }
    s.byte(b'\n');

    label(s, b"bloques");
    s.dec(usados);
    s.text(b" de ");
    s.dec(bloques);
    s.text(b"   de ");
    s.dec(tam);
    s.text(b" B cada uno\n");

    label(s, b"usado");
    s.size(usados.saturating_mul(tam));
    s.text(b"   ");
    s.bar(usados, bloques, 24);
    s.byte(b' ');
    s.pct(usados, bloques);
    s.byte(b'\n');

    label(s, b"libre");
    s.size(libres.saturating_mul(tam));
    s.byte(b'\n');

    label(s, b"nivel");
    let nivel = bmo::info(bmo::INFO_ES_NIVEL);
    s.with_ink(if nivel == 0 { INK_GOOD } else { INK_ERR });
    s.text(match nivel {
        0 => b"holgado" as &[u8],
        1 => b"AVISO: por encima del 70%",
        2 => b"FAULT: por encima del 85%",
        _ => b"SOLO LECTURA: por encima del 95%",
    });
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    label(s, b"escribir");
    if bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"si");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"NO: sin esto no hay sellado ni recorte");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

/// **Lo que ya se le devolvio al aparato.** Cero significa *nadie lo ha pedido*.
fn devuelto(s: &mut Output) {
    let sectores = bmo::info(bmo::INFO_DISCO_TRIM_SECTORES);
    let ordenes = bmo::info(bmo::INFO_DISCO_TRIM_ORDENES);
    label(s, b"devuelto");
    if sectores == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"nada todavia en esta sesion   (el recorte lo pide una persona)");
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
        return;
    }
    s.size(sectores.saturating_mul(512));
    s.text(b"   en ");
    s.dec(ordenes);
    s.text(b" ordenes DATA SET MANAGEMENT\n");
}

/// Las ordenes de esta caja. Van al final de `disco` a secas, que es donde uno
/// se pregunta "y ahora que puedo hacer con esto?".
fn ordenes(s: &mut Output) {
    s.with_ink(INK_ECHO);
    s.text(b"    disco trim      la PROPUESTA de recorte (no manda nada)\n");
    s.text(b"    disco trim ya   la manda de verdad\n");
    s.text(b"    disco espacio   cuanto queda en el volumen\n");
    s.text(b"    disco barrera   FLUSH CACHE: baja al plato lo aceptado\n");
    s.with_ink(INK_PLAIN);
}

/// Solo el espacio.
pub(crate) fn solo_espacio(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    espacio(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "espacio", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **La propuesta**: que se recortaria, cuanto es, y por que no se pierde nada.
///
/// === Los numeros son LOS DE LA ORDEN, no unos parecidos ===
///
/// El rango se pide con `INFO_DISCO_COLA_LBA` y `..._SECTORES`, y al otro lado
/// esos dos campos los sirve **la misma funcion del kernel que ejecuta el
/// recorte**. La primera version los deducia aqui de `INFO_ES_BLOQUES`,
/// `INFO_ES_USADOS` y `INFO_ES_BLOQUE_TAM` -- una cuenta paralela que hoy da lo
/// mismo y que el dia que una de las dos cambie **ensena un rango y recorta
/// otro**. Una propuesta que no es exactamente la orden no es una propuesta.
///
/// [!] Sigue sin llamar al disco: son campos de informe. Una propuesta que
/// tuviera que tocar el aparato para poder ensenarse ya lo habria tocado.
fn propuesta(s: &mut Output) -> bool {
    section(s, b"recorte: la propuesta");
    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    sin volumen ESTRATOS montado no hay cola libre que devolver\n");
        s.with_ink(INK_PLAIN);
        return false;
    }
    // ** Y ANTES DE NADA: lo que el disco dijo. Proponer un recorte a un aparato
    // que no declara TRIM seria ensenar un plan que se va a rechazar solo.
    let juicio = bmo::info(bmo::INFO_DISCO_JUICIO);
    if juicio & bmo::DISCO_JUICIO_TRIM == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    este disco NO declara TRIM (palabra 169): no hay nada que mandar\n");
        s.with_ink(INK_PLAIN);
        return false;
    }

    let lba = bmo::info(bmo::INFO_DISCO_COLA_LBA);
    let sectores = bmo::info(bmo::INFO_DISCO_COLA_SECTORES);
    if sectores == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    la cola libre esta vacia: el volumen esta lleno\n");
        s.with_ink(INK_PLAIN);
        return false;
    }

    label(s, b"cola libre");
    s.size(sectores.saturating_mul(512));
    s.text(b"   desde el bloque ");
    s.dec(bmo::info(bmo::INFO_ES_USADOS));
    s.byte(b'\n');

    label(s, b"sectores");
    s.dec(sectores);
    s.text(b" de 512 B   desde el LBA ");
    s.dec(lba);
    s.byte(b'\n');

    // El numero REAL: lo que cabe en una orden lo dice el disco (palabra 105) y
    // se pregunta, en vez de suponer el minimo y decir "como mucho".
    let por_orden = bmo::info(bmo::INFO_DISCO_TRIM_BLOQUES).max(1)
        .saturating_mul(SECTORES_POR_BLOQUE_DE_PAYLOAD);
    label(s, b"ordenes");
    s.dec(sectores.div_ceil(por_orden));
    s.text(b"   (el disco admite ");
    s.dec(bmo::info(bmo::INFO_DISCO_TRIM_BLOQUES));
    s.text(b" bloque(s) por orden)\n");

    // ** LA FRASE QUE JUSTIFICA QUE ESTO SEA SEGURO, y va en la propuesta y no
    // en un README: es lo que el que va a teclear `ya` necesita saber.
    s.with_ink(INK_ECHO);
    s.text(b"    no se pierde nada: la cola libre es todo lo que hay POR ENCIMA\n");
    s.text(b"    de log_head, y ese puntero solo avanza -- ningun estrato llega\n");
    s.text(b"    ahi. Esto no es el recolector: no suelta ni una version vieja.\n");
    s.with_ink(INK_PLAIN);
    true
}

/// `disco trim` -- la propuesta, y como pedirla.
pub(crate) fn trim_propuesta(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if propuesta(&mut dsk.out.grid) {
        dsk.out.grid.with_ink(INK_GOOD);
        dsk.out.grid.text(b"    escribe `disco trim ya` para mandarlo\n");
        dsk.out.grid.with_ink(INK_PLAIN);
    }
    paint_status(p, &dsk.run_box, "trim: propuesta", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `disco trim <algo que no es "ya">`.
///
/// Se ensena la propuesta igual --no toca nada-- y se dice **cual era la palabra
/// buena**. Contestar "no lo conozco" a alguien que ya escribio `trim` seria
/// mandarle a `help` teniendo la orden medio escrita.
pub(crate) fn trim_argumento(dsk: &mut Desktop, p: &bmo::Pantalla, que: &[u8]) -> After {
    let s = &mut dsk.out.grid;
    s.with_ink(INK_ERR);
    s.text(b"  `");
    s.text(que);
    s.text(b"` no significa nada detras de `trim`\n");
    s.with_ink(INK_PLAIN);
    if propuesta(&mut dsk.out.grid) {
        dsk.out.grid.with_ink(INK_GOOD);
        dsk.out.grid.text(b"    la palabra es `ya`:  disco trim ya\n");
        dsk.out.grid.with_ink(INK_PLAIN);
    }
    paint_status(p, &dsk.run_box, "trim: propuesta", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `disco trim ya` -- **la orden de verdad**.
///
/// El aviso se pinta y se VUELCA antes de llamar, igual que en `smp`: la
/// llamada no vuelve hasta que el disco ha tragado cientos de ordenes, y un
/// mensaje escrito despues no explica nada -- para entonces la espera ya paso y
/// lo que el dueno habria visto es un escritorio congelado sin motivo.
pub(crate) fn trim_ya(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if !propuesta(&mut dsk.out.grid) {
        paint_status(p, &dsk.run_box, "trim", INK_DIM);
        dsk.field.n = 0;
        return After::Settle;
    }
    dsk.out.grid.text(b"    mandando el recorte (esto tarda)...\n");
    paint_output(p, &dsk.run_box, &dsk.out.grid);
    p.volcar();

    let (motivo, sectores) = bmo::trim_libre();
    let s = &mut dsk.out.grid;
    match motivo {
        bmo::DISCO_TRIM_HECHO => {
            s.with_ink(INK_GOOD);
            s.text(b"    DEVUELTO: ");
            s.size(sectores.saturating_mul(512));
            s.with_ink(INK_PLAIN);
            s.text(b"   en ");
            s.dec(bmo::info(bmo::INFO_DISCO_TRIM_ORDENES));
            s.text(b" ordenes (total de la sesion)\n");
            // La barrera la manda el kernel detras del recorte; decirlo aqui es
            // lo que separa "el disco lo acepto" de "el disco lo asumio".
            s.with_ink(INK_ECHO);
            s.text(b"    con FLUSH CACHE detras: este disco no tiene condensadores\n");
            s.with_ink(INK_PLAIN);
        }
        // ** El fallo lleva lo que SI se hizo. Un recorte a medias no se
        // deshace, y sin este numero el que mire creeria que no paso nada.
        bmo::DISCO_TRIM_FALLO => {
            s.with_ink(INK_ERR);
            s.text(b"    el disco RECHAZO la orden a mitad\n");
            s.with_ink(INK_PLAIN);
            s.text(b"    lo que SI se devolvio antes de romperse: ");
            s.size(sectores.saturating_mul(512));
            s.byte(b'\n');
            s.text(b"    el motivo del aparato esta en F11 (CABINA)\n");
        }
        otro => {
            s.with_ink(INK_ERR);
            s.text(b"    no se mando: ");
            s.text(bmo::motivo_en_palabras(otro));
            s.byte(b'\n');
            s.with_ink(INK_PLAIN);
        }
    }
    paint_status(p, &dsk.run_box, "trim", INK_DIM);
    dsk.field.n = 0;
    dsk.field.cur = 0;
    After::NextKey
}

/// `disco barrera` -- el `FLUSH CACHE`, a mano.
pub(crate) fn barrera(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    let ok = bmo::barrera();
    let s = &mut dsk.out.grid;
    label(s, b"barrera");
    if ok {
        s.with_ink(INK_GOOD);
        s.text(b"el disco bajo al plato lo que tenia aceptado");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"NO: sin disco, o la escritura no esta armada");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    paint_status(p, &dsk.run_box, "barrera", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// Una subordem que no existe. Se dice **cual se escribio**, y las que hay.
pub(crate) fn no_existe(dsk: &mut Desktop, p: &bmo::Pantalla, que: &[u8]) -> After {
    let s = &mut dsk.out.grid;
    s.with_ink(INK_ERR);
    s.text(b"  `disco ");
    s.text(que);
    s.text(b"` no es una orden del disco\n");
    s.with_ink(INK_PLAIN);
    ordenes(s);
    paint_status(p, &dsk.run_box, "disco", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}
