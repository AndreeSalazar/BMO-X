# FastOS Locking Primitives Specification
**Capa:** Kernel (Ring 0)
**Prioridad:** CRÍTICA
**Depende de:** `FastOS_Scheduler_Spec.md`
**Inspiración:** Windows `KSPIN_LOCK` / `KMUTEX`, Linux `spinlock_t`.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Cuando tienes múltiples núcleos de CPU (SMP, como el Ryzen 5600X) intentando escribir en el mismo pedazo de memoria, necesitas exclusión mutua.
- **Estructuras Clave (Windows):** Windows desarrolló toda una jerarquía compleja (`IRQL` - Interrupt Request Level) asociada a los locks. Los Spinlocks elevan el `IRQL` a `DISPATCH_LEVEL` para evitar que el Scheduler interrumpa al núcleo mientras tiene el lock.
- **Qué conservamos:** La naturaleza implacable del Spinlock de hardware (Usando el bus lock prefix de x86-64) y el concepto del Mutex durmiente (que cede la CPU si el recurso está ocupado).
- **Qué tiramos:** La pesadilla conceptual de los `IRQLs` de Windows (que provocan las famosas Blue Screens `IRQL_NOT_LESS_OR_EQUAL`). FastOS utiliza el modelo de "Interrupt Disable" local para emular este comportamiento de forma más segura y pura.

---

## FASE 2: Diseño BMO Nativo

Para evitar cuellos de botella en la comunicación entre el CPU Ryzen y la GPU RTX 3060, FastOS confía plenamente en el soporte de atómicos nativo de Rust (`core::sync::atomic`) emparejado con las instrucciones de barrera del hardware de AMD.

### 1. El Spinlock Básico
Para proteger estructuras muy rápidas (Ej. el `CoreRunQueue` del Scheduler o la tabla de mapeo PCIe). Un spinlock "gira" en un bucle quemando CPU hasta que obtiene el recurso. Jamás debe usarse para I/O (ej. Disco duro).

```rust
// bmo_sync.rs
use core::sync::atomic::{AtomicBool, Ordering};
use core::arch::asm;

pub struct BmoSpinlock {
    locked: AtomicBool,
}

impl BmoSpinlock {
    pub const fn new() -> Self {
        Self { locked: AtomicBool::new(false) }
    }

    /// Adquiere el lock. Deshabilita interrupciones locales para evitar deadlocks de Scheduler.
    pub fn lock(&self) {
        unsafe { asm!("cli") }; // Clear Interrupts (Equivalente al Raise IRQL de Windows)
        
        // La instrucción XCHG (hardware x86) hace esto atómicamente
        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // Hint a la CPU de AMD de que estamos en un spinloop para ahorrar energía y evitar sobrecalentamiento
            unsafe { asm!("pause") }; 
        }
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
        unsafe { asm!("sti") }; // Set Interrupts (Restaurar)
    }
}
```

### 2. El Mutex Durmiente (Sleeping Mutex)
Si proteger un recurso toma más de 10 microsegundos, quemar CPU es un desperdicio. El Mutex bloquea al hilo y cede la CPU al Scheduler.

```rust
use crate::scheduler::{BmoThread, ThreadState};

pub struct BmoMutex {
    state: AtomicBool,
    // Lista de hilos esperando este lock (Protegida por un spinlock interno)
    wait_queue: BmoSpinlock<Vec<*mut BmoThread>>, 
}

impl BmoMutex {
    pub fn lock(&self) {
        // Intento rápido (Fast Path)
        if self.state.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            return; 
        }

        // Intento lento (Slow Path): El lock está tomado. Bloqueamos este hilo.
        let mut queue = self.wait_queue.lock();
        let current_thread = get_current_thread();
        
        unsafe { (*current_thread).state = ThreadState::Blocked; }
        queue.push(current_thread);
        drop(queue); // Liberar spinlock antes de dormir
        
        // CEDER LA CPU. El Scheduler de FastOS (DOC-03) tomará el control aquí.
        sys_yield_internal();
    }
}
```

---

## FASE 3: Implementación (Prevención de Deadlocks)

### El Diseño "Wait-Free" para el GSP
Uno de los problemas más graves al escribir drivers de GPU (como para el GSP de NVIDIA) es que el hardware es asíncrono. Si bloqueamos la CPU esperando a la GPU con un Spinlock, el OS colapsa.
Para evitar esto, las colas DMA hacia el Falcon no usarán locks tradicionales, usarán **Ring Buffers Atómicos**:

```rust
/// Comunicación Lock-Free hacia el GSP
pub struct LockFreeRingBuffer {
    pub head: AtomicU32,
    pub tail: AtomicU32,
    pub buffer: [u8; 4096],
}

impl LockFreeRingBuffer {
    pub fn push(&self, data: u8) -> Result<(), BmoError> {
        let mut current_tail = self.tail.load(Ordering::Relaxed);
        loop {
            let next_tail = (current_tail + 1) % 4096;
            if next_tail == self.head.load(Ordering::Acquire) {
                return Err(BmoError::BufferFull);
            }
            // Intento atómico de insertar
            match self.tail.compare_exchange_weak(current_tail, next_tail, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => {
                    self.buffer[current_tail as usize] = data;
                    return Ok(());
                }
                Err(real_tail) => current_tail = real_tail, // Colisión, intentar de nuevo
            }
        }
    }
}
```

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `FastOS_Scheduler_Spec.md`:** El Mutex es literalmente un cliente del Scheduler. Cuando un Mutex no se puede adquirir, altera el campo `ThreadState` del hilo a `Blocked` y fuerza una recarga de la cola de prioridades.
- **Conexión con `FastOS_Hardware_Timers_Spec.md`:** El Spinlock ejecuta `cli` (Clear Interrupts), lo que previene temporalmente que el `IRQ0` del APIC Timer interrumpa al procesador mientras se está escribiendo en memoria crítica.
- **Conexión con el GSP / Gráficos (`BMO_Graphics_Layer.md`):** La estructura `LockFreeRingBuffer` definida arriba será la base arquitectónica para construir el puente `MSG_INIT` hacia la NVIDIA RTX 3060.

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows:**
Hemos adoptado un modelo híbrido brutalmente simple. Nos libramos del complejo cálculo de `IRQL` de Windows usando el enfoque directo de Rust + Hardware Locks. La regla fundamental en Ring 0 de BMO es: "Si tocas el Hardware (Page Tables, APIC), usas Spinlock con `cli`. Si tocas una estructura de datos abstracta de alta latencia, usas un Mutex durmiente apoyado en el Scheduler". Además, las arquitecturas *Lock-Free* sientan las bases para un rendimiento altísimo hacia el bus PCIe.
