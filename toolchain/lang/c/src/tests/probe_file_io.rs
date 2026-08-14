//! # THE FILE I/O PROBE -- where the bytes land
//!
//! ## Why this axis exists, and it is the one that killed DOOM on 2026-08-13
//!
//! DOOM got further than it ever had: it opened its WAD, printed
//! ` adding apps/doom1.wad`, and then said
//!
//! ```text
//! Wad file apps/doom1.wad doesn't have IWAD or PWAD id
//! ```
//!
//! The file was perfect and open. What failed was three lines in `w_wad.c`:
//!
//! ```c
//! wadinfo_t header;                              // :141 -- THE STACK
//! W_Read(wad_file, 0, &header, sizeof(header));  // -> fread(&header, 1, 12, f)
//! if (strncmp(header.identification, "IWAD", 4)) I_Error(...);
//! ```
//!
//! `fread` translated `dst` into an offset inside the block the kernel granted
//! --which is how `ARCH_OP_LEER_EN` avoids validating pointers at all-- and a
//! stack address is not in that block. It returned **zero without writing**,
//! `header` kept whatever was on the stack, and DOOM concluded its own WAD was
//! not a WAD.
//!
//! ## ** The lesson, and this file paid it TWICE
//!
//! The limitation was documented. `bmo/archivo.h` carried it with a `[!]` on
//! it: *"a pointer to the stack does NOT work, and it answers zero rather than
//! writing where it should not"*.
//!
//! And four paragraphs below that, in the same header, is the story of `fseek`
//! ignoring `SEEK_END` -- which also had a comment saying so, and whose written
//! moral was **"saying it was not enough"**. It was not enough again.
//!
//! A documented limit is still a limit. If the standard says `fread` writes
//! where you point it, then `fread` writes where you point it: the fix bounces
//! through a buffer that IS inside the heap, costs one `memcpy` of a few dozen
//! bytes, and leaves the fast path --reading into a `malloc`, which is how
//! lumps are loaded-- paying nothing.
//!
//! ## What the rows check
//!
//! Not "does a file open". **Where the bytes end up**, for each kind of
//! destination a real program uses: the heap, the stack, and a global. Plus the
//! cursor, because `ftell` is a mirror kept on this side and a mirror is a
//! thing that goes wrong on its own.

use super::census::{sweep_seeded, Cell};

/// The seeded file every cell reads: `IWAD` and then twelve bytes that
/// **announce their own offset**. Same trick as `probe_layout` -- a value that
/// comes back wrong says WHERE it was read from, so a failure is a location and
/// not just a mismatch.
fn seeded_wad() -> Vec<u8> {
    let mut v = b"IWAD".to_vec();
    for i in 4u8..16 {
        v.push(i);
    }
    v
}

fn census() -> Vec<Cell> {
    vec![
        Cell {
            // ** THE ONE THAT KILLED DOOM: a header read into a local.
            name: "fread into a STACK buffer",
            source: "#define BMO_MONTON_BYTES (256 * 1024)\n\
                     #include <stdlib.h>\n#include <stdio.h>\n#include <bmo/archivo.h>\n\
                     int main() { FILE *f; char h[16]; int i; \
                       for (i = 0; i < 16; i++) { h[i] = '#'; } \
                       f = fopen(\"prueba.bin\", \"r\"); \
                       if (f == 0) { printf(\"NOFILE\\n\"); return 0; } \
                       printf(\"%d %d %d %d %d\\n\", (int)fread(h, 1, 4, f), \
                         (int)h[0], (int)h[1], (int)h[2], (int)h[3]); return 0; }",
            expects: "4 73 87 65 68",
        },
        Cell {
            // The fast path: a `malloc` block, which is how lumps are loaded.
            name: "fread into a HEAP block",
            source: "#define BMO_MONTON_BYTES (256 * 1024)\n\
                     #include <stdlib.h>\n#include <stdio.h>\n#include <bmo/archivo.h>\n\
                     int main() { FILE *f; unsigned char *m; \
                       m = (unsigned char *)malloc(64); \
                       f = fopen(\"prueba.bin\", \"r\"); \
                       if (f == 0) { printf(\"NOFILE\\n\"); return 0; } \
                       printf(\"%d %d %d\\n\", (int)fread(m, 1, 8, f), \
                         (int)m[0], (int)m[7]); return 0; }",
            expects: "8 73 7",
        },
        Cell {
            // A global lives in `.bss`, which the kernel did not grant either.
            name: "fread into a GLOBAL buffer",
            source: "#define BMO_MONTON_BYTES (256 * 1024)\n\
                     #include <stdlib.h>\n#include <stdio.h>\n#include <bmo/archivo.h>\n\
                     unsigned char g[32];\n\
                     int main() { FILE *f; \
                       f = fopen(\"prueba.bin\", \"r\"); \
                       if (f == 0) { printf(\"NOFILE\\n\"); return 0; } \
                       printf(\"%d %d %d\\n\", (int)fread(g, 1, 6, f), \
                         (int)g[0], (int)g[5]); return 0; }",
            expects: "6 73 5",
        },
        Cell {
            // `fread` counts ELEMENTS, not bytes. `W_StdC_Read` calls it with
            // size 1, but a program reading records calls it with sizeof(rec).
            name: "fread returns ELEMENTS not bytes",
            source: "#define BMO_MONTON_BYTES (256 * 1024)\n\
                     #include <stdlib.h>\n#include <stdio.h>\n#include <bmo/archivo.h>\n\
                     int main() { FILE *f; char h[16]; \
                       f = fopen(\"prueba.bin\", \"r\"); \
                       if (f == 0) { printf(\"NOFILE\\n\"); return 0; } \
                       printf(\"%d\\n\", (int)fread(h, 4, 3, f)); return 0; }",
            expects: "3",
        },
        Cell {
            // ** `W_StdC_Read` seeks before EVERY read, and offset 0 is the one
            // the header uses. A `fseek` that did nothing would still look
            // right on the first read of a file and wrong on the second.
            name: "fseek(0) then read again",
            source: "#define BMO_MONTON_BYTES (256 * 1024)\n\
                     #include <stdlib.h>\n#include <stdio.h>\n#include <bmo/archivo.h>\n\
                     int main() { FILE *f; char a[8]; char b[8]; \
                       f = fopen(\"prueba.bin\", \"r\"); \
                       if (f == 0) { printf(\"NOFILE\\n\"); return 0; } \
                       fread(a, 1, 4, f); \
                       fseek(f, 0, 0); \
                       fread(b, 1, 4, f); \
                       printf(\"%d %d\\n\", (int)a[0], (int)b[0]); return 0; }",
            expects: "73 73",
        },
        Cell {
            // Seeking to a position that is not zero, which is every lump read.
            name: "fseek to the middle, then read",
            source: "#define BMO_MONTON_BYTES (256 * 1024)\n\
                     #include <stdlib.h>\n#include <stdio.h>\n#include <bmo/archivo.h>\n\
                     int main() { FILE *f; char h[8]; \
                       f = fopen(\"prueba.bin\", \"r\"); \
                       if (f == 0) { printf(\"NOFILE\\n\"); return 0; } \
                       fseek(f, 8, 0); \
                       fread(h, 1, 4, f); \
                       printf(\"%d %d\\n\", (int)h[0], (int)h[3]); return 0; }",
            expects: "8 11",
        },
        Cell {
            // `M_FileLength`: seek to the end and ask where you are. This is
            // the one that once made the WAD measure zero bytes.
            name: "SEEK_END then ftell is the size",
            source: "#define BMO_MONTON_BYTES (256 * 1024)\n\
                     #include <stdlib.h>\n#include <stdio.h>\n#include <bmo/archivo.h>\n\
                     int main() { FILE *f; \
                       f = fopen(\"prueba.bin\", \"r\"); \
                       if (f == 0) { printf(\"NOFILE\\n\"); return 0; } \
                       fseek(f, 0, 2); \
                       printf(\"%d\\n\", (int)ftell(f)); return 0; }",
            expects: "16",
        },
        Cell {
            // Reading past the end returns SHORT, and short is not an error.
            // `W_StdC_Read` returns the count and the caller checks it.
            name: "a read past the end is short",
            source: "#define BMO_MONTON_BYTES (256 * 1024)\n\
                     #include <stdlib.h>\n#include <stdio.h>\n#include <bmo/archivo.h>\n\
                     int main() { FILE *f; char h[64]; \
                       f = fopen(\"prueba.bin\", \"r\"); \
                       if (f == 0) { printf(\"NOFILE\\n\"); return 0; } \
                       fseek(f, 12, 0); \
                       printf(\"%d\\n\", (int)fread(h, 1, 40, f)); return 0; }",
            expects: "4",
        },
    ]
}

#[test]
fn the_file_io_census_has_not_changed() {
    sweep_seeded(
        &census(),
        "prueba.bin",
        seeded_wad(),
        CENSUS,
        "THE FILE I/O CENSUS CHANGED.\n\
         If the STACK or GLOBAL row went BROKEN, `fread` stopped bouncing and\n\
         every program that reads a header into a local is back to getting\n\
         garbage -- which is how DOOM concluded its own WAD was not a WAD.\n\
         The implementation is `sem-asm/tables/bmo/archivo.h`.",
    );
}

/// **THE FILE I/O CENSUS, as of 2026-08-13.**
///
/// Green throughout -- but the "before" was MEASURED, not assumed. With the
/// bounce switched off on purpose, **six of the eight go red**:
///
/// ```text
///   fread into a STACK buffer        BROKEN gives "0 35 35 35 35"
///   fread into a HEAP block          GOOD
///   fread into a GLOBAL buffer       BROKEN gives "0 0 0"
///   fread returns ELEMENTS not bytes BROKEN gives "0"
///   fseek(0) then read again         BROKEN gives "0 0"
///   fseek to the middle, then read   BROKEN gives "0 0"
///   SEEK_END then ftell is the size  GOOD
///   a read past the end is short     BROKEN gives "0"
/// ```
///
/// The `35` is the sentinel `'#'` the cell wrote first: `fread` returned zero
/// **and did not touch the buffer**. That is the DOOM symptom exactly. The two
/// that stayed green are the only two that never read into a non-heap
/// destination -- the fast path, and a seek with no read after it.
const CENSUS: &str = "\
fread into a STACK buffer      GOOD
fread into a HEAP block        GOOD
fread into a GLOBAL buffer     GOOD
fread returns ELEMENTS not bytes GOOD
fseek(0) then read again       GOOD
fseek to the middle, then read GOOD
SEEK_END then ftell is the size GOOD
a read past the end is short   GOOD
";
