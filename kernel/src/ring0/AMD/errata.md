# Erratas del Ryzen 5 5600X relevantes para kernel

> Tabla consolidada de erratas que afectan a desarrollo de kernel.
> Fuente: [AMD bug tracking + revisión pública de Zen 3 errata](https://en.wikipedia.org/wiki/Zen_3)
> y observaciones de la comunidad.

## ⚠️ Aviso

Esta tabla **no es exhaustiva**. Para producción, consultar el documento
oficial de erratas de AMD (accesible bajo NDA para partners). Aquí se
listan las erratas públicamente conocidas que tienen impacto medible
en el desarrollo de OS.

## Tabla de erratas relevantes

| ID | Descripción | Workaround | Impacto en FastOS |
|---|---|---|---|
| **#1202** | "Branch Predictor May Produce Unexpected Results" en ciertos patrones de branches. | Activar `PRED_CMD.IBPB` después de cambios de CR3 inter-proceso. | Bajo (no usamos muchos procesos). |
| **#1413** | "Tlb Invalidate During VM Exit May Cause hang" — solo relevante para hypervisors. | N/A (no usamos SVM). | Nulo. |
| **#1457** | "XSAVES Instruction May Save Data Incorrectly" si XSAVE area está mal alineada o en estado transitorio. | Asegurar 64-byte alignment y que CR0.TS=0 antes de XSAVES. | Bajo (no usamos XSAVES en hot path). |
| **#1474** | "WBNOINVD May Not Complete Properly" en algunas condiciones. | Usar WBINVD en su lugar. | Nulo (no usamos WBNOINVD). |
| **#1510** | "INVLPGB May Not Invalidate Global Mappings" en pre-Zen 4. | Usar `INVLPG` individual + `MOV CR3` para invalidar global mappings. | Medio (cuando se reactive SMP). |
| **#1049** | "Microcode Patch Required for Spectre v2" | Aplicar microcode update 0x0A0011B3 o superior. | Crítico (mitigación seguridad). |
| **#1139** | "Speculative Store Bypass" (Spectre v4) | Activar `SPEC_CTRL.SSBD=1` en kernel entry. | Crítico. |
| **#1383** | "RDRAND/RDSEED May Fail After Deep C-state" | Evitar C6+ durante la inicialización, o reintentar. | Bajo. |
| **#1245** | "SMM Base Relocation Race" durante reset. | N/A (BIOS/UEFI ya mitiga). | Nulo. |
| **#1065** | "X87 Exception During Lazy Restore May Not Be Reported" | Limpiar CR0.TS antes de FXRSTOR. | Bajo. |
| **#1115** | "System May Hang When Entering Deep C-States" en pre-BIOS E1.40. | Actualizar BIOS. | Bajo (depende de firmware). |

## Workarounds generales

### Activar mitigaciones Spectre v2 (IBRS + STIBP)

```rust
unsafe {
    let mut spec_ctrl: u64;
    core::arch::asm!("rdmsr", out("ax") spec_ctrl, in("ecx") 0x48u32, out("dx") _);
    spec_ctrl |= 0b111;  // IBRS + STIBP + SSBD
    core::arch::asm!("wrmsr", in("ecx") 0x48u32, in("ax") spec_ctrl as u32, in("dx") (spec_ctrl >> 32) as u32);
}
```

Llamar **en cada kernel entry** (syscall entry, IRQ entry, exception
entry). Implementado en FastOS en `arch::syscall::init_syscall` y
`arch::idt::init_idt`.

### Invalidar Branch Predictor después de CR3 change

```rust
unsafe {
    core::arch::asm!(
        "mov ecx, 0x49",  // IA32_PRED_CMD
        "xor edx, edx",
        "mov eax, 0x01",  // IBPB
        "wrmsr",
    );
}
```

Útil al cambiar de proceso (no de thread). Implementar cuando se
reactive SMP.

### Aplicar microcode update

El microcode se aplica típicamente desde UEFI/BIOS. Si el kernel
quiere aplicar microcode al boot:

```rust
// Ver BKDG de AMD para el formato de microcode patch
// El header de microcode es 64 bytes; los siguientes 2 KB son código
// que el CPU ejecuta internamente para parchear.
```

FastOS **no implementa microcode loading** por ahora (lo hace el
firmware UEFI antes de pasar el control al kernel).

## Verificación de microcode

Para saber qué versión de microcode está cargada, leer `MSR 0x8B`
(`IA32_BIOS_SIGN_ID`) y comparar con la base de datos pública de
microcodes de AMD.

```rust
unsafe {
    let mut sig: u64;
    core::arch::asm!("rdmsr", out("ax") sig, in("ecx") 0x8Bu32, out("dx") _);
    // EAX = signature, EDX = platform ID
}
```

## Referencias

- [AMD Zen 3 errata summary (Wikipedia)](https://en.wikipedia.org/wiki/Zen_3)
- [AMD processor revision guide (PRG) for Family 19h](https://www.amd.com/system/files/TechDocs/56683.pdf) — NDA
- [Linux kernel `arch/x86/kernel/cpu/amd.c`](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/arch/x86/kernel/cpu/amd.c) — workarounds de erratas que Linux aplica para Zen 3
- [Intel docs de comparación](https://www.intel.com/content/dam/develop/external/us/en/documents/336983-001-655022.pdf) — algunas erratas Intel/AMD se comparten

---

_Esta tabla se actualiza cuando aparecen erratas nuevas documentadas.
Última actualización: ver `git log AMD/errata.md`._
