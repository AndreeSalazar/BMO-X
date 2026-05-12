# FastOS Hardware Timers Specification
**Capa:** Kernel (Ring 0)
**Prioridad:** CRÍTICA
**Depende de:** `El_Cerebro_Hardware_PCIe_APIC.md`
**Inspiración:** Windows `KeQueryPerformanceCounter`, Linux `hrtimers`.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
El tiempo es la base del *multitasking*. Sin un temporizador que interrumpa físicamente a la CPU, un bucle infinito `while(true)` congelaría toda la computadora.
- **Estructuras Clave (Windows/Linux):** Windows utiliza una mezcla compleja de `RTC` (obsoleto), `PIT` (8253, de los años 80), `HPET` (High Precision Event Timer) y el `APIC Timer` local integrado en la CPU.
- **Qué conservamos:** El uso del `APIC Timer` como generador de interrupciones (tick del Scheduler) y el `TSC` (Time Stamp Counter) para medir nanosegundos.
- **Qué tiramos:** El `PIT` legacy (i8253) y el `RTC` (Real Time Clock). FastOS confía plenamente en arquitecturas x86-64 modernas.

---

## FASE 2: Diseño BMO Nativo

Para el target de hardware **AMD Ryzen 5 5600X**, la calibración del tiempo se basa en dos piezas maestras:
1. **TSC (Time Stamp Counter) Invariante:** El Ryzen 5600X garantiza que la instrucción `rdtsc` cuenta ciclos a una velocidad constante, sin importar la frecuencia Turbo/P-States del procesador. Es la fuente definitiva para timestamps de alta precisión.
2. **Local APIC Timer:** El temporizador interno del núcleo de la CPU que dispara la interrupción `IRQ0` para el cambio de contexto.

```rust
// bmo_timers.rs

/// Representación del TSC Invariante de AMD
pub struct InvariantTsc {
    pub frequency_hz: u64,
}

/// Descubrimiento ACPI del HPET (High Precision Event Timer)
#[repr(C, packed)]
pub struct HpetAcpiTable {
    pub header: AcpiSdtHeader,
    pub hardware_rev_id: u8,
    pub comparator_count: u8, // Bits 0-4
    pub pci_vendor_id: u16,
    pub base_address: u64, // Dirección física MMIO
}

pub enum FastOsTimerSource {
    Hpet(u64), // Dirección base MMIO
    Apic,
    Unknown,
}
```

---

## FASE 3: Implementación (Pseudocódigo Rust)

### 1. Calibración del APIC Timer usando TSC
En *bare metal*, nadie calibra el timer por nosotros. Tenemos que decirle al APIC cuántos ticks equivalen a 1 milisegundo.

```rust
use core::arch::asm;

/// Lee el contador de ciclos del procesador
#[inline]
pub fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { asm!("rdtsc", out("eax") low, out("edx") high) };
    ((high as u64) << 32) | (low as u64)
}

/// Calibra el APIC usando el HPET (si existe) o un delay rudimentario como fallback
pub fn calibrate_apic_timer(hpet_base: Option<u64>) -> u32 {
    let start_tsc = rdtsc();
    
    // Configuramos el APIC Timer con el valor máximo inicial
    local_apic_write(APIC_TMR_INIT_CNT, 0xFFFFFFFF);
    
    // Esperamos exactamente 10 milisegundos
    if let Some(hpet) = hpet_base {
        hpet_delay_ms(hpet, 10);
    } else {
        pit_legacy_delay_ms(10); // Fallback extremo
    }
    
    let ticks_elapsed = 0xFFFFFFFF - local_apic_read(APIC_TMR_CUR_CNT);
    let end_tsc = rdtsc();
    
    // Calculamos los Ticks por Milisegundo para el Scheduler
    let ticks_per_ms = ticks_elapsed / 10;
    
    // Guardar para el sistema de Profiling
    KERNEL_TSC_FREQ.store((end_tsc - start_tsc) * 100); 
    
    ticks_per_ms
}
```

### 2. Programar la Interrupción `IRQ0` para el Scheduler
```rust
/// Configura el APIC para dispararse periódicamente
pub fn start_scheduler_tick(ticks_per_ms: u32, quantum_ms: u32) {
    let ticks_for_quantum = ticks_per_ms * quantum_ms;
    
    // APIC Timer en modo Periódico, asociado al vector de interrupción 0x20 (IRQ0)
    local_apic_write(APIC_TMR_LVT, 0x20 | APIC_TIMER_PERIODIC);
    local_apic_write(APIC_TMR_DIVIDE, 0x03); // Divide by 16
    local_apic_write(APIC_TMR_INIT_CNT, ticks_for_quantum);
}
```

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `FastOS_Scheduler_Spec.md`:** La función `start_scheduler_tick()` configura el hardware para saltar al vector `0x20` de la IDT. Cuando eso pasa, el hardware guarda el contexto y llama a la función `schedule()` de FastOS.
- **Conexión con `FastOS_Syscall_Table_Spec.md`:** Provee el backend para futuras syscalls como `sys_nanosleep`, usando el reloj hiperpreciso del TSC Invariante del Ryzen 5600X.
- **Conexión con `BEF_Executable_Format_Spec.md`:** Los programas BEF pueden usar la instrucción `rdtsc` directamente desde Ring 3 para medir rendimiento (si los flags del CR4 lo permiten), o usar la Syscall.

---

## Conclusión

**Qué aprendimos y mejoramos:**
En lugar de depender de capas de compatibilidad (`HAL`) masivas como Windows que intentan soportar procesadores de hace 25 años, FastOS apunta directamente a hardware moderno. Aprovechamos el Invariant TSC de AMD para evitar derivas de tiempo (time drifts) causadas por el throttling térmico de la CPU. Logramos un tick para el Scheduler extremadamente predecible programando directamente el *Local APIC* del núcleo de la CPU.
