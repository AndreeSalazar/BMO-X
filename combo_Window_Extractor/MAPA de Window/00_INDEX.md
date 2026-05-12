# 00_INDEX: FastOS Master Architecture

Este es el directorio principal del conocimiento y especificaciones arquitectónicas de **FastOS (Bare Metal Orchestrator)**. 

**Hardware Target Oficial:** AMD Ryzen 5 5600X + NVIDIA RTX 3060 GA106.
**Filosofía:** Cero Legacy (Sin Win32, POSIX, DOS), x86-64 puro, UEFI Boot, Rust-Friendly.

---

## 📁 Estructura del Proyecto

### `01_Windows_DNA/` (Ingeniería Inversa de Windows)
- ✅ `Anatomia_Total_Windows11.md`
- ✅ `El_Mapa_Total_Subsistemas.md`
- ✅ `El_Cerebro_Hardware_PCIe_APIC.md`
- ✅ `El_Corazon_ntoskrnl.md`
- ✅ `Estructuras_Internas_Kernel.md`
- ✅ `El_Cargador_de_Aplicaciones_BMO.md`
- ✅ `Win32_Minimum_Surface.md`
*(También incluye carpetas de datos crudos: `DDK_Reference_DNA`, `WINE_Reference_DNA`, `Driver_Forensics`, `Ring 0`, `Ring 3`, `UI_Resources`)*

### `02_BEF_Format/` (Formato Ejecutable BMO + API Gráfica BareX + BMO ABI)
- ✅ `BMO_ABI_Spec.md` (⭐ **Cimiento** — convención de llamada nativa, reemplaza al C ABI: 7 GPRs args, 0 B shadow, 64 B stack align, handles con generación, SQ/CQ async)
- ✅ `BEF_Executable_Format_Spec.md` (v1.1 con integración de shaders nativos)
- ✅ `BMO_Graphics_Layer_Spec.md` (L1 — Mapeo directo a NVIDIA GSP)
- ✅ `NVK_Shader_Pipeline_Analysis.md` (Transpilación SPIR-V a SASS con NAK)
- ✅ `BareX_Shader_Pipeline.md` (L2 — HLSL/DXBC/DXIL → SPIR-V → SASS GA106)
- ✅ `BareX_API_Spec.md` (L3 — API gráfica nativa, hereda DX12 Ultimate + Agility 1.614)
- ✅ `BareX_Compat_Shim_Spec.md` (L4 — Compat con binarios DX9/10/11/12 de Windows, vía PE loader + COM thunks heredados de WINE/DXVK/VKD3D-Proton)
- ✅ `DX12_to_BareX_Mapping.md` (Mapeo 1-a-1 de cada concepto DX12 → BareX)
- ✅ `BareX_Audio_Spec.md` (`bx_audio` — Audio nativo < 1.5 ms round-trip, hereda XAudio2/WASAPI Exclusive, descarta DirectSound/MMSystem/kmixer)
- ✅ `BareX_Input_Spec.md` (`bx_input` — Input HID directo < 0.5 ms, gamepads/teclado/ratón/volantes/HOTAS/VR/DualSense, descarta DirectInput/MMSystem/WM_INPUT)
- ✅ `BareX_Network_Spec.md` (`bx_net` — Stack TCP/IP/QUIC propio en Rust, HTTP/3 + WebSocket + WebTransport + kernel bypass opcional, descarta Winsock/SChannel/NetBIOS/BITS)

### `03_Kernel_Specs/` (Especificaciones del Kernel FastOS)
- ✅ `FastOS_Syscall_Table_Spec.md` (Puente Ring 3 → Ring 0)
- ✅ `FastOS_Memory_Manager_Spec.md`
- ✅ `FastOS_Hardware_Timers_Spec.md`
- ✅ `FastOS_Scheduler_Spec.md`
- ✅ `FastOS_Locking_Primitives.md`

### `04_Storage/` (Almacenamiento)
- ✅ `FastOS_VFS_Spec.md`
- ✅ `FastOS_NVMe_Driver_Spec.md`
- ✅ `FastOS_Native_FS_Format.md`

### `05_UserSpace/` (Ring 3 y Ecosistema)
- ✅ `FastOS_Standard_Library.md`
- ✅ `FastOS_Rust_Runtime_BEF.md`
- ✅ `FastOS_Window_Compositor.md`

### `06_Ecosystem/` (Ecosistema Completo)
- ✅ `FastOS_Package_Manager.md`
- ✅ `FastOS_Security_Model.md`
- ✅ `FastOS_App_Sandbox.md`

### `07_Audit/` (Auditorías y Roadmap)
- ✅ `Auditoria_Arquitectonica_Global.md`
- ✅ `FastOS_Architecture_Complete.md`
