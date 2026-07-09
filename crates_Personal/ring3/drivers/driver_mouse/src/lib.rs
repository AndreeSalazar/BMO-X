//! Mouse Driver — consumes mouse events from BMO Channel.
//!
//! Ring 0 ISR pushes 3-byte PS/2 mouse packets into the system channel.
//! This driver polls the channel and emits typed mouse events.
//!
//! ## Usage
//!
//! ```rust
//! let mouse = Mouse::connect(sys_channel_phys);
//! loop {
//!     for event in mouse.poll() {
//!         desktop.on_mouse(event);
//!     }
//! }
//! ```

#![no_std]

use ring3_foundation::ChannelClient;

const OP_MOUSE_MOVE: u64   = 0xB000_0010;
const OP_MOUSE_BUTTON: u64 = 0xB000_0011;

#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    pub dx: i64,
    pub dy: i64,
    pub buttons: u8,
}

pub struct Mouse {
    channel: ChannelClient,
    state: MouseState,
}

impl Mouse {
    pub fn connect(sys_channel_phys: u64) -> Self {
        Self {
            channel: ChannelClient::connect_system(sys_channel_phys),
            state: MouseState::default(),
        }
    }

    /// Poll for mouse events. Returns true if state changed.
    pub fn poll(&mut self) -> Option<MouseState> {
        let mut changed = false;
        self.channel.poll_with(|opcode, arg0, arg1, _arg2| {
            match opcode {
                OP_MOUSE_MOVE => {
                    self.state.dx = arg0 as i64;
                    self.state.dy = arg1 as i64;
                    changed = true;
                }
                OP_MOUSE_BUTTON => {
                    self.state.buttons = arg0 as u8;
                    changed = true;
                }
                _ => {}
            }
        });
        if changed { Some(self.state) } else { None }
    }
}
