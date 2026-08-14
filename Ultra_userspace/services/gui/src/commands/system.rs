//! **Commands that ask the KERNEL**: `info`, `cpu`, `mem`, `autopsia`, `smp`,
//! `red`, `audio`, `reboot`.
//!
//! None of them exercises a privilege -- counting RAM is not power. They come
//! down through `OP_INFO` and its siblings and are painted here, which is
//! where the screen is. Several of these used to exist ONLY in the Ring 0
//! shell, and two shells with two vocabularies are two products.

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::commands::reports::{report_autopsy, report_cpu, report_memory, report_net, report_system};
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK, INK_DIM};
use crate::text::decimal;
use crate::paint_output;

/// * `sella` YA NO VIVE AQUI, y esto lo dice.
///
/// La orden se mudo a la ventana de ESTRATOS porque
/// el verbo vive donde vive el objeto. Borrarla y
/// contestar "no lo conozco" habria sido correcto y
/// cruel: estaba escrita en la linea de ayuda de
/// ayer, en dos documentos y en la costumbre del
/// dueno. **Una funcion que se muda sin dejar nota se
/// convierte en una funcion que desaparecio.**
pub(crate) fn seal_moved(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
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
    After::Settle
}

/// ** `audio` -- paso 0 de docs/AUDIO_MAESTRO.md.
///
/// La orden existia SOLO en el shell de Ring 0 y el
/// dueno la escribio aqui, que es donde se trabaja.
/// Contesto "no es un comando ni una ruta" y la
/// prueba se quedo sin hacer. Dos shells con dos
/// vocabularios distintos son dos productos.
/// ** LA RED -- siete campos que hasta hoy no
/// cruzaban a Ring 3. Mismo criterio que `audio`: la
/// orden existia SOLO en el shell de Ring 0, y dos
/// shells con dos vocabularios son dos productos.
pub(crate) fn net(dsk: &mut Desktop, _p: &bmo::Pantalla, what: &[u8]) -> After {
    report_net(&mut dsk.out.grid, what);
    After::Settle
}

pub(crate) fn audio(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
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
    After::Settle
}

pub(crate) fn autopsy(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_autopsy(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "ultimo fallo de Ring 3", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn report(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_system(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "informe del sistema", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn cpu(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_cpu(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "procesador", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn memory(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_memory(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "memoria", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// * El aviso va ANTES y se VUELCA antes, porque la
/// llamada bloquea hasta un segundo entero mientras
/// el kernel manda INIT+SIPI a cada nucleo. Un
/// mensaje escrito despues de volver no explica
/// nada: para entonces la espera ya paso, y lo que
/// el dueno habria visto es un escritorio congelado
/// sin motivo.
pub(crate) fn smp(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
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
        return After::NextKey;
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
        return After::NextKey;
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
    After::Settle
}

/// Se pinta ANTES de pedirlo: la llamada no vuelve,
/// asi que un mensaje despues no lo veria nadie. Y
/// que quede escrito distingue "reinicio pedido" de
/// "se colgo" en la foto. `Pantalla` escribe directo
/// al framebuffer, asi que al volver de `text` ya
/// esta en el cristal: no hay nada que vaciar.
pub(crate) fn reboot(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    dsk.out.grid.text(b"  reiniciando...\n");
    paint_status(&p, &dsk.run_box, "reiniciando", INK_DIM);
    bmo::reiniciar();
}
