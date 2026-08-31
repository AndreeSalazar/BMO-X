//! **THE SHELL SESSION** -- the loop that never returns.
//!
//! [carril]  AMARILLO  el bucle que no retorna y el despacho de ordenes
//!
//! === Why this is separate from the editor and from the commands ===
//!
//! Because it is the only part with a lifetime: `run_shell` is `-> !`, it owns
//! the keyboard for the rest of the machine's life, and everything else here is
//! called *by* it. Keeping the loop apart from what it calls is what makes the
//! order of a boot readable -- the last thing `phase::main` does is hand over,
//! and this is what it hands over to.
//!
//! [!] Printed text stays in Spanish.

use boot_context::BootContext;
use super::super::dashboard::{dash_log, dashboard_log};
use super::super::phase::s_log;
use super::{danger, files, hardware, screen};
use super::ui::{clear_screen, similar_command, row, shell_help, shell_hist,
                shell_layout, shell_prompt, shell_read_line, L, SH_TITLE, SH_VALUE};
use super::super::dashboard::dashboard_log_color;
use super::super::desktop::{death_report, desktop_died, start_desktop,
                             wait_for_demo_tasks, DESKTOP_ATTEMPTS, DESKTOP_MAX_ATTEMPTS};

pub(crate) fn run_shell(ctx: &BootContext) -> ! {
    // Normalize the i8042 (translation -> Set 1, re-enable scanning) so the
    // physical keyboard reaches shell_read_line. No-op if the controller is
    // dead/absent (bounded timeouts inside). El stack USB real (xHCI+HID)
    // ya desperto en el Acto I de main().
    crate::ring0::dev::keyboard::init();
    // * Si el escritorio se admitio y ya no esta, DECIRLO aqui y decir que
    // hacer. Estar en este shell despues de haber lanzado el compositor no es
    // lo normal: significa que se murio, y quien lo mira solo ve un shell.
    if desktop_died() {
        death_report();
        // * Y se vuelve a intentar UNA vez. La entrada a Ring 3 es el estado
        // normal de esta maquina: quedarse en el shell de Ring 0 porque el
        // primer intento se cruzo con algo del arranque es conformarse.
        // Con tope, y diciendolo -- un relanzamiento silencioso convierte un
        // fallo en un misterio.
        if unsafe { DESKTOP_ATTEMPTS } < DESKTOP_MAX_ATTEMPTS {
            row("   reintento", |l| {
                l.txt("levantando el escritorio otra vez (");
                l.dec(unsafe { DESKTOP_ATTEMPTS } as u64 + 1);
                l.txt(" de ");
                l.dec(DESKTOP_MAX_ATTEMPTS as u64);
                l.txt(")");
            });
            crate::ring0::cabina::info("gui", "relanzando el escritorio", unsafe {
                DESKTOP_ATTEMPTS
            } as u64);
            start_desktop();
        } else {
            row("   basta", |l| {
                l.txt("no se relanza mas: dos veces es un bug, no una carrera");
            });
        }
    }
    dash_log("== BMO-X operativo : escribe help ==");
    // Serial-only banner: keep the rolling dashboard rows untouched so the
    // fixed-row Ring 3 diagnostics painted just before timer::enable survive.
    crate::ring0::dev::console::serial_write("\n=== BMO-X Ring 0 shell (type 'help') ===\n");
    shell_prompt();

    let mut buf = [0u8; 64];
    loop {
        let n = shell_read_line(&mut buf);
        if n == 0 { shell_prompt(); continue; }

        let cmd = &buf[..n];

        if cmd == b"help" {
            shell_help();
        } else if cmd == b"ls" {
            super::files::shell_ls();
        } else if cmd == b"disk" {
            super::hardware::shell_disk();
        } else if cmd == b"audio" || cmd == b"sonido" {
            super::hardware::shell_audio();
        } else if cmd == b"net" || cmd == b"red" {
            super::hardware::shell_red(b"");
        } else if cmd.len() > 4 && (&cmd[..4] == b"net " || &cmd[..4] == b"red ") {
            super::hardware::shell_red(&cmd[4..]);
        } else if cmd == b"placa" || cmd == b"firmware" {
            super::placa::shell_placa();
        } else if cmd == b"cabina" {
            super::screen::shell_cabina();
        } else if cmd == b"fallo" {
            super::screen::shell_fallo(b"");
        } else if cmd.len() > 6 && &cmd[..6] == b"fallo " {
            super::screen::shell_fallo(&cmd[6..]);
        } else if cmd == b"estratos" {
            super::files::shell_estratos();
        } else if cmd == b"cpu" {
            super::hardware::shell_cpu();
        } else if cmd == b"ext" || cmd == b"extensiones" {
            super::extensions::shell_ext();
        } else if cmd == b"consumo" || cmd == b"gasto" || cmd == b"w" {
            super::hardware::shell_consumo();
        } else if cmd == b"apps" || cmd == b"programas" {
            super::hardware::shell_apps();
        } else if cmd == b"hist" || cmd == b"history" {
            shell_hist();
        } else if cmd == b"layout" {
            shell_layout(b"");
        } else if cmd.len() > 7 && &cmd[..7] == b"layout " {
            shell_layout(&cmd[7..]);
        } else if cmd.len() > 4 && &cmd[..4] == b"run " {
            super::files::shell_run(&cmd[4..]);
        } else if cmd == b"run" {
            super::files::shell_run(b"");
        } else if cmd == b"cls" || cmd == b"clear" {
            clear_screen();
        } else if cmd == b"info" {
            super::hardware::shell_info(ctx);
        } else if cmd == b"banda" {
            super::banda::shell_banda();
        } else if cmd == b"smp" {
            super::hardware::shell_smp(b"");
        } else if cmd.len() > 4 && &cmd[..4] == b"smp " {
            super::hardware::shell_smp(&cmd[4..]);
        } else if cmd == b"tasks" {
            super::hardware::shell_tasks();
        } else if cmd == b"mem" {
            super::hardware::shell_mem();
        } else if cmd == b"ktest" {
            super::danger::shell_ktest();
        } else if cmd == b"fb" {
            super::screen::shell_fb();
        } else if cmd == b"splash" {
            super::screen::shell_splash();
        } else if cmd == b"bex" {
            super::files::shell_bex();
        } else if cmd == b"panic" {
            super::danger::shell_panic();
        } else if cmd == b"escritorio" || cmd == b"gui" || cmd == b"desktop" {
            shell_escritorio();
        } else if cmd == b"reboot" {
            super::danger::shell_reboot();
        } else if cmd == b"halt" {
            super::danger::shell_halt();
        } else {
            s_log("unknown command (try 'help')");
        }
        shell_prompt();
    }
}

/// **`escritorio`** -- levantar el servidor de Ring 3 otra vez.
///
/// # Por que hacia falta, y lo dijo el metal
///
/// Hasta el 2026-08-26 el escritorio se lanzaba en UN sitio: al arrancar, dentro
/// de `run_shell`, y como mucho un reintento si se habia muerto. O sea que si el
/// dueno acababa en este shell --por la patada, por el rescate, o porque el
/// escritorio se cayo mas tarde-- **la unica forma de recuperarlo era reiniciar
/// la maquina**.
///
/// El dueno lo pidio con las dos salidas juntas: *"eso no se limpio o debe
/// reiniciar el servidor del RING 3"*. La primera la resuelve
/// `core/emergencia.rs`; esta es la segunda.
///
/// # ** Y respeta el tope de intentos, que no es burocracia
///
/// `DESKTOP_MAX_ATTEMPTS` existe porque un relanzamiento automatico en bucle
/// convierte un fallo en un parpadeo infinito: el escritorio arranca, se cae,
/// arranca, se cae, y la pantalla no llega a mostrar el motivo de nada.
///
/// *** Pero un tope pensado para el ARRANQUE no vale para una orden que teclea
/// una persona. Que el sistema no lo reintente solo mas de dos veces es correcto;
/// que le diga "no" a alguien que lo esta pidiendo a proposito, no. Asi que la
/// orden **suma su intento y no se deja frenar por el tope** -- y lo dice, para
/// que el numero siga contando la verdad.
fn shell_escritorio() {
    use super::super::desktop::{desktop_died, start_desktop, DESKTOP_ATTEMPTS};
    if !desktop_died() && unsafe { DESKTOP_ATTEMPTS } > 0 {
        // Ya hay uno vivo. Levantarlo otra vez daria DOS escritorios peleandose
        // por la pantalla, que es peor que no tener ninguno.
        //
        // ** PERO "vivo" Y "se le ve" NO SON LA MISMA COSA, y esa diferencia es
        // un callejon sin salida que se cazo el 2026-08-31.
        //
        // Un programa lanzado con `run` desde AQUI --DOOM, `ray.bex`-- reclama
        // la pantalla y al morir se la devuelve al KERNEL, no al escritorio:
        // por este camino no hay nadie que la recupere, y esta dicho desde el
        // 14-08... en la salida del build, que se lee una vez en la vida.
        //
        // Lo que queda entonces es un escritorio VIVO y CIEGO. Y esta orden
        // contestaba *"ya esta vivo"*, que es verdad y no sirve de nada: manda
        // a esperar a algo que no va a pasar. La unica salida era acordarse de
        // Ctrl+Alt+Esc, o sea **matarlo a mano**.
        //
        // > El dueno lo dijo asi: *"se siente que tengo que hacer manual y matar
        // > el servidor de Ring 3 para volver a entrar"*. Tenia razon, y la
        // > maquina sabia por que sin decirlo.
        //
        // Ahora se pregunta. `fb::owner()` es UNA lectura atomica y la respuesta
        // parte el caso en dos estados que piden cosas distintas.
        if crate::ring0::obj::fb::owner().is_none() {
            // La pantalla es del kernel y el escritorio sigue vivo: esta ahi,
            // corriendo, sin nada donde pintar.
            s_log("el escritorio esta VIVO pero CIEGO: la pantalla la tiene el kernel");
            s_log("  (paso 1) Ctrl+Alt+Esc lo echa    (paso 2) `escritorio` lo levanta");
            s_log("  para la proxima: lanza los programas graficos desde su ICONO,");
            s_log("  que es el unico camino que le devuelve la pantalla al morir");
            return;
        }
        s_log("el escritorio ya esta vivo y tiene la pantalla (si no responde: Ctrl+Alt+Esc)");
        return;
    }
    s_log("levantando el escritorio de Ring 3...");
    start_desktop();
}
