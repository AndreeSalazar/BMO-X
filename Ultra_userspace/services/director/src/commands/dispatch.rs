//! **Which command goes where.** A router and nothing else.
//!
//! The bodies live in three files, and the boundary is not size -- it is WHO
//! THE COMMAND TALKS TO:
//!
//! ```text
//!   shell.rs    the desktop itself   nothing leaves the process
//!   files.rs    the disk             a capability that can be refused
//!   system.rs   the kernel           `OP_INFO` and its siblings
//! ```
//!
//! ## Why `run` is NOT here
//!
//! `Command::Launch` stays in `_start`. It is the only command that takes the
//! screen and the input capability **by value** (`lend_screen` hands the
//! machine to the child and takes it back), and those two are bindings of
//! `_start`, not fields of `Desktop`. Routing it here would mean threading
//! both through the router for one arm.
//!
//! ## Why this returns a value instead of using `continue`
//!
//! Three arms used to `continue` the `for &c in keys` loop -- one in `ls`, two
//! in `smp`. Out here there is no loop to continue, so the intent becomes a
//! value the caller acts on. Rust does not let that be wrong: a `continue`
//! with nothing to continue does not compile.

use bmo_userland as bmo;

use super::{disco, files, guia, shell, system, Command};
use crate::desktop::Desktop;

/// What the key loop should do once the command has run.
pub(crate) enum After {
    /// Skip the rest of this key -- what `continue` used to mean.
    NextKey,
    /// Fall through to putting the caret behind the line.
    Settle,
}

/// Run one command. `Launch` never arrives here; see the header.
pub(crate) fn dispatch(dsk: &mut Desktop, p: &bmo::Pantalla, cmd: Command) -> After {
    match cmd {
        Command::Nothing => shell::nothing(dsk, p),
        Command::NotLinux(verb) => shell::not_linux(dsk, p, verb),
        Command::PaintCost => shell::paint_cost(dsk, p),
        Command::Calculator => shell::calculator(dsk, p),
        Command::Clear => shell::clear(dsk, p),
        Command::Help => shell::help(dsk, p),
        Command::Guia => guia::guia(dsk, p),
        Command::NotAProgram(r) => shell::not_a_program(dsk, p, r),
        Command::Unknown => shell::unknown(dsk, p),
        Command::List(dir_path) => files::list(dsk, p, dir_path),
        Command::Read(file_path) => files::read(dsk, p, file_path),
        Command::Write(file_path, text) => files::write(dsk, p, file_path, text),
        Command::Save(arg) => files::save(dsk, p, arg),
        Command::SealMoved => system::seal_moved(dsk, p),
        Command::EstratosEscribe(n, t) => system::estratos_escribe(dsk, p, n, t),
        Command::Net(what) => system::net(dsk, p, what),
        Command::Placa => system::placa(dsk, p),
        Command::Audio(a) => system::audio(dsk, p, a),
        Command::Autopsy => system::autopsy(dsk, p),
        Command::Cabina(a) => system::cabina(dsk, p, a),
        Command::Report => system::report(dsk, p),
        Command::Cpu => system::cpu(dsk, p),
        Command::Ext => system::ext(dsk, p),
        Command::Consumo => system::consumo(dsk, p),
        Command::Apps => system::apps(dsk, p),
        Command::Memoria => system::memory(dsk, p),
        Command::Smp(arg) => system::smp(dsk, p, arg),
        Command::Banda => system::banda(dsk, p),
        // ** El unico brazo del router que puede ACTUAR sobre el almacen, y por
        // eso las subordenes se separan aqui y no dentro de una funcion: en esta
        // lista se ve de un vistazo que `trim ya` es la unica que hace algo.
        // ** La suborden y su argumento llegan YA partidos (ver `Command::Disco`).
        // El unico brazo que hace algo es `(trim, ya)`, y se lee de un vistazo
        // que hacen falta las DOS palabras para que el disco se mueva.
        Command::Disco(sub, arg) => match (sub, arg) {
            (b"", _) => disco::cuadro(dsk, p),
            (b"espacio" | b"libre", _) => disco::solo_espacio(dsk, p),
            (b"barrera" | b"flush" | b"vacia", _) => disco::barrera(dsk, p),
            (b"trim" | b"recorta" | b"recortar", b"ya") => disco::trim_ya(dsk, p),
            (b"trim" | b"recorta" | b"recortar", b"") => disco::trim_propuesta(dsk, p),
            // `disco trim manana` no es "no lo conozco": es un `trim` con una
            // palabra que sobra. Se ensena la propuesta --que es inofensiva-- y
            // se dice cual era la palabra buena, en vez de mandar a leer `help`.
            (b"trim" | b"recorta" | b"recortar", otro) => disco::trim_argumento(dsk, p, otro),
            (otro, _) => disco::no_existe(dsk, p, otro),
        },
        Command::Reboot => system::reboot(dsk, p),
        // `run` se queda en `_start`: se lleva la pantalla POR VALOR.
        Command::Launch(_) => After::Settle,
    }
}
