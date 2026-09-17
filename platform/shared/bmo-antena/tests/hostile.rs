//! **HOSTILE ANTENNA** -- the phone is a second computer on the LAN, and every
//! line it sends is parsed by BMO-X. Garbage and mutations of good lines, as a
//! byte STREAM, through the same three machines the app uses: the line
//! splitter, the conversation and the page reader.
//!
//! Checked: nothing panics. See `bmo-hostile` for what is not checked.

use bmo_antena::lamina::{self, Lector};
use bmo_antena::{leer, Conversacion, Lineas, Pedido};
use bmo_hostile::{attack, DEFAULT_SEED};

const CASES: u32 = 20_000;

const GOOD_LINES: &[&[u8]] = &[
    b"HOLA ANTENA/1 movil de casa\n",
    b"LISTA 3\n",
    b"ENTRADA v12 Un video de prueba (libre).mp4\n",
    b"VIDEO 12345 mpeg1 1280x720\n",
    b"NO no existe\n",
    b"LAMINA 640 2000 5\r\n",
    b"CAJA 10 20 100 30 1e1e2e\n",
    b"TEXTO 10 20 2 e6edf7 hola mundo\n",
    b"IMAGEN 10 20 100 30 foto-1\n",
    b"ENLACE 10 20 100 30 e7\n",
    b"CAMPO 10 20 100 30 busca\n",
];

/// A whole good conversation in one buffer, so mutations can break the
/// ORDER and the framing, not only one line.
fn good_stream() -> Vec<u8> {
    GOOD_LINES.concat()
}

#[test]
fn a_single_line_never_panics() {
    let stream = good_stream();
    let mut samples: Vec<&[u8]> = GOOD_LINES.to_vec();
    samples.push(&stream);
    attack("leer / leer_cabecera", DEFAULT_SEED, CASES, &samples, 600, |b| {
        let _ = leer(b);
        if let Ok(cab) = lamina::leer_cabecera(b) {
            let _ = lamina::leer_elemento(b, &cab);
        }
        let cab = lamina::leer_cabecera(b"LAMINA 640 2000 5").unwrap();
        let _ = lamina::leer_elemento(b, &cab);
    });
}

#[test]
fn the_stream_machines_never_panic() {
    let stream = good_stream();
    attack("Lineas + Conversacion + Lector", DEFAULT_SEED ^ 1, CASES, &[&stream], 4096, |b| {
        let mut lines = Lineas::nueva();
        let mut talk = Conversacion::nueva();
        let _ = talk.pedir(&Pedido::Hola);
        let mut reader = Lector::nuevo();
        for &byte in b {
            let Some(Ok(line)) = lines.empujar(byte) else { continue };
            let line = line.to_vec();
            let _ = talk.oir(&line);
            // The conversation may be closed by now; ask again the way the app does.
            let _ = talk.pedir(&Pedido::Pagina(b"https://example.com/"));
            let _ = reader.empujar(&line);
        }
        let _ = talk.bytes(b.len());
    });
}

/// ** The guard: a sample that does not parse makes every mutation die at the
/// verb, and the tests above go green without reaching the fields.
#[test]
fn the_samples_are_good_lines() {
    let cab = lamina::leer_cabecera(b"LAMINA 640 2000 5").unwrap();
    for line in GOOD_LINES {
        let text = &line[..line.len() - 1];
        let ok = leer(text).is_ok() || lamina::leer_elemento(text, &cab).is_ok();
        assert!(ok, "not a good line: {}", String::from_utf8_lossy(line));
    }
}
