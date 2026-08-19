# PLAN DE FAT32 -- la frontera, y las perillas que nadie gira

> Escrito el 2026-08-18. Es el **paso 4** que `PLAN_ALMACENAMIENTO.md` dejo
> anotado y saco fuera: *"FAT32, EL OTRO MONOLITO -- 2.453 lineas en un
> fichero, merece su propia sesion."*
>
> **Segunda version, del mismo dia.** La primera proponia ocho pasos hacia un
> FAT32 completo. Se reescribio entera despues de medir el disco de verdad y
> de contestar dos preguntas del dueno: *"si no uso FAT32 sino que me inspiro
> y hago evolucion, que potencial hay"* y *"FAT32 por si misma quiza esta
> subestimando algo"*. Las dos tienen respuesta y ninguna era la que el plan
> de la manana daba por supuesta.

---

## 0. EL TERRENO, MEDIDO -- y esta vez el disco de verdad

### 0.1 Lo que hay en el arbol

```
platform/drivers/storage/fat32/
  Cargo.toml        20    UNA dependencia: bmo-block. El contrato, bien.
  src/lib.rs     2.536    <- TODO
                 1.702    codigo
                   834    pruebas (21 casillas, verdes en 0,00 s)
                    79    funciones
                     6    `match self.fs_type` -- DOS formatos en un tipo
```

**No hay un FAT32 alternativo.** Se busco `bytes_per_sector`, `BytsPerSec`,
`sectors_per_cluster`, `root_cluster`, `0xE5` y `BPB` en todo el repo: fuera de
esta crate no hay ni una linea que interprete un BPB. Los demas aciertos son
opcodes x86 (`0x48 0x89 0xE5` = `mov rbp, rsp`) en los generadores de C, COBOL
y Ada. La consume un solo sitio: el kernel, desde `ring0/fsys/fs.rs`.

### 0.2 Y lo que hay en el metal (2026-08-18)

```
Disco 0  KINGSTON SA400S37480G   447,1 GB  SATA  GPT     <- EL DISCO DE BMO
  P1     0,59 GB   tipo System (ESP)   FAT32  cluster 4.096   offset 1 MB
  P2  A: 32,00 GB  tipo Basic  "BMO"   FAT32  cluster 32.768  offset 601 MB
  P3  F: 414,54 GB tipo Basic          Windows no lo reconoce  <- ESTRATOS

Disco 1  KINGSTON SA400S37120GB  111,8 GB  SATA   Ventoy
Disco 2  KINGSTON SNV2S1000G     931,5 GB  NVMe   [!] EL WINDOWS DEL DUENO
```

El disco de BMO declara **"Recorte admitido"** y **"Sin penalizacion de
busqueda"**: es un SSD y acepta TRIM. Sector logico 512, fisico 512.

Contenido real de `A:` -- **100 MB usados de 32 GB**:

```
A:\EFI\BOOT\BOOTX64.EFI      0,91 MB
A:\EFI\BOOT\BMO-MANIFEST.TXT
A:\sys\gui.bex             494,3 KB
A:\sys\precio.bex           24,8 KB
A:\apps\doom.bex           860,7 KB
A:\apps\doom640.bex        852,3 KB
A:\apps\doom1.wad        4.097,7 KB
A:\datos\*.txt            0 a 5,2 KB
A:\ada  A:\c  A:\cobol
```

### 0.3 Tres cosas que la medicion destapa

**1. `A:` NO es la ESP.** Su tipo GPT es Basic; la ESP es la P1 de 0,59 GB.
`fs.rs` monta la ESP por GUID y monta como volumen de DATOS *"la primera
particion que no es la de arranque y contesta como FAT32"* -- que es `A:`. O
sea: BMO escribe en `A:`, y `A:` lleva ademas su propio `BOOTX64.EFI`. Que las
dos cosas convivan ahi es deliberado o es casualidad, pero **no esta escrito en
ningun sitio**, y el plan no puede fingir que si.

**2. Nadie eligio el tamano de cluster.** `A:` tiene clusters de 32 KiB y la ESP
de 4 KiB. Eso no lo decidio BMO: lo decidio la tabla fija de `format.com` segun
el tamano de la particion. Es la primera pista de la seccion 4.

**3. El limite de 8.3 esta mordiendo HOY.** `fs.rs` declara
`LoadError::NameTooLong -- "Un nombre no cabe en 8.3"`. No es una carencia
teorica: es un error que el kernel ya sabe devolver.

### 0.4 La otra mitad: ESTRATOS ya existe

```
platform/drivers/storage/estratos/   2.206 lineas, 53 pruebas verdes
toolchain/tools/estratos-fmt/        formatea un volumen desde el anfitrion
kernel ring0/fsys/estratos/          monta, comprueba identidad y LEE en metal
```

`Entrada.size` es un **u64** (`objects.rs:153`) y `BLOCK_SIZE` es **4096** por
constante (`lib.rs:90`). O sea que la "evolucion sin limite de 4 GB y alineada
al SSD" no hay que disenarla: esta escrita, con 53 casillas verdes, y le falta
**una** cosa para 1.0 -- escribir contenido de verdad y releerlo tras reiniciar.

**Ese es el hecho que reordena este plan.** BMO no necesita que FAT32 sea su
sistema de ficheros. Necesita que sea su **frontera**.

---

## 1. LA DISECCION -- FAT32 byte a byte

FAT32 son **cuatro regiones** y ni una mas.

```
LBA relativo a la particion
0                            EL SECTOR DE ARRANQUE (BPB)
1                            FSInfo
2..RsvdSecCnt                reservado; 6 = copia del BPB, 7 = copia del FSInfo
RsvdSecCnt                   FAT #0
+ FATSz32                    FAT #1 (espejo)
RsvdSecCnt + NumFATs*FATSz   LA ZONA DE DATOS -- el cluster 2 empieza AQUI
```

### 1.1 El BPB: 90 bytes que definen la geometria

| off | tam | campo | que es | BMO lo lee? |
|---|---|---|---|---|
| 0 | 3 | `BS_jmpBoot` | salto | no (da igual) |
| 3 | 8 | `BS_OEMName` | texto | **si** -- para oler "EXFAT   " |
| 11 | 2 | `BPB_BytsPerSec` | 512/1024/2048/**4096** | si, y **rechaza todo lo que no sea 512** |
| 13 | 1 | `BPB_SecPerClus` | potencia de 2, 1..128 | si |
| 14 | 2 | `BPB_RsvdSecCnt` | donde empieza la FAT (32 tipico) | si |
| 16 | 1 | `BPB_NumFATs` | **1 es legal**; el formateador pone 2 | si |
| 17 | 2 | `BPB_RootEntCnt` | 0 en FAT32 | no |
| 19 | 2 | `BPB_TotSec16` | 0 en FAT32 | no |
| 21 | 1 | `BPB_Media` | 0xF8 fijo, 0xF0 extraible | no |
| 22 | 2 | `BPB_FATSz16` | 0 en FAT32 | no |
| 32 | 4 | `BPB_TotSec32` | sectores del volumen | si |
| 36 | 4 | `BPB_FATSz32` | sectores por FAT | si |
| 40 | 2 | `BPB_ExtFlags` | **que FAT esta activa** | **NO -- ver 3.3** |
| 42 | 2 | `BPB_FSVer` | 0x0000; otro = no montar | **no** |
| 44 | 4 | `BPB_RootClus` | primer cluster de la raiz | si |
| 48 | 2 | `BPB_FSInfo` | sector del FSInfo | no |
| 50 | 2 | `BPB_BkBootSec` | copia del BPB (suele ser 6) | **no** |
| 52 | 12 | `BPB_Reserved` | **12 bytes libres** | no |
| 66 | 1 | `BS_BootSig` | 0x29 si hay VolID/Label | si |
| 67 | 4 | `BS_VolID` | numero de serie | no |
| 71 | 11 | `BS_VolLab` | etiqueta | no |
| 82 | 8 | `BS_FilSysType` | `"FAT32   "` -- **NO ES AUTORIDAD** | no |
| 510 | 2 | firma | `0xAA55` | **NO lo comprueba en FAT32** |

### 1.2 Como se decide QUE FAT es -- el unico metodo valido

```
RootDirSectors = ((RootEntCnt * 32) + (BytsPerSec - 1)) / BytsPerSec   // 0 en FAT32
FATSz    = FATSz16 ? FATSz16 : FATSz32
TotSec   = TotSec16 ? TotSec16 : TotSec32
DataSec  = TotSec - (RsvdSecCnt + NumFATs*FATSz + RootDirSectors)
Clusters = DataSec / SecPerClus

Clusters <  4085   -> FAT12
Clusters < 65525   -> FAT16
resto              -> FAT32
```

**No es una heuristica entre varias: es LA definicion.** `BS_FilSysType` es
decorativa y la especificacion dice que no se use para decidir. BMO decide por
`BS_BootSig`, que vale 0x29 tambien en FAT16 (ver 3.1).

### 1.3 La FAT: un vector de u32 y cuatro valores especiales

| valor (28 bits bajos) | significa |
|---|---|
| `0x0000000` | LIBRE |
| `0x0000002`..`max` | ocupado; **apunta al siguiente** |
| `0x0FFFFFF7` | cluster DEFECTUOSO -- nunca asignar |
| `>= 0x0FFFFFF8` | FIN DE CADENA |

**Los 4 bits altos son reservados: se ignoran al leer y se PRESERVAN al
escribir.** BMO los pisa a cero (3.2). Y ver 4.C1: son un espacio que existe y
que NO es nuestro.

Las dos primeras entradas no son clusters:

- `FAT[0]` = `0x0FFFFF00 | BPB_Media`, o sea `0x0FFFFFF8` normalmente.
- `FAT[1]` = fin de cadena **mas dos banderas de estado del volumen**:
  - bit 27 `0x08000000` **ClnShut** -- 1 = se desmonto limpio. A 0 = sucio.
  - bit 26 `0x04000000` **HrdErr** -- 0 = hubo un error de E/S sin resolver.

### 1.4 El FSInfo (sector 1)

| off | valor |
|---|---|
| 0 | `0x41615252` (`"RRaA"`) |
| 484 | `0x61417272` (`"rrAa"`) |
| 488 | `FSI_Free_Count` -- clusters libres; `0xFFFFFFFF` = no se sabe |
| 492 | `FSI_Nxt_Free` -- por donde seguir buscando; `0xFFFFFFFF` = ni idea |
| 508 | `0xAA550000` |

Son pistas, no verdad: la FAT manda. Pero `FSI_Nxt_Free` es la diferencia entre
asignar en O(1) y en O(toda la FAT) -- ver el numero de 4.A4.

### 1.5 La entrada de directorio: 32 bytes

```
 0  11  Name      8.3, EN MAYUSCULAS, rellenado con espacios, SIN punto
11   1  Attr      0x01 RO | 0x02 Hidden | 0x04 System | 0x08 VolumeID
                  0x10 Directory | 0x20 Archive | 0x0F = FRAGMENTO DE NOMBRE LARGO
12   1  NTRes     bit 3 = base en minusculas, bit 4 = extension en minusculas
                  ** LOS OTROS SEIS BITS ESTAN LIBRES -- ver 4.B3
13   1  CrtTimeTenth   0..199
14   2  CrtTime
16   2  CrtDate
18   2  LstAccDate
20   2  FstClusHI      <- la mitad alta del cluster
22   2  WrtTime
24   2  WrtDate
26   2  FstClusLO
28   4  FileSize       <- u32. AQUI vive el limite de 4 GiB, y solo aqui
```

`Name[0]`: `0x00` = fin del directorio y no hay nada mas detras; `0xE5` =
borrada; `0x05` = el primer byte es realmente 0xE5.

```
hora  = (h << 11) | (min << 5) | (seg / 2)
fecha = ((anio - 1980) << 9) | (mes << 5) | dia
```

BMO escribe **ceros** en los seis campos de tiempo.

### 1.6 El nombre largo (LFN) -- lo que hoy se TIRA

Entradas de `Attr = 0x0F` **justo antes** de la corta y **en orden inverso**:

```
 0   1  Ord       n de orden desde 1; el ULTIMO lleva 0x40 puesto
 1  10  Name1     5 caracteres UCS-2
11   1  Attr      0x0F
12   1  Type      0
13   1  Chksum    <- ATA la cadena a su entrada 8.3
14  12  Name2     6 caracteres UCS-2
26   2  FstClusLO 0 siempre
28   4  Name3     2 caracteres UCS-2
```

13 caracteres por entrada. El pegamento es la suma del nombre 8.3:

```
sum = 0
para cada uno de los 11 bytes del nombre corto:
    sum = ((sum & 1) << 7) + (sum >> 1) + byte      // rotacion a la derecha
```

Si la suma no cuadra, la cadena es basura huerfana: se ignora y manda el 8.3.

BMO hoy hace esto, literalmente, en dos sitios:

```rust
if attr & 0x0F == 0x0F { continue; } // fragmento de nombre largo
```

---

## 2. LO QUE UEFI EXIGE -- y no es opcional

UEFI define su sistema de ficheros en la seccion *File System Format* (13.3 en
las revisiones recientes) y **no inventa un formato**: adopta FAT con reglas.

1. **La ESP es FAT.** En disco fijo, FAT32; en extraible se admiten FAT12 y
   FAT16. GUID `C12A7328-F81F-11D2-BA4B-00A0C93EC93B`, tipo MBR `0xEF`. BMO ya
   lo respeta: pide la ESP **por tipo**.
2. **El tipo se decide por CUENTA DE CLUSTERS**, y UEFI prohibe explicitamente
   usar `BS_FilSysType`. Es el algoritmo de 1.2.
3. **Los nombres son UCS-2.** `EFI_FILE_PROTOCOL::Open` recibe `CHAR16*`. El
   nombre largo no es una extension opcional: **es el nombre**.
4. **Comparacion sin distinguir mayusculas, almacenamiento conservando el
   caso.** `\EFI\Boot\BootX64.efi` y `\EFI\BOOT\BOOTX64.EFI` son el mismo.
5. **La ruta de respaldo** es `\EFI\BOOT\BOOTX64.EFI` en x86-64.
6. **El sector logico no tiene por que ser 512.** `EFI_BLOCK_IO_PROTOCOL`
   declara su `BlockSize` y `BPB_BytsPerSec` admite hasta 4096.
7. La forma de arriba -- `Open`, `Read`, `Write`, `SetPosition`, `GetInfo`,
   `SetInfo`, `Flush`, `Delete` -- es **el examen**, y esta escrito por otro.

**Hoy BMO contesta que si a dos de siete** (1 y 5).

### 2.1 Y por eso FAT no se puede tirar

El firmware de la MSI A320M lee `\EFI\BOOT\BOOTX64.EFI` **con su propio driver
FAT, antes de que exista una sola instruccion de BMO**. La ESP no es una
eleccion de diseno: es el apreton de manos con la maquina. El dia que ESTRATOS
sea 3.0 y perfecto, la ESP seguira siendo FAT32.

La pregunta correcta no es *"FAT32 o evolucion"*. Es **"cuanto FAT necesito, y
que le puedo sacar al que necesito"**.

---

## 3. EL CENSO DE FALLOS -- donde el codigo de hoy se aparta del formato

### 3.1 Un FAT16 se monta como FAT32 y se lee mal, en silencio

`lib.rs:323` acepta el volumen si `BS_BootSig` es 0x29 o 0x28. **FAT16 tambien
vale 0x29.** Luego se leen `BPB_FATSz32` y `BPB_RootClus`, que en un FAT16 son
ceros o basura. No hay comprobacion de `0xAA55` en el camino de FAT32 (si la
hay en el de exFAT, `lib.rs:342`).

Cuando muerde: **cualquier pendrive**. Los extraibles pequenos salen FAT16 y
UEFI los admite como ESP.

### 3.2 Los 4 bits altos de la FAT se pisan a cero

`set_fat_entry` (`lib.rs:1079`) hace `value & 0x0FFF_FFFF` y escribe los 32
bits. La especificacion pide preservar los 4 altos. Hoy valen cero en casi
todos los volumenes: es un fallo **latente**, y se anota porque es una
desviacion real, no porque este rompiendo algo esta semana.

### 3.3 `BPB_ExtFlags` no se lee: se escribe en una FAT que puede estar muerta

Bit 7 (`0x0080`) = *"el espejo esta apagado; solo una FAT esta activa"*; bits
0..3 = **cual**. BMO no lo mira: escribe en las `NumFATs` copias
(`lib.rs:1081`) y lee siempre de la #0.

### 3.4 El volumen se deja marcado como "limpio" siempre

Ni `ClnShut` ni `HrdErr` se tocan nunca. BMO monta `A:` **con escritor**,
escribe, y si la maquina se apaga a medias el siguiente arranque de Windows no
tiene forma de saberlo.

### 3.5 El FSInfo se ignora, y el barrido cuesta lo que cuesta

`find_free_cluster` (`lib.rs:1036`) recorre la FAT **desde el cluster 2** cada
vez. Con los numeros de `A:` (32 GB, cluster 32 KiB):

```
1.048.576 clusters  x 4 B  =  4 MiB de FAT  =  8.192 sectores  (x2 por el espejo)

hoy, con A: casi vacia   -> el primer hueco sale enseguida, apenas se nota
al 50% de ocupacion      -> ~4.096 sectores leidos POR CLUSTER asignado
escribir doom1.wad (4 MB = 128 clusters) al 50%  ->  ~256 MiB leidos para
                                                     escribir 4
```

**El fallo no se ve hoy porque el volumen esta al 0,3%.** Aparece cuando se usa.

### 3.6 Un directorio lleno es un error, no un directorio que crece

`find_free_dir_entry_fat32` (`lib.rs:1139`) recorre la cadena y al acabarse
devuelve `None`, que se traduce a `WriteError::DirFull`. Un directorio de FAT32
crece: se pide un cluster, se encadena, se llena de ceros.

### 3.7 Lo que sencillamente no existe

Borrar. Crear directorio. Renombrar. Truncar. Escribir **en medio**
(`obj/file.rs`: *"bmo_fat32 escribe un archivo ENTERO de una vez"*). Fechas.
Atributos. Etiqueta del volumen.

### 3.7b Lo del sector a sector, CON LA CORRECCION PUESTA

[!] **La primera version de este plan decia que TODA la E/S iba de sector en
sector. Es falso, y se dice con el mismo tamano de letra.** Se comprobo
despues: `leer_tramo`, `leer_en` y `read_file` llaman a
`leer_directo(lba, enteros, dst)` -- **un solo comando por cluster**. El camino
de lectura de datos ya esta bien hecho, y un plan no puede cobrarse un arreglo
que ya estaba hecho.

Lo que si va de uno en uno, contado:

| camino | como | coste en `A:` (cluster de 64 sectores) |
|---|---|---|
| leer datos de un archivo | `leer_directo`, multi-sector | **1 comando por cluster** -- correcto |
| **escribir** | `write_from(lba + s, ..)` en bucle | **64 comandos por cluster** |
| **recorrer un directorio** | `read_sector(lba + s, ..)` en bucle, en SIETE sitios | **64 comandos por cluster de directorio** |
| la FAT | `read_sector`, con cache de un sector | 1 comando por cada 128 entradas de cadena seguidas |

O sea: **leer un archivo ya es barato; escribirlo y encontrarlo no.** Leer
`doom1.wad` son ~129 comandos de datos; escribirlo serian **8.196**.

### 3.8 exFAT es un pasajero que no llego

Cinco funciones `*_exfat`, cuatro structs, seis `match self.fs_type`, y
`create_file_exfat` marcado `#[allow(dead_code)]` por su propio comentario.
Escribir en exFAT devuelve `Unsupported`. **Dos formatos en un tipo, y uno a
medias**: el "cerebro" que la regla de la casa prohibe.

---

## 4. LO QUE FAT32 PERMITE Y NADIE USA

> Esta seccion es la que pidio el dueno: *"quiza FAT32 por si misma esta
> subestimando algo"*. Si. Pero no donde suele buscarse.

**FAT32 no se juzga por lo que la especificacion admite: se juzga por como lo
formatea `format.com`.** El formateador de Windows decide `NumFATs = 2`,
`BytsPerSec = 512` y el tamano de cluster por una tabla fija segun el tamano de
la particion. Ninguna de esas tres es una obligacion del formato. Son valores
por defecto de 1996 pensados para discos que giraban.

Tu `A:` tiene clusters de 32 KiB porque mide 32 GB. Nadie penso en `doom1.wad`.

Van tres niveles, y **el orden importa**: el primero es gratis, el segundo se
paga con un riesgo acotado, y el tercero no se hace.

### NIVEL A -- perillas del estandar, 100% conformes, riesgo cero

#### A1. `NumFATs = 1`: la mitad de la amplificacion de escritura

El espejo de la FAT se invento para disquetes, donde un sector defectuoso se
llevaba a sus vecinos y tener la copia LEJOS salvaba el volumen. En un SSD con
wear leveling **la copia esta donde el controlador quiera** y no protege de
nada: el sector se lee o no se lee.

Lo que si hace es **duplicar las escrituras en los sectores mas calientes del
disco**. Cada cluster que se asigna reescribe el mismo sector de la FAT, dos
veces, y en `A:` esos 8.192 sectores son el punto mas machacado del volumen.

* Conforme: `BPB_NumFATs` admite 1. La especificacion lo dice.
* Ganancia: **-50% de escrituras en la FAT.**
* Coste honesto: `chkdsk` avisa, y si la FAT se corrompe no hay copia. En un
  volumen que es la FRONTERA y no el sistema de ficheros, eso se aguanta.

#### A2. `BPB_BytsPerSec = 4096`: el limite de 2 TiB no es del formato

Con sector logico de 4096 en el BPB:

| | 512 B | 4096 B |
|---|---|---|
| Volumen maximo (`TotSec32` u32) | 2 TiB | **16 TiB** |
| Sectores de FAT en `A:` | 8.192 | **1.024** |
| Alineacion del cluster con la pagina | por suerte | **por construccion** |

* Conforme: la especificacion lista 512, 1024, 2048 y 4096.
* Caveat medido: tu SA400S37480G reporta **512 logico y 512 fisico**, asi que
  Windows no declara ganancia. La NAND de debajo tiene paginas de 8 a 16 KiB de
  todas formas -- la alineacion sigue importando aunque el disco no la anuncie.
* Caveat de arranque: **no tocar la ESP.** Hay firmwares UEFI que solo prueban
  512 ahi. Para `A:`, que la lee BMO y Windows, es seguro.

#### A3. `SecPerClus` elegido a proposito, no por tabla

La FAT crece al reves que el cluster, y el barrido de 3.5 con ella:

```
A: 32 GB     cluster    clusters      FAT      barrido al 50%
             4 KiB      8.388.608     32 MiB   32.768 sectores
             32 KiB     1.048.576      4 MiB    4.096 sectores   <- hoy
             128 KiB      262.144      1 MiB    1.024 sectores
```

Y al reves, el desperdicio: `A:\datos\concs.txt` mide 0 bytes y ocupa **32 KiB
enteros**. Con 128 KiB ocuparia 128.

Para lo que `A:` guarda de verdad --`.bex` de medio mega, un `.wad` de 4 MB, y
un punado de `.txt` diminutos-- **el cluster grande gana**, porque el coste del
desperdicio es de kilobytes y el del barrido es de megabytes. Pero eso hay que
**decidirlo**, y hoy no lo decidio nadie.

#### A4. `FSI_Nxt_Free`: de O(FAT) a O(1)

No es una pista de cortesia: es el puntero de asignacion. Se lee al montar, se
avanza al asignar, se guarda al desmontar, y si esta a `0xFFFFFFFF` se
reconstruye con **un** barrido en vez de uno por cluster.

```
escribir doom1.wad (128 clusters) en A: al 50% de ocupacion
   hoy               ~256 MiB de FAT leidos
   con Nxt_Free      ~128 sectores  =  64 KiB
```

Cuatro mil veces menos, y esta en la especificacion desde el primer dia.

#### A5. `BPB_BkBootSec`: integridad que ya estas pagando

Todo volumen FAT32 lleva una copia del sector de arranque en el sector 6.
**Nadie la compara nunca.** Compararla al montar detecta un BPB pisado antes de
usarlo -- que es exactamente la clase de fallo que manda a un driver a leer
sectores que no son. Cuesta una lectura por montaje.

#### A6. `ExtFlags` en el otro sentido: apagar el espejo en caliente

Es A1 sin reformatear. El bit 7 de `BPB_ExtFlags` desactiva el espejo y los
bits 0..3 dicen cual queda activa. Reversible, conforme, y **hay que
implementarlo de todas formas** para arreglar el fallo 3.3.

### NIVEL B -- hueco que el formato garantiza y nadie ocupa

Escribir aqui es conforme. Lo que nadie garantiza es que otro sistema no lo
pise. **Sirve para pistas, no para verdades**: nada que, si desaparece, rompa
el volumen.

#### B1. Los sectores reservados 8..31 -- ~12 KiB por volumen

`RsvdSecCnt` vale 32 en un FAT32 tipico. De esos 32 se usan **cuatro**: el 0
(BPB), el 1 (FSInfo), el 6 (copia del BPB) y el 7 (copia del FSInfo).

**Los sectores 2-5 y 8-31 no los escribe ni los lee nadie, jamas.** La cola
contigua --del 8 al 31-- son **24 sectores = 12 KiB** dentro de una particion
que Windows monta como FAT32 normal. (Los 2-5 tambien estan libres, pero
quedan ENTRE dos sectores usados; `Geometria::zona_reservada` solo ofrece la
cola, que es lo que se puede dar como un tramo y sin reglas raras.)

Ahi cabe entero un **superbloque de ESTRATOS** (4096 B, un bloque). Es decir:
una particion que Windows abre y usa con total normalidad, y que ademas lleva
dentro la identidad de BMO -- `disco_id`, generacion, puntero al estrato. **Ese
es el puente entre los dos mundos, y es gratis.**

[!] Hay que LEER `RsvdSecCnt`, no darlo por 32. Un volumen formateado por otra
herramienta puede traer menos, y entonces no hay sitio y hay que decirlo.

#### B2. `BPB_Reserved`: 12 bytes en el propio sector de arranque

Offset 52, mas `BS_Reserved1` en el 65. Trece bytes. Poco, pero estan **en el
sector 0**, o sea que se leen sin una lectura extra. Sitio para una marca de
version y poco mas.

#### B3. `NTRes`: seis bits libres POR ARCHIVO

Offset 12 de cada entrada de directorio. Windows usa el bit 3 (base en
minusculas) y el bit 4 (extension en minusculas) -- que es, por cierto, su
propio truco para ahorrarse una entrada LFN en `readme.txt`. **Los otros seis
bits no los mira nadie.**

Seis bits por archivo dan para un tipo de contenido, un nivel de confianza, o
la marca de "esto lo escribio BMO". Y si otro sistema los borra, no se pierde
un archivo: se pierde una pista.

#### B4. `LstAccDate`: dos bytes semi-libres

Casi ningun sistema lo actualiza (es el `noatime` de facto). Mas arriesgado que
B3 porque Windows **si** lo escribe en algunas configuraciones. Se anota por
completitud; no se usa.

### NIVEL C -- donde el estandar dice que no. No se hace.

#### C1. Los 4 bits altos de la FAT -- 512 KiB de canal lateral en `A:`

```
1.048.576 clusters  x  4 bits  =  512 KiB
```

Existen, estan ahi, y **la especificacion dice PRESERVAR, no "son tuyos"**.
Windows los preserva; de `chkdsk /f` no me fiaria, y el dia que los limpie no
avisa. Medio mega no vale un volumen.

#### C2. Cadenas de mas de 4 GiB

**El limite de 4 GiB esta en el campo `FileSize` (u32), no en la cadena.** La
cadena direcciona 28 bits de numero de cluster: con los 32 KiB de `A:` son 8
TiB encadenables. Un lector de BMO que supiera el tamano real por otro sitio
podria recorrer una cadena mas larga sin tocar el formato de la FAT.

Y no se hace, porque **rompe exactamente lo que hace valioso a FAT**: Windows
reportaria `tamano mod 4 GiB` y `chkdsk` lo "arreglaria" truncando la cadena.
Un formato de frontera que solo entiende uno de los dos lados no es una
frontera.

### 4.D LA RESPUESTA CONFORME A "SIN LIMITE DE 4 GB"

Existe, y no necesita ningun truco: **el contenedor partido.**

```
A:\bmo\VOL.001      4 GiB - 1
A:\bmo\VOL.002      4 GiB - 1
A:\bmo\VOL.003      ...
```

Un volumen de ESTRATOS que vive dentro de archivos FAT32 normales, presentados
como **un solo espacio de direcciones**. Es como funcionan los archivos
partidos de toda la vida y varios formatos de disco virtual, y es lo que
Ventoy hace en tu disco 1 con otra forma.

| | |
|---|---|
| Conformidad | total. Son archivos. Windows los ve, los copia, los respalda |
| Limite | ninguno; se anaden partes |
| Lo que desbloquea | **un BMO que cabe en un pendrive** y arranca en cualquier maquina UEFI **sin tocar la tabla de particiones del anfitrion** |
| Coste | una indireccion: offset global -> (parte, offset) -> cadena de clusters |
| Requisito | leer la cadena de cada parte UNA vez y quedarse con los tramos |

Eso ultimo ya existe: es `leer_tramo` y el `Cursor` de la crate de hoy.

### 4.F LOS TRES MEDIOS -- y donde vive el perfil, que es la pregunta de verdad

> Preguntado por el dueno el 18-08: *"tendra jerarquias unicas si ES para SSD,
> otro para Ranura y otro HDD, pero el mio es SSD -- se puede y podria
> aprovechar bien?"*

**Si, las jerarquias son reales.** Las tres se justifican con fisica y no con
gusto. Pero **no van donde parece**, y esa es la mitad importante de la
respuesta.

#### El perfil NO vive en el driver

`bmo-fat` **no puede saber sobre que medio esta**, y no por falta de datos:
porque no debe. Un volumen formateado para SSD acaba enchufado en una ranura;
un pendrive acaba en el SATA. Si el driver se comportara distinto segun el
aparato, **los mismos bytes darian resultados distintos segun donde estan** --
y eso es un cerebro, no un contrato: deja de poderse probar contra un disco de
mentira, que es lo unico que hace falsable a esta crate.

El perfil es **una decision del momento de FORMATEAR**, y queda GRABADA en el
BPB. Una vez escrito, **el BPB ES el perfil**: `NumFATs`, `SecPerClus`,
`BytsPerSec`, `RsvdSecCnt` y donde empieza el cluster 2. El driver lee el BPB y
obedece. No hay una tercera cosa que recordar.

```
   bmo-identify   dice QUE ES el aparato    (Medio, Geometria, Trim)
        |             no decide nada -- es su propia doctrina
        v
   fat-fmt        ELIGE el perfil y lo escribe en el BPB
        |
        v
   bmo-fat        lee el BPB. Cero conciencia del medio.
```

Y el que decide vive fuera y con pruebas, que es exactamente el reparto que
`bmo-identify` ya declara: *"este crate no decide nada... todo eso es el nieto,
y vive fuera porque alli se puede probar"*.

#### La tabla: tres filas, y solo una verificable en esta casa

| | **SSD** `Medio::NoRota` | **HDD** `Medio::Rota{rpm}` | **Ranura** (extraible) |
|---|---|---|---|
| `NumFATs` | **1** | **2** | **2** |
| por que | el espejo duplica la escritura en el sector mas caliente del disco, y en NAND la copia acaba donde el FTL quiera: no protege de nada | un sector defectuoso es un evento fisico **local** --un roce, un tacto de cabezal-- y la copia, a `FATSz` sectores de distancia, si salva. Y las escrituras no desgastan un plato | el medio es barato y falla de verdad; el espejo se paga a gusto |
| `SecPerClus` | **grande** -- FAT pequena = menos escritura caliente y barrido corto | **medio** -- lo que manda es la contiguidad, no el tamano de la FAT | **grande**, y ademas alineado |
| alinear el cluster 2 a | `logicos_por_fisico` y el desplazamiento de la palabra 209 | da casi igual | el bloque de borrado del controlador, que en una SD llega a **4 MiB** |
| `FSI_Nxt_Free` | se guarda al desmontar | igual | **se guarda a menudo**: aqui el tiron sin desmontar es el modo de fallo NORMAL |
| `ClnShut` | al montar y al desmontar | igual | **critico** |
| TRIM | si `Trim::soportado` | no aplica | casi ninguna lo soporta |

[!] **Solo la fila del SSD se puede verificar en esta casa.** Los tres discos
son SSD SATA. Las otras dos filas se escriben porque el formateador tiene que
tener un valor para ellas y porque el razonamiento es comprobable **en papel**,
pero van marcadas como NO VERIFICADAS EN METAL hasta que haya un plato girando
o una tarjeta en una ranura. No se construye maquinaria para hardware que no
existe: se deja la fila puesta y se dice que no se ha probado.

#### Y para tu SSD, con tus numeros

Lo que la fila del SSD compra en `A:` (32 GB, cluster 32 KiB, FAT de 4 MiB):

| perilla | hoy | con perfil SSD |
|---|---|---|
| `NumFATs` | 2 | **1** -- la mitad de escrituras en los 8.192 sectores mas calientes |
| barrido por cluster asignado (al 50%) | ~4.096 sectores | **~1 sector** con `FSI_Nxt_Free` |
| FAT total | 4 MiB x 2 = 8 MiB | 4 MiB x 1, o **1 MiB x 1** con cluster de 128 KiB |
| alineacion | Windows la declara `Alineado (0x000)` y la particion empieza en 601 MB = multiplo de 4 KiB | **ya esta bien**; no hay nada que arreglar |
| zona reservada | 12 KiB sin usar | **12 KiB para la identidad de BMO** (4.B1) |

La alineacion, que suele ser el gran fallo de un FAT32 en SSD, en tu disco **ya
esta resuelta** por como se particiono. Lo que queda por ganar es lo de arriba:
menos escrituras y menos barridos, no menos desalineacion.

### 4.E LA SINTESIS

Lo que FAT32 esta subestimando **no es un hueco secreto: es que nadie lo
formatea a proposito.** Las perillas de A1 a A6 llevan en la especificacion
desde 1996 y no las gira nadie porque el formateador decide por ti.

**Y por eso la casilla nueva de este plan es un formateador.** No
`mkfs.estratos` --ese ya lo tienes-- sino un **`mkfs.fat` propio**, que es la
unica manera de girar A1, A2, A3 y B1. Ahi esta el "aprovechar todo el poder":
no en romper el formato, sino en ser **el primero que lo formatea sabiendo lo
que hace**.

---

## 5. LA CRATE NUEVA -- `bmo-fat`, la frontera

Se llama `bmo-fat` y no `bmo-fat32` porque habla FAT12, FAT16 y FAT32 -- que
son **un formato con tres anchuras de entrada**: mismo BPB, misma entrada de
directorio, mismo LFN.

**exFAT se muda a `bmo-exfat`.** No se borra. exFAT no comparte casi nada
(mapa de bits en vez de FAT libre, tabla de mayusculas, entradas 0x85/0xC0/0xC1,
sin LFN); juntarlos es lo que obliga a los seis `match` de hoy y a los sesenta
de manana.

```
platform/drivers/storage/
  fat/       FAT12/16/32. LA FRONTERA.
  exfat/     lo que ya hay de exFAT, entero, con su propio `mount`
```

### 5.1 El reparto, por PREGUNTA (criterio C de `PLAN_ALMACENAMIENTO.md`)

```
fat/src/
  lib.rs        ~140   LA PUERTA. `mount`, `FatVolume`, reexports.
  bpb.rs        ~260   QUE VOLUMEN ES ESTE?
                       BPB, firma 0xAA55, FSVer, cuenta de clusters (1.2),
                       FSInfo, ExtFlags, y la COMPARACION con el sector 6 (4.A5).
                       PURA sobre un buffer -> se prueba sin disco.
  tabla.rs      ~220   DONDE SIGUE ESTO?
                       entrada de 12/16/32 bits; nibble alto preservado; la FAT
                       ACTIVA segun ExtFlags; recorrer cadena con tope;
                       FAT[1] y sus dos banderas.
  espacio.rs    ~200   DE DONDE SACO SITIO?
                       FSI_Nxt_Free (4.A4), reservar en tandas, soltar cadena.
  dir/
    corta.rs    ~170   LA ENTRADA DE 32 BYTES. 8.3, atributos, NTRes.
    larga.rs    ~230   EL NOMBRE DE VERDAD. LFN: UCS-2, suma de control,
                       orden inverso, alias `NOMBRE~1`. PURA -> sin disco.
    recorrer.rs ~210   QUE HAY AQUI DENTRO? iterar juntando LFN + 8.3,
                       hueco de N entradas, y HACER CRECER el directorio (3.6).
  datos.rs      ~200   LOS BYTES. Cursor, tramos, multi-sector de una vez.
  reservada.rs  ~130   LA ZONA DE 4.B1. Leer RsvdSecCnt de verdad, decir cuanto
                       sitio hay, y leer/escribir ahi. Nada mas: no sabe QUE se
                       guarda, solo DONDE cabe.
  tiempo.rs      ~90   La fecha y hora empaquetadas de 1.5. PURA.
  estado.rs     ~130   MONTAR LIMPIO Y DESMONTAR LIMPIO. ClnShut, HrdErr, flush.
```

Once ficheros, ninguno pasa de 260 lineas. **Cuatro no tocan disco** (`bpb`,
`dir/larga`, `tiempo`, y la mitad de `reservada`): entra un buffer, sale una
respuesta. Son censos, como el de particiones y el de C.

### 5.2 Y una crate hermana

```
toolchain/tools/fat-fmt/    el formateador de 4.E. Corre en el ANFITRION,
                            como estratos-fmt, y usa bmo-fat para releer lo
                            que acaba de escribir.
```

Vive en `toolchain/tools/` y no en el kernel por la regla que dejo el paso 2 de
`PLAN_ALMACENAMIENTO.md`: **si una funcion merece pruebas, no pertenece al
kernel.**

---

## 6. LAS CASILLAS

Orden: **lo que se prueba sin disco primero**, y dentro de eso, lo que esta mal
antes de lo que falta. Se arregla el formato antes de ampliarlo.

### [x] Paso 0 -- `bpb.rs`, y el fallo 3.1 muere ahi -- **HECHO el 2026-08-18**

```
platform/drivers/storage/fat/
  Cargo.toml      cero dependencias: no lee del disco
  src/lib.rs      la puerta
  src/bpb.rs      identificar / cuadra_con_respaldo / leer_fsinfo
                  23 casillas verdes en 0,00 s, `forbid(unsafe_code)`, clippy limpio
```

`identificar(&[u8]) -> Result<Geometria, NoEs>` con el algoritmo de 1.2 y **el
orden que impone la especificacion**: la firma, luego la aritmetica, luego la
CUENTA DE CLUSTERS, y solo entonces los campos que dependen del tipo. Ese
ultimo paso es literalmente el fallo 3.1: el `BS_VolID` de un FAT16 vive en el
39 y el de un FAT32 en el 67, asi que leerlos antes de saber el tipo es leer
basura.

**Y el `unsafe` se fue.** El driver de antes hacia
`unsafe { &*(buf.as_ptr() as *const FatBpb) }` -- alineacion supuesta y orden
de bytes de la maquina, a cambio de nada, porque estos campos se leen una vez
al montar. Ahora se leen en little-endian explicito y la crate declara
`#![forbid(unsafe_code)]`.

Lo que se llevo ademas de lo previsto, porque salio gratis del mismo sector:

* `Geometria::zona_reservada()` -- **la zona de 4.B1, calculada y no supuesta.**
  Contesta `(8, 24)` en un FAT32 normal y `None` en uno apretado.
* `cuadra_con_respaldo()` -- la perilla 4.A5. Compara los 90 bytes del BPB
  contra su copia del sector 6, no el sector entero: los 420 de en medio son
  codigo de arranque de 16 bits que algunos formateadores dejan distinto.
* `leer_fsinfo()` -- las **tres** firmas, y `0xFFFFFFFF` como `None` y no como
  cero. "No se sabe cuanto queda libre" y "no queda nada libre" no son lo mismo.
* `espejo` / `fat_activa` de `ExtFlags`, que es la mitad de lectura del fallo
  3.3. Y una FAT activa que no existe cae a la 0 en vez de escribir a ciegas.

### [!] La leccion del paso 0, y hay que conservarla

Las trece primeras casillas fallaron con `NoCabe`, todas, y **el fallo estaba
en el banco de pruebas**: el constructor de sectores escribia las dos colas del
BPB a la vez -- la de FAT12/16 (`BS_BootSig` en el 38, `BS_VolID` en el 39) y
la de FAT32. **Esos bytes son los mismos.** En FAT32 el 38 y el 39 son la mitad
alta de `BPB_FATSz32`, y el 40..43 son `ExtFlags` y `FSVer`. Poner el 0x29 de
FAT16 convertia una FAT de 512 sectores en una de 2.687.488.

> El censo se equivoco **exactamente igual que se equivocaba el driver**: dando
> por supuesto que los campos de los dos tipos conviven. No conviven, y ese
> solapamiento es la razon de que el orden de `identificar` sea el que es.

La prueba se quedo con la trampa dentro: el sector de FAT16 lleva un 0x29 **en
el byte 66 tambien** --donde en FAT12/16 no es mas que codigo de arranque--,
para que el dia que alguien vuelva a decidir el tipo mirando ese byte, la
casilla lo cace.

### [x] Paso 1 -- `dir/larga.rs`: el nombre largo -- **HECHO el 2026-08-18**

Es el punto 3 de la seccion 2 --lo que UEFI exige-- y ya estaba mordiendo
(`LoadError::NameTooLong`).

```
  src/dir/mod.rs      13   el reparto de los tres: larga [x], corta, recorrer
  src/dir/larga.rs   482   =  262 codigo + 220 censo, 15 casillas
```

`montar` parte un nombre UTF-16 en la cadena de fragmentos **ya en el orden en
que van al disco**, y `desmontar` la vuelve a juntar. Quien lea el directorio
no tiene que acordarse de que la cadena va del reves.

**Y la suma de control no es opcional en la firma**: `desmontar` exige la
entrada 8.3 a la que la cadena dice pertenecer. No se puede llamar sin ella, o
sea que no se puede *olvidar* comprobarla. Es la unica cosa que ata una cadena
larga a su archivo, y sin ella un directorio con restos de un borrado devuelve
nombres de archivos que ya no existen -- lo comprueba
`una_cadena_con_la_suma_cambiada_se_descarta`.

Lo que quedo cubierto, y por que cada uno:

| casilla | lo que impide |
|---|---|
| suma con los once bytes **y su relleno** | `"LEEME.TXT"` y `"LEEME   TXT"` dan sumas distintas |
| ida y vuelta de 27 caracteres | 3 fragmentos, y el nombre vuelve identico |
| **la enye y los acentos sobreviven** | por esto el nombre viaja en UTF-16 y no en bytes |
| suma cambiada -> `SumaMala` | el nombre de un archivo borrado colandose en otro |
| cadena sin cabeza -> `SinCabeza` | medio nombre, que **parece bueno** |
| orden que no baja de uno en uno | una cadena remendada |
| relleno `0x0000` para cerrar y `0xFFFF` detras | que Windows lo lea igual que lo escribe |
| `cabe_en_8_3` | no gastar una entrada de directorio en `KERNEL.ELF` |
| 255 si, 256 no | el limite del formato, por los dos lados |
| destino corto -> `NoCabe` | cortar un nombre en silencio |

Y una que no estaba prevista: `cabe_en_8_3` dice que **`kernel.elf` NO cabe**.
Las minusculas obligan a cadena larga (o al truco de `NTRes`, que es 4.B3 y es
otra cosa). Lo que parece un nombre de MS-DOS ya no lo es en cuanto lo escribe
alguien con el teclado.

### [ ] Paso 2 -- `tabla.rs`: 3.2, 3.3 y la perilla A6

* **Hecho cuando**: una casilla escribe un cluster en un volumen cuyo nibble
  alto vale `0xF` y **lo relee intacto**; y otra, con el espejo apagado y la
  FAT #1 activa, comprueba que la #0 **no se toco**.

### [ ] Paso 3 -- `reservada.rs`: abrir la zona de 4.B1

Leer `RsvdSecCnt` de verdad, calcular cuantos sectores sobran, y dar acceso.

* **Bloquea**: el paso 0 (hace falta la geometria).
* **Hecho cuando**: una casilla escribe 4096 bytes en la zona reservada de un
  volumen de prueba, lo remonta, los relee **y comprueba que el BPB, el FSInfo
  y sus dos copias siguen intactos**.

### [ ] Paso 4 -- `espacio.rs`: `FSI_Nxt_Free` (3.5 y 4.A4)

* **Hecho cuando**: escribir un archivo de 100 clusters cuesta **menos de 100
  barridos de FAT**, medido con el contador de lecturas que el disco de mentira
  ya lleva.

### [ ] Paso 5 -- `dir/recorrer.rs`: el directorio crece (3.6)

* **Hecho cuando**: una casilla llena un directorio de un solo cluster, crea el
  archivo siguiente y **existe**, y el volumen sigue legible al remontarlo.

### [ ] Paso 6 -- `datos.rs`: multi-sector donde AUN no lo es (3.7b)

Ojo al alcance, que la correccion de 3.7b lo encoge: **leer ya esta bien**. Lo
que falta es ESCRIBIR un cluster de una vez y RECORRER un directorio de una vez.

* **Hecho cuando**: escribir un cluster de 64 sectores --los 32 KiB de `A:`--
  hace **una** llamada al contrato, y recorrer un directorio de un cluster
  tambien; las dos contadas por el disco de mentira.

### [ ] Paso 7 -- exFAT se muda a `bmo-exfat`

* **Hecho cuando**: `grep fs_type` en `fat/` no encuentra nada.

### [ ] Paso 8 -- `fat-fmt`: el formateador que gira las perillas

`NumFATs = 1` (A1), `SecPerClus` **elegido** (A3), zona reservada preparada
(B1), y `BytsPerSec` configurable (A2) con el aviso de no tocar la ESP.

* **Bloquea**: los pasos 0, 3 y 4.
* **Hecho cuando**: formatea una imagen en el anfitrion, **Windows la monta y
  escribe en ella**, y `bmo-fat` la relee. Las dos cosas, o no vale.

### [ ] Paso 9 -- `estado.rs`: el volumen dice la verdad (3.4)

Ultimo a proposito: `A:` se monta con escritor, asi que esto importa, pero
importa **despues** de que asignar y recorrer sean correctos.

---

## 7. LO QUE NO SE HACE -- esencia acotada = terminable

- **Ni borrar, ni renombrar, ni mkdir en FAT.** El explorador va sobre
  ESTRATOS. FAT es la frontera: se lee, y se escribe lo que tiene que cruzarla.
- **No se escribe en medio de un archivo ni se trunca.** Eso lo hace ESTRATOS.
- **No hay cache de directorio ni de bloques.** Una cache es un cerebro.
- **No hay desfragmentador** ni asignador que busque huecos contiguos.
- **No hay `chkdsk`.** Detectar que el volumen viene sucio si; repararlo no.
- **No se tocan los 4 bits altos ni las cadenas de mas de 4 GiB** (nivel C).
- **No se reformatea la ESP.** Es de donde arranca la maquina.

---

## 8. LA META

Dos frases, y las dos se pueden comprobar sin arrancar el Ryzen:

1. Que `bmo-fat` se ponga delante de las siete lineas de la seccion 2 y
   **conteste que si a las siete**, cada una con su casilla. Hoy son dos.
2. Que `A:` sea la primera particion FAT32 de esta casa **formateada a
   proposito** -- con las perillas de la seccion 4 giradas por alguien que
   sabia lo que hacia, y no por la tabla de 1996 de `format.com`.

Y que el trabajo grande siga yendo donde tiene que ir: a la casilla del 1.0 de
ESTRATOS, que es escribir contenido y releerlo tras reiniciar.
