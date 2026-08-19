//! Ring 0 ownership of the shared BMO Channel pages.
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! F3: each estuary is a capability-addressed kernel object. `CHANNEL_KICK`
//! and the timer tick service submissions with a budget, publish
//! completions, and wake `WAIT`ers through `scheduler::wake_by_key` using
//! the estuary's physical page address as the key.

use bmo_channel::Channel;
use boot_context::{BootContext, MAX_CHANNEL_PAGES};
use core::sync::atomic::Ordering;

pub const REQUEST_BUDGET_PER_CHANNEL: usize = 8;

/// A capability service bound to one estuary: receives each submission
/// `(opcode, a0, a1, a2)` and returns the completion entry, or `None`
/// to consume the request without publishing a completion.
pub type ServiceFn = fn(u64, u64, u64, u64) -> Option<(u64, u64, u64, u64)>;

/// Per-estuary service registry. Written only during single-threaded
/// boot (before `timer::enable`), read from trap paths afterwards.
static mut SERVICES: [Option<ServiceFn>; MAX_CHANNEL_PAGES] = [None; MAX_CHANNEL_PAGES];

/// Bind `service` to estuary `index`. Boot-time only (pre-timer).
pub fn register_service(index: usize, service: ServiceFn) {
    assert!(index < MAX_CHANNEL_PAGES);
    unsafe { (*core::ptr::addr_of_mut!(SERVICES))[index] = Some(service) };
}

fn service_for(index: usize) -> Option<ServiceFn> {
    unsafe { (*core::ptr::addr_of!(SERVICES))[index] }
}

#[repr(C, align(4096))]
#[derive(Clone, Copy)]
struct ChannelPage([u8; 4096]);

static mut CHANNEL_PAGES: [ChannelPage; MAX_CHANNEL_PAGES] =
    [ChannelPage([0; 4096]); MAX_CHANNEL_PAGES];

pub fn init(ctx: &mut BootContext) {
    for index in 0..MAX_CHANNEL_PAGES {
        let page = unsafe { core::ptr::addr_of_mut!(CHANNEL_PAGES[index]) };
        let channel = unsafe { &mut *(page.cast::<Channel>()) };
        channel.init();
        ctx.channel_pages[index] = page as u64;
    }
}

/// Physical address of a shared channel page (identity-mapped kernel
/// statics, so the symbol address IS the physical address). Used by the
/// process loader to map the estuaries into a Ring 3 address space.
pub fn page_phys(index: usize) -> u64 {
    assert!(index < MAX_CHANNEL_PAGES);
    unsafe { core::ptr::addr_of!(CHANNEL_PAGES[index]) as u64 }
}

fn channel(index: usize) -> &'static Channel {
    let page = unsafe { core::ptr::addr_of!(CHANNEL_PAGES[index]) };
    unsafe { &*(page.cast::<Channel>()) }
}

/// Wait key an estuary's `WAIT`ers block on: the page's physical address
/// is unique per estuary and stable for the life of the system.
pub fn wait_key(index: usize) -> u64 {
    page_phys(index)
}

/// Completion-side sequence: the value `WAIT(observed_sequence)` compares
/// against. Advances whenever Ring 0 publishes a completion.
pub fn complete_seq(index: usize) -> u64 {
    channel(index).complete_head.load(Ordering::Acquire)
}

/// Service one estuary with the per-kick budget. Returns the number of
/// requests processed; wakes waiters when completions were published.
///
/// Estuaries with a registered service dispatch to it; the rest fall
/// back to a transport acknowledgement that never interprets user-owned
/// pointers.
pub fn service(index: usize) -> usize {
    let ch = channel(index);
    if !ch.ring0_has_work() {
        return 0;
    }
    let processed = match service_for(index) {
        Some(handler) => ch.ring0_process_n(REQUEST_BUDGET_PER_CHANNEL, handler),
        None => ch.ring0_process_n(REQUEST_BUDGET_PER_CHANNEL, |opcode, a0, a1, a2| {
            Some((opcode, a0, a1, a2))
        }),
    };
    if processed > 0 {
        crate::ring0::task::scheduler::wake_by_key(wait_key(index));
    }
    processed
}

/// Service every estuary (timer tick path). Budgeted per channel so a
/// busy producer cannot monopolize a tick.
pub fn service_all() -> usize {
    let mut total = 0;
    for index in 0..MAX_CHANNEL_PAGES {
        total += service(index);
    }
    total
}
