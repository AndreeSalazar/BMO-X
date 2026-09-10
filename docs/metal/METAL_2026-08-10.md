# PRUEBA EN METAL -- que mirar en el arranque del 2026-08-10

Guia para el Ryzen. **No es una lista de deseos: es lo que hay que traer de
vuelta** para que el siguiente paso se decida con datos y no con teorias.

Han entrado seis commits y **ninguno ha visto un CPU**. Lo que cambia es por donde
entran los bytes de TODOS los programas del sistema.

---

# PARTE 0 -- Antes de arrancar, desde Windows

Dos cosas se pueden comprobar **sin reiniciar**, y si fallan no hace falta ir al
Ryzen.

## 0.1 -- Que el build produjo lo nuevo

Despues de `build.ps1`, cualquier `.bex` de `staging` tiene que cumplir dos cosas
que antes no cumplia:

```bash
python -c "import io;b=io.open('Ultra_kernel_x86-64/staging/BMO-DATA/sys/gui.bex','rb').read();n=int.from_bytes(b[40:44],'little');t=int.from_bytes(b[32:40],'little');print('secciones',n);[print(' kind=0x%02X off=%d %s'%(b[t+i*48],int.from_bytes(b[t+i*48+8:t+i*48+16],'little'),'<-- 512 OK' if int.from_bytes(b[t+i*48+8:t+i*48+16],'little')%512==0 else '')) for i in range(n)]"
```

| que buscar | por que importa |
|---|---|
| una seccion `kind=0x15` | los **Requisitos**. Si no esta, el escritor no los emitio |
| `Code`, `RoData` y `Data` con offset multiplo de **512** | sin eso, el camino rapido del disco no se toma nunca |

** Si falta cualquiera de las dos, **para aqui**: el problema es del build, no del
kernel, y arrancar no va a contestar nada.

## 0.2 -- Que el fichero no crecio de mas

`gui.bex` media 308.184 B. Con la alineacion nueva tiene que subir **menos de 1,5
KB**. Si sube mucho mas, algo se esta alineando que no deberia.

---

# PARTE 1 -- El arranque, en orden

## 1.1 -- La pregunta grande, y se contesta sola

> **Sale el escritorio?**

Si sale, `gui.bex` cargo por el camino nuevo y el bloqueo del 08-10 ya no esta.
Si no sale, sigue el resto de la guia: **ahora el sistema dice por que**.

## 1.2 -- Las tres lineas nuevas de CABINA

Buscar estas, en este orden:

```text
   proc:   programa admitido SIN MESA =<tid>
   lanzar: bytes DIRECTOS del disco al marco =<N>
   proc:   secciones sin hash con el que comparar =<N>
```

| linea | que significa si sale | que significa si NO sale |
|---|---|---|
| `admitido SIN MESA` | se tomo el camino sin bufer: la pieza B esta viva | se fue por `EnMemoria`, o sea que `Fuente` no dio rangos |
| `bytes DIRECTOS ... al marco` | **el numero que mide todo**: tiene que parecerse a lo que ocupan las secciones cargables | el camino rapido no se tomo |
| `secciones sin hash` | **tiene que valer 0** en un `.bex` del escritor | perfecto: todo cubierto |

** El numero de `bytes DIRECTOS` es el que hay que apuntar. Para `gui.bex`, las
secciones cargables suman unos **308 KB**; si el numero anda por ahi, los bytes
fueron del disco al marco sin pasar por ningun sitio. Si es mucho menor, algo
sigue rebotando.

## 1.3 -- Y el disco, que quedo abierto del arranque anterior

```text
   disk: avisos del disco por interrupcion =<N>
```

La vez pasada salio **sin numero, o sea CERO**: MSI quedo armado y no entregaba
nada. Si esta vez sube, la interrupcion llega. Si sigue en cero, **no es un
fallo** --la red de seguridad cubre-- pero entonces esa funcion esta muerta y hay
que decidir si se arregla o se quita.

---

# PARTE 2 -- Si falla: que mensaje sale, y a donde manda

Esto es lo que se compro con el trabajo de hoy. **Cada mensaje apunta a un sitio
distinto**, y son sitios que no se parecen en nada.

| mensaje | que paso de verdad | donde mirar |
|---|---|---|
| `la seccion Code no cuadra con su hash =N` | llego entera y llego MAL | transporte: el dato se corrompio por el camino. `N` = bytes que pasaron |
| `una seccion se quedo a medias al aterrizar =N` | llegaron `N` de los que hacian falta | lectura corta: cursor, cadena FAT, o el disco parando |
| `la tabla de relocs se quedo sin leer =N` | idem, pero en la tabla de punteros | lo mismo, y afecta sobre todo a DOOM (30.840 B) |
| `la tabla de hashes se quedo sin leer =N` | no se pudo traer la firma | el rango de `Signature` no se leyo |
| `cabecera invalida (magic, version o 0 secciones)` | el PROLOGO --los primeros 2 KB-- vino mal | la primera lectura del fichero. Es lo mas grave: falla lo mas simple |
| `el prologo traia un BEX valido y la lectura larga NO` | **no deberia salir ya**: ese camino no lee dos veces | si sale, se fue por `EnMemoria` sin querer |
| `otra version del ABI` / `otra arquitectura` | el `.bex` no es para este sistema | el toolchain, no el kernel |

## 2.1 -- Y la pregunta que hay que contestar sea cual sea el resultado

> **Se arreglo el fallo del 08-10, o simplemente ya no esta en el camino?**

No es lo mismo y conviene saberlo:

- **La mesa ya no existe** por FAT32. Una corrupcion que solo afectara a ese bufer
  compartido **no puede volver a pasar** -- eso es arreglado por retirada.
- **`tramo_dma` sigue en el camino.** `disk::read` sigue preguntandole a las tablas
  de pagina donde vive un buffer. Si el fallo era ese, **sigue ahi**.

Asi que:

| resultado | lo que dice |
|---|---|
| arranca todo | el sospechoso era la mesa o como se llenaba |
| sigue fallando, con mensaje de seccion | el sospechoso es la traduccion o el propio DMA |
| falla en el prologo | es lo mas basico del disco, y ninguno de los dos |

---

# PARTE 3 -- Los dos experimentos

Por este orden, y **solo si el escritorio salio**.

## 3.1 -- `run c/sonda.bex`

La sonda escrita para usar la superficie MAL a proposito, y que **nunca ha
llegado a correr**. Siete empujones: operaciones que no existen, handles
inventados, el renglon de ruta inundado, el tope de memoria forzado, tamanos
imposibles, un prestamo que nadie ofrecio, y reclamar la pantalla dos veces.

- **Lo que se espera:** `agujeros: 0`.
- **La prueba de fondo no es esa**: es que el programa **llegue a imprimir el
  recuento**. Si se cuelga en el empujon 3, el empujon 3 es el agujero, y el
  sintoma es que las lineas del 4 en adelante no salen.
- Cualquier `[FALLO]` es un agujero de verdad, no una sonda rota.

## 3.2 -- `run apps/doom.bex`

La meta. Y es el caso mas duro que existe hoy:

| | |
|---|---|
| **30.840 B de relocations** | primer programa que las usa en serio. Ahora se traen a marcos prestados y se sueltan |
| **el `.bex` mas grande** | 814.664 B, y su WAD son 4,2 MB **sueltos al lado**, no dentro |
| **nunca ha corrido** | no se sabe que mas hay roto detras |

Si arranca y pide el WAD, el siguiente muro probable es la memoria: 4,2 MB de
golpe contra el tope de `MEMORIA_PEDIR`, que hoy **no sabe devolver**.

---

# PARTE 4 -- Que traer de vuelta

Con esto se decide el siguiente paso sin adivinar nada:

1. **Foto de CABINA completa** en el momento del fallo (o del arranque bueno).
2. **El numero de `bytes DIRECTOS del disco al marco`**. Es el que mide si la
   pieza B funciona.
3. **`avisos del disco por interrupcion`**: cero o no cero.
4. **El mensaje exacto** si algo no paso la admision -- la tabla de la Parte 2
   traduce cada uno a un sitio distinto donde mirar.
5. Si corrio la sonda: **hasta que numero de empujon llego**.

** Y si no arranca ni el panel: eso tambien es informacion, y de la buena. Seria
la primera vez que este trabajo rompe algo mas gordo que el cargador, y acota el
problema a la parte del kernel que corre antes que nada.
