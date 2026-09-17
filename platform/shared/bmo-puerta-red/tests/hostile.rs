//! **THE HOSTILE PROCESS** -- the mailbox is memory a Ring 3 process writes
//! while the kernel reads it. This is the one parser in the network path whose
//! attacker is not on the cable but on the same machine, and it runs in Ring 0.
//!
//! Each case is a SCRIPT: the bytes decide what the process scribbles into the
//! header and into the length of every slot, between kernel reads. Checked:
//! nothing panics, the kernel never copies more than a frame, and it never
//! writes outside the mailbox.

use bmo_hostile::{attack, DEFAULT_SEED};
use bmo_puerta_red::buzon::{self, campo, Lado, BYTES, CASILLAS, TRAMA_MAXIMA};

fn u32_at(s: &[u8], i: usize) -> u32 {
    let mut b = [0u8; 4];
    for (k, v) in b.iter_mut().enumerate() {
        *v = s.get(i + k).copied().unwrap_or(0);
    }
    u32::from_le_bytes(b)
}

#[test]
fn a_lying_process_never_breaks_the_kernel_side() {
    // A plausible script: small indices and frame-sized lengths, so mutations
    // land near the edges instead of always far away.
    let mut good = vec![0u8; 64];
    good[0] = 3; // tx written
    good[4] = 1; // rx read
    for i in 0..CASILLAS {
        good[8 + 2 * i] = 60;
    }
    attack("buzon", DEFAULT_SEED, 5_000, &[&good], 128, |script| {
        let mut m = vec![0u8; BYTES];
        let mut side = Lado::nuevo();
        side.preparar(&mut m[..]).unwrap();
        let mut out = [0u8; TRAMA_MAXIMA + 64];
        let frame = [0x5Au8; 200];
        for round in 0..4 {
            let shift = round * 3;
            buzon::escribir32(&mut m[..], campo::TX_ESCRITO, u32_at(script, shift));
            buzon::escribir32(&mut m[..], campo::RX_LEIDO, u32_at(script, shift + 4));
            for i in 0..CASILLAS {
                let len = u32_at(script, 8 + 2 * i + round) as u16;
                buzon::escribir16(&mut m[..], buzon::casilla_tx(i), len);
            }
            for _ in 0..CASILLAS + 2 {
                if let Ok(Some(n)) = side.sacar(&mut m[..], &mut out) {
                    assert!(n <= TRAMA_MAXIMA, "the kernel copied {} bytes out of one slot", n);
                }
                let _ = side.meter(&mut m[..], &frame);
            }
            side.publicar(&mut m[..], round as u64, 0, 0);
        }
        assert_eq!(m.len(), BYTES, "the mailbox did not grow");
    });
}
