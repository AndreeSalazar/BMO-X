# Bitácora de guerra — BMO-X en hardware real

Episodios de debugging en metal desnudo (MSI A320M PRO MAX + Ryzen 5 5600X),
sin debugger, sin serial conectado: **solo fotos de la pantalla**. Cada
episodio: el síntoma, el culpable, y la moraleja que quedó grabada en el
código.

---

## Ep. 1 — El firmware que no quería soltar sus archivos
**Síntoma**: "no FAT filesystem found" — la MSI arranca con lector FAT
interno y jamás conecta drivers SimpleFS.
**Culpable**: fast-boot de fábrica sin opción visible.
**Moraleja**: no le pidas archivos al firmware — **embébelo todo** en un solo
BOOTX64.EFI (shim unificado con las etapas y el kernel adentro). Cero
dependencias, cero mercedes.

## Ep. 2 — El triple fault que solo pasaba en hardware
**Síntoma**: bootea en QEMU, reset instantáneo en la placa real.
**Culpable**: el firmware entrega con interrupciones ENCENDIDAS; un IRQ en
plena cirugía de GDT despacha con tablas inconsistentes.
**Moraleja**: `cli` + enmascarar el PIC ANTES de tocar la GDT. QEMU es un
mundo sin ruido; el hardware real tiene tráfico.

## Ep. 3 — Los GUIDs mal copiados (o: por qué nunca hubo framebuffer)
**Síntoma**: meses creyendo que la placa "no tenía GOP".
**Culpable**: los GUID de GOP y SimpleFS estaban mal escritos (data4
corrupto). El proyecto siempre corrió por serial y nadie lo notó.
**Moraleja**: un GUID es una contraseña de 16 bytes: o es EXACTA o el
universo responde "no existe".

## Ep. 4 — El CS fantasma de UEFI (la saga del #GP, capa 1)
**Síntoma**: #GP(0) eterno en el iretq del timer; frame fabricado PERFECTO,
GDT PERFECTA, CR3 compartida PERFECTA. Semanas de misterio.
**Culpable**: `init_gdt` hacía `lgdt` + recargaba los segmentos de datos...
**pero nunca el CS**. El CPU ejecutó Ring 0 entero con el descriptor UEFI
(cs=0x38) cacheado en el shadow register. Todo funcionaba — hasta que un
iretq re-validó ese selector contra NUESTRA GDT (entrada 7: vacía).
**Moraleja**: `lgdt` no recarga CS. El far-return (`push CS; push RIP;
retfq`) no es opcional — es el bautizo real del kernel.

## Ep. 5 — El split-brain de gs (la saga del #GP, capa 2)
**Síntoma**: el contexto se publicaba y el epílogo leía CEROS.
**Culpable**: el asm escribía por `gs:[0x10]` (MSR GS_BASE) y el Rust leía
el static directo. Dos caminos a "la misma" memoria que solo coinciden si
GS_BASE apunta donde crees — y el CS fantasma (Ep. 4) disparaba swapgs
espurios que lo movían.
**Moraleja**: para datos per-CPU, **un solo camino de acceso**. Escritor y
lector deben concordar POR CONSTRUCCIÓN, no por fe.

## Ep. 6 — El framebuffer invisible (la saga del #GP, capa 3)
**Síntoma**: con las capas 1 y 2 arregladas… congelamiento TOTAL sin
pantalla ni fault. El hola mundo Ring 3 SÍ ejecutaba — moría *pintando*.
**Culpable**: el address space de usuario comparte identidad solo 0..1 GiB;
el fb GOP vive en ~3.5 GiB. El flush de consola pintaba bajo la CR3 del
usuario → #PF → el reporter de faults TAMBIÉN pinta → #PF recursivo
infinito en IST1.
**Moraleja**: pregunta siempre **bajo qué CR3 corres** antes de tocar MMIO.
Y un fault handler jamás debe poder causar su propio fault.

## Ep. 7 — El teclado que funcionaba de prestado
**Síntoma**: en BMO/FastOS v0.6–0.9 el teclado escribía; en el BMO-X real,
silencio (solo ruido 0xFE del i8042, ni el LED de Bloq Mayús responde).
**Culpable**: antes los Boot Services estaban vivos y el firmware hacía el
USB por nosotros (emulación SMM USB→PS/2). Al convertirnos en un OS de
verdad (ExitBootServices), el firmware se llevó su magia.
**Moraleja**: la soberanía se paga con drivers. Lo que el firmware te
"regala" es un préstamo con fecha de vencimiento.

## Ep. 8 — El xHC escondido detrás del bridge
**Síntoma**: "[usb] no se encontro controlador xHCI" — con el controlador
ahí, funcionando.
**Culpable**: el scan PCI del boot era plano (bus 0); en Ryzen los xHC
cuelgan de buses detrás de bridges.
**Moraleja**: en PCI, si no recorres TODOS los buses, no has buscado. Y sin
habilitar Bus Master (BME), un controlador DMA es un adorno.

## Ep. 9 — "Nel, llegas tarde" (el CPU impaciente)
**Síntoma**: xHC inicializado perfecto (127 slots, 22 puertos)… y cero
dispositivos en los puertos. El teclado SEISA conectado, ignorado.
**Culpable**: el spec USB exige ~100 ms de debounce para detectar conexión.
El driver (criado en QEMU, donde todo es instantáneo) esperaba
~microsegundos. Para un Zen 3 a 4.6 GHz, 100 ms son una era geológica — y
no estaba dispuesto a esperarla.
**Moraleja**: el hardware real tiene TIEMPOS FÍSICOS. La paciencia no es
una virtud del CPU: hay que programársela (delays por TSC, no spin-counts).

---

## Las tres leyes que dejó esta guerra

1. **QEMU miente por omisión**: sin IRQs vivos, sin tiempos físicos, sin
   memoria con huecos. Todo lo que "funciona en QEMU" es una hipótesis.
2. **Los bugs viejos disfrazan a los nuevos**: el CS fantasma (Ep. 4)
   causaba el split-brain (Ep. 5) que tapaba el fb invisible (Ep. 6). Se
   pelan como cebolla, en orden, con una foto por capa.
3. **La telemetría en pantalla vale más que mil teorías**: cada episodio
   cayó cuando el sistema mismo confesó (filas de diagnóstico, censos,
   heartbeats). Si no puedes verlo, no puedes matarlo.

*Debuggeado a fotos de pantalla, entre un humano con hardware y una IA sin
ojos. 2026.*
