//! ByteDefender signature database
//!
//! Known threat signatures for pre-execution detection.

#![allow(dead_code)]

pub struct SignatureMatch {
    pub id: u32,
    pub name: [u8; 128],
    pub offset: u64,
    pub severity: u8,
}

/// Helper to create a name array
fn make_name(src: &[u8]) -> [u8; 128] {
    let mut name = [0u8; 128];
    let len = src.len().min(127);
    name[..len].copy_from_slice(&src[..len]);
    name
}

/// Scan data against known signatures
pub fn scan(data: &[u8]) -> Option<SignatureMatch> {
    if data.len() < 16 { return None; }

    // Check for known malware patterns
    if let Some(m) = scan_known_malware(data) { return Some(m); }
    if let Some(m) = scan_shellcode_kits(data) { return Some(m); }
    if let Some(m) = scan_exploit_patterns(data) { return Some(m); }

    None
}

fn scan_known_malware(data: &[u8]) -> Option<SignatureMatch> {
    // EICAR test file (standard antivirus test)
    let eicar = b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*";
    for i in 0..data.len().saturating_sub(eicar.len()) {
        if &data[i..i + eicar.len()] == eicar {
            return Some(SignatureMatch {
                id: 0x0001,
                name: make_name(b"EICAR-Test-File"),
                offset: i as u64,
                severity: 1,
            });
        }
    }

    // Known ransomware patterns
    let ransom_note = b"All your files have been encrypted";
    for i in 0..data.len().saturating_sub(ransom_note.len()) {
        if &data[i..i + ransom_note.len()] == ransom_note {
            return Some(SignatureMatch {
                id: 0x0100,
                name: make_name(b"Ransomware-Note-Detected"),
                offset: i as u64,
                severity: 4,
            });
        }
    }

    None
}

fn scan_shellcode_kits(data: &[u8]) -> Option<SignatureMatch> {
    // Metasploit patterns
    let msf_pattern = [
        0x64, 0x8B, 0x35, 0x30, 0x00, 0x00, 0x00, // mov esi, dword ptr fs:[0x30]
        0x8B, 0x76, 0x0C,                            // mov esi, dword ptr [esi+0Ch]
    ];

    for i in 0..data.len().saturating_sub(msf_pattern.len()) {
        if &data[i..i + msf_pattern.len()] == &msf_pattern {
            return Some(SignatureMatch {
                id: 0x0200,
                name: make_name(b"Metasploit-Shellcode"),
                offset: i as u64,
                severity: 4,
            });
        }
    }

    // Generic NOP+INT3+JMP pattern (shellcode stub)
    for i in 0..data.len().saturating_sub(8) {
        if data[i] == 0x90 &&                    // NOP
           data[i + 1] == 0x90 &&                // NOP
           data[i + 2] == 0xCC &&                // INT3
           data[i + 5] == 0xEB {                 // JMP short
            return Some(SignatureMatch {
                id: 0x0201,
                name: make_name(b"Shellcode-Stub-Pattern"),
                offset: i as u64,
                severity: 3,
            });
        }
    }

    None
}

fn scan_exploit_patterns(data: &[u8]) -> Option<SignatureMatch> {
    // ROP chain indicators (gadget patterns)
    let mut rop_count = 0u32;
    for window in data.windows(5) {
        // RET instruction (C3) followed by MOV/MOVZX/LEA
        if window[0] == 0xC3 &&
           (window[1] == 0x8B || window[1] == 0x0F ||
            window[1] == 0x8D || window[1] == 0xB6) {
            rop_count += 1;
        }
    }

    if rop_count > 10 {
        return Some(SignatureMatch {
            id: 0x0300,
            name: make_name(b"ROP-Chain-Detected"),
            offset: 0,
            severity: 3,
        });
    }

    // Heap spray pattern (NOP sled + shellcode)
    let mut nop_runs = 0u32;
    let mut run_len = 0u32;
    for &byte in data {
        if byte == 0x90 {
            run_len += 1;
        } else {
            if run_len > 64 { nop_runs += 1; }
            run_len = 0;
        }
    }

    if nop_runs > 5 {
        return Some(SignatureMatch {
            id: 0x0301,
            name: make_name(b"Heap-Spray-Pattern"),
            offset: 0,
            severity: 3,
        });
    }

    None
}
