//! Snapshot diff module
//!
//! Compares two kernel snapshots and shows what changed.

#![allow(dead_code)]

use super::Snapshot;

/// Diff result between two snapshots
#[derive(Clone, Copy)]
pub struct SnapshotDiff {
    pub memory_changed: bool,
    pub processes_changed: bool,
    pub interrupts_changed: bool,
    pub network_changed: bool,
    pub lapic_changed: bool,
    pub threats_detected: bool,
    pub details: [DiffDetail; 32],
    pub detail_count: usize,
}

#[derive(Clone, Copy)]
pub struct DiffDetail {
    pub category: [u8; 16],
    pub description: [u8; 64],
    pub old_value: u64,
    pub new_value: u64,
}

/// Compare two snapshots
pub fn diff_snapshots(a: &Snapshot, b: &Snapshot) -> SnapshotDiff {
    let mut diff = SnapshotDiff {
        memory_changed: false,
        processes_changed: false,
        interrupts_changed: false,
        network_changed: false,
        lapic_changed: false,
        threats_detected: false,
        details: [DiffDetail {
            category: [0; 16],
            description: [0; 64],
            old_value: 0,
            new_value: 0,
        }; 32],
        detail_count: 0,
    };

    // Memory diff
    if a.state.free_pages != b.state.free_pages {
        diff.memory_changed = true;
        add_detail(&mut diff, b"Memory", b"Free pages changed", a.state.free_pages, b.state.free_pages);
    }

    if a.state.used_pages != b.state.used_pages {
        diff.memory_changed = true;
        add_detail(&mut diff, b"Memory", b"Used pages changed", a.state.used_pages, b.state.used_pages);
    }

    // Process diff
    for i in 0..16 {
        if a.state.processes[i].active != b.state.processes[i].active ||
           a.state.processes[i].cr3 != b.state.processes[i].cr3 {
            diff.processes_changed = true;
            break;
        }
    }

    // LAPIC diff
    if a.state.lapic_timer_div != b.state.lapic_timer_div ||
       a.state.lapic_timer_init != b.state.lapic_timer_init {
        diff.lapic_changed = true;
    }

    // Network diff
    if a.state.ip_address != b.state.ip_address {
        diff.network_changed = true;
        add_detail(&mut diff, b"Network", b"IP changed", a.state.ip_address as u64, b.state.ip_address as u64);
    }

    // Threats
    if b.state.threat_count > a.state.threat_count {
        diff.threats_detected = true;
        add_detail(&mut diff, b"Security", b"New threats", a.state.threat_count, b.state.threat_count);
    }

    diff
}

/// Generate human-readable diff report
pub fn generate_report(a: &Snapshot, b: &Snapshot, diff: &SnapshotDiff) -> [u8; 2048] {
    let mut report = [0u8; 2048];
    let mut pos = 0;

    // Header
    let header = b"=== Restaurer Diff Report ===\n";
    report[pos..pos + header.len()].copy_from_slice(header);
    pos += header.len();

    // Snapshot A
    pos = write_label(&mut report, pos, b"From: #", a.id);
    pos = write_label(&mut report, pos, b"\nTo:   #", b.id);

    // Summary
    if diff.memory_changed {
        let msg = b"\n[MEMORY] Page usage changed";
        report[pos..pos + msg.len()].copy_from_slice(msg);
        pos += msg.len();
    }

    if diff.processes_changed {
        let msg = b"\n[PROCESS] Process state modified";
        report[pos..pos + msg.len()].copy_from_slice(msg);
        pos += msg.len();
    }

    if diff.threats_detected {
        let msg = b"\n[SECURITY] New threats detected!";
        report[pos..pos + msg.len()].copy_from_slice(msg);
        pos += msg.len();
    }

    // Details
    for i in 0..diff.detail_count {
        let d = &diff.details[i];
        if pos + d.category.len() + d.description.len() + 2 < 2048 {
            report[pos] = b'\n';
            pos += 1;

            let cat_len = d.category.iter().position(|&c| c == 0).unwrap_or(d.category.len());
            report[pos..pos + cat_len].copy_from_slice(&d.category[..cat_len]);
            pos += cat_len;

            report[pos] = b':';
            pos += 1;

            let desc_len = d.description.iter().position(|&c| c == 0).unwrap_or(d.description.len());
            report[pos..pos + desc_len].copy_from_slice(&d.description[..desc_len]);
            pos += desc_len;
        }
    }

    report[pos] = b'\n';

    report
}

fn add_detail(diff: &mut SnapshotDiff, category: &[u8], desc: &[u8], old: u64, new: u64) {
    if diff.detail_count >= 32 { return; }

    let d = &mut diff.details[diff.detail_count];
    let cat_len = category.len().min(15);
    d.category[..cat_len].copy_from_slice(&category[..cat_len]);

    let desc_len = desc.len().min(63);
    d.description[..desc_len].copy_from_slice(&desc[..desc_len]);

    d.old_value = old;
    d.new_value = new;

    diff.detail_count += 1;
}

fn write_label(buf: &mut [u8], mut pos: usize, prefix: &[u8], id: u64) -> usize {
    if pos + prefix.len() >= buf.len() { return pos; }

    buf[pos..pos + prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();

    // Write ID as decimal
    let mut n = id;
    let mut digits = [0u8; 20];
    let mut len = 0;
    if n == 0 {
        digits[0] = b'0';
        len = 1;
    } else {
        while n > 0 && len < 20 {
            digits[len] = b'0' + (n % 10) as u8;
            n /= 10;
            len += 1;
        }
    }

    // Reverse digits
    for i in 0..len / 2 {
        digits.swap(i, len - 1 - i);
    }

    if pos + len <= buf.len() {
        buf[pos..pos + len].copy_from_slice(&digits[..len]);
        pos + len
    } else {
        pos
    }
}
