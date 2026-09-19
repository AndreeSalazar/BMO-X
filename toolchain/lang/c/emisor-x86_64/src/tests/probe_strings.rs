//! # THE STRING PROBE -- the eight bytes of a lump name
//!
//! ## The axis, and why it is worth its own file
//!
//! The other probes ask whether the COMPILER emits the right instructions.
//! This one asks something the compiler cannot get wrong on its own: whether
//! the LIBRARY behind `<string.h>` behaves the way C says it does. Those
//! functions are written in C in `sem-asm/tables/`, or synthesised in the
//! codegen, and either way a wrong byte there is indistinguishable from a wrong
//! byte anywhere else.
//!
//! ## Where the rows come from, counted rather than guessed
//!
//! Over DOOM's tree: `strlen` 64 times, `memset` 50, `strcmp` 16, `atoi` 14,
//! `strcasecmp` 9, `sscanf` 8, `strchr` 7, `strncasecmp` 7, `strrchr` 5,
//! `strncpy` 3, `snprintf` 1.
//!
//! ** But the count is not what decides the order. **This one line is**:
//!
//! ```c
//! if (!strncasecmp(lump_p->name, name, 8))     // w_wad.c:273
//! ```
//!
//! That is `W_CheckNumForName`, and **every lookup in the WAD goes through
//! it**. `R_Init` asks for textures, `P_Init` for sprites, `HU_Init` for the
//! font, `ST_Init` for the status bar -- all of them by name, all of them here.
//! If this comparison is wrong, DOOM does not find one single graphic, and the
//! symptom is `R_InitTextures: Missing patch`, which reads like a bad WAD.
//!
//! ## The two traps a lump name carries
//!
//! 1. **A lump name is eight bytes and is NOT null-terminated when it fills
//!    them.** `w_wad.c:222` does `strncpy(lump_p->name, filerover->name, 8)`
//!    and relies on the standard's ugliest corner: `strncpy` pads with zeros
//!    when the source is short and **writes no terminator at all** when it is
//!    not. An implementation that "helpfully" terminates truncates every
//!    eight-character name in the WAD -- and `TEXTURE1`, `PLAYPAL` and `COLORMAP`
//!    are exactly eight.
//! 2. **The comparison is case-insensitive and bounded.** Bounded because there
//!    is no terminator to stop at; case-insensitive because DOOM asks for
//!    `"map01"` in lower case and the WAD stores `MAP01`.
//!
//! ## ** Result of the first sweep
//!
//! Written in the census constant at the bottom. The rows that would hurt most
//! are the two above; the rest are the ordinary ones that hold up the first
//! two.

use super::census::{sweep, Cell};

fn census() -> Vec<Cell> {
    vec![
        // == The lump name, which is the whole reason this file exists =====
        Cell {
            // ** `w_wad.c:273`. Case-insensitive AND bounded at 8.
            name: "strncasecmp finds a lump",
            source: "#include <string.h>\n#include <strings.h>\n\
                     int main() { \
                       printf(\"%d %d %d\\n\", \
                         strncasecmp(\"TEXTURE1\", \"texture1\", 8), \
                         strncasecmp(\"PLAYPAL\", \"playpal\", 8), \
                         (int)(strncasecmp(\"TEXTURE1\", \"TEXTURE2\", 8) != 0)); return 0; }",
            expects: "0 0 1",
        },
        Cell {
            // The bound is what makes an 8-byte name comparable at all: past
            // byte 8 there is another lump's name, not a terminator.
            name: "strncasecmp stops at n",
            source: "#include <strings.h>\n\
                     int main() { \
                       printf(\"%d %d\\n\", \
                         strncasecmp(\"TEXTURE1XXXX\", \"texture1YYYY\", 8), \
                         (int)(strncasecmp(\"TEXTURE1XXXX\", \"texture1YYYY\", 9) != 0)); \
                       return 0; }",
            expects: "0 1",
        },
        Cell {
            // ** The corner that decides whether DOOM sees its textures:
            // `strncpy` writes NO terminator when the source fills n. If the
            // implementation adds one, every 8-character lump name loses its
            // last letter -- and TEXTURE1, PLAYPAL and COLORMAP are 8 or more.
            name: "strncpy(8) writes no terminator",
            source: "#include <string.h>\n\
                     int main() { char b[10]; int i; \
                       for (i = 0; i < 10; i++) { b[i] = '#'; } \
                       strncpy(b, \"TEXTURE1\", 8); \
                       printf(\"%d %d %d\\n\", (int)b[7], (int)b[8], (int)b[9]); return 0; }",
            expects: "49 35 35",
        },
        Cell {
            // And the other half of the same rule: a SHORT source is padded
            // with zeros all the way to n, not just terminated once.
            name: "strncpy pads short with zeros",
            source: "#include <string.h>\n\
                     int main() { char b[9]; int i; \
                       for (i = 0; i < 9; i++) { b[i] = '#'; } \
                       strncpy(b, \"MAP01\", 8); \
                       printf(\"%d %d %d %s\\n\", (int)b[5], (int)b[7], (int)b[8], b); \
                       return 0; }",
            expects: "0 0 35 MAP01",
        },
        // == The ordinary ones the two above stand on =======================
        Cell {
            name: "strlen counts, no terminator",
            source: "#include <string.h>\n\
                     int main() { printf(\"%d %d\\n\", (int)strlen(\"TEXTURE1\"), \
                       (int)strlen(\"\")); return 0; }",
            expects: "8 0",
        },
        Cell {
            name: "strcmp orders, not just differs",
            source: "#include <string.h>\n\
                     int main() { \
                       printf(\"%d %d %d\\n\", (int)(strcmp(\"a\",\"a\") == 0), \
                         (int)(strcmp(\"a\",\"b\") < 0), (int)(strcmp(\"b\",\"a\") > 0)); \
                       return 0; }",
            expects: "1 1 1",
        },
        Cell {
            name: "strcasecmp ignores case",
            source: "#include <strings.h>\n\
                     int main() { \
                       printf(\"%d %d\\n\", strcasecmp(\"-IWAD\", \"-iwad\"), \
                         (int)(strcasecmp(\"-iwad\", \"-config\") != 0)); return 0; }",
            expects: "0 1",
        },
        Cell {
            // `d_iwad.c` and `m_misc.c` split paths with these two.
            name: "strchr and strrchr differ",
            source: "#include <string.h>\n\
                     int main() { char *s; char *a; char *b; s = \"apps/doom1.wad\"; \
                       a = strchr(s, '/'); b = strrchr(s, '.'); \
                       printf(\"%s %s\\n\", a, b); return 0; }",
            expects: "/doom1.wad .wad",
        },
        Cell {
            name: "strchr misses and returns NULL",
            source: "#include <string.h>\n\
                     int main() { printf(\"%d\\n\", (int)(strchr(\"abc\", 'z') == 0)); \
                       return 0; }",
            expects: "1",
        },
        Cell {
            // `W_CheckNumForName` on a name that is not there has to say so,
            // and the caller checks `< 0`. Getting the miss wrong is worse than
            // getting the hit wrong: it looks like a corrupt WAD.
            name: "strstr finds and misses",
            source: "#include <string.h>\n\
                     int main() { \
                       printf(\"%d %d\\n\", (int)(strstr(\"FREEDOOM\", \"DOOM\") != 0), \
                         (int)(strstr(\"TEXTURE1\", \"DOOM\") == 0)); return 0; }",
            expects: "1 1",
        },
        Cell {
            // 50 calls in DOOM, and `Z_Malloc` uses it on every block it hands
            // out. `rep stosb` now, so this is also the check on that change.
            name: "memset fills exactly n",
            source: "#include <string.h>\n\
                     int main() { char b[8]; int i; \
                       for (i = 0; i < 8; i++) { b[i] = '#'; } \
                       memset(b, 0, 4); \
                       printf(\"%d %d %d\\n\", (int)b[0], (int)b[3], (int)b[4]); return 0; }",
            expects: "0 0 35",
        },
        Cell {
            // The other half of `rep movsb`: n exact, nothing past it.
            name: "memcpy moves exactly n",
            source: "#include <string.h>\n\
                     int main() { char a[8]; char b[8]; int i; \
                       for (i = 0; i < 8; i++) { a[i] = '#'; b[i] = 'X'; } \
                       memcpy(a, b, 3); \
                       printf(\"%d %d\\n\", (int)a[2], (int)a[3]); return 0; }",
            expects: "88 35",
        },
        Cell {
            // ** A big one, because `DG_DrawFrame` moves 1,024,000 bytes with
            // it. The byte loop was replaced by `rep movsb`; a size that is not
            // a multiple of anything is what catches an off-by-one there.
            name: "memcpy of 1000 bytes exactly",
            source: "#include <string.h>\n\
                     unsigned char src[1024];\n\
                     unsigned char dst[1024];\n\
                     int main() { int i; int bad; \
                       for (i = 0; i < 1024; i++) { src[i] = (unsigned char)(i & 0xFF); \
                         dst[i] = 0; } \
                       memcpy(dst, src, 1000); \
                       bad = 0; \
                       for (i = 0; i < 1000; i++) { if (dst[i] != src[i]) { bad = bad + 1; } } \
                       printf(\"%d %d %d\\n\", bad, (int)dst[999], (int)dst[1000]); return 0; }",
            expects: "0 231 0",
        },
        Cell {
            // `m_argv.c` and `m_config.c` turn text into numbers, and a
            // negative is what a config file carries for an unset control.
            name: "atoi, with sign and junk",
            source: "#include <stdlib.h>\n\
                     int main() { printf(\"%d %d %d\\n\", atoi(\"3\"), atoi(\"-12\"), \
                       atoi(\"0\")); return 0; }",
            expects: "3 -12 0",
        },
        Cell {
            // `M_StringJoin` builds `default.cfg`'s path this way, and this is
            // the exact call that killed DOOM before `va_arg` was fixed.
            name: "strdup copies and stands alone",
            source: "#include <string.h>\n#include <stdlib.h>\n\
                     int main() { char *a; char b[8]; \
                       b[0]='c'; b[1]='f'; b[2]='g'; b[3]=0; \
                       a = strdup(b); b[0]='X'; \
                       printf(\"%s %s\\n\", a, b); return 0; }",
            expects: "cfg Xfg",
        },
        Cell {
            // `snprintf` is what a bounded build of a lump name uses, and the
            // return value is the length it WANTED, not what it wrote.
            name: "snprintf truncates and counts",
            source: "#include <stdio.h>\n\
                     int main() { char b[6]; int n; \
                       n = snprintf(b, 6, \"%s%d\", \"MAP\", 1); \
                       printf(\"%s %d\\n\", b, n); return 0; }",
            expects: "MAP1 4",
        },
    ]
}

#[test]
fn the_string_census_has_not_changed() {
    sweep(
        &census(),
        CENSUS,
        "THE STRING CENSUS CHANGED.\n\
         If `strncasecmp` or `strncpy(8)` went BROKEN, stop everything else:\n\
         those two are every lump lookup in the WAD, and the symptom on metal\n\
         is `R_InitTextures: Missing patch`, which reads like a corrupt file.\n\
         The library lives in `sem-asm/tables/string.h` and in the codegen's\n\
         synthesised functions.",
    );
}

/// **THE STRING CENSUS, as of 2026-08-13.**
///
/// ** Green throughout, but only after the first sweep found a hole that was
/// not a bug: **three cells did not COMPILE**, and all three were the ones
/// reaching for `<strings.h>` -- which is where POSIX keeps `strcasecmp` and
/// `strncasecmp`, and which BMO simply did not have. Both functions existed,
/// in `<string.h>`. The surface had a hole with a name on it.
///
/// Fixed by adding `sem-asm/tables/strings.h`, four lines that forward to
/// `<string.h>` and let its guard stop the double definition.
///
/// [!] DOOM does not hit that hole --its own `m_misc.h` declares the two, and
/// the out-of-tree probe directory carries a `strings.h` stub-- so nobody
/// should "verify" the fix with DOOM and conclude nothing happened. The
/// programs it unblocks are the next ones.
const CENSUS: &str = "\
strncasecmp finds a lump       GOOD
strncasecmp stops at n         GOOD
strncpy(8) writes no terminator GOOD
strncpy pads short with zeros  GOOD
strlen counts, no terminator   GOOD
strcmp orders, not just differs GOOD
strcasecmp ignores case        GOOD
strchr and strrchr differ      GOOD
strchr misses and returns NULL GOOD
strstr finds and misses        GOOD
memset fills exactly n         GOOD
memcpy moves exactly n         GOOD
memcpy of 1000 bytes exactly   GOOD
atoi, with sign and junk       GOOD
strdup copies and stands alone GOOD
snprintf truncates and counts  GOOD
";
