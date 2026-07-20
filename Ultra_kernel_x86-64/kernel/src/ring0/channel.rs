//! Ring 0 ownership of the shared BMO Channel pages.

use bmo_channel::Channel;
use boot_context::{BootContext, MAX_CHANNEL_PAGES};

pub const REQUEST_BUDGET_PER_CHANNEL: usize = 8;

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

pub fn service_all() -> usize {
    let mut total = 0;
    for index in 0..MAX_CHANNEL_PAGES {
        let page = unsafe { core::ptr::addr_of!(CHANNEL_PAGES[index]) };
        let channel = unsafe { &*(page.cast::<Channel>()) };
        if !channel.ring0_has_work() { continue; }
        total += channel.ring0_process_n(REQUEST_BUDGET_PER_CHANNEL, |opcode, a0, a1, a2| {
            // Until capability-specific services are registered, acknowledge
            // transport delivery without interpreting user-owned pointers.
            Some((opcode, a0, a1, a2))
        });
    }
    total
}
