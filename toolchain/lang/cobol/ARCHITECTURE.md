# Arquitectura del frontend COBOL de BMO

Pipeline completo: **COBOL Source → BEF** (el ejecutable que corre en Ring 3).
Todo es Rust propio; lo GIGANTE lo genera la fábrica Python (cobol-gen). Ver
también `cobol.md` (la esencia/teoría) y `../../tools/cobol-gen/README.md`.

## Las fases (Source → BEF)

```
  .cob (texto)
     │
 ┌───▼───────────────┐
 │ 1. LEXER          │  src/lexer.rs        ✅  Source → Tokens
 │    tokeniza       │  usa generated::words (reservadas)
 └───┬───────────────┘  distingue . decimal vs . terminador
     │  Vec<Token>
 ┌───▼───────────────┐
 │ 2. PARSER         │  src/parser.rs       ◐  Tokens/líneas → AST
 │    reconoce COBOL │  conoce TODO el vocabulario (tablas Python);
 └───┬───────────────┘  compila un subconjunto de verbos
     │  CobolProgram (AST)
 ┌───▼───────────────┐
 │ 3. AST            │  src/ast/*           ✅  estructura del programa
 │    datos + sents. │  DataItem (PIC propio, escala), CobolStatement
 └───┬───────────────┘
     │
 ┌───▼───────────────┐
 │ 4. CODEGEN        │  src/codegen.rs      ◐  AST → bytes x86-64
 │    decimal exacto │  escala PIC (centavos), encoder sem-asm
 └───┬───────────────┘
     │  Vec<u8> (código máquina)
 ┌───▼───────────────┐
 │ 5. BEF            │  bmo-abi::bef::writer ✅  empaqueta → .bef/.bex
 │    contenedor     │  → verificación → BMO ABI (3 syscalls) → corre
 └───────────────────┘
```

Leyenda: ✅ hecho · ◐ funciona en un subconjunto, crece · ⬜ pendiente.

## Componentes propios (100% BMO)

| Archivo | Fase | Estado |
|---|---|---|
| `src/lexer.rs` | Tokenizador | ✅ palabras/números/decimales/strings/comentarios, líneas |
| `src/pic.rs` | Cláusula PICTURE | ✅ propio (sin gnucobol-rs GPL), da la `scale` decimal |
| `src/parser.rs` | Sintaxis → AST | ◐ verbos principales; conoce todo el vocabulario |
| `src/ast/*` | AST | ✅ DataItem, CobolStatement, condiciones |
| `src/codegen.rs` | Emisión x86-64 | ◐ decimal exacto (ADD/SUB/MUL/DIV), usa encoder sem-asm |
| `src/dialect.rs` | Dialectos (85/2002/2014/2023) | ✅ perfiles |
| `src/generated/words.rs` | Vocabulario | ✅ **GENERADO por Python** (217 reservadas, 55 intrínsecas) |

### Lo que NO es de COBOL, y por eso no vive aquí

`USAGE COMP-3` lo decide la PICTURE —eso es de COBOL— pero **empaquetar dos
dígitos en un byte no lo es**: es una representación, y los mismos nibbles en el
mismo orden los piden el `Decimal` del Annex F de Ada y el `FIXED DECIMAL` de
PL/I. Por eso los dos emisores viven en **`bmo-lower::packed`**, al lado de
`fmt` y por la misma razón que él: se comparten **contratos y librerías, nunca
cerebros**.

Del lado de COBOL sólo quedan tres cosas, y todas son del lenguaje: quién es
COMP-3, cuántos dígitos tiene y si lleva `S`. En `codegen.rs` lo miran
**únicamente `load_var` y `store_var`** — las dos puertas a la memoria de una
variable— así que la aritmética sigue viendo el entero escalado de siempre y no
se entera de cómo se guarda. Ese reparto es lo que mantiene exacto el decimal.

## Lo que genera Python (la fábrica cobol-gen)

`toolchain/tools/cobol-gen/definition.py` (compacto) → `generate.py` →
`src/generated/words.rs` (556 palabras reservadas, 55 intrínsecas):
- `RESERVED[]` + `is_reserved()` (búsqueda binaria)
- `RESERVED_STD[]` + `reserved_since()` — etiqueta por origen
- `verb_kind()`, `INTRINSIC[]` + `is_intrinsic()`
- **`is_essence()` / `is_vendor()`** — separan la ESENCIA del VENDOR

### COBOL devorado → BMO COBOL (esencia vs vendor)

GnuCOBOL infla a 1130+ palabras porque **traduce COBOL a C** y mete TODOS los
dialectos + su runtime. Eso NO es la esencia. BMO devora COBOL y lo hace suyo,
separando:

- **STANDARD** = la esencia (COBOL74/85/2002/2023 + corpus ISO/ANSI). Primera
  clase. Es el idioma de Grace Hopper.
- **VENDOR** = extensiones de vendor (VAX DBMS, IBM obsoletas, pantalla VAX/MF,
  COMP no estándar). Se RECONOCEN (`is_vendor`) pero marcadas aparte, jamás
  confundidas con el núcleo.

BMO compila COBOL a BEF **nativo** (sin C intermedio, sin runtime ajeno) — lo
opuesto de GnuCOBOL. Crecer la esencia = ampliar `RESERVED_STANDARD`/las eras
en `definition.py` + `py generate.py`; nunca perseguir el número inflado de
GnuCOBOL.

## Esencia protegida (ver cobol.md)

- **Decimal exacto** = alma bancaria. Vive SOLO en `codegen.rs` (escala PIC),
  jamás en el kernel ni en un IR compartido.
- **Contratos y librerías, nunca cerebros.** El encoder `sem-asm` es máquina
  neutral; la aritmética de COBOL es privada de COBOL.

## Lo que falta (roadmap del frontend)

> ⚠️ **Esta lista es del COMPILADOR.** Lo que le falta al *sistema* para banca
> de verdad es más grande y vive en **[`BANCA_REAL.md`](BANCA_REAL.md)**: los
> ficheros indexados (VSAM/KSDS), el despachador de transacciones (CICS), el
> batch declarativo (JCL) y qué extensiones de IBM valen la pena.
>
> El resumen de allí: de las cuatro piezas, **la más difícil ya está hecha** —
> ESTRATOS es transaccional en el fondo, que es lo que a CICS le costó cincuenta
> años atornillar. Y **el hueco real es el índice por clave**: hoy hay File I/O
> secuencial, y sin índice no hay banca, hay listados.

1. **Parser sobre tokens**: migrar `parser.rs` del modo por-líneas al flujo
   de tokens del lexer (parser recursivo-descendente) → más COBOL, mejor.
2. **Más verbos con codegen**: EVALUATE, INITIALIZE, STRING/UNSTRING, PERFORM
   VARYING real, INSPECT.
3. **Variables como operandos**: hoy la aritmética es literal+variable;
   falta variable+variable y expresiones en COMPUTE.
4. **ROUNDED / ON SIZE ERROR**: cláusulas de la aritmética decimal.
5. ~~**Packed decimal (COMP-3)** real en almacenamiento.~~ ✅ **hecho el
   2026-08-03.** Lo que falta ahora es su segunda mitad: el **registro
   BINARIO**, o sea leer del fichero los bytes empaquetados tal cual vienen en
   vez de una línea de texto por registro. Eso pide un `01` con varios campos y
   posiciones fijas, que es el punto 6.
6. **Records anidados / OCCURS** (tablas).
