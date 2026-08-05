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

If it boots — say so, with your CPU and motherboard.
If it does not — **that is the more useful result.** Photograph the screen where
it stops and open an issue. A panic message from unknown hardware is worth more
to me than a pull request.

See [docs/evidencia/](docs/evidencia/) for what a working boot looks like, so
you can tell how far you got.

---

## What is open, and what is frozen

This project has a floor and a building. They have different rules.

### 🔒 The Base does not move

- the three syscalls — `INVOKE`, `CHANNEL_KICK`, `WAIT`
- the BEF/BEX container format
- the capability model itself

These are frozen **on purpose**. A system aiming at critical sectors is only
auditable if the thing being audited stops changing — the point is that **one
audit serves everyone**. A pull request that adds a fourth syscall, or a field
to BEF, will be declined no matter how good it is. That is not a judgement of
the code.

If you think the Base is genuinely wrong, open an issue and argue it. That
conversation is welcome. A patch is not the way to have it.

### 🟢 Everything above it is open ground

- **Ports to other CPUs and firmware** — this is the reason the project is
  opening up at all
- **Drivers** — network, storage, anything the hardware in front of you has and
  mine does not
- **Applications** — anything that compiles to a `.bex`
- **Table-driven mods** — see the mods contract; tables, not plugins
- **New instructions** in the semantic layer — it is a TOML table, not a code
  generator
- **New language frontends** on top of the existing toolchain
- **Documentation, and corrections to it** — including telling me that a 🟢 in
  the README is not actually green

---

## The rule that matters most here

**Do not mark something green that you have not seen run.**

The README labels every claim 🟢 / 🟡 / ⚪, and that system is the most valuable
thing in this repository. It only works if nobody bends it.

| | State | Meaning |
|:--:|---|---|
| 🟢 | **Runs on metal** | You watched it work on a real CPU, and you have a photo or a telemetry line |
| 🟡 | **Written, never executed** | It compiles, it links, it passes its tests — and no CPU has run it |
| ⚪ | **Design only** | Documented, not built |

If your patch compiles and passes tests but you never booted it, it is 🟡. Say
so in the PR. **That is not a weaker contribution — it is an honest one**, and it
will be merged as 🟡 and marked as such.

Claiming 🟢 for something you did not watch run is the only thing here that will
get a contribution rejected on principle.

---

## Before you open a pull request

1. **`cargo test` passes.** The test bench is described in the README under
   *How it is verified*, including what it cannot prove.
2. **Any emitted `.bex` goes through `bmo-verify`.** Since 2 August no frontend
   may write an executable that has not been validated. Do not add a fifth path
   around it.
3. **State the colour.** 🟢 with evidence, or 🟡 honestly.
4. **One concern per PR.** A driver and a refactor in the same patch is two
   patches.
5. **Comments in the language of the surrounding file.** The codebase is
   bilingual — Spanish in places, English in others. Match what is around you
   rather than converting it.

---

## Issues

Good issue:

> Ryzen 9 7950X, ASUS X670E, firmware 2.14. Stops after `AHCI: puerto 0` with
> the attached photo. USB stick is a SanDisk 32 GB.

Also good:

> The README marks X as 🟢 but I built it and it does not do what that line
> says.

Less useful: feature requests for things the roadmap explicitly declines. The
README has a section called *What it deliberately does not do* — network stack,
GPU acceleration, a full libc, Wine, a mainframe migration path. Those are not
oversights and asking for them will not change them.

---

## Licensing, said plainly

BMO-X is under the [Techne License v2.0](LICENSE.txt). The source is public and
you may read, build and audit all of it without asking anyone. It is **not OSI
open source** — the rights stay with the author and commercial use above the
threshold is licensed.

What that means for you as a contributor: **by opening a pull request you are
offering your contribution under that same licence**, and the rights to it sit
with the project the way they do for the rest of the code.

If that does not work for you, that is a legitimate position and no hard
feelings — the boot reports above are still enormously useful and carry none of
this.

If you are contributing on behalf of an employer, please check with them first.
I would rather sort that out before a patch than after.

---

## Contact

Issues and pull requests are the preferred channel — they leave a public record,
which is the whole spirit of this repository.

Built from scratch in Lima, Peru, by **Eddi Salazar**.
