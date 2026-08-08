//! Generates the first *visible* BMO Ring 3 program: a freestanding BEX that
//! greets the world by asking Ring 0 to draw, then exits cleanly.
//!
//! The whole point is to demonstrate the complete privilege-crossing chain
//! end to end, on real metal, without a keyboard:
//!
//!   Ring 3 program  --INVOKE(CURRENT_TASK, CONSOLE_WRITE, bytes)-->  Ring 0
//!                   <-- kernel paints the text on the framebuffer --
//!   Ring 3 program  --INVOKE(CURRENT_TASK, EXIT)-->  scheduler reaps it
//!
//! Like `bef-bootstrap`, this is deliberately assembly-only: no runtime, no
//! relocations, imports, TLS or privileged instructions. Every byte is
//! auditable. The kernel embeds the result via `include_bytes!` and runs it
//! as the init process when the boot chain reserved no Ring 3 payload.

use std::path::PathBuf;

use bmo_abi::bef::writer::{BefBuilder, BefSection};

// Mirror of the frozen BMO ABI v2 surface (see bmo-abi syscalls::surface).
const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
const TASK_OP_EXIT: u32 = 0x04;
const TASK_OP_CONSOLE_WRITE: u32 = 0x06;

/// The greeting Ring 0 will render on the user's behalf. Each line ends in
/// `\n`, which flushes it to its own row in the kernel-log panel.
const MESSAGE: &str = "BMO-X: hola mundo desde Ring 3\nCPL3 -> INVOKE -> CPL0 OK\n";

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("init_hello.bex"));

    let mut code: Vec<u8> = Vec::new();

    // mov rdi, CURRENT_TASK        (48 BF <imm64>)   -- held across all calls
    code.extend_from_slice(&[0x48, 0xBF]);
    code.extend_from_slice(&CURRENT_TASK.to_le_bytes());
    // mov esi, TASK_OP_CONSOLE_WRITE (BE <imm32>)    -- zero-extends into rsi
    code.push(0xBE);
    code.extend_from_slice(&TASK_OP_CONSOLE_WRITE.to_le_bytes());

    // One INVOKE per 8-byte little-endian chunk of the message. A syscall
    // clobbers rax (status) and rdx (return value) but preserves rdi/rsi, so
    // each iteration only reloads rax = NR_INVOKE (0) and rdx = chunk.
    for chunk in MESSAGE.as_bytes().chunks(8) {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        // xor eax, eax             (31 C0)           -- NR_INVOKE
        code.extend_from_slice(&[0x31, 0xC0]);
        // mov rdx, <chunk>         (48 BA <imm64>)
        code.extend_from_slice(&[0x48, 0xBA]);
        code.extend_from_slice(&word);
        // syscall                  (0F 05)
        code.extend_from_slice(&[0x0F, 0x05]);
    }

    // INVOKE(CURRENT_TASK, EXIT): rdi still holds CURRENT_TASK.
    code.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax -- NR_INVOKE
    code.push(0xBE); // mov esi, TASK_OP_EXIT
    code.extend_from_slice(&TASK_OP_EXIT.to_le_bytes());
    code.extend_from_slice(&[0x0F, 0x05]); // syscall (does not return)

    // Safety net if EXIT ever returns: spin instead of running off the end.
    code.extend_from_slice(&[0xF3, 0x90]); // pause
    code.extend_from_slice(&[0xEB, 0xFC]); // jmp -4 (back to pause)

    let mut builder = BefBuilder::new();
    builder.add_section(BefSection::code(code));
    builder.entry_offset = 0;
    let image = builder.build().expect("hello BEX must be valid");
    if image.len() > 16 * 1024 {
        panic!("hello BEX exceeds the 16 KiB boot contract");
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).expect("create BEX output directory");
        }
    }
    std::fs::write(&output, &image).expect("write hello BEX");
    println!("generated {} ({} bytes)", output.display(), image.len());
}
