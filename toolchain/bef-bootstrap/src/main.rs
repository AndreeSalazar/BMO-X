use std::path::PathBuf;

use bmo_abi::bef::writer::{BefBuilder, BefSection};

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("get_pid.bef"));

    // Deliberately assembly-only: no runtime, relocations, imports, TLS, or
    // privileged instructions. The payload proves the BMO ABI v2 entry path
    // with INVOKE(CURRENT_TASK, GET_PID), then remains in userspace.
    let code = vec![
        0x31, 0xC0, // xor eax, eax                    ; NR_INVOKE
        0x48, 0xBF, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                    // mov rdi, 0xffff_ffff_ffff_fffe ; CURRENT_TASK
        0xBE, 0x01, 0x00, 0x00, 0x00, // mov esi, 1 ; GET_PID
        0x31, 0xD2,                   // xor edx, edx
        0x45, 0x31, 0xD2,             // xor r10d, r10d
        0x45, 0x31, 0xC0,             // xor r8d, r8d
        0x45, 0x31, 0xC9,             // xor r9d, r9d
        0x0F, 0x05,                   // syscall
        0xF3, 0x90,                   // pause
        0xEB, 0xFC,                   // jmp pause
    ];

    let mut builder = BefBuilder::new();
    builder.add_section(BefSection::code(code));
    let image = builder.build().expect("bootstrap BEF must be valid");
    if image.len() > 16 * 1024 {
        panic!("bootstrap BEF exceeds the 16 KiB boot contract");
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).expect("create BEF output directory");
    }
    std::fs::write(&output, image).expect("write bootstrap BEF");
    println!("generated {}", output.display());
}
