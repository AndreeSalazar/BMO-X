# nexo_ring3 (formerly `nexo/`) — BSF loader for FastOS Ring 3

> **v1.1.0 rename**: This crate was previously named `nexo/`, which
> collided with `kernel::lang::nexo` (the ÑEXO language compiler).
> Renamed to make the separation explicit.

## What it is

A small `no_std + alloc` library intended to live in **Ring 3 userland**
once the kernel has its Ring 3 transition working. Its only job:

1. Parse and validate BSF (BareX Shader Format) blobs.
2. Hand the raw SPIR-V bytes to the GPU driver (or to the
   framebuffer blitter in the meantime).

## What it is NOT

- **Not** the ÑEXO language compiler. That lives at
  `kernel/src/lang/nexo/` and is built into the kernel image directly.
- **Not** a CLI. The CLI is `nexo-sh-tool/` (uses naga to compile
  HLSL/GLSL/WGSL → BSF, runs on the host).
- **Not** built by `build_uefi.ps1`. It's a future userland crate that
  will be loaded as a BEF/ELF blob once the kernel supports
  Ring 3 process loading.

## Current status

Code complete (BSF_MAGIC, BSF_VERSION, header parser, hash verification),
but **not yet integrated** because:

- The kernel still runs everything in Ring 0 (no Ring 3 processes yet).
- The BSF loader in the kernel (`kernel/src/barex/shader/bsf/`) has
  its own copy of the parsing logic for now.
- Once Ring 3 lands, this crate will be the canonical place for the
  loader, and the kernel-side copy will be deleted.

## How to test (host)

```sh
cargo test -p nexo
```

When run on the host (not no_std), the tests can do real file I/O
and exercise the full header parsing pipeline.

## Integration plan

1. Land Ring 3 process loader (BEF/ELF → user process).
2. Move `kernel/src/barex/shader/bsf/` logic here.
3. Expose via `BMO_ABI_INTEROP` to userland.
4. The `nexo-sh-tool` CLI stays separate (it's a host-side
   compiler, not a runtime).
