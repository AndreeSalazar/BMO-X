//! **El raton sobre la BARRA DE TAREAS**: las fichas que traen una ventana al
//! frente.
//!
//! [consumo] NADA      no corre en reposo: lo llama el bucle SOLO si hubo una
//!                     tecla o el raton se movio. Sin entrada, no se entra
//!                     aqui (L6h)

use bmo_userland as bmo;

use super::Golpe;
use crate::desktop::{Desktop, Ventana};
use crate::scene;
use crate::{erase_window, uncover, TASKBAR_H};

pub(crate) fn on_pointer(dsk: &mut Desktop, p: &bmo::Pantalla, g: &Golpe) {
    let pos = g.pos;
    let button = g.button;

    // -- Clic en una FICHA de la barra: traer esa ventana --
    //
    // Es la mitad que hace que minimizar signifique algo. Sin esto, el
    // boton de minimizar seria uno de "desaparece para siempre".
    // * Una ficha hace SIEMPRE lo mismo: **trae su ventana y le da el
    // foco**, este minimizada, escondida o simplemente detras.
    //
    // La primera version solo actuaba `si estaba minimized` o `si
    // estaba escondida`, y por eso pulsar la ficha de una ventana que
    // ya se veia no hacia nada. En el Ryzen eso se lee como *"la barra
    // se olvida de mis clics"*, y con razon: un control que a veces
    // responde y a veces no es peor que uno que no esta.
    if button && !dsk.tick.button_before && pos.y < TASKBAR_H {
        let (fichas, n) = dsk.table.fichas();
        if let Some(i) = scene::chip_at(pos.x, pos.y, scene::FICHA_APPS + n as u32) {
            if i >= scene::FICHA_APPS {
                // ** LA FICHA DE UNA APP (2026-09-12): la trae --este minimizada
                // o detras--, le da el foco y la pone delante. Es la misma regla
                // que las otras fichas: una ficha hace SIEMPRE lo mismo.
                let hueco = fichas[(i - scene::FICHA_APPS) as usize];
                if dsk.table.traer(hueco, &p) {
                    let v = Ventana::App(hueco as u8);
                    dsk.win.focus.open(v);
                    dsk.win.focus.clic_en(v);
                    dsk.win.taskbar_dirty = true;
                }
            } else if i == 1 && dsk.win.data_open {
                // Estaba minimizada o no, da igual: acaba visible,
                // encajada, con el foco y delante.
                dsk.win.data.chrome.minimized = false;
                dsk.win.data.chrome.fit(&p);
                dsk.win.focus.open(Ventana::Data);
                dsk.win.focus.clic_en(Ventana::Data);
                dsk.win.data.relayout();
                scene::data::paint(&p, &dsk.win.data);
                dsk.win.top_before = Ventana::Data;
                dsk.win.taskbar_dirty = true;
            } else if i == 0 {
                if !dsk.win.visible {
                    dsk.win.visible = true;
                }
                dsk.win.focus.open(Ventana::Run);
                dsk.win.focus.clic_en(Ventana::Run);
                uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                dsk.win.top_before = Ventana::Run;
                dsk.win.taskbar_dirty = true;
            } else if i == 2 {
                // ** CABINA CON EL RATON, que es lo que la hace util.
                //
                // Misma secuencia que F11 (`keys/windows.rs`) y no una
                // parecida: abrir por lo ultimo, dar el foco, pintar. Si
                // las dos puertas dejaran la ventana en estados distintos,
                // el que la abre con el raton veria otra cosa que el que la
                // abre con la tecla -- y una de las dos estaria mal sin que
                // nadie pudiera decir cual.
                dsk.win.cabina_open = !dsk.win.cabina_open;
                if dsk.win.cabina_open {
                    dsk.win.cabina.from = 0;
                    dsk.win.cabina.chrome.minimized = false;
                    dsk.win.focus.open(Ventana::Cabina);
                    dsk.win.focus.clic_en(Ventana::Cabina);
                    scene::cabina::paint(&p, &dsk.win.cabina);
                    dsk.win.top_before = Ventana::Cabina;
                } else {
                    dsk.win.focus.close(Ventana::Cabina);
                    erase_window(
                        &p, &dsk.run_box,
                        dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
                        dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height,
                        dsk.win.visible,
                    );
                    dsk.win.top_before = Ventana::Run;
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    if dsk.win.data_open {
                        scene::data::paint(&p, &dsk.win.data);
                    }
                }
                dsk.win.taskbar_dirty = true;
            }
        }
    }
    // Los flancos (`button_before`, `derecho_before`) ya no se apuntan aqui:
    // esta es la ULTIMA parada del reparto, y cualquier `return` de antes se
    // la saltaba. Los apunta `mouse::on_pointer`, pase lo que pase.
}
