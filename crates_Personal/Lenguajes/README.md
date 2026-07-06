# Lenguajes FastOS

Este directorio agrupa frontends y tooling de lenguajes que generan BEF para
FastOS usando `bmo_abi` como contrato.

## Regla de arquitectura

- El kernel/Ring 0 no compila lenguajes.
- Los frontends viven aquí como tooling offline.
- La salida canónica es BEF.
- `bmo_abi` define perfiles, tipos, syscalls y runtime contracts.

## Frontends previstos

- `c/`: frontend C hacia BEF/BMO ABI. Los avances históricos siguen restaurados
  en `kernel/src/Temporal/lang/` para migrarlos con calma.
- `cobol/`: frontend COBOL hacia BEF/BMO ABI. Debe apuntar primero a CPU/AOT:
  decimal fijo, records, `DISPLAY`, `ACCEPT`, FS y procesos.

## COBOL actual

`cobol/` contiene el primer frontend offline. Por ahora parsea un subconjunto
mínimo (`PROGRAM-ID`, `DISPLAY`, `ACCEPT`, `MOVE`, `STOP RUN`) y emite un IR
textual orientado a BMO. El siguiente paso es sustituir ese emisor por BEF real
usando `bmo_abi::bef::writer`, sin meter el compilador en Ring 0.

GPU queda fuera de esta fase; COBOL y C arrancan sobre CPU y syscalls BMO.
