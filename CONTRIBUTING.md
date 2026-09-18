# Contributing to BMO-X

Read this before opening anything. It is short, and it is honest about what
can and cannot be accepted.

---

## The one thing I actually need

**Boot it on a machine that is not mine.**

BMO-X has been verified on exactly one CPU: an AMD Ryzen 5 5600X. Every 🟢 in
the README means *that* machine. Nobody knows what happens on an Intel, on a
Zen 5, on a laptop with a different xHCI controller, or on firmware that lays
out its memory map differently.

That is the single most valuable thing anyone can give this project right now,
and it costs you a USB stick and ten minutes.

> The licence asks nothing of you in return -- it is Apache 2.0. This is a
> request, not a condition, and it is the only one in this file.

If it boots -- say so, with your CPU and motherboard.
If it does not -- **that is the more useful result.** Photograph the screen where
it stops and open an issue. A panic message from unknown hardware is worth more
to me than a pull request.

[!] **You need Windows to build it today.** `build.ps1` is PowerShell and hunts
for `llvm-objcopy.exe` under `%USERPROFILE%\.rustup`, so there is currently no
way to produce a bootable image on Linux or macOS. A prebuilt archive on the
Releases page fixes that for people who only want to *run* it. Making the build
portable is on me, not open to patches: the build is what manufactures Ring 0
(see below). Telling me exactly where it breaks on your system IS useful -- as
an issue.

[!] **Check Secure Boot before you report it.** `BOOTX64.EFI` is self-signed, so
a machine with Secure Boot on refuses to load it and the firmware never runs a
single instruction. That looks exactly like "it does not boot" and is not a bug.
Turn it off, try again, and only then is the report interesting.

See [docs/evidencia/](docs/evidencia/) for what a working boot looks like, so
you can tell how far you got.

---

## What is open, and what is closed

This project has a floor and a building. They have different rules.

### 🔒 Ring 0 is closed to external contributions -- entirely

In the owner's words, 2026-09-17:

> *Ring 0 es totalmente inmutable y cerrado a contribuciones externas.
> Cualquier integracion, driver o logica de cliente debe residir en Ring 3 o en
> el nodo Antena.*

**Ring 0 is closed to external contributions, entirely.** Any integration,
driver or client logic belongs in **Ring 3** or on the **Antena** node.

"Ring 0" here is not a feeling, it is a list, and the authoritative copy is
[`.github/CODEOWNERS`](.github/CODEOWNERS):

- **everything `bmo-kernel` links.** Today that is 37 crates -- the kernel, the
  drivers, the storage and USB stacks, the judges in `platform/shared/` it
  depends on -- and `cargo tree` says **zero of them are third-party**. This
  rule is what keeps that number at zero;
- **the boot chain** (`Ultra_kernel_x86-64/`: the UEFI stages and the loader);
- **the Base** -- the two syscalls `INVOKE` and `WAIT` (number `1` is a
  reserved tombstone: `CHANNEL_KICK` was withdrawn on 2026-08-10 and is not
  recycled), the BEF/BEX container, and the capability model;
- **everything that BUILDS, JUDGES, SIGNS or LOADS the above**: `build.ps1`,
  `bmo.ps1`, the guardians under `toolchain/tools/` that the build runs, the
  `bmo-verify` gate, the linkers, the signer, and the hardware profiles in
  `PERFIL/`, which are data that Ring 0 obeys.

**Why, and why it includes the build.** The model is the xz backdoor of 2024
(CVE-2024-3094). It was not written into the code everyone reviewed. It came from
a contributor who spent two years earning maintainer trust, and it lived in
**build scripts and test files** -- and one of its steps disabled a security
check with a **single character**. The equivalent here is not only the kernel:
it is the script that builds it and the guardians that judge it. A guardian
that is quietly weakened says `COMPLETE` exactly like a guardian that works.

**What this is not.** It is not a judgement of your code, and it is not the
project closing: BMO-X is Apache 2.0, and you may fork Ring 0 and change
anything in your fork (see *Licensing* below). This is a decision about what is
merged into *this* repository.

If you think Ring 0 is wrong somewhere, **open an issue and argue it**. That
conversation is welcome, and a well-argued issue is how things in Ring 0 get
changed -- by the owner. A patch is not the way to have it, and a pull request
that touches Ring 0 will be closed pointing here, whatever its quality.

### 🟢 Where contributions go

- **Your hardware, booted and reported.** Still the most valuable thing anyone
  can give -- see the top of this file. A panic photo from a machine I do not
  have is a contribution; a patch to make it boot is not accepted, the report
  is what lets me fix it.
- **Drivers -- in Ring 3, or on the Antena.** A driver in Ring 0 is exactly what
  this rule closes. If your device needs one, it goes in Ring 3 over the frozen
  syscalls, or on the Antena: **the Antena runs Linux**, so a device BMO-X does
  not speak can be driven there and reach BMO-X through the protocol. If you
  want to contribute kernel code, Linux is the kernel for that.
- **The Antena node** -- `toolchain/tools/antena/` and `platform/shared/bmo-antena`.
  It sits in front of BMO-X and never commands it.
- **Applications** -- anything in `Ultra_userspace/` or that compiles to a
  `.bex`. Ring 3, behind the capability model.
- **New language frontends** on top of the existing toolchain. What they emit
  still passes the `bmo-verify` gate, and the gate is closed.
- **New instructions** in the semantic layer -- a TOML table, not a code
  generator.
- **Table-driven mods** -- tables, not plugins.
- **Documentation, and corrections to it** -- including telling me that a 🟢 in
  the README is not actually green.

---

## The rule that matters most here

**Do not mark something green that you have not seen run.**

The README labels every claim 🟢 / 🟡 / ⚪, and that system is the most valuable
thing in this repository. It only works if nobody bends it.

| State | Meaning |
|---|---|
| 🟢 **Runs on metal** | You watched it work on a real CPU, and you have a photo or a telemetry line |
| 🟡 **Written, never executed** | It compiles, it links, it passes its tests -- and no CPU has run it |
| ⚪ **Design only** | Documented, not built |

If your patch compiles and passes tests but you never booted it, it is 🟡. Say
so in the PR. **That is not a weaker contribution -- it is an honest one**, and it
will be merged as 🟡 and marked as such.

Claiming 🟢 for something you did not watch run is the only thing here that will
get a contribution rejected on principle.

---

## Before you open a pull request

1. **`cargo test` passes.** The test bench is described in the README under
   *How to be suspicious of it*, including what it cannot prove.
2. **Any emitted `.bex` goes through `bmo-verify`.** Since 2 August no frontend
   may write an executable that has not been validated. Do not add another path
   around it -- whichever number of frontends there happen to be that week.
3. **State the colour.** 🟢 with evidence, or 🟡 honestly.
4. **One concern per PR.** A driver and a refactor in the same patch is two
   patches.
5. **Sources are ASCII. The build checks it.**
   `python toolchain/tools/ascii-sweep/ascii_sweep.py --check` runs inside
   `build.ps1`, in the same step that validates the syscall contract, and it
   fails the build.

   This is not a style rule and it did not come from taste. A single accented
   letter in a C string literal once grew a `.bex` from 512 bytes to 492.032,
   and the kernel console is Latin-1 by design -- one byte per character, no
   decoder -- while Rust strings are UTF-8 and every print path hands them over
   raw. Two encodings that never agreed.

   New identifiers go in **English**. Roughly 900 Spanish ones are still being
   migrated batch by batch (`rename_to_english.py`), so you will meet both --
   write English, and do not convert a module you are not otherwise touching:
   a rename mixed into a feature PR is two patches.

6. **What BMO-X prints stays in Spanish, without accents.** The system speaks
   Spanish to its author, and the Latin-1 renderer cannot draw an accent that
   arrives as UTF-8. Kernel and userspace strings are therefore plain ASCII
   Spanish and the build enforces that too. Toolchain messages go to a host
   console and are exempt.

---

## Issues

Good issue:

> Ryzen 9 7950X, ASUS X670E, firmware 2.14. Stops after `AHCI: puerto 0` with
> the attached photo. USB stick is a SanDisk 32 GB.

Also good:

> The README marks X as 🟢 but I built it and it does not do what that line
> says.

Less useful: feature requests for things the roadmap declines outright -- a full
libc, Wine, a mainframe migration path. Those are not oversights and asking for
them will not change them.

[!] Networking and the GPU used to be on that list and **no longer are**: both
are queued with a written plan, and the README's *What is next, and what blocks
it* says what each one waits on. This paragraph contradicted that table for
weeks. If you find another place where these two documents disagree, that is a
bug worth an issue -- the README is the one that gets updated.

---

## Licensing, said plainly

BMO-X is under the **[Apache License 2.0](LICENSE)**. Real OSI open source:
use it, fork it, ship it, sell it, patent grant included, no fee. See
**[NOTICE](NOTICE)** for the copyright and the trademark line.

What that means for you as a contributor: **by opening a pull request you are
offering your contribution under Apache 2.0**, which is exactly section 5 of the
licence and needs no separate agreement from you. There is no CLA.

If you are contributing on behalf of an employer, please check with them first.
I would rather sort that out before a patch than after.

### And the one thing the licence no longer says

The previous licence made *"the Base does not fork"* a binding term. **Apache
2.0 does not, and I am not going to pretend otherwise.** You may fork the
kernel, add a third syscall, and change BEF, and the licence permits all three.

The reason not to has not changed and now has to stand on its own:

> With the Base fixed, **one audit is worth something to everyone**. Forked,
> every audit is worth something to one person.

So a pull request that touches Ring 0 -- a third syscall, a field in BEF, a
driver in the kernel, a line in the build -- will be declined here. That is a
decision about *this* repository, which is a thing a maintainer gets to make.
It is not a decision about yours.

---

## Contact

Issues and pull requests are the preferred channel -- they leave a public record,
which is the whole spirit of this repository. Pull requests for what is open;
issues for everything, Ring 0 included.

Built from scratch in Lima, Peru, by **Eddi Salazar**.
