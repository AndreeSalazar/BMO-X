//! Heuristic analysis engine for ByteDefender
//!
//! Analyzes code patterns to detect unknown threats without signatures.

#![allow(dead_code)]

/// Analyze executable for suspicious patterns
/// Returns score 0-100 (0=clean, 100=definite threat)
pub fn analyze_heuristic(data: &[u8]) -> u32 {
    if data.len() < 16 { return 0; }

    let mut score: u32 = 0;

    // Check for shellcode patterns
    score += detect_shellcode_patterns(data);

    // Check for packing/obfuscation indicators
    score += detect_packing(data);

    // Check for suspicious API calls
    score += detect_suspicious_imports(data);

    // Check for code injection patterns
    score += detect_injection(data);

    // Check entropy (high entropy = packed/encrypted)
    score += analyze_entropy(data);

    score.min(100)
}

/// Detect known shellcode patterns
fn detect_shellcode_patterns(data: &[u8]) -> u32 {
    let mut score: u32 = 0;

    // Windows API hash patterns (common in shellcode)
    // PEB access pattern: fs:[0x30] or gs:[0x60]
    for window in data.windows(3) {
        if window == [0x64, 0xA1, 0x30] ||  // fs:[0x30]
           window == [0x65, 0x48, 0xA1] {    // gs:[0x60] (x64)
            score += 30;
        }
    }

    // NOP sled detection (10+ consecutive NOPs)
    let mut nop_count = 0u32;
    for &byte in data {
        if byte == 0x90 { // NOP
            nop_count += 1;
            if nop_count > 10 {
                score += 20;
                break;
            }
        } else {
            nop_count = 0;
        }
    }

    // INT 3 breakpoint pattern (debugging/anti-analysis)
    let mut int3_count = 0u32;
    for &byte in data {
        if byte == 0xCC { // INT3
            int3_count += 1;
        }
    }
    if int3_count > 5 {
        score += 10;
    }

    score.min(50)
}

/// Detect executable packing/obfuscation
fn detect_packing(data: &[u8]) -> u32 {
    let mut score: u32 = 0;

    // Check for UPX packer signature
    if data.len() > 4 {
        for window in data.windows(4) {
            if window == *b"UPX!" || window == *b"UPX0" {
                score += 40;
            }
        }
    }

    // Check for high concentration of null bytes in code section
    // (indicates encrypted/packed data)
    if data.len() > 256 {
        let mut null_count = 0u32;
        for &byte in &data[256..512.min(data.len())] {
            if byte == 0 { null_count += 1; }
        }
        let ratio = (null_count * 100) / 256;
        if ratio > 80 {
            score += 15;
        }
    }

    score.min(40)
}

/// Detect suspicious import patterns
fn detect_suspicious_imports(data: &[u8]) -> u32 {
    let mut score: u32 = 0;

    // Check for known suspicious function names in strings
    let suspicious_patterns: &[&[u8]] = &[
        b"VirtualAlloc",
        b"WriteProcessMemory",
        b"CreateRemoteThread",
        b"NtUnmapViewOfSection",
        b"IsDebuggerPresent",
        b"GetTickCount",
        b"QueryPerformanceCounter",
        b"rdtsc",
    ];

    for pattern in suspicious_patterns {
        for window in data.windows(pattern.len()) {
            if window == *pattern {
                score += 15;
                break;
            }
        }
    }

    score.min(40)
}

/// Detect code injection patterns
fn detect_injection(data: &[u8]) -> u32 {
    let mut score: u32 = 0;

    // Self-modifying code indicators
    // Check for VirtualProtect + Write pattern
    for window in data.windows(6) {
        // MOV EAX, VA (B8 xx xx xx xx) pattern
        if window[0] == 0xB8 && window[1..4] != [0, 0, 0] {
            // Check if next instruction is a CALL
            if window[5] == 0xE8 || window[5] == 0xFF {
                score += 10;
            }
        }
    }

    // Check for decode loops (common in unpackers)
    for window in data.windows(8) {
        // XOR [reg], reg pattern (30/31/32/33 followed by same register)
        if (window[0] & 0xF0) == 0x30 && (window[0] & 0x07) == (window[1] & 0x07) {
            score += 5;
        }
    }

    score.min(30)
}

/// Analyze byte entropy (high entropy = suspicious)
fn analyze_entropy(data: &[u8]) -> u32 {
    if data.len() < 256 { return 0; }

    // Calculate byte frequency
    let mut freq = [0u32; 256];
    for &byte in data {
        freq[byte as usize] += 1;
    }

    // Calculate Shannon entropy
    let len = data.len() as f32;
    let mut entropy: f32 = 0.0;
    for &f in &freq {
        if f > 0 {
            let p = f as f32 / len;
            entropy -= p * log2_approx(p);
        }
    }

    // Max entropy is 8.0 for random data
    // Normal code is 4.0-6.0
    // Packed/encrypted is 7.0-8.0
    if entropy > 7.5 {
        25
    } else if entropy > 7.0 {
        15
    } else if entropy > 6.5 {
        5
    } else {
        0
    }
}

fn log2_approx(x: f32) -> f32 {
    if x <= 0.0 { return 0.0; }
    // Simple log2 approximation
    let mut result = 0.0;
    let mut val = x;
    while val < 1.0 {
        val *= 2.0;
        result -= 1.0;
    }
    while val >= 2.0 {
        val /= 2.0;
        result += 1.0;
    }
    // Linear approximation for fractional part
    result + (val - 1.0)
}
