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

GPU queda fuera de esta fase; COBOL y C arrancan sobre CPU y syscalls BMO.
