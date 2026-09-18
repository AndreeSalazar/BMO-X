<!--
  Read this before you write anything below. It is short.

  Ring 0 is CLOSED to external contributions, entirely: the kernel, the
  drivers, the boot chain, the two syscalls, BEF, and everything that builds,
  judges, signs or loads them. The exact list is .github/CODEOWNERS.

  A pull request that touches any of it will be closed pointing to
  CONTRIBUTING.md, whatever its quality. That is not a judgement of your code.
  If you think Ring 0 is wrong, open an ISSUE and argue it -- that is welcome.

  Drivers and integrations belong in Ring 3, or on the Antena (it runs Linux).
-->

## What this changes

## Where it lives

- [ ] It touches nothing listed in `.github/CODEOWNERS`

## Its colour

- [ ] 🟢 I watched it run on a real CPU -- photo or telemetry attached
- [ ] 🟡 It compiles and passes its tests; no CPU has run it

## Checks

- [ ] `cargo test` passes
- [ ] Sources are ASCII (`python toolchain/tools/ascii-sweep/ascii_sweep.py --check`)
- [ ] One concern in this PR
