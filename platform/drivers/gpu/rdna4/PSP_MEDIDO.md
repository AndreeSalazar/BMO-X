# EL PSP, MEDIDO -- leyendo `amdgpu`, sin tarjeta y sin suposiciones

> Hecho el **2026-09-07**, porque `PLAN_VULKAN.md` lo dejo escrito como la unica
> pieza que puede convertir la meta B en imposible, y con su precio al lado:
> *"un dia de leer `amdgpu` y contar pasos"*.
>
> El dueno dijo **"mide el PSP entonces, lee amdgpu"**. Esto es esa medida.

---

## 0. ⚠ QUE ES ESTO Y QUE NO ES

```text
   ES     una LECTURA del driver de Linux, que es codigo publico
   NO ES  una prueba en metal. Aqui no se ha encendido ninguna GPU
   NO ES  codigo copiado. Ver la frontera de abajo
```

** Nada de lo que sigue esta demostrado en una maquina. Es lo que dice el driver
que hay que hacer, contado paso a paso. **Un plan medido leyendo sigue siendo un
plan**, y este documento no vale como prueba de nada hasta que una tarjeta
conteste.

### ⚠ La frontera del copyright, dicha antes que nada

`amdgpu` es **GPL-2.0**. BMO-X es **Apache-2.0**. Son incompatibles en una
direccion, asi que la regla aqui es dura y no se negocia:

```text
   [x] se describen los PASOS y se nombran los REGISTROS
   [x] se cuenta el orden y lo que se espera en cada uno
   [ ] NO se copia ni una linea de codigo
   [ ] NO se copia ni una estructura de datos
```

Un nombre de registro y el orden de un protocolo son **hechos de interfaz**, que
es exactamente lo que la Directiva 2009/24/CE articulo 6 permite obtener para
interoperar -- y aqui ni siquiera hace falta invocarla, porque el codigo esta
publicado y solo hace falta **leerlo**.

> Se lee para saber QUE hay que hacer. Se escribe desde cero para hacerlo.

---

## 1. ★★ EL VEREDICTO, primero

```text
   EL PSP NO ES UN MURO CRIPTOGRAFICO. ES UN BUZON.
```

Esto es lo que cambia el plan. La palabra *"Platform Security Processor"* y la
frase *"autentica el microcodigo"* hacian pensar en un desafio criptografico --
firmar algo, negociar una clave, responder a un reto. **No lo hay.**

```text
   quien firma el firmware      AMD, antes de publicarlo
   quien lo verifica            el PSP, dentro de la GPU
   que hace el driver           ENTREGAR el blob y esperar un bit
```

★ **Nosotros no firmamos nada, no desciframos nada y no negociamos nada.**
Copiamos un blob a memoria, escribimos su direccion en un registro, escribimos
una orden en otro, y esperamos a que se encienda el bit 31. Doce veces.

**La meta B no esta bloqueada por el PSP.** Sigue siendo grande por otras
razones --el compilador de sombreadores, la memoria de video, Vulkan-- pero el
PSP deja de ser la incognita que podia matarla.

---

## 2. La secuencia, paso por paso

Medida sobre `psp_v13_0.c` y `psp_v14_0.c` del arbol principal de Linux. **Los
dos tienen la MISMA forma**: cambian el prefijo del bloque de registros
(`MP0_SMN_...` en v13, `MPASP_SMN_...` en v14) y algun componente, no el
protocolo.

### 2.1 El patron, que se repite doce veces

```text
   1. copiar el blob a un buffer en memoria que la GPU pueda leer
   2. C2PMSG_36  <-  direccion del buffer DESPLAZADA 20 BITS A LA DERECHA
   3. C2PMSG_35  <-  la orden (que componente es)
   4. esperar a que C2PMSG_35 encienda el bit 31
```

** El desplazamiento de 20 bits no es un detalle: significa que el buffer tiene
que estar alineado a **1 MiB**, porque los 20 bits de abajo no viajan. Es el
tipo de cosa que cuesta un dia si se descubre depurando.

### 2.2 Los pasos, en orden

| # | paso | registros | que se espera |
|---|---|---|---|
| 0 | mirar si el SOS ya esta vivo | `C2PMSG_81` | si no es cero, **saltar casi todo** |
| 1 | esperar al gestor de arranque | `C2PMSG_35` | bit 31, hasta 3.000 vueltas |
| 2 | cargar **KDB** (base de claves) | `36` + `35` | bit 31 |
| 3 | cargar **SPL** | `36` + `35` | bit 31 |
| 4 | cargar **SYSDRV** | `36` + `35` | bit 31 |
| 5 | cargar **SOCDRV** | `36` + `35` | bit 31 |
| 6 | cargar **INTFDRV** | `36` + `35` | bit 31 |
| 7 | cargar **DBGDRV** (HAD en v14) | `36` + `35` | bit 31 |
| 8 | cargar **RASDRV** | `36` + `35` | bit 31 |
| 9 | cargar **SPDMDRV** (v13) / **IPKEYMGR** (v14) | `36` + `35` | bit 31 |
| 10 | cargar el **SOS**, el sistema seguro | `36` + `35`, **20 ms**, luego `81` | que `81` CAMBIE |
| 11 | leer la version del SOS | `C2PMSG_58` | -- |
| 12 | entrenamiento de memoria (DRAM) | `36` + `35` | bit 31, hasta 3 s |
| 13 | reservar **4 KiB** en VRAM para el anillo | -- | -- |
| 14 | **crear el anillo** | `69` baja, `70` alta, `71` tamano, `64` orden | bit 31 en `64` |
| 15 | crear la **TMR** (memoria de confianza) | por el anillo | respuesta |
| 16 | cargar el resto del firmware **por el anillo** | `67` = puntero de escritura | respuesta por cada uno |

### 2.3 Y lo que sube por el anillo en el paso 16

Aqui esta la parte que nadie cuenta cuando dice *"el PSP"*: una vez el buzon
funciona, **todo el demas firmware de la GPU entra por ahi**.

```text
   CP_ME, CP_PFP, CP_CE, CP_MEC     los motores de graficos
   RLC                              el que gobierna las unidades de computo
   SDMA (varias instancias)         ** EL MOTOR DE COPIA. La meta A
   SMU + PPTABLE                    energia y relojes
   VCN                              video por hardware
   IMU, DMCUB, ...                  segun el chip
```

★★ **Y ahi esta el hallazgo que reordena el plan entero**: el SDMA --el motor de
copia que la meta A necesita para pintar rapido-- **tambien entra por el PSP**.

> No hay una "meta A barata sin PSP". Para tener el motor de copia hay que
> arrancar el PSP igual. La buena noticia es que arrancarlo son doce mensajes.

---

## 3. ★★ ES EL MOLDE DE xHCI OTRA VEZ

`PLAN_VULKAN.md` lo aposto --*"la forma es la de xHCI, ya peleada en metal"*--
y la lectura lo confirma. No se parece: **es lo mismo**.

```text
   xHCI (ya hecho, y en metal)          PSP (por hacer)
   ------------------------------------------------------------------
   CRCR   <- direccion del anillo       C2PMSG_69/70 <- direccion
   ERSTSZ <- tamano                     C2PMSG_71    <- tamano
   tocar el TIMBRE                      C2PMSG_64    <- la orden
   esperar un bit de USBSTS             esperar el bit 31
   anillo de eventos con su puntero     C2PMSG_67 = puntero de escritura
   un aparcadero para no perder avisos  respuestas por el mismo anillo
```

** BMO-X ya tiene ese molde escrito, partido en tres ficheros y **probado en el
Ryzen**: `xhci/lib.rs` (anillos, TRB, arranque, eventos), `enumerar.rs`,
`transferencia.rs`. Y el portero del bus del 07-09 ya sabe encontrar la tarjeta
y decir que esta.

---

## 4. Los numeros -- que es lo que se vino a buscar

```text
   [MEDIDO]  registros distintos que hacen falta          12
             35, 36, 58, 64, 67, 69, 70, 71, 81, 101, 102, 103
   [MEDIDO]  pasos hasta el primer anillo vivo            14
   [MEDIDO]  componentes de firmware del propio PSP        9
   [MEDIDO]  tipos de firmware que suben DESPUES          ~12
   [MEDIDO]  psp_v14_0.c                              ~800 lineas
   [MEDIDO]  amdgpu_psp.c                           ~3.200 lineas
   [DATO]    los blobs estan en `linux-firmware`, redistribuibles
```

### ⚠ Y el numero que NO es 4.000

De esas 4.000 lineas, **la mayoria no responde a ninguna pregunta que BMO-X se
haga** -- es la ley 24 otra vez, ahora medida:

```text
   TA: RAS, HDCP, DTM, XGMI          FUERA. Aplicaciones de confianza que
                                     BMO-X no usa (proteccion de contenido,
                                     telemetria de errores, multi-GPU)
   SR-IOV y virtualizacion           FUERA. Hay un camino ENTERO duplicado
                                     para maquinas virtuales
   debugfs, sysfs                    FUERA. Son APIs de Linux
   SPI-ROM, USB-PD                   FUERA. Actualizar la BIOS de la tarjeta
   quince familias de chips           -> UNA
   entrenamiento de memoria           condicional; se salta si el SOS ya vive
```

★ Lo que queda es **el patron de la seccion 2.1 escrito una vez, una tabla de
nueve componentes, y el anillo** -- que es el molde de xHCI. No es un fin de
semana, pero **es del orden del driver de AHCI**, que es exactamente lo que el
plan estimaba para la meta A.

---

## 5. Lo que sigue SIN saberse, y hay que decirlo

```text
   [POR CONFIRMAR]  que version de PSP usa Navi 48 exactamente.
                    Se mira en `amdgpu_discovery.c`, en el switch de MP0_HWIP.
                    ** Y NO ES UN RIESGO: v13 y v14 tienen la misma forma y
                    los mismos indices de registro. Cambia el prefijo del
                    bloque y algun componente
   [POR CONFIRMAR]  los OFFSETS numericos de esos registros para esa tarjeta.
                    Estan en las cabeceras del propio arbol de Linux
   [SIN COMPROBAR]  cuanto tarda la secuencia entera. Hay esperas de 20 ms
                    entre pasos y una de hasta 3 s en el entrenamiento
   [SIN COMPROBAR]  si el firmware de Navi 48 esta ya en `linux-firmware`
                    con licencia redistribuible
```

** Ninguno de los cuatro puede convertir esto en imposible. Son consultas, no
incognitas.

---

## 6. Que desbloquea esta medida

```text
   [x] el PSP deja de ser la pieza que puede matar la meta B
   [x] la meta A YA NO es independiente: el SDMA sube por el PSP.
       Hay que corregir el plan, y este documento es la correccion
   [x] el molde ya existe y esta probado en metal: es xHCI
   [ ] y sigue sin ser lo siguiente
```

### ⚠ La correccion al plan, dicha en voz alta

`PLAN_VULKAN.md` presenta la meta A como *"no toca el display (DCN)... el
firmware UEFI ya lo dejo programado"*, y eso **sigue siendo verdad para el
display**. Lo que no dice --y ahora se sabe-- es que **el motor de copia no se
enciende sin arrancar el PSP antes**.

```text
   antes   meta A = anillos + SDMA
   ahora   meta A = PSP (12 mensajes) + anillos + SDMA
```

No la hace imposible: le pone el precio real delante, que es lo que este
documento vino a hacer.

---

## 7. Fuentes

Todo lo de arriba sale de leer estos ficheros del arbol principal de Linux, que
son publicos:

* `drivers/gpu/drm/amd/amdgpu/psp_v13_0.c` -- la secuencia para RDNA 3 y afines
* `drivers/gpu/drm/amd/amdgpu/psp_v14_0.c` -- la de la generacion siguiente
* `drivers/gpu/drm/amd/amdgpu/amdgpu_psp.c` -- la capa generica: anillo, TMR y
  la carga del resto del firmware
* `drivers/gpu/drm/amd/amdgpu/amdgpu_discovery.c` -- que version le toca a cada
  chip

**Leidos, no copiados.** Ver la frontera de la seccion 0.
