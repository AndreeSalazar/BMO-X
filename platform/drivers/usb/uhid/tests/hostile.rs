//! **HOSTILE DESCRIPTORS** -- what a USB device says about itself, read by the
//! kernel while enumerating, before anything decided the device can be trusted.
//!
//! Checked: nothing panics, and no walker claims more interfaces than its
//! output holds. The hand-written cases next to the parser
//! (`formato.rs::hostiles`) check that the answers are right.

use bmo_hostile::{attack, DEFAULT_SEED};
use bmo_uhid::{enumera, formato};

/// A boot mouse, byte for byte as USB HID 1.11 Appendix B.2 prints it, with a
/// wheel added -- the shape most real mice answer with.
const MOUSE_REPORT: &[u8] = &[
    0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x09, 0x01, 0xA1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29, 0x03,
    0x15, 0x00, 0x25, 0x01, 0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01, 0x75, 0x05, 0x81, 0x01,
    0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x38, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x03,
    0x81, 0x06, 0xC0, 0xC0,
];

/// Configuration + interface (HID boot mouse) + HID descriptor + interrupt IN.
const CONFIG: &[u8] = &[
    0x09, 0x02, 0x22, 0x00, 0x01, 0x01, 0x00, 0xA0, 0x32, //
    0x09, 0x04, 0x00, 0x00, 0x01, 0x03, 0x01, 0x02, 0x00, //
    0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x34, 0x00, //
    0x07, 0x05, 0x81, 0x03, 0x04, 0x00, 0x0A,
];

#[test]
fn the_samples_are_good() {
    let mut out = [(0u8, 0u8, 0u8, 0u8); enumera::MAX_IFACES];
    assert_eq!(enumera::interfaces(CONFIG, &mut out), 1, "the config sample must show its interface");
    assert!(enumera::intr_in(CONFIG, 0).is_some(), "and its interrupt endpoint");
    assert!(formato::raton(MOUSE_REPORT).is_some(), "the report sample must be a mouse");
}

#[test]
fn hostile_configurations_never_panic() {
    attack("uhid enumera", DEFAULT_SEED, 40_000, &[CONFIG], 256, |x| {
        let mut out = [(0u8, 0u8, 0u8, 0u8); enumera::MAX_IFACES];
        assert!(enumera::interfaces(x, &mut out) <= enumera::MAX_IFACES);
        for iface in [0u8, 1, 0xFF] {
            let _ = enumera::intr_in(x, iface);
            let _ = enumera::hid_report_len(x, iface);
        }
        for off in [0usize, 2, 31, 33, usize::MAX - 1] {
            if off.checked_add(2).map_or(false, |end| end <= x.len()) {
                let _ = enumera::le_u16(x, off);
            }
        }
    });
}

#[test]
fn hostile_report_descriptors_never_panic() {
    attack("uhid formato", DEFAULT_SEED ^ 1, 40_000, &[MOUSE_REPORT], 512, |x| {
        if let Some(f) = formato::raton(x) {
            // And the formats it produced must read any report without panicking.
            for report in [&[][..], &[0xFF; 3][..], &[0x80; 8][..], &[0x55; 64][..]] {
                for c in [f.botones, f.x, f.y, f.rueda].into_iter().flatten() {
                    let _ = c.leer_crudo(report);
                    let _ = c.leer_con_signo(report);
                }
                let _ = f.desplazamiento();
            }
        }
    });
}

/// *** THE BUG OF 2026-09-17 (1 of 2), named: a device whose configuration says
/// `wTotalLength = 3` was sliced to three bytes and read at `[3]` -- a panic in
/// Ring 0. Every walker must answer "nothing" instead.
#[test]
fn a_config_shorter_than_its_length_field_is_empty_not_a_panic() {
    let mut out = [(0u8, 0u8, 0u8, 0u8); enumera::MAX_IFACES];
    for cut in 0..4 {
        let short = &CONFIG[..cut];
        assert_eq!(enumera::interfaces(short, &mut out), 0);
        assert_eq!(enumera::intr_in(short, 0), None);
        assert_eq!(enumera::hid_report_len(short, 0), None);
    }
}

/// *** THE BUG OF 2026-09-17 (2 of 2), named: a field wider than 32 bits made
/// the shift wrap in the release kernel. Reading it now gives its low 32 bits,
/// the same answer on every build.
#[test]
fn a_field_wider_than_32_bits_reads_its_low_32() {
    let wide = formato::Campo { bit: 0, bits: 200 };
    let report = [0xFFu8; 64];
    assert_eq!(wide.leer_crudo(&report), u32::MAX);
    let far = formato::Campo { bit: u16::MAX - 2, bits: 8 };
    assert_eq!(far.leer_crudo(&report), 0, "a bit position past the report reads nothing");
}
