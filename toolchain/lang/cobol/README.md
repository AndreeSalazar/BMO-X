# BMO COBOL

**A native COBOL compiler. No GCC. No LLVM. No transpiling to C.**

COBOL source goes in, a native x86-64 executable comes out — emitted by a
compiler written from scratch in Rust. Money arithmetic is exact by
construction: `19.99 × 3 = 59.97`, not `59.969999999999999`.

It targets [BMO-X](https://github.com/AndreeSalazar/BMO-X), a bare-metal
system that boots on real hardware. Not on QEMU. On a real Ryzen.

<!-- TODO Eddi: pon aqui el enlace al video del Ryzen -->
▶ **[Watch it run on real hardware (90s)](VIDEO_URL_AQUI)**

---

## Try it in 60 seconds

You don't need the operating system, a USB stick, or an emulator. Download
the compiler and run it on your own machine.

<!-- TODO Eddi: sube los binarios a Releases y ajusta el enlace -->
1. Download `bmo-cobol` from [Releases](../../releases) — Windows and Linux
2. Grab `examples/2-decimal/banco.cob` from this repo
3. Run it:

```bash
./bmo-cobol banco.cob
```

Exact money arithmetic, on your screen, in under a minute. No toolchain to
install, nothing to build, no runtime.

### How it is tested

Every feature listed below has a row in a **conformance matrix** that
compiles the COBOL and then *executes the emitted machine code*, checking
the real output. Not a comparison against hand-written byte strings — the
bytes actually run. Adding a feature to the compiler means adding its row.

That is why there are no percentages on this page. A percentage would need
a denominator, and the COBOL standard doesn't have one.

---

## Why exact decimals

Most languages store money in binary floating point, so `0.1 + 0.2` is not
`0.3`. Banks cannot work that way, which is one reason COBOL never died.

BMO COBOL keeps every value in **integer scale** — cents, not floats. The
compiler knows each field's `PICTURE` and its scale, and emits integer
instructions. There is no rounding drift because there is no float.

That result, `59.97`, was confirmed on real silicon, not in a simulator.

---

## What runs

Stated as features, not as a percentage of the standard. COBOL has no
finish line; claiming "85% of COBOL" would mean inventing a denominator
that does not exist.

**Data and arithmetic**
- `PICTURE` clauses with exact decimal scale
- `COMPUTE` with real operator precedence
- `MOVE`, `ADD`, and edited-picture output generated as instructions —
  no interpreter and no mask survive into the binary
- **`ROUNDED`, with all six standard modes** — `ROUNDED MODE IS NEAREST-EVEN`
  is banker's rounding, and it is there because rounding is a **legal
  decision**: the classic mode is biased (ties always go up), and some
  jurisdictions require the unbiased one. The **result** is rounded, not the
  operands — with the asymmetric modes those are not the same thing
- `USAGE COMP-3` (packed decimal) — **the format real bank data is stored in**.
  Two digits per byte, sign in the last nibble, and the field occupies exactly
  what its PICTURE says, so it truncates like the standard requires. The BCD
  emitters live in `bmo-lower::packed`, because packing is a representation and
  not any language's semantics

**Control flow**
- `IF` / `ELSE` — real branching
- `EVALUATE` — both forms: with a subject (`WHEN 1`, `WHEN 2 THRU 5`,
  `WHEN 6, 7`, `WHEN OTHER`) and `EVALUATE TRUE`, the **decision table** a bank
  writes a fee tier with. First branch that matches wins; the rest are not tested
- Conditions combined with `AND` and `OR`, as a tree with correct precedence
  and **short-circuit** evaluation
- **Paragraphs**, and every out-of-line `PERFORM`: `PERFORM p`,
  `PERFORM p THRU q`, `PERFORM p n TIMES`, `PERFORM p UNTIL cond`. This is how
  real COBOL is structured — a readable main body and the work in named steps
- `PERFORM` — real loops, in-line and out-of-line
- Level `88` condition names, including `VALUE 1 THRU 5` and `VALUE 6, 7`

**Tables**
- `OCCURS n TIMES`, literal and variable subscripts
- Range checking with a named error, not silent corruption
- Nested subscripts, e.g. `E(IDX(1))`

**Files** — sequential batch I/O
- `SELECT … ASSIGN TO`, `FD` and its record
- `OPEN INPUT|OUTPUT`, `READ … AT END`, `WRITE`, `CLOSE`

**Terminal**
- `DISPLAY`, `DISPLAY <var>`, `ACCEPT`

Reserved-word tables are generated from the ISO/ANSI vocabulary, kept
separate from vendor extensions.

---

## What does not run — and says so

Every unsupported construct is **rejected with a reason**. Nothing is
silently ignored, and nothing is stubbed out to look like it works.

Not implemented yet: `STRING`, `INSPECT`, `SEARCH`, `CALL`, `SORT`, `GO TO`,
`REDEFINES`, `PIC X` fields with actual text, nested records beyond group level,
and the intrinsic function library.

Deliberately rejected with an explanation rather than guessed at:
- `READ` without `AT END` — it would compile into a loop that never ends
- `OPEN EXTEND` — the underlying gate creates files from scratch, so this
  would silently destroy history
- `USAGE COMP` / `BINARY` / `COMP-5` — the binary layout is not stored
  differently from `DISPLAY` yet, and accepting the word would promise a format
  it does not deliver. `COMP-3` **is** implemented and does store packed
- `USAGE COMP-1` / `COMP-2` — binary floating point cannot represent `19.99`,
  and that is where one-cent discrepancies come from
- `OCCURS` on an `01` level — the error shows the correct group form

A compiler that quietly accepts what it cannot do is worse than one that
refuses. This one refuses, out loud.

---

## Examples

Real programs, each one compiled and executed by the test suite:

| File | What it does |
|---|---|
| `examples/2-decimal/banco.cob` | Exact money arithmetic |
| `examples/3-presentacion/extracto.cob` | Statement with edited `PICTURE` output |
| `examples/4-ficheros/batch.cob` | Reads transactions, totals in cents, writes the close |
| `examples/5-tablas/conceptos.cob` | Per-concept close over two parallel files |
| `examples/6-condiciones/cartera.cob` | Level 88 condition names |
| `examples/7-empaquetado/cuentas.cob` | `COMP-3` packed decimal — the storage real bank data uses |
| `examples/8-parrafos/cierre.cob` | An end-of-day close written the way real COBOL is: paragraphs |

`batch.cob` is the one to read first. It is an ordinary end-of-day batch:
read a file, compute, write a report. That is what banking software
actually is.

---

## The compiler prints your file format

```bash
bmo-cobol --copybook cierre.cob
```

```
IMPORTE   (5 bytes)  [FICHERO]
  desde  hasta  bytes  nivel  campo        como     PICTURE
      0      5      5      1  IMPORTE      PACKED   S9(7)V99

  PACKED  dos digitos por byte. El ULTIMO nibble es el signo:
          C positivo, D negativo, F sin signo.
```

In banking, that document is called a **copybook**, and it is what two systems
exchange so they can read the same file. The one kept by hand **always ends up
lying** — the code changes and the document does not.

This one cannot. It comes out of **the same table the code generator uses** to
emit the `READ` and the `WRITE`, so there is nowhere for the two to drift apart.
The document does not describe the format — it *is* the format.

---

## And it can read the file back

Once a `COMP-3` field reaches disk, the file **stops being readable** — packed
nibbles are not text, and `cat` shows garbage. So the compiler decodes it:

```bash
bmo-cobol --ver datos/ctas.bin cuentas.cob
```

```
* 48 bytes = 3 registro(s) de 16, segun REG-CUENTA

#1    byte 0
  CTA-NUMERO        4471998200   34 34 37 31 39 39 38 32 30 30
  CTA-SALDO           15234.75   00 15 23 47 5C
  CTA-ESTADO                 1   31
```

The decoded value *and* the raw bytes, side by side. And the viewer reads with
**the same rule the program wrote with** — its decoders are checked against the
emitted ones over every two-byte pattern, so it cannot show one amount while the
program reads another.

If the file size is not a multiple of the record, it says so and shows the
leftover: that is the classic symptom of the wrong copybook, and staying quiet
about it would leave you believing the last record is just odd.

---

## The road to real banking

Having COBOL is not having a banking system. A mainframe one leans on four
things *besides* the compiler — transaction dispatch, batch scheduling,
**indexed files**, and IBM's extensions — and until each is named one by one,
"COBOL" promises more than it delivers.

- **[`BANCA_REAL.md`](BANCA_REAL.md)** *(Spanish)* — what is actually missing
  and why, with a verdict on each piece. Short version: the hardest part is
  already done, because ESTRATOS is transactional at the bottom, which is what
  took CICS fifty years to bolt on. The real gap is **the key index** — today
  there is sequential file I/O, and without an index you have listings, not
  banking.
- **[`PLAN_BANCA.md`](PLAN_BANCA.md)** *(Spanish)* — the task list: nine phases
  with checkboxes, from the compiler's own floor to a small bank running
  end-to-end on real hardware. Every item says what blocks it and how you know
  it is done.

This is **not** a migration target for z/OS code, and that boundary is stated in
both documents rather than left for someone to assume.

---

## How it is built

```
lexer.rs      tokens
parser.rs     statements
tparser.rs    token-based program parser
pic.rs        PICTURE clauses and decimal scale
edicion.rs    edited-picture output, emitted as instructions
codegen.rs    x86-64 machine code
```

Roughly 6,700 lines of Rust, 94 tests. The backend emits x86-64 directly
through a table-driven assembler: adding an instruction is a row in a TOML
file, not new Rust code.

The output is a `.bex` — a native binary in BMO-X's own format. C, COBOL,
and Ada all compile to that same format, and the system's entry gate never
asks which language a binary came from.

---

## Verified on hardware, not on QEMU

Most hobby operating systems live in an emulator and never touch metal.
The order of authority here is written down and enforced:

1. **The real Ryzen**
2. The specification document
3. The emulator

When the emulator and the hardware disagree, *the emulator gets fixed*.
That rule caught a broken `lea [rip+disp]` that passed green in simulation
and would have read garbage on real silicon.

---

## Where this came from

BMO-X was never designed for banking. It was designed for **games** — a
minimal bare-metal system to get out of the way of Vulkan.

The graphics stack turned out to be a decade of work for one person. The
decimal arithmetic underneath it turned out to be exactly what banking
software has needed for sixty years. So the project kept the minimalism and
changed the destination.

That origin explains the architecture: three frozen syscalls, no libc, no
window system, nothing general-purpose. This machine will never open a web
browser — and neither will an ATM, a payment terminal, or a flight
computer. Some machines exist to do one thing, exactly, for twenty years.

---

## License

[Techne License v2.0](../../../LICENSE.txt). Free for individuals,
students, research, open source, nonprofits, and companies under
USD $1M/year. Commercial use above that is a published rate, not a
negotiation you have to win.

The source is private, but it is not a black box: every licensee receives
the complete source, compiles it themselves, and audits it themselves.

---

Built from scratch in Lima, Peru, by **Eddi Andree Salazar Matos**.

<!-- TODO Eddi: enlaces a tu web y a tu correo -->
