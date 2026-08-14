# PLAN DE ALMACENAMIENTO -- repartir la pila de disco

> Escrito el 2026-08-14. Mismo formato que `PLAN_DOOM.md` y `PLAN_BANCA.md`:
> casillas ordenadas, cada una con **que la bloquea** y **como se sabe que
> quedo hecha**.

---

## 0. El terreno, MEDIDO (no estimado)

```
platform/drivers/storage/
  block/       200    EL CONTRATO -- sin dependencias, bien escrito
  ahci/        993    controller 929 + storage_hal 44 + lib 20
  fat32/     2.453    EN UN SOLO FICHERO
  estratos/  1.838    ya repartido en 5

Ultra_kernel_x86-64/kernel/src/ring0/dev/disk/
  mod.rs       893    <- EL MONOLITO
  transfer.rs  297
  gate.rs      120
  owner.rs      94
```

Se confirma lo que ya decia la memoria del proyecto: **el monolito vive en el
pegamento del kernel, no en los drivers.** El driver de AHCI son 929 lineas que
hacen UNA cosa; `dev/disk/mod.rs` son 893 que hacen siete.

### 0.1 El contrato existe y tiene DOS PUERTAS TRASERAS

| Camino | Quien | Pasa por el contrato? |
|---|---|---|
| `bmo_block::BlockDevice` | el kernel registra `AhciDisk` | SI -- es el contrato |
| puntero a funcion | FAT32 (`lib.rs:168`) | **NO** |
| `dev::disk::block_read` y companeros, publicas | quien quiera | **NO** |

**Un contrato con dos bypasses no es un contrato.** Misma clase de hallazgo que
`cpu_features` y `_reserved` en la cabecera BEF (patron 40): la regla queda
escrita, es buena, y nada la hace cumplir.

[!] **Por eso el paso 0 va antes que cualquier reparto.** Partir una pila que
todavia tiene tres puertas es repartir el problema en tres sitios.

---

## 1. POR QUE se divide asi, y no como Linux

Se consideraron cuatro criterios. No son "miles de formas": son cuatro, y el
criterio elige uno.

| | Como divide | Veredicto |
|---|---|---|
| A. Por hardware | una crate por controlador | No sirve: el monolito no vive en el driver |
| B. Por capa (Linux) | dispositivo -> block layer -> particion -> fs | Invita a construir maquinaria que no hace falta |
| C. **Por PREGUNTA que responde** | patron 39 | **ESTE** |
| D. Por testabilidad | lo que se prueba sin disco | El desempate, y coincide con C |

### Por que B no, aunque sea Linux

El block layer de Linux son ~50k lineas: *elevators* (mq-deadline, bfq, kyber),
fusion de peticiones, plugging, splitting de bios. Todo eso resuelve problemas
que BMO **no tiene todavia**: muchos peticionarios a la vez, discos rotatorios,
miles de IOPS por segundo. De Linux se copia la **estratificacion**; la
maquinaria, no. *Esencia acotada = terminable.*

### Las siete preguntas de `dev/disk/mod.rs`

```
que hardware hay?        deteccion + IDENTIFY        info del dispositivo
puedo tocarlo?           el gate de identidad        -> gate.rs (a medias)
de quien es ahora?       el boss                     -> owner.rs
donde estan las cosas?   MBR / GPT                   ** CERO HARDWARE: formato
puedo escribir AQUI?     la ventana de escritura     ** CERO HARDWARE: politica
mueve estos bytes        transfer                    -> transfer.rs
avisame cuando acabe     IRQ                         plumbing
```

**Dos de las siete no tocan hardware.** Parsear una tabla de particiones es leer
bytes con un formato --igual que BEF--, asi que admite un CENSO como los de C,
sin encender la maquina. La ventana de escritura es politica pura y hoy vive
mezclada con los registros del HBA.

Eso es el corte, y sale del criterio C con D de desempate.

---

## 2. DONDE BMO PUEDE SER MEJOR QUE LINUX (no solo mas pequeno)

Tres cosas que Linux, por estructura, no puede hacer:

1. **Un disco es una capability, no `/dev/sda`.** En Linux el dispositivo es un
   nombre global y el acceso son bits de permiso; por eso existe
   `dd if=/dev/nvme0n1` y por eso root lo puede todo. En BMO el gate de
   identidad puede ser **la unica forma de obtener el handle**, no una politica
   encima. Medio construido ya: `gate.rs`, `owner.rs`.

2. **La ventana de escritura no tiene equivalente en Linux.** Ninguno. Un
   dispositivo que solo acepta escrituras dentro de un rango de LBA declarado
   es una idea mejor, y merece vivir **en el contrato**, no como caso especial
   del pegamento. Es lo que protege el disco de Windows del boss.

3. **Asincrono por construccion.** Sin legado, el contrato puede nacer async
   --la leccion del IRP de NT-- en vez de sincrono con async pegado despues,
   que es lo que Linux arrastra desde hace veinte anos.

---

## 3. LAS CASILLAS

### [x] Paso 0 -- UNA SOLA PUERTA

FAT32 deja los punteros a funcion y toma `&'static dyn BlockDevice`.

* **Tamano real, medido**: FAT32 embudo TODO su I/O en **cuatro metodos**
  (`leer_directo`, `read_sector`, `write_sector`, `write_from`). Un solo sitio
  invoca `(self.read)`. El fichero son 2.453 lineas y la conversion toca cuatro.
* **Que gana**: identidad, capacidad, `flush` de verdad, `writable()` y errores
  tipados en vez de `bool`.
* **Que NO se pierde**: las pruebas siguen inyectando un disco falso, ahora como
  un `static` que implementa el trait.
* **Hecho cuando**: `grep BlockReader` no encuentra nada y las pruebas de fat32
  siguen verdes.

### [x] Paso 1 -- LAS PARTICIONES FUERA (MBR + GPT)

A `platform/drivers/storage/particiones/`. Cero hardware: entra un buffer de
sectores, sale una lista.

* **Que se lleva**: `scan_partitions`, `partitions`, `data_partition`,
  `fijar_particion_datos`, `last_lba`, `le32`, `le64`.
* **Hecho cuando**: la crate compila sin dependencias de kernel Y tiene un censo
  con tablas escritas a mano (una MBR, una GPT, una GPT con CRC malo).

### [x] Paso 2 -- LA VENTANA DE ESCRITURA, SOLA

A `ring0/dev/disk/ventana.rs`. Politica pura, funcion pura, con test.

* **Que se lleva**: `armar_ventana_estratos`, `desarmar_ventana_estratos`,
  `ventana_estratos`, `write_armed`, y la comprobacion de `write_window`.
* **Hecho cuando**: existe un test que pide escribir FUERA de la ventana y falla
  con motivo, sin tocar un disco.

### [x] Paso 3 -- EL IRQ FUERA

A `ring0/dev/disk/irq.rs`: `atender_irq`, `irq_estado`, `IRQ_ARMADA`, `IRQS`.

* **Hecho cuando**: `mod.rs` no menciona interrupciones.

### [ ] Paso 4 -- FAT32, EL OTRO MONOLITO (trabajo aparte)

2.453 lineas en un fichero. **No entra en este plan**: es un reparto tan grande
como el del compositor y merece su propia sesion, con el metodo del diff por
lineas (patron del reparto de `_start`).

---

## 4. HECHO EL 2026-08-14



** Lo que se gano NO son las 111 lineas de : son **14 casillas nuevas
que corren en 0 segundos y sin disco**, sobre codigo que antes solo se podia
probar arrancando la maquina.

[!] **La leccion del paso 2, y hay que conservarla**: la decision de la ventana
se escribio primero en  con sus pruebas, y **las
pruebas no corrian** --  es un binario  para
 y  ni compila. Un 
en el kernel es codigo que parece una prueba y no se ejecuta jamas. La funcion
pura tuvo que irse al CONTRATO para que sus casillas existieran de verdad.

**Regla que queda: si una funcion del kernel merece pruebas, es que no
pertenece al kernel.**

⏳ Pendiente de metal: nada de esto se ha arrancado.  sigue siendo el
unico camino por el que el kernel encuentra el disco del que arranca.

## 5. LA META

`dev/disk/mod.rs` se queda con **el registro del dispositivo y poco mas**. Todo
lo que responda una pregunta que no sea *"que disco es este y como se le habla"*
vive en otro sitio.

[!] **Riesgo, dicho**: `census` es el unico camino por el que el kernel
encuentra el disco del que arranca. Cualquier casilla que lo toque se verifica
en metal ANTES de seguir con la siguiente.
