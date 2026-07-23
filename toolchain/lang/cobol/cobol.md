# Arquitectura de lenguajes en BMO-X (COBOL como primer ciudadano)

> **Estado**: diseño/visión. Parte del esqueleto ya existe (frontend COBOL,
> BEF, validator/signing en bmo-abi). Las tres librerías compartidas y el
> gate de verificación están **por construir**. Documento vivo.

## Ley fundamental

> **Compartes CONTRATOS y FORMATOS, nunca CEREBROS.**

Un IR central por donde todos los lenguajes pasan sería un cerebro compartido
= monolito disfrazado. Prohibido. Lo único común son cosas que cada lenguaje
produce/consume **por su cuenta**: el formato **BEF**, el gate de
**verificación**, y el **BMO ABI** congelado (3 syscalls).

- Compartir un **IR + optimizador obligatorio** = embudo central ❌ (monolítico).
- Compartir un **contrato/formato** = cada uno lo cumple solo ✅ (modular).
- Reusar código = **librería que el lenguaje ELIGE enlazar** ✅, jamás una
  etapa por la que lo fuerzan.

La diferencia es **quién manda**: el lenguaje llama a la librería (modular), o
el pipeline empuja al lenguaje (central). Siempre lo primero.

## El pipeline (cada lenguaje, individual y completo)

```
COBOL  → parser → AST → semántica → codegen → BEF ─┐
C      → (su pipeline propio, completo)      → BEF ─┼─► VERIFICACIÓN ─► BMO ABI ─► corre
C++    → (su pipeline propio, completo)      → BEF ─┘     (el gate)    (3 syscalls)
```

Cada lenguaje nace, vive y muere en su **propio** pipeline. COBOL conserva
TODA su esencia de principio a fin: nadie lo toca. Los lenguajes solo se
encuentran en la puerta de verificación — un contrato, no un jefe.

## Las 3 librerías que compensan la duplicación (opcionales)

La modularidad estricta tiene un costo: cada lenguaje podría reimplementar lo
mismo (su propia optimización, su propio descenso al ABI). Se compensa con
**tres librerías compartidas que cada lenguaje ELIGE enlazar** — reúso sin
centralización, sin embudo:

| # | Nombre | Crate | Qué hace | Quién la usa |
|---|--------|-------|----------|--------------|
| 1 | **Genérica — Optimización** | `bmo-opt` | Pasadas clásicas: register allocation, constant folding, dead-code elimination, strength reduction | El que quiera velocidad (COBOL con loops pesados) |
| 2 | **Semántica — Descenso al ABI** | `bmo-lower` | Traduce las *exigencias* del lenguaje (print, I/O, archivos) a operaciones del BMO ABI (`INVOKE`/subsyscalls). Mantiene esencia: el lenguaje decide QUÉ baja; la librería da el vocabulario del ABI | Todos los que hablan al sistema |
| 3 | **Codificación — Encoder** | `sem-asm` | Diccionario token→bytes (tabla TOML). El piso final de bytes y la escotilla de control | C/C++ directo; COBOL opcional |

Regla: **son librerías, no etapas.** Un lenguaje puede usar las 3, una, o
ninguna. C/C++ pueden bajar directo a `sem-asm` para control absoluto de
bytes (= inline asm). COBOL puede optimizar con `bmo-opt` y tener su propio
encoder. **La esencia de cada lenguaje nunca se duplica** (es única por
diseño); lo que se compensa con librerías es el trabajo mecánico repetido.

## El gate de VERIFICACIÓN (el único checkpoint común)

Reemplaza el rol de seguridad que tendría un IR central — pero como
**contrato**, no como embudo. Cada lenguaje emite BEF por su cuenta; el
verificador lo revisa de forma independiente:

```
BEF (de cualquier lenguaje) ─► [VERIFICACIÓN] ─► ¿pasa?
                                                  sí → admitido al BMO ABI, corre
                                                  no → rechazado
```

Qué verifica:
- Solo usa operaciones de **capability** válidas (respeta el modelo).
- Es **memory-safe** y respeta el ABI congelado.
- **Firma/hash** correctos.

Esqueleto ya presente en `platform/abi/bmo-abi/src/bef/`:
`validator.rs`, `signing.rs`, `blake3.rs`.

**Conexión Singularity**: si el BEF **pasa la verificación**, está probado
seguro → puede correr como *Software Isolated Process* (SIP) en el mismo
espacio de direcciones, **sin transición de anillo**. El aislamiento lo da la
verificación (compilador), no el hardware (CPU). Es la verificación —no un
IR— la que habilita el aislamiento barato.

## Por qué COBOL es el primer ciudadano

- COBOL es **library OS por naturaleza**: los programas COBOL nunca hablaron
  al hardware, hablaron a su *runtime*. En BMO ese runtime es `bmo-rt` sobre
  los 3 syscalls. `DISPLAY` → `bmo-lower` → `INVOKE(console, WRITE)`.
- COBOL es la **prueba de fuego del ABI**: si un lenguaje de 1959, verboso,
  de records y batch, baja limpio a 3 syscalls, **cualquier cosa puede**.
- Todos los dialectos (85/2002/2014/2023) comparten el **contrato** (BEF+ABI),
  cada uno con su propia tabla de reglas en `sem-asm/standards/COBOL/`.

## La ESENCIA de COBOL (Grace Hopper, 1959) — protegerla

COBOL no es "otro lenguaje que baja a bytes". Su alma, tal como la diseñó
Grace Hopper y su equipo para la banca:

1. **Legible por humanos de negocio** — English-like, DIVISIONs
   (IDENTIFICATION / ENVIRONMENT / DATA / PROCEDURE). El gerente lee el código.
2. **Centrado en datos / records** — niveles (01, 05…), cláusulas `PIC`.
3. **★ Aritmética DECIMAL exacta** — `PIC 9(5)V99`, packed decimal (COMP-3),
   `ROUNDED`, `ON SIZE ERROR`. **Centavos sin error de redondeo.** Esta es la
   razón por la que los bancos usan COBOL 60 años después: el float binario
   pierde centavos; el decimal de COBOL no. **Es el corazón, no un detalle.**

### Cómo la arquitectura la protege

- El descenso de COBOL es **individual** (`lang/cobol`, nunca un IR/cerebro
  compartido). Un IR central presionaría a representar todo como binario
  (modelo C) y **mancharía** la esencia. Mantenerlo aparte la salva.
- El encoder `sem-asm` es **máquina neutral** (cómo se escribe un byte, igual
  para todos) — NUNCA toca la semántica. Migrar a él no mancha nada.

### Deuda de fidelidad ACTUAL (a saldar para honrar la esencia)

- ❌ `ADD`/`COMPUTE` hoy bajan a aritmética **binaria** (`add rax, rdx`), no
  decimal. Parsea como COBOL (PIC existe) pero **calcula como C**. Para el
  alma bancaria: aritmética que respete la escala del `PIC` y, a futuro,
  packed decimal. Esto vive en el descenso propio de COBOL — no afecta a C.
- ⚠️ La PIC la parsea `gnucobol-rs` (**GPL**). Para un BMO 100% propio y
  limpio, la esencia pediría un parser de PIC propio (hoy el corazón del
  DATA DIVISION es código ajeno con copyleft).

> Regla: **el encoder puede ser compartido; la ARITMÉTICA de COBOL jamás.**
> El decimal es sagrado y vive solo en `lang/cobol`.

## Orden de construcción

1. **COBOL y C primero** (los dos primeros ciudadanos, pipelines individuales).
2. Cerrar **BEF** como formato de salida común.
3. **Verificación** como gate (crecer desde validator/signing existentes).
4. **BMO ABI**: ya congelado (INVOKE / CHANNEL_KICK / WAIT).
5. Las 3 librerías **a dial**: empezar sin ellas (BEF directo), añadir
   `bmo-lower`, luego `bmo-opt` (regalloc), luego pulir `sem-asm`.
6. Cuando la verificación pruebe memory-safety → activar **SIPs** (Singularity).

## Lo que NO se hace

- ❌ Un IR compartido (monolito).
- ❌ Forzar a COBOL por `sem-asm` (tiene su propio encoder si quiere).
- ❌ Competir con LLVM optimizando: en vez de eso se **borran costos**
  cambiando el modelo (library OS borra la frontera de syscall; lenguajes
  nativos borran el impuesto del C ABI; perfil per-CPU borra el impuesto
  genérico). Límite honesto: se borra el *tax del sistema*, no la física del
  cómputo.
