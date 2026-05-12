# FastOS Package Manager Specification
**Capa:** Ecosistema
**Prioridad:** MEDIA
**Depende de:** `BEF_Executable_Format_Spec.md`, `FastOS_Native_FS_Format.md`
**Inspiración:** `cargo` (Rust).

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Windows utiliza instaladores masivos (`.msi`, `.exe`) que escriben miles de claves en un registro central (Registry) y esparcen `.dll`s por todo el disco. Linux usa `apt` o `rpm`, sufriendo frecuentemente de *Dependency Hell* cuando dos paquetes requieren versiones diferentes de la misma librería C.
- **Qué conservamos:** La simplicidad de la CLI moderna tipo Rust/Cargo y la verificación criptográfica.
- **Qué tiramos:** El Registro de Windows, el Infierno de Dependencias de APT/NPM, y la fragmentación de archivos. En FastOS, **1 Aplicación = 1 Archivo BEF**. No hay instalación global de librerías dinámicas, todo está compilado estáticamente en Rust.

---

## FASE 2: Diseño BMO Nativo

El gestor de paquetes BMO (`bmo`) es una herramienta de terminal que gestiona el formato `.bpkg` (BMO Package).

### El Formato `.bpkg`
Esencialmente es un archivo ZIP ultra-optimizado que contiene exactamente dos archivos:
1. `metadata.toml` (Los metadatos)
2. `app.bef` (El binario FastOS)

```toml
# metadata.toml (Ejemplo)
[package]
name = "bmo_dwm"
version = "1.0.0"
author = "FastOS Core Team"
description = "Window Compositor Privilegiado"

[requirements]
min_kernel_version = 100
gpu_required = true      # Si es true, el PM verifica que la RTX 3060 esté presente

[security]
sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
signature = "30450220... (Opcional, firma ECDSA/Ed25519)"
```

### Arquitectura del Repositorio BMO
El ecosistema no requiere un complejo servidor de base de datos.
- **Servidor:** Un servidor HTTPS extremadamente simple (ej. `pkg.fastos.org`).
- **Índice:** El servidor simplemente expone un archivo `index.json` que contiene la lista de paquetes y las URLs de descarga directa de los `.bpkg`.

---

## FASE 3: Implementación (CLI y Flujo de Instalación)

La herramienta `bmo` corre en Ring 3 (nivel SYSTEM) y expone comandos limpios:
- `bmo install <nombre>`
- `bmo update` (Actualiza todo, ya que no hay dependencias, nunca rompe el sistema)
- `bmo remove <nombre>` (Eliminación atómica, solo borra 1 archivo)
- `bmo list`

### Flujo de la Instalación
Cuando el usuario ejecuta `bmo install firefox`:
1. El PM descarga `firefox.bpkg` desde el servidor HTTPS.
2. Extrae `metadata.toml` y verifica que el hash `sha256` coincida matemáticamente con `app.bef`.
3. Si existe una `signature`, valida criptográficamente que provenga de un desarrollador confiable (Clave Pública).
4. El PM usa la API VFS (`DOC-06`) para copiar atómicamente el `app.bef` a `/system/apps/firefox.bef` en el disco NVMe BMOFS.
5. El PM finaliza. **No hay scripts post-instalación ni modificaciones a nivel de sistema operativo.**

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `BEF_Executable_Format_Spec.md`:** El `.bpkg` es meramente un vehículo de transporte de red para el archivo `.bef`.
- **Conexión con `FastOS_Native_FS_Format.md` (BMOFS):** La instalación se reduce a escribir los bloques de `4KB` del ejecutable directamente al SSD de manera contigua.
- **Conexión con `FastOS_App_Sandbox.md`:** Si el PM detecta que un paquete no tiene firma criptográfica (`signature`), lo marca automáticamente en sus atributos VFS como `UNTRUSTED`, lo que forzará al Kernel a lanzarlo en SANDBOX.

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows:**
La instalación de software en FastOS no es destructiva. No hay "Desinstaladores" que puedan fallar, ni librerías compartidas que rompan otros programas. Al abrazar la compilación estática de Rust y el modelo de aplicación 100% autocontenida de BEF, la instalación y eliminación de software toma milisegundos y el sistema operativo no se degrada con el paso de los años (el "Software Rot" de Windows).
