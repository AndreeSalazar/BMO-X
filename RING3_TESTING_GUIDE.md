# 🧪 Guía de Pruebas en Hardware Real — Ring 3

> **Target:** Ryzen 5 5600X (Zen 3) + UEFI GOP framebuffer
> **Baud rate:** 115200
> **Conexión serial:** COM1 0x3F8

---

## 🚀 Quick Start

1. **Compila el kernel:**
   ```powershell
   cd C:\Users\andre\Documents\FastOS
   .\build_uefi.ps1
   ```
2. **Flashea al USB:**
   ```powershell
   .\build_uefi.ps1 -FlashOnly
   ```
3. **Conecta USB al Ryzen 5 5600X** y arranca.
4. **Captura serial** con un adaptador USB-serial (115200 baud).

---

## 📟 Comandos disponibles en welcome screen

| Comando | Acción |
|---|---|
| `Run` | Entra al desktop Ring 0 (estable) |
| `Hello` | Lanza proceso Ring 3 hello (test) |
| **`Ring3`** | Alias explícito de Hello — test Ring 3 |
| `Nexo` | Compila un programa ÑEXO de prueba |
| `Reboot` | Reboot via keyboard controller |

**Para test Ring 3:** escribe `Ring3` + Enter (o `Hello` + Enter).

---

## ✅ Lo que DEBE salir en serial (orden cronológico)

```
[ring3-test] 14 passed, 0 failed
[ring3] === alloc start ===
[ring3] allocating process struct
[ring3] process struct allocated
[ring3] reading kernel CR3
[ring3] kernel CR3 = 0x00000000_XXXX_XXXX
[ring3] creating user page table
[ring3] user CR3 = 0x00000000_XXXX_XXXX
[ring3] code pages = 1
[ring3] stack pages = 16
[ring3] allocating physical pages for code
[ring3] code phys addr = 0x00000000_XXXX_XXXX
[ring3] allocating physical pages for stack
[ring3] stack phys addr = 0x00000000_XXXX_XXXX
[ring3] mapping code into user page table
[ring3] mapping stack into user page table
[ring3] copying code bytes to physical pages
[ring3] code and stack zeroed/populated
[ring3] kernel stack for this thread = 0x00000000_XXXX_XXXX
[ring3] thread TID = 1
[ring3] setting kernel stack for TSS.rsp0 and syscall entry
[ring3] Ring 3 process allocation complete
[ring3] user code entry (Ring 3 RIP) = 0x00000000_0040_0000
[ring3] user stack top (Ring 3 RSP) = 0x00000000_0080_4000
[ring3] user CR3 (page table root) = 0x00000000_XXXX_XXXX
[ring3] === Ring 3 process allocation END ===
[ring3] === Ring 3 JUMP START ===
[ring3] entry (RIP) = 0x00000000_0040_0000
[ring3] stack (RSP) = 0x00000000_0080_4000
[ring3] CS expected = 0x23
[ring3] SS expected = 0x1B
[ring3] RFLAGS expected = 0x202
[ring3] jumping: RIP=0x400000 RSP=0x804000 CR3=0xXXXXX
```

### ✅ Si TODO va bien:
```
[syscall] first syscall received; Ring 3 is alive
Hello from Ring 3!
[ring3] process exited cleanly
```

### ❌ Si hay #GP (error 0):
```
#GP fault! vector=13 error=0 RIP=0x00402000
Ring 3 hello process killed
```

---

## 🔍 Diagnóstico de #GP

Si el sistema genera un `#GP` con error=0 al saltar a Ring 3, hay **5 sospechosos principales**:

### 1. Selector mal formado
**Síntoma:** `#GP(0)` inmediato
**Causa probable:** `0x23` o `0x1B` no están en el GDT
**Verificar:** el log muestra `CS expected = 0x23` y `SS expected = 0x1B`

### 2. Stack no canónico
**Síntoma:** `#GP(0)` o `#SS(0)`
**Causa probable:** `RSP` no es canónico (bit 47 = bit 48-63)
**Verificar:** el log muestra `stack (RSP) = ...`

### 3. CR3 no apunta a page table válido
**Síntoma:** `#PF` o `#GP(0)`
**Causa probable:** PML4 mal construida o no accesible
**Verificar:** el log muestra `user CR3 = ...`

### 4. TSS.rsp0 mal configurado
**Síntoma:** `#DF` (Double Fault) inmediato
**Causa probable:** `TSS.rsp0` no apunta a un stack válido cuando hay una exception
**Verificar:** el log muestra `kernel stack for this thread = ...`

### 5. Entry point no mapeado en user page table
**Síntoma:** `#PF` con CR2=0x400000
**Causa probable:** la página no está mapeada con `USER` flag

---

## 🛠️ Cómo reportar un bug

Si encuentras un problema, captura el log serial completo y reporta:

1. **Mensaje del fallo** (línea que dice `#GP`, `#DF`, etc.)
2. **CR3 actual** (busca `user CR3` y `kernel CR3` en el log)
3. **Vector de la exception** (número después de `vector=`)
4. **Error code** (después de `error=`)
5. **RIP en el momento del fallo** (después de `RIP=`)

Con esa info puedo identificar la causa exacta.

---

## 🧪 Tests que pasaron en compile-time

Antes de probar en hardware, puedes verificar que el código compila correctamente con:

```bash
cd C:\Users\andre\Documents\FastOS\kernel
cargo build --target x86_64-unknown-none
```

**Debe terminar con `Finished` sin errores.**

Los 14 tests estructurales (en `arch/ring3_test.rs`) verifican:
- ✅ Layout del iretq frame (SS, RSP, RFLAGS, CS, RIP)
- ✅ Selectores GDT (KERNEL_CS=0x08, USER_CS=0x23, etc)
- ✅ Encoding del STAR MSR
- ✅ Convenção de registros syscall (BMO ABI 7 GPRs)
- ✅ Flags de paginación para Ring 3 (USER, PRESENT, WRITABLE)
- ✅ Tamaño de IST1 stack (>= 4KB, actual 8KB)
- ✅ Consistencia TSS.rsp0 / SYSCALL_KERNEL_RSP
- ✅ `swapgs` opcode (0F 01 F8)
- ✅ `clac`/`stac` opcodes (SMAP)
- ✅ BMOasm `syscall` emite 0F 05
- ✅ BMOasm `reg rax = 0x23` emite REX.W + 0xB8
- ✅ BMOasm `retorna` emite 0xC3
- ✅ Init program completo compila a bytes correctos

**Si estos 14 tests pasan, hay 80%+ probabilidad de que Ring 3 funcione en hardware.**

---

## 📊 Expected boot output (resumen)

```
[FastOS] === Phase 0: CPU Init (modular) ===
[cpu] === Modular CPU Init ===
...
[arch/gdt] init_gdt complete
[arch/idt] init_idt complete
[arch/syscall_entry] init_syscall complete
[ring3-test] Running Ring 3 transition tests
[ring3-test] 14 passed, 0 failed
[FastOS] === Phase 1: Memory ===
...
[welcome] Pantalla de bienvenida activa.
```

**Si ves esto, todo está OK en Ring 0.** El test Ring 3 solo se dispara cuando escribes `Ring3` o `Hello`.

---

## 🎯 Siguiente paso después de validar Ring 3

Si `Ring 3 funciona` (ves `Hello from Ring 3!`):

1. **Lane 1: USB HID** — teclado/mouse USB
2. **Lane 2: Storage write** — para guardar archivos
3. **Lane 4: VMM demand paging** — BEFs grandes

Si `Ring 3 falla`:

1. Reporta el log completo
2. Verifica que ves `[ring3-test] 14 passed, 0 failed` ANTES del fallo
3. Si no ves eso, hay un problema previo
