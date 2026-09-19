# PLAN: BEF nativo -- un formato de BMO-X x86-64, no un ELF con otro nombre

> Pedido por Eddi el 2026-09-19: *"investigar BEF: no quiero que sea ELF como
> tipico, empieza por completo BEF"*.
>
> Estado: **INVESTIGADO, SIN DECIDIR.** Nada de esto esta hecho salvo el corte 0
> (seccion 3). Las decisiones que faltan estan en la seccion 5 y son de Eddi.

---

## 1. Lo que BEF es hoy, medido

BEF1 es la idea central de ELF: **una cabecera y una tabla de secciones
tipadas**, y la regla de oro de ELF escrita tal cual en
`bef/BEF_EXTENSIONES.md` (*"una seccion desconocida se salta: es lo que ha
mantenido vivo a ELF treinta anios"*).

### La cabecera (48 B) -- que lee de verdad alguien

| Offset | Campo | Quien lo lee | Veredicto para un BMO-X solo x86-64 |
|---|---|---|---|
| 0 | `magic` `BEF1` | puerta | se queda |
| 4 | `version_major/minor` (2 x u16) | puerta | una version basta, y puede ir en el magic |
| 8 | `flags` (u32) | puerta, validador | de 16 bits definidos, VIVEN 5 (abajo) |
| 12 | `arch` | puerta | siempre `0x01`; existe para rechazar ARM "por su nombre" |
| 13 | `endianness` | puerta | siempre `0`; se justifico "por si PowerPC" |
| 14 | `cpu_features` (u16) | puerta | **se queda y crece**: es lo mas x86 del formato |
| 16 | `abi_version` (2 x u8) | puerta | tercera version del mismo fichero |
| 18 | `_reserved` (6 B) | nadie | -- |
| 24 | `entry_offset` | puerta | se queda |
| 32 | `section_table_offset` | puerta | ver seccion 4 |
| 40 | `section_count` | puerta | ver seccion 4 |
| 44 | `total_size` (u32) | puerta | techo de 4 GiB |

**Banderas.** Vivas: `EXECUTABLE`, `OBJECT`, `SIGNED`, `HAS_MANIFEST`,
`WANTS_SCREEN`. Sin consumidor: `PIE` (siempre puesta), `USES_BAREX` (siempre
puesta), `PROVENANCE_PE/ELF` ("devorar" no existe), `SHARED_LIBRARY` (retirada
el 17-09). Rechazadas: `COMPRESSED`, `HOT_RELOADABLE`, y desde el corte 0
`HAS_TLS` y `HAS_SHADERS`. **No se pueden quitar sin romper los `.bex` que
existen**: todos llevan `PIE | USES_BAREX` y el validador rechaza un bit que no
conoce. Por eso esto es un formato NUEVO y no una limpieza.

### La entrada de seccion (48 B) = `Elf64_Shdr` y `Elf64_Phdr` fundidos

| Campo | Quien lo lee |
|---|---|
| `kind`, `flags`, `file_offset`, `file_size`, `mem_size`, `alignment` | puerta y cargador |
| `virt_addr` | **nadie** (en una seccion; el de `Symbol` si) |
| `hash_index` | lo escribe `writer`, **no lo lee nadie** |
| `_pad` (3 B), `_reserved` (4 B) | nadie |

Banderas de seccion: `READ`, `WRITE`, `EXEC`, y `COMPRESSED`, `PAGE_ALIGNED`,
`HUGE_ALIGNED`, `LAZY`, `HASHED`, `SYNTHETIC` sin consumidor.

### *** El hallazgo que no es de formato: NO HAY PAGINAS DE SOLO LECTURA

`admitir.rs:606`: `let writable = s.flags & SECTION_FLAG_EXEC == 0;` y
`vmm/amarilla.rs::map_page_tipo` pone NX solo si la pagina es escribible.
El kernel tiene **dos** estados: RX (codigo) y RW+NX (todo lo demas).
**`RoData` se mapea escribible**: la constante de un programa se puede pisar,
y un literal de C modificado no da `#PF`, da otro programa. Es herencia
directa de decidir el permiso por una BANDERA y no por lo que la seccion ES.

### Simbolos y relocs: el vocabulario de ELF

- `Symbol`: `Local/Global/Weak`, visibilidad, `File`, `Tls` -- el `st_info` de
  ELF. Solo lo necesitan los objetos (`.bo`) al enlazar; un ejecutable no.
- `Relocation`: `Abs64` (`R_X86_64_64`), `Rel32` (`R_X86_64_PC32`), `Got64`
  (`R_X86_64_GLOB_DAT`: enlazado dinamico, ya REHUSADO en `objeto.rs`) y
  `SeccionAbs64`, la unica que BMO invento porque la necesitaba.

---

## 2. Lo que un `.bex` NECESITA en BMO-X, y nada mas

Lo que hace el cargador del kernel, en orden:

1. saber que es un BEX de esta maquina y de este ABI;
2. saber cuanto estado de CPU tiene que preservar (`cpu_features`);
3. mapear **cuatro regiones**: codigo (RX), constantes (R), datos (RW), ceros (RW);
4. aplicar relocaciones de **un solo tipo** en un ejecutable estatico: "aqui
   va la direccion de region+offset" (`SeccionAbs64`);
5. comprobar hashes/firma y requisitos;
6. saltar al punto de entrada.

Todo lo demas (manifiesto, recursos, katanas, simbolos de depuracion) es para
OTRO, y el kernel no lo abre.

---

## 3. Corte 0 -- HECHO el 2026-09-19 (`31602e7f`)

Dentro de BEF1, sin romper ningun `.bex`: la puerta y el validador RECHAZAN
imports, exports y TLS (`Falta::EnlazadoDinamico`) y las banderas `HAS_TLS` y
`HAS_SHADERS`; fuera once tipos de seccion sin productor y `BefMagic` (la
deteccion de `\x7FELF` y `MZ`). Censo: 49 imagenes, 0 afectadas.

---

## 4. La propuesta: BEX2, disenado desde lo que BMO-X hace

```text
   cabecera fija (64 B, alineada a 64 = una linea de cache)
     0   magic        "BEX2"                  version en el magic: una, no tres
     4   abi          u8   (2)
     5   banderas     u8   EJECUTABLE | OBJETO | FIRMADO | QUIERE_PANTALLA
     6   reservado    u16  = 0, rechazado si no
     8   xcr0         u64  los componentes XSAVE que el programa usa (SSE,
                           AVX, AVX-512...) -- el mapa EXACTO que el kernel
                           necesita para preservar su estado. x86 puro.
    16   entrada      u32  offset dentro de CODIGO
    20   anexos       u32  cuantos anexos hay (tabla detras de la cabecera)
    24   CODIGO       {offset u32, bytes u32}          RX   siempre
    32   CONSTANTES   {offset u32, bytes u32}          R+NX siempre
    40   DATOS        {offset u32, bytes u32}          RW+NX siempre
    48   CEROS        {bytes u32}                      RW+NX siempre
    52   total        u32  bytes del fichero
    56   hash_cab     u64  primeros 8 B del BLAKE3 de la cabecera + anexos
   64..  anexos: {tipo u8, pad, bytes u32, offset u32} x N   (relocs, firma,
                  requisitos, recursos, manifiesto, katanas...)
```

Lo que cambia, y por que es BMO y no ELF:

- **El permiso lo da el HUECO, no una bandera.** Las cuatro regiones tienen
  sitio fijo en la cabecera: no hay forma de escribir un `.bex` con codigo
  escribible ni constantes escribibles. Cierra por construccion el hallazgo
  de la seccion 1.
- **Sin tabla de secciones para lo que se carga.** El kernel lee 64 bytes y
  sabe todo lo que tiene que mapear. Los anexos son la unica lista, y son
  todos para OTRO salvo tres (relocs, firma, requisitos).
- **`xcr0` en vez de `cpu_features`.** El dato que Ring 0 necesita para
  `XSAVE/XRSTOR` es literalmente la mascara de XCR0. Declararla es un contrato
  que el kernel comprueba contra el XCR0 de la maquina de un vistazo.
- **Un solo tipo de reloc en un ejecutable** (region + offset -> 64 bits). Los
  `.bo` conservan simbolos y `Rel32`, que `bmo-enlazar` consume y no pasa al
  `.bex`.
- **Sin `arch` ni `endianness`**: el magic ya dice "BMO-X x86-64". Un binario
  de otro repositorio tendria otro magic, y se rechaza por el primero.
- **Cabecera de 64 B alineada a 64**: una linea de cache, un solo acceso.

---

## 5. Lo que tiene que decidir Eddi antes de escribir una linea

1. **Corte limpio o transicion.** BEX2 rompe todos los `.bex`: los cinco
   payloads del kernel (se regeneran con el build), DOOM y las apps del
   Kingston (hay que recompilar y desplegar). Propuesta: corte limpio, el
   kernel solo acepta BEX2 -- un formato, no dos.
2. **Paginas alineadas en el fichero, o compacto.** Si cada region empieza en
   un multiplo de 4 KiB del fichero, el cargador puede REFLEJAR paginas en vez
   de copiar (`bmo-ram-quirofano`: "reflejar en vez de copiar"). Cuesta
   relleno: el 07-08 se quito a proposito para encoger los `.bex`. Medir DOOM
   con y sin antes de elegir.
3. **La regla congelada** (`BEF_EXTENSIONES.md`): sus secciones 3 y 4 (los
   campos `endianness` y `cpu_features`, la tabla de lo que el kernel lee) se
   REESCRIBEN. La regla 2 (lo que no me incumbe se salta) sigue valiendo para
   los anexos.
4. **Primero el hallazgo de solo lectura, dentro de BEF1.** Se puede arreglar
   hoy sin esperar a BEX2: tres estados de pagina (RX, R+NX, RW+NX) decididos
   por el TIPO de seccion. Es Ring 0 y pide metal.

## 6. El orden propuesto

```text
   B1   paginas de solo lectura en BEF1 (kernel + prueba)        -- metal
   B2   BEX2 en bmo-abi: cabecera, writer, validador + pruebas
   B3   la puerta (bmo-bex-gate) y el cargador del kernel leen BEX2
   B4   bmo-enlazar, bmo-pack, bmo-firmar, bmo-verify
   B5   los emisores (escriben por `writer`: casi nada)
   B6   regenerar payloads, DOOM y apps; borrar BEF1
```
