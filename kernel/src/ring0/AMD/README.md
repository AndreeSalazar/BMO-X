# AMD/ — Documentación técnica del Ryzen 5 5600X

> **Carpeta:** `kernel/src/ring0/AMD/`
> **Propósito:** referencia técnica profunda del CPU target de FastOS.
> El Ryzen 5 5600X (Vermeer, Zen 3, Family 19h Model 01h) es el
> **único CPU soportado** en esta versión. Todo el código en
> `ring0/amd64/`, `ring0/cpu_init/` y `ring0/memory_management/` está
> optimizado para este microprocesador.

## 📚 Contenido

| Archivo | Contenido | Líneas |
|---|---|---|
| [`ryzen_5_5600x.md`](./ryzen_5_5600x.md) | Documento principal. CPUID, MSRs, APIC, paging, TSC, P-states, erratas, etc. | 1832 |
| [`errata.md`](./errata.md) | Tabla de erratas del 5600X relevantes para kernel (workarounds) | (en construcción) |
| [`boot_sequence.md`](./boot_sequence.md) | Secuencia completa de arranque sobre el 5600X (reset → kernel) | (en construcción) |
| [`glossary.md`](./glossary.md) | Glosario de términos (CCX, CCD, TAGE, Op Cache, etc.) | (en construcción) |

## 🎯 Cuándo consultar esta carpeta

- **Añadir un nuevo driver** → consulta sección 9 (Local APIC) y 14 (MTRR/PAT)
- **Implementar scheduling SMP** → consulta secciones 2 (topología) y 3 (microarquitectura)
- **Optimizar TSC/timers** → consulta sección 12 (TSC) y 13 (P-states)
- **Debuguear triple faults** → consulta sección 8 (excepciones) y 15 (erratas)
- **Implementar paging avanzado** → consulta sección 7 (paging) y apéndice B (mapa de memoria)
- **Portar a otro CPU AMD** → consulta sección 16 (comparación Zen 2/3/4) y §1 (identificación)

## 🔬 Cobertura del documento principal

El documento `ryzen_5_5600x.md` cubre en profundidad:

1. **Identificación del CPU** (Family 19h, Model 01h, stepping)
2. **Topología física y SMT** (1 CCD, 6C/12T, 3.7/4.6 GHz)
3. **Microarquitectura Zen 3** (4-wide decode, 256-entry renamer, TAGE predictor, 6-wide retire)
4. **CPUID leaves importantes** (0x00000001, 0x00000007, 0x80000001, 0x8000001D, 0x8000001E, etc.)
5. **Ordenamiento de memoria TSO débil** (loads pueden reordenar con stores a direcciones distintas)
6. **Cache, TLB y coherencia** (32 KB L1, 512 KB L2, 32 MB L3 victim, INVLPGB, PCID)
7. **Paging y memoria virtual** (4-level, 4KB/2MB/1GB, no LA57)
8. **Excepciones e IDT** (256 entries, IST, DPL, gate types)
9. **Local APIC** (TSC-deadline mode, ICR para SMP)
10. **MSRs fundamentales** (EFER, STAR, LSTAR, GS_BASE, APIC_BASE, MTRR_*, PAT, etc.)
11. **SYSCALL/SYSRET ABI** (AMD64, rdi/rsi/rdx/r10/r8/r9, return en rax)
12. **TSC y timers** (no es invariant TSC en Zen 3 — varía con P-state)
13. **P-states, C-states y boost** (Precision Boost 2, STAPM/PPT/TDC/EDC)
14. **MTRR y PAT** (8 pairs MTRR, 8-entry PAT)
15. **Erratas relevantes** (con workarounds)
16. **Zen 3 vs Zen 2 vs Zen 4** (compatibilidad y diferencias)
17. **Recursos oficiales** (AMD APM, AMD64 ABI, etc.)
18. **Practical notes for kernel development** (cheatsheet, pitfalls, prioridades)

## 📋 Fuentes de información

- **AMD Architecture Programmer's Manual (APM) Vol 1, 2, 3** — `developer.amd.com`
- **AMD64 ABI** (System V AMD64) — `refspecs.linuxfoundation.org/elf/x86-64-abi-0.99.pdf`
- **Wikipedia** — Zen 3, Ryzen 5 5600X, CPUID, Memory ordering, MTRR, PAT, APIC
- **AnandTech / Chips and Cheese** — análisis de microarquitectura
- **datasheets de AMD** — BKDG para Family 19h (algunos son NDA)

## ⚠️ Política de uso

Esta documentación es **interna del proyecto**. Si en el futuro se portea
FastOS a otro CPU, no reutilizar este directorio tal cual: cada CPU
necesita su propia carpeta (e.g. `AMD/`, `INTEL/`, `ARM/`). La idea es
que `AMD/` describa **exclusivamente** el 5600X sin contaminarse con
generalizaciones de otros CPUs.

---

**Última actualización:** ver `git log AMD/`
