//! Virtual memory: address spaces built on the `s2_mem` physmap.
//!
//! Every user address space is a private PML4 that shares the kernel half
//! (PML4[256..512] -- the physmap and future kernel higher-half mappings) and
//! owns a private PDPT under PML4[0]. Entry 0 of that PDPT is a copy of the
//! kernel's supervisor identity map (0..32 MiB); everything the user touches
//! lives at PDPT index >= 1 so the shared identity tables are never polluted.
//!
//! Layout contract (BMO ABI, stable):
//! ```text
//!   1 GiB  USER_IMAGE_BASE   BEX sections (code/rodata/data/bss)
//!   2 GiB  USER_STACK_TOP    user stack, grows down (mapped in F2)
//!   3 GiB  CHANNEL_VA_BASE   16 BMO Channel pages, U/S shared with Ring 0
//! ```

use super::{phys, phys_to_virt, PAGE};

pub const PTE_PRESENT: u64 = 1 << 0;
pub const PTE_WRITABLE: u64 = 1 << 1;
pub const PTE_USER: u64 = 1 << 2;
pub const PTE_HUGE: u64 = 1 << 7;
/// * En una PTE de 4 KiB el bit 7 **no** es "pagina grande": es el bit alto
/// del indice de PAT. Con `PWT`(3) y `PCD`(4) a cero, ponerlo selecciona la
/// entrada **4** de la tabla -- la que `s1_cpu` deja en Write-Combining.
///
/// El mismo numero significa dos cosas distintas segun el nivel de tabla, y
/// por eso lleva nombre propio: en una PDE seria `PS` y convertiria la entrada
/// en una pagina de 2 MiB.
pub const PTE_PAT_4K: u64 = 1 << 7;
const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

pub const USER_IMAGE_BASE: u64 = 0x0000_0000_4000_0000;
pub const USER_STACK_TOP: u64 = 0x0000_0000_8000_0000;
pub const USER_STACK_SIZE: u64 = 0x10_0000;
pub const CHANNEL_VA_BASE: u64 = 0x0000_0000_C000_0000;
/// Donde se mapea el framebuffer en el espacio de quien reclame la pantalla.
/// Por encima de los estuarios y con sitio de sobra: 4K x 4K x 4 B son 64 MiB
/// y aqui hay un hueco entero de 1 GiB antes del limite del canonical bajo.
pub const FRAMEBUFFER_VA_BASE: u64 = 0x0000_0000_D000_0000;
/// Donde empiezan los bloques que un proceso PIDE (`KIND_MEMORIA`).
///
/// Detras del framebuffer y con 256 MiB de hueco antes: el tope por peticion
/// son 64 MiB y hay cuatro peticiones, asi que el peor caso cabe entero sin
/// acercarse a nada. Cada proceso avanza su propio cursor desde aqui -- dos
/// bloques del mismo proceso no se pisan y cada uno tiene su rango.
pub const MEMORIA_VA_BASE: u64 = 0x0000_0000_E000_0000;

static mut KERNEL_PML4: u64 = 0;

/// Kernel-half PML4 slots that were still empty at init and could not get a
/// pre-populated PDPT (allocator exhaustion -- should never happen at boot).
static mut KERNEL_HALF_HOLES: u32 = 0;

/// Capture the boot CR3 (installed by `s2_mem`) and **pre-populate the whole
/// kernel half**: every empty PML4 slot in [256..512) gets a zeroed,
/// supervisor-only PDPT right now, *before any user address space exists*.
///
/// This is the invariant that makes `new_address_space`'s entry copy a true
/// share-by-pointer forever: user PML4s copy these 256 entries once, so any
/// higher-half mapping the kernel adds later (physmap growth toward 1 TiB,
/// MMIO windows, a kernel heap) lands *inside* a PDPT every address space
/// already points at -- visible under every CR3, no per-process patching.
/// Cost: <=256 frames (1 MiB), once.
pub fn init() {
    unsafe { KERNEL_PML4 = read_cr3() };
    let kernel = table(kernel_pml4());
    for i in 256..512 {
        if kernel[i] & PTE_PRESENT == 0 {
            match phys::alloc_frame() {
                Some(f) => {
                    phys::zero_frame(f);
                    // Supervisor-only on purpose: no PTE_USER at any level of
                    // the kernel half, ever.
                    kernel[i] = (f & ADDR_MASK) | PTE_PRESENT | PTE_WRITABLE;
                }
                None => unsafe { KERNEL_HALF_HOLES += 1 },
            }
        }
    }
    // Fresh top-level entries: reload CR3 so no stale paging-structure cache
    // survives (cheap, once at boot).
    switch_to(kernel_pml4());
    if unsafe { KERNEL_HALF_HOLES } != 0 {
        crate::ring0::dev::console::serial_write(
            "[vmm] WARN: kernel-half pre-population incomplete\n",
        );
    }
}

/// Number of kernel-half PML4 slots left unpopulated at init (0 = healthy).
pub fn kernel_half_holes() -> u32 {
    unsafe { KERNEL_HALF_HOLES }
}

pub fn read_cr3() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("mov {}, cr3", out(reg) v, options(nostack)); }
    v & ADDR_MASK
}

pub fn kernel_pml4() -> u64 {
    unsafe { KERNEL_PML4 }
}

/// Load CR3 with another address space. Only safe while running on
/// kernel-owned mappings (physmap / identity), which exist in every
/// address space the process loader creates.
pub fn switch_to(pml4: u64) {
    unsafe { core::arch::asm!("mov cr3, {}", in(reg) pml4, options(nostack)) };
}

/// Page-table frame as a mutable 512-entry array, through the physmap.
fn table(phys: u64) -> &'static mut [u64; 512] {
    unsafe { &mut *(phys_to_virt(phys) as *mut [u64; 512]) }
}

/// Create an empty user address space. Returns the physical PML4 address.
/// Kernel entries are shared read-write (supervisor-only leaves); the user
/// half starts empty except for the copied identity entry.
pub fn new_address_space() -> Option<u64> {
    let pml4 = phys::alloc_frame()?;
    let pdpt = match phys::alloc_frame() {
        Some(f) => f,
        None => {
            phys::free_frame(pml4);
            return None;
        }
    };
    phys::zero_frame(pml4);
    phys::zero_frame(pdpt);

    let kernel = table(kernel_pml4());
    let user = table(pml4);
    // Share the entire kernel half (physmap lives at index 256..). Since
    // `init` pre-populated every slot, this copy is share-by-pointer of the
    // PDPTs themselves: kernel-half mappings added *after* this process was
    // created are still visible under its CR3.
    for i in 256..512 {
        user[i] = kernel[i];
    }
    // PTE_USER here is required so user mappings under PDPT[1..] are
    // reachable; the identity region stays supervisor because the copied
    // PDPT[0] entry and its huge-page leaves do not carry PTE_USER.
    user[0] = pdpt | PTE_PRESENT | PTE_WRITABLE | PTE_USER;
    let kernel_pdpt = table(kernel[0] & ADDR_MASK);
    let user_pdpt = table(pdpt);
    user_pdpt[0] = kernel_pdpt[0];
    Some(pml4)
}

fn get_or_create(t: &mut [u64; 512], idx: usize, flags: u64) -> Result<u64, ()> {
    let e = t[idx];
    if e & PTE_PRESENT != 0 {
        if e & PTE_HUGE != 0 {
            return Err(());
        }
        return Ok(e & ADDR_MASK);
    }
    let f = phys::alloc_frame().ok_or(())?;
    phys::zero_frame(f);
    t[idx] = (f & ADDR_MASK) | flags;
    Ok(f)
}

/// Map one 4 KiB page. `user` sets U/S on every level touched; `writable`
/// controls the leaf's R/W bit. Fails on misalignment or a huge-page
/// collision (which would mean the VA overlaps the kernel identity map).
pub fn map_page(pml4: u64, va: u64, pa: u64, user: bool, writable: bool) -> Result<(), ()> {
    map_page_tipo(pml4, va, pa, user, writable, false)
}

/// Igual, pero eligiendo **Write-Combining** para esta pagina.
///
/// Se usa para el framebuffer y nada mas: es donde se escriben millones de
/// pixeles seguidos y donde juntar las escrituras cambia el orden de magnitud.
/// Para memoria normal seria lo contrario de lo que se quiere -- WC no garantiza
/// el orden de las escrituras, y eso en una estructura de datos es un bug.
pub fn map_page_wc(pml4: u64, va: u64, pa: u64, user: bool, writable: bool) -> Result<(), ()> {
    map_page_tipo(pml4, va, pa, user, writable, true)
}

fn map_page_tipo(
    pml4: u64,
    va: u64,
    pa: u64,
    user: bool,
    writable: bool,
    combinar_escrituras: bool,
) -> Result<(), ()> {
    if va % PAGE != 0 || pa % PAGE != 0 {
        return Err(());
    }
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;
    let mid = PTE_PRESENT | PTE_WRITABLE | if user { PTE_USER } else { 0 };

    let pt_phys = {
        let p = table(pml4);
        let pdpt_phys = get_or_create(p, i4, mid)?;
        let pdpt = table(pdpt_phys);
        let pd_phys = get_or_create(pdpt, i3, mid)?;
        let pd = table(pd_phys);
        get_or_create(pd, i2, mid)?
    };
    let pt = table(pt_phys);

    let mut entry = (pa & ADDR_MASK) | PTE_PRESENT;
    if writable {
        entry |= PTE_WRITABLE;
    }
    if user {
        entry |= PTE_USER;
    }
    if combinar_escrituras {
        entry |= PTE_PAT_4K;
    }
    let old = pt[i1];
    pt[i1] = entry;
    if old & PTE_PRESENT != 0 {
        unsafe { core::arch::asm!("invlpg [{}]", in(reg) va, options(nostack)) };
    }
    Ok(())
}

/// Remove a mapping. Returns the physical address that was mapped, if any.
/// Does not free table frames (they are recycled by the address space).
pub fn unmap_page(pml4: u64, va: u64) -> Option<u64> {
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;

    let p = table(pml4);
    let e = p[i4];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pdpt = table(e & ADDR_MASK);
    let e = pdpt[i3];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pd = table(e & ADDR_MASK);
    let e = pd[i2];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pt = table(e & ADDR_MASK);
    let e = pt[i1];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    pt[i1] = 0;
    unsafe { core::arch::asm!("invlpg [{}]", in(reg) va, options(nostack)) };
    Some(e & ADDR_MASK)
}

/// Resolve a virtual address through the tables. Debugging aid; returns the
/// physical base of the mapped page.
pub fn translate(pml4: u64, va: u64) -> Option<u64> {
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;

    let p = table(pml4);
    let e = p[i4];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    let pdpt = table(e & ADDR_MASK);
    let e = pdpt[i3];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_C000_0000) + (va & 0x3FFF_FFFF));
    }
    let pd = table(e & ADDR_MASK);
    let e = pd[i2];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_FFE0_0000) + (va & 0x1F_FFFF));
    }
    let pt = table(e & ADDR_MASK);
    let e = pt[i1];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    Some(e & ADDR_MASK)
}

/// Free every page-table frame owned by the user half (PML4[0]'s private
/// PDPT, skipping the shared identity entry at PDPT[0]) plus the PML4 itself.
/// Leaf frames are owned by the process layer and must be freed by it first.
pub fn destroy_address_space(pml4: u64) {
    let user = table(pml4);
    let e0 = user[0];
    if e0 & PTE_PRESENT != 0 {
        let pdpt_phys = e0 & ADDR_MASK;
        let pdpt = table(pdpt_phys);
        for i3 in 1..512 {
            let e = pdpt[i3];
            if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
                continue;
            }
            let pd_phys = e & ADDR_MASK;
            let pd = table(pd_phys);
            for i2 in 0..512 {
                let e2 = pd[i2];
                if e2 & PTE_PRESENT == 0 || e2 & PTE_HUGE != 0 {
                    continue;
                }
                phys::free_frame(e2 & ADDR_MASK);
            }
            phys::free_frame(pd_phys);
        }
        phys::free_frame(pdpt_phys);
    }
    phys::free_frame(pml4);
}

/// End-to-end check: allocate, build an address space, map, translate,
/// write through the physmap, unmap, destroy, free. Returns false on the
/// first failed step. Safe to run from the serial shell at any time.
pub fn self_test() -> bool {
    let frame = match phys::alloc_frame() {
        Some(f) => f,
        None => return false,
    };
    let aspace = match new_address_space() {
        Some(s) => s,
        None => {
            phys::free_frame(frame);
            return false;
        }
    };
    let va = USER_IMAGE_BASE;
    let mut ok = map_page(aspace, va, frame, true, true).is_ok();
    if ok {
        ok = translate(aspace, va) == Some(frame);
    }
    if ok {
        unsafe {
            let p = phys_to_virt(frame) as *mut u64;
            p.write_volatile(0xB400_0000_0000_0001);
            ok = p.read_volatile() == 0xB400_0000_0000_0001;
        }
    }
    if unmap_page(aspace, va) != Some(frame) {
        ok = false;
    }
    if ok {
        ok = translate(aspace, va).is_none();
    }
    destroy_address_space(aspace);
    phys::free_frame(frame);
    ok
}
