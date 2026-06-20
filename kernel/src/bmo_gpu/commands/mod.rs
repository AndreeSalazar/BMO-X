//! BMO GPU Commands — GPU command buffers (ring submission).
//!
//! v1.7.9: stub. The real implementation lives in v1.8 with AMDGPU.

#![allow(dead_code)]

/// A single GPU command in a command buffer.
#[derive(Debug, Clone, Copy)]
pub enum GpuCommand {
    /// Wait for the previous command to complete (fence).
    WaitFence(u64),
    /// Submit a BSF shader to a specific stage.
    SubmitShader { stage: u32, shader_id: u64 },
    /// Draw a triangle (test command).
    DrawTriangle { v0: [f32; 3], v1: [f32; 3], v2: [f32; 3] },
    /// Signal a fence when this command completes.
    SignalFence(u64),
}

/// A command buffer holding a list of GPU commands.
pub struct CommandBuffer {
    pub commands: [GpuCommand; 256],
    pub len: usize,
}

impl CommandBuffer {
    pub const fn new() -> Self {
        Self {
            // const fn can't have [expr; 256] for enum. Use [T; N] for Copy
            // but with [GpuCommand::WaitFence(0); 256] requires Copy.
            // GpuCommand is Copy, so this works.
            commands: [GpuCommand::WaitFence(0); 256],
            len: 0,
        }
    }

    pub fn push(&mut self, cmd: GpuCommand) -> Result<(), &'static str> {
        if self.len >= self.commands.len() {
            return Err("command buffer full");
        }
        self.commands[self.len] = cmd;
        self.len += 1;
        Ok(())
    }

    pub fn clear(&mut self) { self.len = 0; }
}

/// Submit a command buffer to the GPU (v1.7.9: stub).
pub fn submit(_cb: &CommandBuffer) -> u64 {
    // v1.8: write to AMDGPU ring buffer, return fence id.
    0
}
