//! Keyboard Driver — consumes keyboard events from BMO Channel.
//!
//! Ring 0 ISR pushes scancodes into the system channel.
//! This driver polls the channel and emits typed keyboard events.
//!
//! ## Usage
//!
//! ```rust
//! let kbd = Keyboard::connect(sys_channel_phys);
//! loop {
//!     for event in kbd.poll() {
//!         desktop.on_key(event);
//!     }
//! }
//! ```

#![no_std]

extern crate alloc;

use ring3_foundation::ChannelClient;

/// BMO Channel opcodes (must match kernel irq/keyboard.rs).
const OP_KEY: u64 = 0xB000_0002;

/// Keyboard event.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    /// PS/2 Set 1 scancode (0-127).
    pub scancode: u8,
    /// true = pressed, false = released.
    pub pressed: bool,
}

/// Keyboard driver connected to the system BMO Channel.
pub struct Keyboard {
    channel: ChannelClient,
}

impl Keyboard {
    /// Connect to the system channel where kernel ISR pushes keyboard events.
    pub fn connect(sys_channel_phys: u64) -> Self {
        Self {
            channel: ChannelClient::connect_system(sys_channel_phys),
        }
    }

    /// Poll for keyboard events. Returns all pending events since last poll.
    pub fn poll(&mut self) -> alloc::vec::Vec<KeyEvent> {
        let mut events = alloc::vec::Vec::new();
        self.channel.poll_with(|opcode, arg0, arg1, _arg2| {
            if opcode == OP_KEY {
                events.push(KeyEvent {
                    scancode: arg0 as u8,
                    pressed: arg1 != 0,
                });
            }
        });
        events
    }
}
