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

use super::reports::{campo, report_disco, section};
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
    section(s, b"volume");
    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        campo(s, b"estratos");
        s.with_ink(INK_ERR);
        s.text(b"ningun volumen montado: no hay espacio del que hablar\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let usados = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);

    campo(s, b"estratos");
    s.with_ink(INK_GOOD);
    s.text(b"gen ");
    s.dec(bmo::info(bmo::INFO_ES_GENERACION));
    s.with_ink(INK_PLAIN);
    // La identidad va pegada: un volumen clonado se monta y se lee igual, y la
    // diferencia es que NO tiene ventana de escritura. Sin esta linea,
    // "montado" se lee como "listo para todo".
    if bmo::info(bmo::INFO_ES_IDENTIDAD) == 0 {
        s.with_ink(INK_ERR);
        s.text(b"   NO nacio en este disco (clonado?)");
        s.with_ink(INK_PLAIN);
    } else {
        s.text(b"   de este disco");
    }
    s.byte(b'\n');

    campo(s, b"used");
    s.size(usados.saturating_mul(tam));
    s.text(b" de ");
    s.size(bloques.saturating_mul(tam));
    s.text(b"   ");
    s.pct(usados, bloques);
    s.byte(b'\n');

    campo(s, b"blocks");
    s.dec(usados);
    s.text(b" de ");
    s.dec(bloques);
    s.text(b"   de ");
    s.dec(tam / 1024);
    s.text(b" KiB\n");

    campo(s, b"level");
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

    campo(s, b"write");
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
    campo(s, b"trimmed");
    if sectores == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"nada en esta sesion   (lo pide una persona)");
        s.with_ink(INK_PLAIN);
    } else {
        s.size(sectores.saturating_mul(512));
        s.text(b"   en ");
        s.dec(ordenes);
        s.text(b" ordenes");
    }
    s.byte(b'\n');
    // ** Y el ultimo fallo, si lo hubo, EN LA MISMA TABLA. Un recorte que fallo
    // hace un rato y no deja rastro en `disco` obliga a repetirlo para volver a
    // ver el motivo -- y repetir es justo lo que no se debe hacer con la unica
    // orden destructiva de la caja.
    fallo(s);
}

/// El ultimo fallo del recorte, con **el numero del aparato al lado**.
fn fallo(s: &mut Output) {
    let v = bmo::info(bmo::INFO_DISCO_TRIM_FALLO);
    if v == 0 {
        return;
    }
    let clase = v >> bmo::DISCO_FALLO_CLASE_SHIFT;
    let tfd = v & bmo::DISCO_FALLO_TFD_MASK;
    campo(s, b"fallo");
    s.with_ink(INK_ERR);
    s.text(bmo::fallo_en_palabras(clase));
    s.with_ink(INK_PLAIN);
    if clase == bmo::DISCO_FALLO_APARATO {
        // El PxTFD CRUDO y luego los bits que se saben leer: el byte es la
        // prueba y las palabras son la opinion. Mismo trato que el PHYstatus.
        s.text(b"\n");
        campo(s, b"PxTFD");
        s.text(b"0x");
        s.hex(tfd, 8);
        let err = (tfd >> 8) & 0xFF;
        if err & 0x04 != 0 {
            s.text(b"   ABRT: el disco NO CONOCE esa orden");
        } else if err & 0x10 != 0 {
            s.text(b"   IDNF: ese sector no");
        } else if err != 0 {
            s.text(b"   error 0x");
            s.hex(err, 2);
        }
    }
    s.byte(b'\n');
}

/// Las ordenes de esta caja. Van al final de `disco` a secas, que es donde uno
/// se pregunta "y ahora que puedo hacer con esto?".
fn ordenes(s: &mut Output) {
    s.with_ink(INK_ECHO);
    s.text(b"    disco trim   propone el recorte    trim ya   lo manda\n");
    s.text(b"    disco espacio / barrera\n");
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
fn propuesta(s: &mut Output, explica: bool) -> bool {
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

    campo(s, b"free tail");
    s.size(sectores.saturating_mul(512));
    s.text(b"   desde el bloque ");
    s.dec(bmo::info(bmo::INFO_ES_USADOS));
    s.byte(b'\n');

    campo(s, b"sectors");
    s.dec(sectores);
    s.text(b" de 512 B   desde el LBA ");
    s.dec(lba);
    s.byte(b'\n');

    // El numero REAL: lo que cabe en una orden lo dice el disco (palabra 105) y
    // se pregunta, en vez de suponer el minimo y decir "como mucho".
    let por_orden = bmo::info(bmo::INFO_DISCO_TRIM_BLOQUES).max(1)
        .saturating_mul(SECTORES_POR_BLOQUE_DE_PAYLOAD);
    campo(s, b"orders");
    s.dec(sectores.div_ceil(por_orden));
    s.text(b"   (el disco admite ");
    s.dec(bmo::info(bmo::INFO_DISCO_TRIM_BLOQUES));
    s.text(b" bloque(s) por orden)\n");

    // ** LA FRASE QUE JUSTIFICA QUE ESTO SEA SEGURO, y va en la propuesta y no
    // en un README: es lo que el que va a teclear `ya` necesita saber.
    //
    // Y va SOLO cuando se propone. Al ejecutar se repetia entera, o sea tres
    // renglones identicos a los de hace dos segundos -- y lo que el que mira
    // busca en ese momento es el resultado, no el argumento que ya leyo.
    if explica {
        s.with_ink(INK_ECHO);
        s.text(b"    no se pierde nada: por encima de log_head no llega ningun\n");
        s.text(b"    estrato, y ese puntero solo avanza. NO es el recolector.\n");
        s.with_ink(INK_PLAIN);
    }
    true
}

/// `disco trim` -- la propuesta, y como pedirla.
pub(crate) fn trim_propuesta(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if propuesta(&mut dsk.out.grid, true) {
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
    if propuesta(&mut dsk.out.grid, true) {
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
    if !propuesta(&mut dsk.out.grid, false) {
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
        // ** EL MOTIVO SE PINTA AQUI, y esa es la leccion del 17-08.
        //
        // Antes decia "el disco RECHAZO la orden" y mandaba a F11. Las dos
        // mitades estaban mal: **no siempre rechaza** --puede no contestar a
        // tiempo, que acusa al driver y no al aparato-- y mandar a otra ventana
        // por el numero es pedir un viaje mas cuando el que mira ya esta aqui.
        bmo::DISCO_TRIM_FALLO => {
            s.with_ink(INK_ERR);
            s.text(b"    NO SE PUDO: ");
            let v = bmo::info(bmo::INFO_DISCO_TRIM_FALLO);
            s.text(bmo::fallo_en_palabras(v >> bmo::DISCO_FALLO_CLASE_SHIFT));
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
            fallo(s);
            if sectores > 0 {
                s.text(b"    se devolvio antes de romperse: ");
                s.size(sectores.saturating_mul(512));
                s.byte(b'\n');
            }
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
    campo(s, b"barrier");
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
