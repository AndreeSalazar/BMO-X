//! **CARRIL ROJO** -- EL BITMAP: quien es dueno de cada marco.
//!
//! [carril]  ROJO      dar dos veces el mismo marco es dos duenos de un byte
//!
//! [cuesta]  MAQUINA -- entregar dos veces el mismo marco no da un fallo: da
//!           dos duenos del mismo byte, y el sintoma tres arranques despues.
//!
//! [riesgo]  ESPEJO -- `MAX_PHYS` es el techo de lo que el physmap alcanza, y
//!           NO es el unico sitio que lo decide: `vmm::caminable` juzga la
//!           misma direccion. El 30-08 los dos numeros no eran el mismo y la
//!           maquina se paro. Si este techo cambia, ese cambia con el.
//!
//! [prueba]  bmo-mmio-juicio
//!
//! ** Aqui NO SE TOCA UN SOLO BYTE DE MEMORIA. Se encienden y se apagan bits de
//! un bitmap de 512 KiB que dice quien tiene que marco. Esa es la linea con el
//! carril amarillo de al lado, y es exacta:
//!
//! ```text
//!    roja.rs      cambia el BITMAP    -> equivocarse reparte mal la RAM
//!    amarilla.rs  cambia la MEMORIA   -> equivocarse borra 4 KiB de alguien
//! ```

use boot_context::BootContext;

use super::super::PAGE;
use crate::ring0::plat::spin::SpinLock;

const MAX_PHYS: u64 = super::super::PHYSMAP_SIZE; // 16 GiB -- capped by the physmap
const FRAME_SLOTS: usize = (MAX_PHYS / PAGE) as usize / 64; // 65536 words

static mut BITMAP: [u64; FRAME_SLOTS] = [0; FRAME_SLOTS];
static mut TOTAL_FRAMES: u64 = 0;
static mut FREE_FRAMES: u64 = 0;
static mut HINT: usize = 0;
static LOCK: SpinLock = SpinLock::new("phys");

// -- *** EL MAPA SE GUARDA, Y NO SE GUARDABA -------------------------------
//
// `init` recorria el mapa del arranque, marcaba los marcos libres y lo TIRABA.
// Con eso basta para asignar memoria: el mapa de bits ya sabe que marco esta
// libre. Lo que el mapa de bits **no puede contestar nunca** es la otra
// pregunta, y es la que hace falta para ceder un aparato a Ring 3:
//
// ```text
//    esta libre este marco?      lo contesta el mapa de bits
//    es RAM esta direccion?      NO lo contesta: una direccion de RAM reservada
//                                y una que no es memoria se ven IGUAL ahi
// ```
//
// ** Y esa distincion es el veto que sostiene la cesion entera: si un rango que
// se cede pisa RAM usable, Ring 3 gana una ventana a la memoria del kernel. Ver
// `bmo_mmio_juicio::Veto::PisaRam`.
//
// Se guardan solo los tramos USABLES, que son los unicos contra los que se
// juzga, y en el mismo tope que trae el arranque.
const MAX_TRAMOS: usize = boot_context::MAX_MEMORY_ENTRIES;
static mut TRAMOS: [bmo_mmio_juicio::Tramo; MAX_TRAMOS] =
    [bmo_mmio_juicio::Tramo { base: 0, bytes: 0, es_ram: false }; MAX_TRAMOS];
static mut TRAMOS_N: usize = 0;

/// **Los tramos de RAM usable que declaro el arranque.**
///
/// Para el juez de la cesion, y no para asignar: quien asigna usa el mapa de
/// bits. Ver la nota de arriba.
pub fn tramos() -> &'static [bmo_mmio_juicio::Tramo] {
    unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(TRAMOS) as *const _, TRAMOS_N) }
}

extern "C" {
    /// End of the kernel image in memory (identity-mapped 1:1 at 0x400000).
    static __bss_end: u8;
}

#[inline]
fn bitmap() -> &'static mut [u64; FRAME_SLOTS] {
    unsafe { &mut *core::ptr::addr_of_mut!(BITMAP) }
}

/// Mark `[base, base+size)` as used. Only affects frames currently free;
/// counters stay exact because double-reserving is a no-op by design.
fn reserve_range(base: u64, size: u64) {
    if size == 0 || base >= MAX_PHYS {
        return;
    }
    let mut a = base & !(PAGE - 1);
    let mut end = (base + size + PAGE - 1) & !(PAGE - 1);
    if end > MAX_PHYS {
        end = MAX_PHYS;
    }
    let bm = bitmap();
    while a < end {
        let frame = (a / PAGE) as usize;
        let (w, b) = (frame / 64, frame % 64);
        if bm[w] & (1 << b) == 0 {
            bm[w] |= 1 << b;
            unsafe { FREE_FRAMES -= 1 };
        }
        a += PAGE;
    }
}

/// Initialize from the BootContext memory map. Called once from `phase::main`
/// before any allocation happens. Interrupts are off at that point on the BSP.
pub fn init(ctx: &BootContext) {
    let _g = LOCK.lock();
    let bm = bitmap();

    // 1. Everything is used until proven usable.
    for w in bm.iter_mut() {
        *w = !0;
    }
    unsafe {
        TOTAL_FRAMES = 0;
        FREE_FRAMES = 0;
        HINT = 0;
    }

    // 2. Free the usable entries (kind == 1), clipped to [0, 4 GiB).
    unsafe { TRAMOS_N = 0 };
    for e in ctx.memory_map[..ctx.memory_map_count as usize].iter() {
        if e.kind != 1 || e.size == 0 {
            continue;
        }
        // ** El tramo se apunta ENTERO y sin recortar al physmap, a proposito.
        //
        // El bucle de abajo recorta a `MAX_PHYS` porque el asignador no puede
        // entregar lo que el physmap no alcanza. El JUEZ es otra pregunta: una
        // direccion de RAM que este por encima de los 16 GiB sigue siendo RAM, y
        // cederla seguiria siendo una ventana. Recortar aqui seria dejar un
        // agujero por el que se cede memoria de verdad.
        unsafe {
            if TRAMOS_N < MAX_TRAMOS {
                (*core::ptr::addr_of_mut!(TRAMOS))[TRAMOS_N] =
                    bmo_mmio_juicio::Tramo { base: e.base, bytes: e.size, es_ram: true };
                TRAMOS_N += 1;
            }
        }
        let mut base = (e.base + PAGE - 1) & !(PAGE - 1);
        let mut end = (e.base + e.size) & !(PAGE - 1);
        if base >= MAX_PHYS {
            continue;
        }
        if end > MAX_PHYS {
            end = MAX_PHYS;
        }
        while base < end {
            let frame = (base / PAGE) as usize;
            bm[frame / 64] &= !(1 << (frame % 64));
            unsafe {
                TOTAL_FRAMES += 1;
                FREE_FRAMES += 1;
            }
            base += PAGE;
        }
    }

    // 3. Kernel-owned reservations.
    reserve_range(0, 0x10_0000); // legacy <1 MiB (future SMP trampoline lives here)
    let kernel_end = unsafe { (&__bss_end as *const u8 as u64 + PAGE - 1) & !(PAGE - 1) };
    reserve_range(0x40_0000, kernel_end - 0x40_0000); // kernel image + .bss (identity)
    for i in 0..ctx.stage_base.len() {
        let (b, s) = (ctx.stage_base[i], ctx.stage_size[i]);
        if b != 0 && s != 0 {
            reserve_range(b, s);
        }
    }
    reserve_range(ctx as *const _ as u64, core::mem::size_of::<BootContext>() as u64);
    if ctx.fb_addr != 0 {
        let fb_start = ctx.fb_addr & !(PAGE - 1);
        let fb_tail = (ctx.fb_stride as u64) * (ctx.fb_height as u64) * 4 + (ctx.fb_addr - fb_start);
        reserve_range(fb_start, fb_tail);
    }
    reserve_range(0xFEC0_0000, 0x140_0000); // LAPIC / I/O APIC / HPET window up to 4 GiB
    if ctx.ring3_payload_phys != 0 {
        reserve_range(ctx.ring3_payload_phys, ctx.ring3_payload_size);
    }
    if ctx.ring3_workspace_phys != 0 {
        reserve_range(ctx.ring3_workspace_phys, ctx.ring3_workspace_size);
    }
}

/// Allocate one 4 KiB frame. Returns its physical address, or `None` if the
/// pool is exhausted. Contents are unspecified; use `zero_frame` if the frame
/// will back page tables or user memory.
pub fn alloc_frame() -> Option<u64> {
    let _g = LOCK.lock();
    unsafe {
        if FREE_FRAMES == 0 {
            return None;
        }
        let bm = bitmap();
        let mut i = HINT % FRAME_SLOTS;
        loop {
            let w = bm[i];
            if w != !0 {
                let bit = (!w).trailing_zeros() as usize;
                bm[i] = w | (1 << bit);
                FREE_FRAMES -= 1;
                HINT = i;
                return Some((i * 64 + bit) as u64 * PAGE);
            }
            i = (i + 1) % FRAME_SLOTS;
        }
    }
}

/// Allocate `count` physically CONTIGUOUS frames; returns the base address.
///
/// Required by every multi-page region addressed linearly through the
/// physmap (kernel task stacks, the Ring 3 trap-landing stacks): the physmap
/// maps physical memory 1:1, so `phys_to_virt(base) + n*PAGE` is physical
/// `base + n*PAGE` -- N independent `alloc_frame` calls only produce that by
/// accident (clean QEMU maps) and not on real memory maps with holes, where
/// the tail pages would land in frames the caller does not own.
pub fn alloc_frames_contig(count: u64) -> Option<u64> {
    if count == 0 {
        return None;
    }
    let _g = LOCK.lock();
    unsafe {
        if FREE_FRAMES < count {
            return None;
        }
        let bm = bitmap();
        let total = FRAME_SLOTS * 64;
        let mut run: u64 = 0;
        let mut start = 0usize;
        let mut frame = 0usize;
        while frame < total {
            // Fast-skip fully used words when no run is open.
            if run == 0 && frame % 64 == 0 && bm[frame / 64] == !0 {
                frame += 64;
                continue;
            }
            if bm[frame / 64] & (1 << (frame % 64)) == 0 {
                if run == 0 {
                    start = frame;
                }
                run += 1;
                if run == count {
                    for f in start..start + count as usize {
                        bm[f / 64] |= 1 << (f % 64);
                    }
                    FREE_FRAMES -= count;
                    HINT = start / 64;
                    return Some(start as u64 * PAGE);
                }
            } else {
                run = 0;
            }
            frame += 1;
        }
        None
    }
}

/// Free a frame previously returned by `alloc_frame`. Freeing anything else
/// (reserved, unaligned, out of range, or double free) is a kernel bug and is
/// silently ignored -- callers must keep their own ownership straight.
pub fn free_frame(phys: u64) {
    if phys % PAGE != 0 || phys >= MAX_PHYS {
        return;
    }
    let _g = LOCK.lock();
    let frame = (phys / PAGE) as usize;
    let (w, b) = (frame / 64, frame % 64);
    let bm = bitmap();
    unsafe {
        if bm[w] & (1 << b) != 0 {
            bm[w] &= !(1 << b);
            FREE_FRAMES += 1;
        }
    }
}

/// `(total_usable_frames, free_frames)`.
pub fn stats() -> (u64, u64) {
    unsafe { (TOTAL_FRAMES, FREE_FRAMES) }
}
