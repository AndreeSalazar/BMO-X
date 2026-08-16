//! **The desktop's state**, and nothing that paints.
//!
//! ## Why this file exists before any of the splitting does
//!
//! `_start` had **fifty-two live locals** and 3.076 lines around them. That is
//! not a long function: it is a function whose state has no owner. Pulling a
//! block out of it meant a signature with twenty parameters, which moves the
//! problem into the call rather than solving it -- and that is exactly why
//! `watch.rs` was the only block ever extracted: it touched three.
//!
//! So the state gets an owner first. Every piece that used to be a `let mut`
//! in the prologue lives here, grouped by **what asks about it**, and every
//! block that comes out next takes `&mut Desktop` instead of a shopping list.
//!
//! ## What is NOT in here, and why that is the whole trick
//!
//! The screen and the input capability stay as locals in `_start`.
//!
//! - `Pantalla`'s painting is `&self` (only `activar_doble_bufer` is `&mut`,
//!   and that happens once). Keeping it OUTSIDE the struct means `&hw.screen`
//!   and `&mut desktop` are two different variables, so painting while
//!   mutating never needs split borrows, `RefCell`, or a dance.
//! - `lend_screen` takes both **by value** and hands them back, so they have to
//!   be movable bindings, not fields.
//!
//! Put the screen in here and every method that paints and mutates in the same
//! breath becomes a fight with the borrow checker. That is a design decision,
//! not an oversight.

pub(crate) mod boot;
pub(crate) mod keys;
pub(crate) mod mouse;
pub(crate) mod paint;
pub(crate) use boot::boot;

use core::mem::MaybeUninit;

use bmo_userland as bmo;

use crate::commands::history::History;
use crate::scene::calc::{Calc, CalcPad};
use crate::scene::cursor::SaveUnder;
use crate::scene::launcher::Launcher;
use crate::scene::output::Output;
use crate::scene::surface::Table;
use crate::scene::RunBox;
use crate::watch::Run;
use crate::PATH_MAX;

/// Which window a key belongs to. The policy lives in `bmo_input::Foco`; these
/// are just the ids it is told about.
pub(crate) const W_RUN: u8 = 0;
pub(crate) const W_DATA: u8 = 1;
pub(crate) const W_CABINA: u8 = 2;
/// F7 -- the CPU. See `scene::vitals`.
pub(crate) const W_CPU: u8 = 3;
/// F8 -- memory, with WHO is eating it.
pub(crate) const W_MEM: u8 = 4;
pub(crate) const W_SOUND: u8 = 3;

/// How many turns of the loop between blinks of the writing caret.
///
/// Counted in frames and not in time because there is no clock here: the three
/// syscalls do not include "what time is it". It is a blink that depends on the
/// speed of the machine, and for saying "you type here" that is enough.
pub(crate) const BLINK: u32 = 12_000;

/// The one line of the Run box, and everything needed to edit it.
pub(crate) struct Field {
    pub path: [u8; PATH_MAX],
    pub n: usize,
    /// Caret position INSIDE the line. Without it you can only type at the end
    /// and delete from the end: getting the third letter of a long path wrong
    /// forces you to delete everything back to it.
    pub cur: usize,
    /// Ctrl+C copies the whole line, Ctrl+V pastes it at the caret.
    pub clipboard: [u8; PATH_MAX],
    pub clipboard_n: usize,
    pub history: History,
    pub caret: bool,
    /// Turns since the last key. Reset on typing so the caret is ALWAYS lit
    /// while you write.
    pub since_key: u32,
    /// Keys the desktop feeds itself, not the keyboard. Today only the launcher
    /// puts any there, when an icon is clicked.
    pub injected: [u8; 32],
    pub ni: usize,
}

impl Field {
    pub fn new() -> Self {
        Self {
            path: [0; PATH_MAX],
            n: 0,
            cur: 0,
            clipboard: [0; PATH_MAX],
            clipboard_n: 0,
            history: History::new(),
            caret: true,
            since_key: 0,
            injected: [0; 32],
            ni: 0,
        }
    }

    /// The line as it stands. Written once here instead of `&path[..n]` in the
    /// forty places that need it -- and that slice is the one that panicked on
    /// 2026-08-09.
    pub fn line(&self) -> &[u8] {
        &self.path[..self.n]
    }
}

/// Every window, whether it is open, and who is on top.
///
/// ** OPEN is not the same as ON TOP. Open is "it exists and is drawn"; on top
/// is "it covers the other one". They are separate because there is no clipping
/// here: windows are painted whole, one over another, and the last one painted
/// wins.
pub(crate) struct Windows {
    pub data: crate::scene::data::DataWindow,
    pub data_open: bool,
    pub cabina: crate::scene::cabina::CabinaWindow,
    pub cabina_open: bool,
    pub cpu: crate::scene::vitals::VitalsWindow,
    pub cpu_open: bool,
    pub mem: crate::scene::vitals::VitalsWindow,
    pub mem_open: bool,
    pub sound: crate::scene::sound::SoundWindow,
    pub sound_open: bool,
    /// Who gets the keys. The policy lives in `bmo_input` and is tested THERE;
    /// here it is only asked, and what it decided is painted.
    pub focus: bmo_input::Foco,
    /// Who covered whom last turn, so the paint happens only on a change.
    pub top_before: u8,
    pub visible: bool,
    pub taskbar_dirty: bool,
    pub taskbar_state_before: (bool, u8, bool, bool, bool),
    pub switcher_painted: bool,
    pub alt_before: bool,
}

impl Windows {
    pub fn new(p: &bmo::Pantalla) -> Self {
        let mut focus = bmo_input::Foco::nuevo();
        focus.open(W_RUN);
        Self {
            data: crate::scene::data::DataWindow::new(p),
            data_open: false,
            cabina: crate::scene::cabina::CabinaWindow::new(p),
            cabina_open: false,
            cpu: crate::scene::vitals::VitalsWindow::new(p, crate::scene::vitals::Which::Cpu),
            cpu_open: false,
            mem: crate::scene::vitals::VitalsWindow::new(p, crate::scene::vitals::Which::Memoria),
            mem_open: false,
            sound: crate::scene::sound::SoundWindow::new(p),
            sound_open: false,
            focus,
            top_before: W_RUN,
            visible: true,
            taskbar_dirty: true,
            taskbar_state_before: (false, 0u8, false, false, false),
            switcher_painted: false,
            alt_before: false,
        }
    }
}

/// The output grid, the child's console, and the run being watched.
pub(crate) struct Out {
    pub grid: Output,
    /// This terminal's console. Everything launched from here writes into THIS
    /// ring and not into the kernel's panel.
    pub console: Option<bmo::Consola>,
    /// A launched program whose end is still being waited for. See `watch.rs`.
    pub run: Option<Run>,
    /// How many Ring 3 faults had been seen. Starts at the current total and
    /// not at zero: the ones from before the desktop started are already saved.
    pub faults_seen: u64,
}

/// The sound panel (F10).
///
/// The device is taken ON OPEN and given back ON CLOSE, and that is the design
/// decision of the whole window: `KIND_AUDIO` is exclusive, so claiming it at
/// startup would mean nothing launched from here could ever make a sound.
pub(crate) struct SoundState {
    pub cap: Option<bmo::Sonido>,
    pub devices: u64,
    /// Session state, not the painting module's: the volume survives closing
    /// and reopening the window.
    pub volume: u8,
    pub pressed: Option<usize>,
}

/// What only lives for one turn of the loop, plus the two edge detectors that
/// have to remember the turn before.
pub(crate) struct Tick {
    pub frames: u32,
    pub will_paint: bool,
    pub repaint_field: bool,
    /// Where the mouse cursor was left. `u32::MAX` means "nowhere yet".
    pub ax: u32,
    pub ay: u32,
    /// A click is a BUTTON GOING DOWN, not "the button is down". Without the
    /// edge, holding it would type a hundred times a second.
    pub button_before: bool,
    pub combo_before: bool,
    pub key_during_combo: bool,
    /// Which calculator key the pointer is over, if any. Carried as state
    /// because the highlight is repainted only WHEN IT CHANGES.
    pub calc_hover: Option<u8>,
    /// The rectangles left behind by windows whose app died, to give back to
    /// the desktop. A mailbox, not a state: filled and emptied in one turn.
    pub dead_boxes: [(u32, u32, u32, u32); crate::scene::surface::MAX],
}

/// The whole desktop, minus the screen and the input capability.
pub(crate) struct Desktop {
    pub field: Field,
    pub win: Windows,
    pub out: Out,
    pub snd: SoundState,
    pub tick: Tick,
    /// Apps that drew in their own memory and offered it. See `scene::surface`.
    pub table: Table,
    pub launcher: Launcher,
    pub run_box: RunBox,
    pub calc: Calc,
    pub calc_pad: CalcPad,
    /// What is UNDERNEATH the mouse cursor. Lifted at the start of the frame
    /// and placed back at the end; in between, everything else paints.
    pub save_under: SaveUnder,
    /// While the engine has not answered, its output does NOT go to the grid:
    /// it is the result, not a message. It piles up here.
    pub resp: [u8; 24],
    pub resp_n: usize,
}

impl Desktop {
    /// **La terminal cambio de sitio o de tamano.** Todo lo que se coloca a
    /// partir de ella tiene que enterarse, y por eso hay UNA funcion.
    ///
    /// Hoy son dos cosas: su propia geometria interior --el campo, el estado,
    /// la rejilla-- y la CALCULADORA, que se pinta pegada a su derecha y cuyo
    /// sitio se calculaba una sola vez al arrancar, cuando la ventana no se
    /// movia. Sin esto, arrastrar la terminal dejaria la calculadora plantada
    /// donde estaba: pintandose en el vacio y, peor, **respondiendo a clics en
    /// un sitio donde ya no hay nada** -- que es un fallo mudo, de los que este
    /// arbol persigue.
    pub(crate) fn run_relayout(&mut self) {
        self.run_box.relayout();
        self.calc_pad = CalcPad::new(&self.run_box);
    }
}

/// ** EL ESCRITORIO NO VIVE EN LA PILA, y eso no es una preferencia de estilo.
///
/// El 2026-08-14 el compositor murio en el Ryzen antes de escribir una sola
/// linea, con `#PF` en `rip=0x4000001B`. Ese `rip` es la SONDA DE PILA que LLVM
/// emite cuando un marco es tan grande que hay que ir tocando pagina a pagina:
///
/// ```text
///   4000000d:  subq $0x17000, %r11    <- el marco: 94.208 bytes
///   40000014:  subq $0x1000, %rsp        una pagina menos
///   4000001B:  movq $0x0, (%rsp)      <- aqui. La pagina 17 ya no esta mapeada
/// ```
///
/// Ring 3 tiene **16 paginas de pila = 65.536 bytes** (`USER_STACK_PAGES` en
/// `proc.rs`). El marco pedia 94.208. En la vuelta 17 la sonda pisa por debajo
/// de lo mapeado y el CPU dice `#PF`. No es azar ni es hardware: es resta.
///
/// ** Y el culpable, MEDIDO commit a commit con `llvm-objdump`:
///
/// ```text
///   bb671fb9  pre-struct   35.560   cabe
///   70df8f73  el struct    95.544   NO CABE   <- aqui murio, 5 commits atras
///   c6febd8e  hoy          95.528   NO CABE
/// ```
///
/// ** El motivo de que juntar 52 locales en un struct TRIPLIQUE el marco: como
/// variables sueltas, LLVM solapaba las ranuras de las que no viven a la vez y
/// se llevaba muchas a registros. Como struct contiguo **todo esta vivo desde
/// que se construye** y no hay nada que solapar. El reparto no rompio nada --el
/// `.text` bajo, los `#[inline(never)]` ganaron 29.616 bytes de verdad-- pero
/// el commit que ORDENO las variables mato la maquina en un eje que nadie
/// miraba.
///
/// [!] La regla que queda, y vale para cualquier programa de Ring 3, no solo
/// para este: **el estado que vive todo el programa va a `.bss`, no a la pila.**
/// Con 64 KiB de pila o con 1 MiB sigue siendo cierto; subir la pila solo mueve
/// el dia en que el siguiente struct la desborde. Y por el escalon 0 de
/// `docs/LA_RAM.md`, `.bss` **se declara y no viaja**: cero bytes de fichero.
static mut DESKTOP: MaybeUninit<Desktop> = MaybeUninit::uninit();

/// Construye el escritorio DENTRO de `.bss` y entrega la unica referencia.
///
/// Se llama **una vez**, desde `boot`. No hay forma de pedir la segunda: la
/// referencia es `&'static mut` y quien la tiene la tiene entera, que es
/// exactamente la propiedad que un `static mut` suelto no da.
///
/// [!] `#[inline(never)]` en las DOS --aqui y en `Desktop::new`-- y no es un
/// ajuste de tamano: es lo que obliga a LLVM a pasar la direccion de `.bss`
/// como puntero de retorno (`sret`) en vez de construir el struct en una ranura
/// de la pila y copiarlo despues. Inlineadas, los temporales de `Out` (17.936)
/// y `Launcher` (12.968) se acumulan en el marco de `_start`. Medido.
/// ** SE ESCRIBE CAMPO A CAMPO, y eso es el punto entero de esta funcion.
///
/// La primera version era `slot.write(Desktop::new(...))`, o sea construir el
/// struct y moverlo. **No basto, y el Ryzen lo dijo con numeros** el 2026-08-14:
///
/// ```text
///   veredicto *** PILA DESBORDADA: 3536 B bajo el fondo, pila 65536
///   rip       0x40011ffb  +0x11ffb     <- Desktop::new + 27, su sonda de pila
/// ```
///
/// `sret` llevaba el `Desktop` a `.bss`, si -- pero los TEMPORALES INTERMEDIOS
/// seguian en la pila: el `Out` del literal (17.936 B) y el `Launcher` que
/// entraba **por valor** como parametro (12.968 B). Medido:
///
/// ```text
///   _start         26.616
///   Desktop::new   35.656
///                  ------
///                  62.272  + install + los push  ->  69.072 de 65.536
/// ```
///
/// Escribiendo campo por campo con `addr_of_mut!`, **cada constructor recibe la
/// direccion final de SU campo** y escribe alli directamente: no hay un momento
/// en que exista un `Desktop` ni un `Out` completos fuera de `.bss`.
///
/// [!] Y el `Launcher` **se construye aqui dentro**, no se recibe. Un parametro
/// por valor de 12.968 bytes lo copia el LLAMANTE en su propio marco, asi que
/// mientras entrara por la firma no habia forma de quitarlo desde aqui.
#[inline(never)]
pub(crate) fn install(p: &bmo::Pantalla, console: Option<bmo::Consola>) -> &'static mut Desktop {
    // `addr_of_mut!` y no `&mut DESKTOP`: tomar una referencia a un `static mut`
    // es UB aunque compile, y aqui hay un solo llamante que puede demostrarlo.
    let slot = core::ptr::addr_of_mut!(DESKTOP) as *mut Desktop;
    unsafe {
        // El orden importa en uno solo: `calc_pad` se coloca a partir de la caja
        // de ejecucion, asi que `run_box` va antes y se lee desde su sitio ya
        // definitivo. Los demas son independientes.
        core::ptr::addr_of_mut!((*slot).run_box).write(RunBox::new(p));
        core::ptr::addr_of_mut!((*slot).calc_pad).write(CalcPad::new(&(*slot).run_box));
        core::ptr::addr_of_mut!((*slot).field).write(Field::new());
        core::ptr::addr_of_mut!((*slot).win).write(Windows::new(p));
        core::ptr::addr_of_mut!((*slot).launcher).write(Launcher::new());
        core::ptr::addr_of_mut!((*slot).table).write(Table::new());
        core::ptr::addr_of_mut!((*slot).calc).write(Calc::new());
        core::ptr::addr_of_mut!((*slot).save_under).write(SaveUnder::new());
        // `Out` tambien por campos: su literal entero media 17.936 B de
        // temporal, que era la mitad del marco de la version anterior.
        core::ptr::addr_of_mut!((*slot).out.grid).write(Output::new());
        core::ptr::addr_of_mut!((*slot).out.console).write(console);
        core::ptr::addr_of_mut!((*slot).out.run).write(None);
        core::ptr::addr_of_mut!((*slot).out.faults_seen).write(bmo::autopsia_total());
        core::ptr::addr_of_mut!((*slot).snd).write(SoundState {
            cap: None,
            devices: 0,
            volume: 80,
            pressed: None,
        });
        core::ptr::addr_of_mut!((*slot).tick).write(Tick {
            frames: 0,
            will_paint: false,
            repaint_field: false,
            ax: u32::MAX,
            ay: u32::MAX,
            button_before: false,
            combo_before: false,
            key_during_combo: false,
            calc_hover: None,
            dead_boxes: [(0, 0, 0, 0); crate::scene::surface::MAX],
        });
        core::ptr::addr_of_mut!((*slot).resp).write([0; 24]);
        core::ptr::addr_of_mut!((*slot).resp_n).write(0);
        &mut *slot
    }
}

