//! Framebuffer globals ??? populated from `BootContext` at kernel entry.
//!
//! These are the simplest possible "interface" that the splash, syscall
//! handlers, and any future pixel-drawing code consume. They are written
//! exactly once from `_start` and then treated as read-only.

use boot_context::BootContext;

/// Linear framebuffer base address (XRGB-8888, 4 bytes per pixel).
pub static mut FB_ADDR: u64 = 0;
/// Framebuffer width in pixels.
pub static mut FB_WIDTH: u32 = 0;
/// Framebuffer height in pixels.
pub static mut FB_HEIGHT: u32 = 0;
/// Framebuffer stride in pixels (NOT bytes).
pub static mut FB_STRIDE: u32 = 0;
/// Framebuffer pixel format code (0=Unknown/RGB, 1=BGR, 2=RGB).
pub static mut FB_PIXEL_FORMAT: u32 = 0;

/// Populate the globals from a `BootContext` populated by the UEFI
/// chain. Safe to call once at kernel entry.
pub fn init_from(ctx: &BootContext) {
    unsafe {
        FB_ADDR = ctx.fb_addr;
        FB_WIDTH = ctx.fb_width;
        FB_HEIGHT = ctx.fb_height;
        FB_STRIDE = ctx.fb_stride;
        FB_PIXEL_FORMAT = ctx.fb_pixel_format;
    }
}

/// La pantalla esta CEDIDA a un proceso Ring 3.
///
/// Mientras lo este, el kernel no dibuja: ni el panel, ni CABINA, ni los logs
/// de los drivers. No es una optimizacion, es la definicion de haber cedido --
/// dos duenos pintando el mismo framebuffer no es "compartir", es parpadeo.
///
/// La excepcion es el reporter de faults, que usa `hay_fb_crudo`: un fault de
/// kernel es terminal y recuperar la pantalla para contarlo es exactamente lo
/// que hay que hacer.
static CEDIDO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

pub fn ceder_fb(cedido: bool) {
    CEDIDO.store(cedido, core::sync::atomic::Ordering::SeqCst);
}

pub fn fb_cedido() -> bool {
    CEDIDO.load(core::sync::atomic::Ordering::SeqCst)
}

/// Returns true if a framebuffer is available and the splash should
/// render. The chain may pass an empty FB if UEFI GOP was not present.
///
/// Falso tambien cuando la pantalla esta cedida a Ring 3: **todos** los
/// caminos de dibujo del kernel ya preguntan por aqui, asi que ceder la
/// pantalla los apaga a todos de una vez, sin tocarlos uno a uno.
pub fn has_fb() -> bool {
    hay_fb_crudo() && !fb_cedido()
}

/// Existe fisicamente un framebuffer? Sin mirar quien es su dueno.
/// Solo para el reporter de faults y para el propio objeto de capability.
pub fn hay_fb_crudo() -> bool {
    unsafe { FB_ADDR != 0 && FB_WIDTH != 0 && FB_HEIGHT != 0 }
}
