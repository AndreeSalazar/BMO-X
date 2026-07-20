# Lenguajes FastOS

Este directorio agrupa frontends y tooling de lenguajes que generan ejecutables
**BEX** para BMO usando `bmo_abi` como contrato. BEX v1 usa el formato binario
BEF1 canónico; `.bex` es la extensión pública de un programa ejecutable.

## Regla de arquitectura

- El kernel/Ring 0 no compila lenguajes.
- Los frontends viven aquí como tooling offline.
- La salida canónica ejecutable es BEX (`.bex`), codificado como BEF1 en v1.
- `bmo_abi` define perfiles, tipos, syscalls y runtime contracts.

## Frontends previstos

- `c/`: frontend C hacia BEF/BMO ABI. Los avances históricos siguen restaurados
  en `kernel/src/Temporal/lang/` para migrarlos con calma.
- `cobol/`: frontend COBOL hacia BEF/BMO ABI. Debe apuntar primero a CPU/AOT:
  decimal fijo, records, `DISPLAY`, `ACCEPT`, FS y procesos.

## COBOL actual

`cobol/` contiene el primer frontend offline. Por ahora parsea un subconjunto
mínimo (`PROGRAM-ID`, `DISPLAY`, `ACCEPT`, `MOVE`, `STOP RUN`) y emite un BEX
validado usando `bmo_abi::bef::writer`, sin meter el compilador en Ring 0.

GPU queda fuera de esta fase; COBOL y C arrancan sobre CPU y syscalls BMO.
