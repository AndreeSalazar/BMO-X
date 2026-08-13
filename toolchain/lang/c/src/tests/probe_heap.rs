//! # THE HEAP PROBE -- the one block everything else is carved out of
//!
//! ## The axis
//!
//! DOOM does not use `malloc` for its own data. It calls it **once**, in
//! `I_ZoneBase`, asks for six megabytes, and then hands that block to its own
//! allocator: 94 `Z_Malloc` calls and 37 frees all live inside it, tagged
//! `PU_STATIC`, `PU_LEVEL` or `PU_CACHE`.
//!
//! That shape is what makes this a distinct axis rather than "does malloc
//! work". Everything the game does depends on **one** allocation being right,
//! and on the ordinary properties holding underneath it:
//!
//! - the block is really as big as it says, all the way to the last byte;
//! - two allocations never overlap;
//! - a pointer comes back aligned enough for whatever gets stored in it;
//! - running out says NO instead of handing back something unusable.
//!
//! ## Where the numbers come from
//!
//! `<bmo/monton.h>` is BMO's Ring 3 allocator, sitting on a single
//! `KIND_MEMORY` block. Its default is 1 MiB and the program raises it with
//! `#define BMO_MONTON_BYTES` **before the first include** -- DOOM asks for 12
//! MiB, which is the 6 MiB zone plus the 1 MiB frame buffer plus the WAD
//! directory, with room left over.
//!
//! [!] The cells here allocate in the hundreds of kilobytes, not megabytes: the
//! probe runs in the emulator, and what is being checked is the arithmetic of
//! the allocator, not how much RAM a Ryzen has. The 12 MiB request is a
//! contract with the kernel and lives in `probe_syscalls` territory, not here.
//!
//! ## ** The failure mode this axis is built to catch
//!
//! An allocator that returns overlapping blocks does not crash: it produces a
//! program where writing one thing quietly changes another. In DOOM that would
//! surface as a texture with someone else's pixels in it, or a sector height
//! that changes when a sprite moves -- symptoms nobody would trace back to
//! `malloc`. So the rows check **disjointness and extent**, not just that a
//! pointer came back non-null.

use super::census::{sweep, Cell};

/// Every cell needs the heap sized up front, and `<stdlib.h>` has to come
/// AFTER the `#define` -- that ordering is a real trap and DOOM hit it: the
/// first include wins, and by the time the second one runs the heap is already
/// built at the default size.
const HEAP: &str = "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n";

fn cell(name: &'static str, body: &'static str, expects: &'static str) -> Cell {
    Cell { name, source: body, expects }
}

fn census() -> Vec<Cell> {
    vec![
        cell(
            "malloc gives back memory",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *p; p = (char *)malloc(64); \
                   if (p == 0) { printf(\"NULL\\n\"); return 0; } \
                   p[0] = 7; p[63] = 9; \
                   printf(\"%d %d\\n\", (int)p[0], (int)p[63]); return 0; }"
            ),
            "7 9",
        ),
        cell(
            // ** The `I_ZoneBase` shape: ONE big block, and every byte of it
            // has to be writable. A block that is short only at the end is the
            // worst case -- the zone allocator fills from the front, so the
            // corruption shows up hours later.
            "one big block, first and last byte",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *p; int n; n = 300 * 1024; \
                   p = (char *)malloc(n); \
                   if (p == 0) { printf(\"NULL\\n\"); return 0; } \
                   p[0] = 1; p[n - 1] = 2; \
                   printf(\"%d %d\\n\", (int)p[0], (int)p[n - 1]); return 0; }"
            ),
            "1 2",
        ),
        cell(
            // ** Disjointness, checked by WRITING rather than by comparing
            // pointers. Two blocks whose addresses differ can still overlap if
            // the size arithmetic is wrong.
            "two blocks do not overlap",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *a; char *b; int i; int bad; \
                   a = (char *)malloc(100); b = (char *)malloc(100); \
                   for (i = 0; i < 100; i++) { a[i] = 'A'; } \
                   for (i = 0; i < 100; i++) { b[i] = 'B'; } \
                   bad = 0; \
                   for (i = 0; i < 100; i++) { if (a[i] != 'A') { bad = bad + 1; } } \
                   printf(\"%d %d %d\\n\", bad, (int)a[0], (int)b[0]); return 0; }"
            ),
            "0 65 66",
        ),
        cell(
            // Twenty of them, which is closer to what the boot sequence does:
            // `I_AtExit` alone is seven.
            "twenty blocks stay disjoint",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *p[20]; int i; int j; int bad; \
                   for (i = 0; i < 20; i++) { p[i] = (char *)malloc(50); \
                     for (j = 0; j < 50; j++) { p[i][j] = (char)i; } } \
                   bad = 0; \
                   for (i = 0; i < 20; i++) { for (j = 0; j < 50; j++) { \
                     if (p[i][j] != (char)i) { bad = bad + 1; } } } \
                   printf(\"%d\\n\", bad); return 0; }"
            ),
            "0",
        ),
        cell(
            // ** Alignment. The zone stores `memblock_t` headers with pointers
            // in them; a misaligned pointer does not fault on x86, it just
            // makes every access slower and every atomic wrong. The allocator
            // documents 16.
            "pointers come back aligned to 16",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *a; char *b; char *c; \
                   a = (char *)malloc(1); b = (char *)malloc(7); c = (char *)malloc(33); \
                   printf(\"%d %d %d\\n\", \
                     (int)(((unsigned long long)a) & 15), \
                     (int)(((unsigned long long)b) & 15), \
                     (int)(((unsigned long long)c) & 15)); return 0; }"
            ),
            "0 0 0",
        ),
        cell(
            // ** Running out has to say NO. `I_ZoneBase` checks the result and
            // calls `I_Error`; an allocator that hands back a pointer past the
            // end instead turns an honest "out of memory" into a page fault
            // somewhere else entirely.
            "out of memory returns NULL",
            concat!(
                "#define BMO_MONTON_BYTES (64 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *a; char *b; \
                   a = (char *)malloc(60 * 1024); \
                   b = (char *)malloc(60 * 1024); \
                   printf(\"%d %d\\n\", (int)(a != 0), (int)(b == 0)); return 0; }"
            ),
            "1 1",
        ),
        cell(
            "free then malloc reuses the room",
            concat!(
                "#define BMO_MONTON_BYTES (64 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *a; char *b; \
                   a = (char *)malloc(40 * 1024); \
                   free(a); \
                   b = (char *)malloc(40 * 1024); \
                   printf(\"%d\\n\", (int)(b != 0)); return 0; }"
            ),
            "1",
        ),
        cell(
            // `free(NULL)` is a no-op by the standard, and DOOM leans on it:
            // `Z_Free` is called on pointers that may never have been set.
            "free(NULL) is a no-op",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *p; p = 0; free(p); \
                   p = (char *)malloc(16); p[0] = 5; \
                   printf(\"%d\\n\", (int)p[0]); return 0; }"
            ),
            "5",
        ),
        cell(
            // `realloc` has to carry the old contents across. `M_StringJoin`
            // and the lump directory both grow this way.
            "realloc keeps the contents",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *p; int i; int bad; \
                   p = (char *)malloc(32); \
                   for (i = 0; i < 32; i++) { p[i] = (char)(i + 1); } \
                   p = (char *)realloc(p, 256); \
                   bad = 0; \
                   for (i = 0; i < 32; i++) { if (p[i] != (char)(i + 1)) { bad = bad + 1; } } \
                   p[255] = 99; \
                   printf(\"%d %d %d\\n\", bad, (int)p[0], (int)p[255]); return 0; }"
            ),
            "0 1 99",
        ),
        cell(
            // `calloc` zeroes, and the zone relies on it for the block headers.
            // With `rep stosb` behind `memset` this also exercises that path.
            "calloc zeroes what it hands out",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { unsigned char *p; int i; int bad; \
                   p = (unsigned char *)calloc(200, 1); \
                   bad = 0; \
                   for (i = 0; i < 200; i++) { if (p[i] != 0) { bad = bad + 1; } } \
                   printf(\"%d\\n\", bad); return 0; }"
            ),
            "0",
        ),
        cell(
            // Interleaved alloc and free, which is what `PU_CACHE` does every
            // frame: lumps get thrown away and re-read constantly.
            "alloc and free interleaved",
            concat!(
                "#define BMO_MONTON_BYTES (128 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *p; int i; int failures; failures = 0; \
                   for (i = 0; i < 200; i++) { \
                     p = (char *)malloc(1024); \
                     if (p == 0) { failures = failures + 1; } else { p[0] = 1; p[1023] = 2; free(p); } } \
                   printf(\"%d\\n\", failures); return 0; }"
            ),
            "0",
        ),
        cell(
            // The block still has to be usable after a neighbour is freed --
            // i.e. coalescing must not swallow a live block.
            "freeing a neighbour spares the live one",
            concat!(
                "#define BMO_MONTON_BYTES (512 * 1024)\n#include <stdlib.h>\n",
                "int main() { char *a; char *b; char *c; int i; int bad; \
                   a = (char *)malloc(1000); b = (char *)malloc(1000); c = (char *)malloc(1000); \
                   for (i = 0; i < 1000; i++) { b[i] = 'B'; } \
                   free(a); free(c); \
                   bad = 0; \
                   for (i = 0; i < 1000; i++) { if (b[i] != 'B') { bad = bad + 1; } } \
                   printf(\"%d\\n\", bad); return 0; }"
            ),
            "0",
        ),
    ]
}

#[test]
fn the_heap_census_has_not_changed() {
    let _ = HEAP; // documents the ordering trap; each cell carries its own copy
    sweep(
        &census(),
        CENSUS,
        "THE HEAP CENSUS CHANGED.\n\
         If the disjointness rows went BROKEN, stop: an allocator that hands\n\
         out overlapping blocks does not crash, it makes writing one thing\n\
         quietly change another. On DOOM that reads as a texture with someone\n\
         else's pixels in it. The allocator is `sem-asm/tables/bmo/monton.h`.",
    );
}

/// **THE HEAP CENSUS, as of 2026-08-13.** Green throughout from the first
/// sweep -- including the two that matter most, disjointness and extent.
///
/// So when `R_Init` runs out of room on metal, the allocator underneath is not
/// the suspect: the zone's own arithmetic is, or the 12 MiB block the kernel
/// hands over, which is a different contract and a different probe.
const CENSUS: &str = "\
malloc gives back memory       GOOD
one big block, first and last byte GOOD
two blocks do not overlap      GOOD
twenty blocks stay disjoint    GOOD
pointers come back aligned to 16 GOOD
out of memory returns NULL     GOOD
free then malloc reuses the room GOOD
free(NULL) is a no-op          GOOD
realloc keeps the contents     GOOD
calloc zeroes what it hands out GOOD
alloc and free interleaved     GOOD
freeing a neighbour spares the live one GOOD
";
