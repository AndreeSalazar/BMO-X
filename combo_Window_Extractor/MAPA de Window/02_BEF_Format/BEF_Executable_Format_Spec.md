# BMO Executable Format (BEF) Specification v1.1
**Target OS:** FastOS (Bare Metal Orchestrator)
**Architecture:** x86-64 Puro / UEFI Boot
**Design Philosophy:** Cero Legacy, Ultra-Permisivo, Dinámico, Rust-Friendly.

---

## FASE 1: Anatomía del PE64 (Extracción de ADN)
El formato de Microsoft (PE) arrastra 40 años de compatibilidad hacia atrás. Vamos a destripar `winnt.h` para saber qué ignorar en el diseño de BMO.

### 1. `IMAGE_DOS_HEADER` (La Reliquia de 1981)
```c
typedef struct _IMAGE_DOS_HEADER { 
    WORD e_magic;    // [TIRAR] "MZ"
    WORD e_cblp;     // [TIRAR] Bytes on last page of file
    // ... 12 campos inútiles omitidos ...
    LONG e_lfanew;   // [REEMPLAZAR] File address of new exe header
} IMAGE_DOS_HEADER, *PIMAGE_DOS_HEADER;
```
> **BMO Decision:** `TIRAR POR COMPLETO`. FastOS no arranca en MS-DOS. El offset al header real no es necesario si el archivo empieza directamente con el header BMO.

### 2. `IMAGE_FILE_HEADER` (El Header COFF Clásico)
```c
typedef struct _IMAGE_FILE_HEADER {
    WORD  Machine;               // [CONSERVAR] Pero fijo a x86_64
    WORD  NumberOfSections;      // [CONSERVAR] Esencial
    DWORD TimeDateStamp;         // [TIRAR/OPCIONAL] No vital para la ejecución
    DWORD PointerToSymbolTable;  // [TIRAR] Obsoleto en PE moderno
    DWORD NumberOfSymbols;       // [TIRAR] Obsoleto
    WORD  SizeOfOptionalHeader;  // [REEMPLAZAR] En BEF el tamaño del header es fijo/explícito
    WORD  Characteristics;       // [CONSERVAR] Útil para flags de permisos/ejecución
} IMAGE_FILE_HEADER, *PIMAGE_FILE_HEADER;
```

### 3. `IMAGE_OPTIONAL_HEADER64` (El "Optional" Obligatorio)
```c
typedef struct _IMAGE_OPTIONAL_HEADER64 {
    WORD  Magic;                 // [REEMPLAZAR] PE32+ (0x20B). En BEF será "BMO\0"
    // ... Versionamiento ...    // [TIRAR] Linker versions
    DWORD SizeOfCode;            // [CONSERVAR]
    DWORD SizeOfInitializedData; // [CONSERVAR]
    DWORD SizeOfUninitializedData;// [CONSERVAR] BSS
    DWORD AddressOfEntryPoint;   // [CONSERVAR] RVA del punto de inicio
    DWORD BaseOfCode;            // [TIRAR] Inútil en 64-bit
    ULONGLONG ImageBase;         // [TIRAR/REEMPLAZAR] BEF usa Relocations por defecto. El Kernel decide el ImageBase dinámico.
    DWORD SectionAlignment;      // [CONSERVAR] Usualmente 4096 (PAGE_SIZE)
    DWORD FileAlignment;         // [CONSERVAR] Usualmente 512
    // ... 6 campos de OS version // [TIRAR] FastOS no emula Windows versioning
    DWORD SizeOfImage;           // [CONSERVAR] Tamaño total virtual
    DWORD SizeOfHeaders;         // [CONSERVAR]
    // ... Checksum, Subsystem.. // [TIRAR] No hay "Subsystem GUI/CUI" en BMO. Todo es nativo.
    ULONGLONG SizeOfStackReserve;// [REEMPLAZAR] Movido al BmoProcessEnv
    ULONGLONG SizeOfStackCommit; // [REEMPLAZAR]
    // ... HeapReserve ...       // [TIRAR] El allocator en Rust gestiona esto dinámicamente.
    DWORD NumberOfRvaAndSizes;   // [TIRAR] En BEF, los directorios son tipados directamente.
    IMAGE_DATA_DIRECTORY DataDirectory[16]; // [REEMPLAZAR] Array propenso a errores.
} IMAGE_OPTIONAL_HEADER64, *PIMAGE_OPTIONAL_HEADER64;
```

### 4. `IMAGE_SECTION_HEADER` (Los Segmentos de Memoria)
```c
typedef struct _IMAGE_SECTION_HEADER {
    BYTE  Name[8];               // [CONSERVAR] "CODE", "DATA", etc.
    union {
        DWORD PhysicalAddress;
        DWORD VirtualSize;       // [CONSERVAR] Tamaño en RAM
    } Misc;
    DWORD VirtualAddress;        // [CONSERVAR] RVA
    DWORD SizeOfRawData;         // [CONSERVAR] Tamaño en el archivo
    DWORD PointerToRawData;      // [CONSERVAR] Offset en el archivo
    DWORD PointerToRelocations;  // [TIRAR] Obsoleto en PE
    DWORD PointerToLinenumbers;  // [TIRAR] Obsoleto
    WORD  NumberOfRelocations;   // [TIRAR] Obsoleto
    WORD  NumberOfLinenumbers;   // [TIRAR] Obsoleto
    DWORD Characteristics;       // [CONSERVAR] RX, RW, RO. Vital para la MMU.
} IMAGE_SECTION_HEADER, *PIMAGE_SECTION_HEADER;
```

### 5. `IMAGE_IMPORT_DESCRIPTOR` / `IMAGE_BASE_RELOCATION` / `EXPORT` / `TLS`
- **Imports:** PE usa Thunks (IAT/INT) complejos. BMO usará un array lineal dinámico ultra permisivo.
- **Relocations:** PE usa bloques de 4KB con tipos (DIR64). BMO usará solo `RELATIVE_U32` (Offsets delta) porque estamos en un entorno x86-64 puro de position-independent code (PIC).
- **Exports/TLS:** PE hace arboles bicolores. BMO utilizará arreglos de offsets lineales.

---

## FASE 2: Diseño del BMO Executable Format (BEF)

### Estructuras en C (Listas para Bindings en Rust)
```c
#include <stdint.h>

// El Header Principal de BEF. No hay DOS stub, esto es el byte 0 del archivo.
typedef struct _BEF_HEADER {
    uint8_t  Magic[4];          // "BMO\0"
    uint16_t Version;           // Versión del formato BEF (0x0101 para v1.1)
    uint16_t Flags;             // Flags Ultra-Permisivos (ej. 0x1 = REQUIRE_GSP, 0x2 = DYNAMIC_LINK)
    
    uint64_t EntryPoint;        // RVA absoluto del punto de inicio
    uint64_t SignatureOffset;   // Offset a la firma digital (0 si no está firmado)
    uint32_t SignatureSize;     // Tamaño de la firma
    
    uint32_t SectionCount;      // Número de secciones
    uint32_t SectionAlignment;  // Alineación en RAM (siempre 4096)
    uint32_t FileAlignment;     // Alineación en disco (generalmente 512 o 4096)
    uint32_t SizeOfImage;       // Tamaño virtual total
    uint32_t SizeOfHeaders;     // Tamaño de todos los headers sumados
    
    // Directorios Específicos (Reemplazo del array mágico de DataDirectory)
    uint32_t RelocRva;          // RVA de las relocalizaciones
    uint32_t RelocSize;         
    uint32_t ImportRva;         // RVA de los imports de librerías BMO
    uint32_t ImportSize;
    uint32_t ExportRva;
    uint32_t ExportSize;
    
    // NUEVO v1.1: Shader Integration
    uint8_t  shader_format;    // 0x00 = NONE, 0x01 = SASS_AMPERE, 0x02 = SPIR-V (runtime compile)
    uint32_t shader_rva;       // Offset a la sección SHADER dentro del .bef
    uint32_t shader_size;      // Tamaño del bloque ShaderBinary
    uint8_t  gpu_arch;         // 0x01 = GA106 (Ampere), 0x02 = TU106 (Turing)
} __attribute__((packed)) BEF_HEADER;

// Definición de Secciones (Código, Datos)
typedef struct _BEF_SECTION {
    char     Name[8];           // "CODE", "DATA", "RODATA", "BSS"
    uint32_t VirtualSize;       // Cuánta RAM requiere (sin rellenar ceros)
    uint32_t VirtualAddress;    // RVA en memoria
    uint32_t SizeOfRawData;     // Cuánto ocupa físicamente en el archivo
    uint32_t PointerToRawData;  // Offset en el archivo BEF
    uint32_t Flags;             // PERM_EXECUTE (0x1), PERM_READ (0x2), PERM_WRITE (0x4)
} __attribute__((packed)) BEF_SECTION;

// Sección de Shaders embebidos nativos (v1.1)
typedef struct _BEF_SHADER_SECTION {
    uint32_t magic;            // "SHDR"
    uint32_t num_gprs;         // Registros usados (para occupancy GA106)
    uint32_t shared_mem_size;  // Shared memory requerida
    uint32_t local_mem_size;
    uint32_t code_size;        // Tamaño del SASS binario
    uint8_t  code[];           // SASS crudo listo para DMA → VRAM
} __attribute__((packed)) BEF_SHADER_SECTION;

// Relocalizaciones Modernas: Solo base-relative x86-64 (Offset de 32 bits a la variable a fixear)
typedef struct _BEF_RELOCATION {
    uint32_t Rva;               // Dónde aplicar el fixup (El valor ahí += BaseAddress)
} __attribute__((packed)) BEF_RELOCATION;

// Importaciones ultra permisivas y dinámicas (Resolución por Hash)
typedef struct _BEF_IMPORT {
    uint64_t NameHash;          // Hash djb2 o FNV-1a de la dependencia (ej. hash("fast_gfx.bdll"))
    uint64_t ResolvedAddrRva;   // RVA donde el Kernel debe escribir la dirección resuelta
} __attribute__((packed)) BEF_IMPORT;
```

### Comparativa: PE64 vs BEF (Tamaño Mínimo)
| Componente | PE64 | BEF | Mejora |
| :--- | :--- | :--- | :--- |
| DOS Stub | 64 bytes | 0 bytes | Eliminado.
| PE Signature | 4 bytes | 4 bytes | Igual ("MZ" -> "BMO").
| File Header | 20 bytes | 0 bytes | Integrado.
| Optional Header | 240 bytes | 58 bytes | Se eliminaron versionamientos y offsets win32.
| Shaders | Externos (DLL) | Embebidos SASS nativos | Zero runtime compile |
| Total Header Mínimo | **~328 Bytes** | **62 Bytes** | **81% más pequeño.**

---

## FASE 3: FastOS Loader Design (Rust Pseudocode)
Este es el cargador que tu Kernel usará al atrapar la ejecución.

```rust
#[derive(Debug)]
pub enum LoadError {
    InvalidMagic,
    UnsupportedVersion,
    SignatureInvalid,
    OutOfMemory,
    MissingImport,
}

/// 1. Parsear y Validar el Header (Safe Parsing)
pub fn parse_bef_header(bytes: &[u8]) -> Result<BefHeader, LoadError> {
    // Usamos bytemuck para evitar undefined behavior y punteros crudos en el Kernel
    let header: &BefHeader = bytemuck::try_from_bytes(&bytes[..core::mem::size_of::<BefHeader>()])
        .map_err(|_| LoadError::InvalidMagic)?; // InvalidSize handled naturally
        
    if &header.magic != b"BMO\0" {
        return Err(LoadError::InvalidMagic);
    }
    // Verificación ultra-permisiva: Si tiene firma, valídala. Si no, confía (modo dev).
    if header.signature_offset > 0 {
        validate_signature(header, bytes)?;
    }
    Ok(header.clone())
}

/// 3. Paginación de Secciones (MMU)
pub fn map_sections(
    header: &BefHeader, 
    bytes: &[u8], 
    phys_mem: &mut PhysicalAllocator
) -> Result<VirtualMapping, LoadError> {
    // Alocar ImageBase aleatorio para ASLR (Security)
    let base_address = phys_mem.allocate_aslr(header.size_of_image)?;
    let sections = parse_sections(bytes, header);
    
    for sec in sections {
        let dest = base_address + sec.virtual_address as u64;
        
        // Copiar Raw Data
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(sec.pointer_to_raw_data as usize),
                dest as *mut u8,
                sec.size_of_raw_data as usize,
            );
        }
        
        // Cero-fill para la sección BSS
        if sec.virtual_size > sec.size_of_raw_data {
            let bss_start = dest + sec.size_of_raw_data as u64;
            let bss_size = sec.virtual_size - sec.size_of_raw_data;
            unsafe { core::ptr::write_bytes(bss_start as *mut u8, 0, bss_size as usize); }
        }
        
        // Aplicar protecciones de página (RX, RW) según sec.flags
        phys_mem.protect_pages(dest, sec.virtual_size, sec.flags);
    }
    
    Ok(VirtualMapping { base_address, size: header.size_of_image })
}

/// 4. Aplicar Relocalizaciones Dinámicas
pub fn apply_relocations(mapping: &VirtualMapping, header: &BefHeader, bytes: &[u8]) -> Result<(), LoadError> {
    if header.reloc_size == 0 { return Ok(()); }
    
    let relocs = get_reloc_slice(bytes, header);
    for reloc in relocs {
        let target_ptr = (mapping.base_address + reloc.rva as u64) as *mut u64;
        unsafe {
            // El valor estático + la dirección base real donde el kernel alojó el programa
            *target_ptr = (*target_ptr).wrapping_add(mapping.base_address);
        }
    }
    Ok(())
}

/// 5. Resolver Imports (Resolución Rápida por Hash)
pub fn resolve_imports(mapping: &VirtualMapping, kernel_syms: &SymbolTable, header: &BefHeader, bytes: &[u8]) -> Result<(), LoadError> {
    if header.import_size == 0 { return Ok(()); }
    
    // Convertimos el slice de bytes a estructuras con bytemuck (Safe cast)
    let import_bytes = get_import_bytes(bytes, header);
    let imports: &[BefImport] = bytemuck::cast_slice(import_bytes);
    
    for imp in imports {
        // Búsqueda ultrarrápida O(1) usando el hash de la dependencia (sin strings en runtime)
        let fn_address = kernel_syms.lookup_by_hash(imp.name_hash).ok_or(LoadError::MissingImport)?;
        
        // Escribimos la dirección resuelta en memoria
        let resolved_ptr = (mapping.base_address + imp.resolved_addr_rva as u64) as *mut u64;
        unsafe { *resolved_ptr = fn_address; }
    }
    Ok(())
}
```

---

## FASE 4: `BmoProcessEnv` (El reemplazo del PEB/TEB de Windows)

En Windows, el `PEB` es una estructura arcaica llena de `UNICODE_STRING` y punteros de compatibilidad a subsistemas viejos. En FastOS, el Kernel inyectará una estructura moderna y limpia (`BmoProcessEnv`) en el registro `RDI` antes de saltar al EntryPoint del BEF.

```rust
/// Estructura de Entorno inyectada por el Kernel al arrancar el proceso BEF
#[repr(C)]
pub struct BmoProcessEnv {
    pub magic: u32,                  // 0xB00B1E55 (Verificación de integridad)
    
    // Entorno puro sin manipulación de strings ineficiente
    pub argc: usize,
    pub argv: *const *const u8,      // Array de punteros a strings UTF-8 null-terminated
    
    pub image_base: u64,             // Base virtual asignada por ASLR
    
    // Gestión Dinámica Ultra-Permisiva
    pub handle_table: *mut BmoHandleTable, // Puntero opaco a los recursos del OS
    pub syscall_dispatcher: u64,           // Dirección rápida vDSO (Si no usamos instrucción SYSCALL)
    
    // Límites de Stack para detección de desbordamientos
    pub stack_base: u64,
    pub stack_limit: u64,
    
    // Información de Hardware expuesta al user-mode (opcional)
    pub cpu_cores_available: u32,
    pub gsp_enabled: bool,                 // Si true, el app puede invocar funciones GPU nativas
    
    // NUEVO v1.1: Referencia directa a los shaders del programa
    pub shader_binary: *const u8,          // null si shader_format = NONE (Puntero a BmoShaderHandle)
    pub gpu_arch: u8,
}

/// Función 6 (Final). El Salto al Usuario.
pub fn transfer_control(entry_rva: u64, mapping: &VirtualMapping, env: &BmoProcessEnv) -> ! {
    let entry_point = mapping.base_address + entry_rva;
    
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack_top}",     // 1. Setear el Stack nuevo
            "mov rdi, {env_ptr}",       // 2. Pasar el BmoProcessEnv como primer argumento (RDI en SysV ABI)
            "jmp {entry}",              // 3. Salto cuántico al espacio de usuario (Ring 3)
            stack_top = in(reg) env.stack_base,
            env_ptr = in(reg) env as *const _,
            entry = in(reg) entry_point,
            options(noreturn)
        );
    }
}
```

## FASE 5: Pipeline de Compilación de Shaders (Toolchain)

El formato BEF v1.1 elimina el dolor de compilar shaders en tiempo de ejecución. Todo se hace AOT (Ahead-Of-Time).

### Flujo Completo:
```text
Dev escribe shaders en HLSL o WGSL
    ↓ DXC (Microsoft, open source) o NAGA (Rust puro)
SPIR-V intermedio
    ↓ NAK (Mesa3D, Rust puro) 
SASS binario GA106/Ampere
    ↓ BEF Linker
Embebido en sección SHADER del .bef
    ↓ FastOS loader en runtime
DMA copy → VRAM via GSP
Zero compilación en runtime
```

### Elección de Herramientas:
- **DXC**: Es el frontend oficial open source para HLSL.
- **NAGA**: Transpilador en Rust puro que FastOS puede utilizar como dependencia nativa, sin tocar dependencias C/C++.
- **NAK**: El compilador de NVIDIA escrito en Rust. Permite tomar el SPIR-V y emitir el binario SASS. Al usar NAK en el Linker BEF, el juego ya viene pre-digerido para la tarjeta gráfica.

---

## Conclusión Arquitectónica (v1.1)
El formato BEF v1.1 es ahora un ecosistema completo y autocontenido que:
- Carga código ejecutable (`CODE` section)
- Carga datos (`DATA` section)  
- **Carga shaders nativos GA106 (`SHADER` section)**
- Todo vive en un solo archivo con zero dependencias externas.
- El kernel FastOS solo hace DMA copies a la VRAM, logrando cero compilación en runtime y eliminando la necesidad de APIs mastodónticas como DirectX o Vulkan.
