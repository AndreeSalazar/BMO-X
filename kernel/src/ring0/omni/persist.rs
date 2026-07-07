//! Persist â€” flush cabina-daemon ring buffer to NVRAM/SSD.
//!
//! Uses nvram-log for post-mortem crash storage (survives reboot)
//! and cabina-daemon spool for real-time buffering.
//! exFAT driver (future) enables live writes to T: partition.

/// Max events to flush per NVRAM variable (256-byte limit).
const NVRAM_VAR_PREFIX: &str = "BMODiag";

/// Start the persist subsystem.
pub fn start() {
    cabina_daemon::info("omni/persist", "persist subsystem active");
}

/// Flush recent events to NVRAM (called on panic / crash).
pub fn flush_to_nvram() {
    let cur = cabina_daemon::ring_buffer::next_seq();
    if cur == 0 { return; }

    // Collect last 8 events into a single buffer
    let start = if cur > 8 { cur - 8 } else { 1 };
    let mut buf = [0u8; 2048];
    let mut pos = 0usize;

    for seq in start..cur {
        if let Some(ev) = cabina_daemon::ring_buffer::event_by_seq(seq) {
            let severity = ev.severity.name();
            for b in severity.bytes() { if pos < buf.len() { buf[pos] = b; pos += 1; } }
            if pos < buf.len() { buf[pos] = b' '; pos += 1; }
            for b in ev.module_str().bytes() { if pos < buf.len() { buf[pos] = b; pos += 1; } }
            if pos < buf.len() { buf[pos] = b':'; pos += 1; }
            if pos < buf.len() { buf[pos] = b' '; pos += 1; }
            for b in ev.msg_str().bytes() { if pos < buf.len() { buf[pos] = b; pos += 1; } }
            if pos < buf.len() { buf[pos] = b'\n'; pos += 1; }
        }
    }

    if pos == 0 { return; }

    // Write to NVRAM in chunks (max ~200 bytes each for safety)
    let chunk_max = 192usize;
    let mut offset = 0;
    let mut var_idx = 0u32;
    while offset < pos && var_idx < 8 {
        let end = core::cmp::min(offset + chunk_max, pos);
        let chunk = &buf[offset..end];

        // Build variable name "BMODiag0" through "BMODiag7"
        let name = build_var_name(var_idx);
        nvram_log::set_variable(&name, chunk);

        offset = end;
        var_idx += 1;
    }
}

/// Build NVRAM variable name without allocation.
fn build_var_name(idx: u32) -> alloc::string::String {
    let mut s = alloc::string::String::from(NVRAM_VAR_PREFIX);
    s.push_str(&alloc::format!("{}", idx));
    s
}
