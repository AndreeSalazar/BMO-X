# Arquitectura de lenguajes en BMO-X (COBOL como primer ciudadano)

> **Estado**: diseno/vision. Parte del esqueleto ya existe (frontend COBOL,
> BEF, validator/signing en bmo-abi). Las tres librerias compartidas y el
> gate de verificacion estan **por construir**. Documento vivo.

## Ley fundamental

> **Compartes CONTRATOS y FORMATOS, nunca CEREBROS.**

Un IR central por donde todos los lenguajes pasan seria un cerebro compartido
= monolito disfrazado. Prohibido. Lo unico comun son cosas que cada lenguaje
produce/consume **por su cuenta**: el formato **BEF**, el gate de
**verificacion**, y el **BMO ABI** congelado (2 syscalls).

- Compartir un **IR + optimizador obligatorio** = embudo central ❌ (monolitico).
- Compartir un **contrato/formato** = cada uno lo cumple solo ✅ (modular).
- Reusar codigo = **libreria que el lenguaje ELIGE enlazar** ✅, jamas una
  etapa por la que lo fuerzan.

La diferencia es **quien manda**: el lenguaje llama a la libreria (modular), o
el pipeline empuja al lenguaje (central). Siempre lo primero.

## El pipeline (cada lenguaje, individual y completo)

```
COBOL  -> parser -> AST -> semantica -> codegen -> BEF -+
C      -> (su pipeline propio, completo)      -> BEF -+-► VERIFICACION -► BMO ABI -► corre
C++    -> (su pipeline propio, completo)      -> BEF -+     (el gate)    (2 syscalls)
```

Cada lenguaje nace, vive y muere en su **propio** pipeline. COBOL conserva
TODA su esencia de principio a fin: nadie lo toca. Los lenguajes solo se
encuentran en la puerta de verificacion -- un contrato, no un jefe.

## Las 3 librerias que compensan la duplicacion (opcionales)

La modularidad estricta tiene un costo: cada lenguaje podria reimplementar lo
mismo (su propia optimizacion, su propio descenso al ABI). Se compensa con
**tres librerias compartidas que cada lenguaje ELIGE enlazar** -- reuso sin
centralizacion, sin embudo:

| # | Nombre | Crate | Que hace | Quien la usa |
|---|--------|-------|----------|--------------|
| 1 | **Generica -- Optimizacion** | `bmo-opt` | Pasadas clasicas: register allocation, constant folding, dead-code elimination, strength reduction | El que quiera velocidad (COBOL con loops pesados) |
| 2 | **Semantica -- Descenso al ABI** | `bmo-lower` | Traduce las *exigencias* del lenguaje (print, I/O, archivos) a operaciones del BMO ABI (`INVOKE`/subsyscalls). Mantiene esencia: el lenguaje decide QUE baja; la libreria da el vocabulario del ABI | Todos los que hablan al sistema |
| 3 | **Codificacion -- Encoder** | `sem-asm` | Diccionario token->bytes (tabla TOML). El piso final de bytes y la escotilla de control | C/C++ directo; COBOL opcional |

Regla: **son librerias, no etapas.** Un lenguaje puede usar las 3, una, o
ninguna. C/C++ pueden bajar directo a `sem-asm` para control absoluto de
bytes (= inline asm). COBOL puede optimizar con `bmo-opt` y tener su propio
encoder. **La esencia de cada lenguaje nunca se duplica** (es unica por
diseno); lo que se compensa con librerias es el trabajo mecanico repetido.

## El gate de VERIFICACION (el unico checkpoint comun)

Reemplaza el rol de seguridad que tendria un IR central -- pero como
**contrato**, no como embudo. Cada lenguaje emite BEF por su cuenta; el
verificador lo revisa de forma independiente:

```
BEF (de cualquier lenguaje) -► [VERIFICACION] -► pasa?
                                                  si -> admitido al BMO ABI, corre
                                                  no -> rechazado
```

Que verifica:
- Solo usa operaciones de **capability** validas (respeta el modelo).
- Es **memory-safe** y respeta el ABI congelado.
- **Firma/hash** correctos.

Esqueleto ya presente en `platform/abi/bmo-abi/src/bef/`:
`validator.rs`, `signing.rs`, `blake3.rs`.

**Conexion Singularity**: si el BEF **pasa la verificacion**, esta probado
seguro -> puede correr como *Software Isolated Process* (SIP) en el mismo
espacio de direcciones, **sin transicion de anillo**. El aislamiento lo da la
verificacion (compilador), no el hardware (CPU). Es la verificacion --no un
IR-- la que habilita el aislamiento barato.

## Por que COBOL es el primer ciudadano

- COBOL es **library OS por naturaleza**: los programas COBOL nunca hablaron
  al hardware, hablaron a su *runtime*. En BMO ese runtime es `bmo-rt` sobre
  los 2 syscalls. `DISPLAY` -> `bmo-lower` -> `INVOKE(console, WRITE)`.
- COBOL es la **prueba de fuego del ABI**: si un lenguaje de 1959, verboso,
  de records y batch, baja limpio a 2 syscalls, **cualquier cosa puede**.
- Todos los dialectos (85/2002/2014/2023) comparten el **contrato** (BEF+ABI),
  cada uno con su propia tabla de reglas en `sem-asm/standards/COBOL/`.

## La ESENCIA de COBOL (Grace Hopper, 1959) -- protegerla

COBOL no es "otro lenguaje que baja a bytes". Su alma, tal como la diseno
Grace Hopper y su equipo para la banca:

1. **Legible por humanos de negocio** -- English-like, DIVISIONs
   (IDENTIFICATION / ENVIRONMENT / DATA / PROCEDURE). El gerente lee el codigo.
2. **Centrado en datos / records** -- niveles (01, 05...), clausulas `PIC`.
3. **★ Aritmetica DECIMAL exacta** -- `PIC 9(5)V99`, packed decimal (COMP-3),
   `ROUNDED`, `ON SIZE ERROR`. **Centavos sin error de redondeo.** Esta es la
   razon por la que los bancos usan COBOL 60 anos despues: el float binario
   pierde centavos; el decimal de COBOL no. **Es el corazon, no un detalle.**

### Como la arquitectura la protege

- El descenso de COBOL es **individual** (`lang/cobol`, nunca un IR/cerebro
  compartido). Un IR central presionaria a representar todo como binario
  (modelo C) y **mancharia** la esencia. Mantenerlo aparte la salva.
- El encoder `sem-asm` es **maquina neutral** (como se escribe un byte, igual
  para todos) -- NUNCA toca la semantica. Migrar a el no mancha nada.

### Deuda de fidelidad ACTUAL (a saldar para honrar la esencia)

- ❌ `ADD`/`COMPUTE` hoy bajan a aritmetica **binaria** (`add rax, rdx`), no
  decimal. Parsea como COBOL (PIC existe) pero **calcula como C**. Para el
  alma bancaria: aritmetica que respete la escala del `PIC` y, a futuro,
  packed decimal. Esto vive en el descenso propio de COBOL -- no afecta a C.
- ⚠ La PIC la parsea `gnucobol-rs` (**GPL**). Para un BMO 100% propio y
  limpio, la esencia pediria un parser de PIC propio (hoy el corazon del
  DATA DIVISION es codigo ajeno con copyleft).

> Regla: **el encoder puede ser compartido; la ARITMETICA de COBOL jamas.**
> El decimal es sagrado y vive solo en `lang/cobol`.

## Orden de construccion

1. **COBOL y C primero** (los dos primeros ciudadanos, pipelines individuales).
2. Cerrar **BEF** como formato de salida comun.
3. **Verificacion** como gate (crecer desde validator/signing existentes).
4. **BMO ABI**: ya congelado (INVOKE / CHANNEL_KICK / WAIT).
5. Las 3 librerias **a dial**: empezar sin ellas (BEF directo), anadir
   `bmo-lower`, luego `bmo-opt` (regalloc), luego pulir `sem-asm`.
6. Cuando la verificacion pruebe memory-safety -> activar **SIPs** (Singularity).

## Lo que NO se hace

- ❌ Un IR compartido (monolito).
- ❌ Forzar a COBOL por `sem-asm` (tiene su propio encoder si quiere).
- ❌ Competir con LLVM optimizando: en vez de eso se **borran costos**
  cambiando el modelo (library OS borra la frontera de syscall; lenguajes
  nativos borran el impuesto del C ABI; perfil per-CPU borra el impuesto
  generico). Limite honesto: se borra el *tax del sistema*, no la fisica del
  computo.
