# FastOS Scheduler Specification
**Capa:** Kernel (Ring 0)
**Prioridad:** CRÍTICA
**Depende de:** `FastOS_Hardware_Timers_Spec.md`, `FastOS_Memory_Manager_Spec.md`
**Inspiración:** Windows `ETHREAD`/`KTHREAD`, Linux `CFS`, seL4 Scheduler.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
El Scheduler dicta qué hilo de ejecución recibe tiempo en la CPU.
- **Estructuras Clave:** Windows utiliza `EPROCESS` (Process) y `KTHREAD` / `ETHREAD` (Thread). Linux unifica esto bajo `task_struct`.
- **Qué conservamos:** El concepto de prioridades estrictas, Time Quantums (Milisegundos de ejecución permitidos), y el manejo de colas de ejecución por cada núcleo físico (SMP).
- **Qué tiramos:** El peso masivo del `task_struct` de Linux (que tiene cientos de campos) y la complejidad de Windows con APCs asíncronos encolados por todas partes. BMO necesita estructuras espartanas para lograr *Context Switches* ultrarrápidos.

---

## FASE 2: Diseño BMO Nativo

Para nuestro target (**AMD Ryzen 5 5600X** de 6 núcleos físicos / 12 lógicos), el Scheduler debe ser SMP (Symmetric Multiprocessing) Aware. Cada núcleo lógico tendrá su propia cola de tareas (`RunQueue`) para evitar bloqueos por Spinlocks globales.

### Las Estructuras en Rust

```rust
// bmo_scheduler.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoPriority {
    Idle = 0,
    Normal = 1,
    Realtime = 2,
    KernelCritical = 3, // Reservado para handlers del Kernel y GSP IRQs
}

#[derive(Debug, PartialEq, Eq)]
pub enum ThreadState {
    Ready,          // Esperando CPU
    Running,        // Ejecutándose actualmente
    Blocked,        // Esperando I/O o Mutex
    Terminated,     // Zombie
}

/// Equivalente hiper-optimizado del KTHREAD de Windows
#[repr(C)]
pub struct BmoThread {
    pub id: u64,
    
    // El puntero al stack del Kernel (Donde están salvados los registros x86-64)
    // ESTE CAMPO DEBE SER EL PRIMERO PORQUE SE LEE DESDE ENSAMBLADOR
    pub kernel_rsp: u64, 
    
    // El Page Table (PML4) para el Context Switch de memoria
    pub cr3_phys_addr: u64, 
    
    pub state: ThreadState,
    pub priority: BmoPriority,
    pub quantum_remaining: u32,  // Ticks que le quedan
    pub cpu_core_id: u8,         // Afinidad: a qué núcleo SMP pertenece
}

/// Cola de Ejecución Per-Core (No global)
pub struct CoreRunQueue {
    pub current_thread: Option<*mut BmoThread>,
    // Colas por prioridad (Round-Robin simple para la v1)
    pub ready_queues: [Vec<*mut BmoThread>; 4], 
}
```

---

## FASE 3: Implementación (Pseudocódigo Rust)

### El Context Switch (Hardware Level)
Cuando el APIC dispara la `IRQ0`, el Hardware llama al handler de interrupciones, el cual eventualmente invoca a `bmo_schedule_next()`.

```rust
/// Algoritmo Round-Robin con prioridades estrictas
pub fn bmo_schedule_next(current_rsp: u64) -> u64 {
    let core_id = get_current_core_id(); // Leído del Local APIC ID
    let mut run_queue = KERNEL_RUN_QUEUES[core_id].lock();
    
    // 1. Guardar el estado del hilo actual
    if let Some(mut current) = run_queue.current_thread {
        unsafe { (*current).kernel_rsp = current_rsp; }
        
        if unsafe { (*current).state } == ThreadState::Running {
            unsafe { (*current).state = ThreadState::Ready; }
            let prio = unsafe { (*current).priority as usize };
            run_queue.ready_queues[prio].push(current);
        }
    }
    
    // 2. Elegir el siguiente hilo (Prioridad más alta primero)
    let next_thread = pick_next_thread(&mut run_queue);
    
    // 3. Restaurar el nuevo estado
    unsafe {
        (*next_thread).state = ThreadState::Running;
        run_queue.current_thread = Some(next_thread);
        
        // Cargar el espacio de memoria del nuevo proceso (ASLR/VRAM context)
        reload_cr3((*next_thread).cr3_phys_addr);
        
        // Retornar el nuevo Kernel RSP para que el ASM haga el 'pop' de los registros
        (*next_thread).kernel_rsp
    }
}
```

### ASM del Switch (x86-64)
```nasm
; context_switch.asm
; Llamado desde la IDT (IRQ0)
irq0_timer_handler:
    ; Salvar registros generales (15 registros x86-64)
    push r15
    push r14
    ...
    push rdi

    ; Pasamos el RSP actual como argumento a la función en Rust
    mov rdi, rsp
    call bmo_schedule_next
    
    ; La función en Rust retorna en RAX el RSP del SIGUIENTE hilo.
    ; ¡Aquí ocurre la magia del cambio de contexto!
    mov rsp, rax

    ; Restauramos los registros del nuevo hilo
    pop rdi
    ...
    pop r14
    pop r15

    iret ; Retornar de la interrupción (Salta a Ring 3 u otro código Ring 0)
```

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `FastOS_Hardware_Timers_Spec.md`:** El APIC Timer genera la interrupción que dispara el archivo `.asm` que inicia el Context Switch. Sin el timer, la función `bmo_schedule_next` nunca se ejecutaría, y el sistema no tendría multitarea.
- **Conexión con `FastOS_Memory_Manager_Spec.md`:** La invocación a `reload_cr3()` realiza la recarga estricta de la Tabla de Páginas para cambiar el aislamiento virtual al proceso entrante.
- **Conexión con `FastOS_Syscall_Table_Spec.md`:** Implementa indirectamente la Syscall `sys_yield` (ID `0x03`), la cual fuerza manualmente el disparo de esta rutina sin esperar a que el APIC Timer termine su Quantum.

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows:**
Al hacer que el campo `kernel_rsp` sea el primer valor absoluto de la estructura `BmoThread`, eliminamos el cálculo de offsets dinámicos en el ensamblador crítico. Hemos diseñado una arquitectura O(1) de *RunQueues* separadas por núcleo, evitando la contención de memoria que Windows sufría en el pasado con su dispatcher lock central. El *Context Switch* de BMO es salvajemente minimalista: salvar registros, cambiar puntero CR3, cambiar RSP y restaurar registros. Cero código obsoleto.
