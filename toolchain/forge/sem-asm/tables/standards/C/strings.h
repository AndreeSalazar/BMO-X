/* strings.h -- where POSIX keeps the case-insensitive comparisons.
 *
 * == Why this file exists at all, when it declares nothing new ==
 *
 * `strcasecmp` and `strncasecmp` are NOT in C11. They are POSIX, and POSIX puts
 * them in `<strings.h>` -- plural. BMO implements both, but it had them only in
 * `<string.h>`, so a program written anywhere else in the world did this
 *
 *     #include <strings.h>
 *     if (!strncasecmp(lump->name, name, 8)) ...
 *
 * and got "file not found" on the include line. Not a missing feature: a
 * missing NAME. The function was already there.
 *
 * That is the cheapest kind of porting blocker there is, and the most annoying
 * to diagnose, because the error points at the `#include` while the thing it
 * asks for is sitting in the next header along.
 *
 * == Found by the string probe, and that is the point of the probes ==
 *
 * Three cells of `probe_strings` came back `DOES NOT COMPILE` on their first
 * sweep -- the three that reached for `<strings.h>` the way a C programmer
 * would. The other thirteen passed. Nothing on BMO's side was broken; the
 * surface just had a hole with a name on it.
 *
 * ** DOOM does not hit this, and that is worth writing down so nobody
 * "verifies" the fix with DOOM and concludes nothing happened: its own
 * `m_misc.h` declares the two, and the out-of-tree probe directory carries a
 * `strings.h` stub of its own. The programs this unblocks are the NEXT ones.
 *
 * == Why it forwards instead of declaring ==
 *
 * The headers in this table carry real bodies, not prototypes -- `string.h`
 * defines `strcasecmp` at line 91. Declaring them again here would give two
 * definitions to any program that includes both, which is most of them. So
 * this includes `<string.h>` and lets its guard do the work: include either
 * one, or both, in any order, and you get exactly one copy.
 */
#ifndef BMO_STRINGS_H
#define BMO_STRINGS_H

#include <string.h>

#endif /* BMO_STRINGS_H */
