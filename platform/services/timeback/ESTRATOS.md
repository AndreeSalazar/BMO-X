# ESTRATOS — el sistema de ficheros de BMO-X

> *Cada escritura deja una capa nueva encima sin destruir la de abajo.
> Leer hacia atrás en el tiempo es bajar por los estratos.*

**Estado**: pasos 1-4 del §10 HECHOS — el kernel monta ESTRATOS y lo lee. La
escritura (paso 5) es lo siguiente. Este documento existe para que el formato se
decida ANTES de tocar un sector — en un sistema de ficheros, equivocarse cuesta
datos.

---

## 1. La idea

TimeBack y BMO-FS parecían dos proyectos. No lo son.

Git guarda **blobs** (contenido direccionado por su hash), **árboles**
(nombre → hash) y **commits** (una raíz con padres). Un sistema de ficheros
copy-on-write guarda **bloques** (contenido), **nodos** (nombre → bloque) y
**superbloques** (una raíz que se cambia al final, cuando todo lo demás ya
está en disco).

Son la misma forma. Git es un sistema de ficheros que resultó ser control de
versiones; un FS copy-on-write es control de versiones que resultó ser un
sistema de ficheros. Nadie los ha unificado del todo porque en Unix el FS ya
venía dado y Git tuvo que construirse encima, con un `.git/` que duplica todo
lo que el FS ya sabía.

BMO-X no tiene esa herencia. Puede hacer que **el historial no sea una
carpeta, sino una propiedad del suelo**:

- Guardar **es** commitear, porque nunca se sobreescribe nada.
- Recuperar un archivo de la semana pasada no es restaurar un respaldo: es
  leer un puntero viejo que jamás se borró.
- El sistema no puede "perder" el estado anterior por un fallo a media
  escritura, porque el estado anterior sigue intacto hasta que la raíz nueva
  está completa y verificada.

---

## 2. Principios (no negociables)

1. **Nunca se sobreescribe un bloque vivo.** Se escribe uno nuevo y se cambia
   el puntero al final. Un corte de luz a media escritura deja el sistema en
   el estado ANTERIOR, entero, no en uno a medias.
2. **Todo lleva suma de verificación.** El sistema de ficheros detecta su
   propia corrupción en vez de confiar en que el disco devuelve lo que
   guardó. Un bloque que no cuadra con su hash es un FAULT en CABINA, no un
   archivo raro.
3. **Tener el handle ES el permiso.** Sin root, sin `chmod`, sin uid/gid, sin
   autoridad ambiental. Un proceso no puede *nombrar* un archivo al que
   nadie le dio acceso.
4. **La verificación vive dentro.** El FS no entrega una capability
   *ejecutable* sobre una imagen que no pasó `bmo-verify`. La admisión deja
   de ser un paso del arranque y pasa a ser una propiedad del almacenamiento.
5. **El dueño manda sobre el espacio.** El recolector nunca decide solo qué
   se pierde: avisa, propone y obedece (§9).
6. **La partición de arranque NUNCA depende de ESTRATOS** hasta que ESTRATOS
   se lo haya ganado. A: se queda en FAT32; ESTRATOS vive en BMO-DATA.

---

## 3. Herencia: qué se roba y qué no

| Sistema | Lo que vale | Lo que se deja |
|---|---|---|
| **NTFS** | **Todo es un archivo, incluidos los metadatos** (la tabla maestra es ella misma un archivo, y por eso el FS puede hacer crecer sus propias estructuras con su propio asignador). **Atributos con nombre**: un archivo no es un chorro de bytes, es un conjunto de flujos. **Archivos pequeños residentes**: los que caben viven *dentro* de su registro y no gastan bloque | 30 años de compatibilidad hacia atrás |
| **ZFS / btrfs** | Copy-on-write, checksums en todo, árbol de Merkle (el hash raíz valida el árbol entero), instantáneas gratis | Volúmenes, RAID, caché ARC, compresión — nada de eso hace falta todavía |
| **Git** | Direccionamiento por contenido: **deduplicación gratis**, y el historial son solo raíces extra | Que viva en una carpeta aparte del FS |
| **Log-structured (NILFS2, LFS)** | Escribir **siempre secuencial**, que es exactamente lo que ama un SSD, y da instantáneas continuas | — (pero trae el recolector: §9) |
| **Plan 9 / Venti** | Archivo permanente por hash: lo que entra no se pierde | El servidor de red |
| **BMO** | Capabilities como permiso, `bmo-verify` como gate, CABINA como testigo | — |

---

## 4. Modelo de objetos

Cuatro tipos, todos identificados por el **hash BLAKE3 de su contenido**.

```
  BLOQUE     bytes crudos. La unidad de datos.
  ATRIBUTO   un flujo con nombre: lista de bloques + tamaño.
  NODO       un archivo o directorio = conjunto de atributos.
  ESTRATO    una raíz: nodo raíz + padre(s) + marca de tiempo + autor.
```

### Por qué atributos y no "el contenido del archivo"

Esta es la idea que se le roba a NTFS y la que mejor encaja con el ABI de BMO.
Un `.bex` en ESTRATOS no es *un archivo*: es un nodo con varios flujos.

```
  hola_C.bex
    ├── :datos        el código, el que se ejecuta
    ├── :firma        el hash BLAKE3 firmado (lo que mira bmo-verify)
    ├── :manifiesto   qué capabilities pide para correr
    └── :origen       de qué fuente salió, con qué compilador, cuándo
```

Ningún sistema de ficheros clásico permite eso sin inventarse convenciones de
nombres o archivos `.meta` sueltos que se pierden al copiar. Aquí el
manifiesto de capabilities **no puede separarse del binario**, porque es parte
del mismo objeto.

### ESTRATO: el commit que también es el superbloque

```
  estrato {
      raiz:     Hash        // el nodo raíz del árbol de directorios
      padre:    Hash        // el estrato anterior (0 = el primero)
      tiempo:   u64
      autor:    Autor       // kernel / usuario / proceso, con su pid
      motivo:   [u8; 64]    // "auto", "antes de instalar X", ...
      suma:     Hash        // BLAKE3 de todo lo anterior
  }
```

Montar el sistema de ficheros = leer el último estrato válido. Volver atrás
en el tiempo = leer uno anterior. **Son la misma operación.** No hay código de
"restaurar": hay código de "montar", y se le pasa otro estrato.

---

## 5. Formato en disco

```
  LBA 0     SUPERBLOQUE A   ┐ dos copias alternas. Se escribe la que NO
  LBA 1     SUPERBLOQUE B   ┘ está en uso; si el corte llega a media
                              escritura, la otra sigue entera.
  LBA 2..   MAPA DE ESPACIO  bitmap de bloques, él mismo un archivo
  ...       LOG              todo lo demás: bloques, nodos, estratos,
                             escritos SIEMPRE hacia adelante
```

**Superbloque** (el único sitio con posición fija):

```
  magico:      b"ESTRATOS"
  version:     u32
  bloque_tam:  u32           // 4096
  disco_id:    [u8; 20]      // modelo+serie del disco (IDENTIFY)
  estrato:     Hash          // el estrato más reciente
  generacion:  u64           // el más alto de los dos superbloques gana
  suma:        Hash
```

`disco_id` no es decoración: es el **gate de identidad** grabado en el propio
volumen. Si ESTRATOS se monta en un disco cuyo `IDENTIFY` no coincide con el
que dice el superbloque, se monta **solo lectura** y CABINA grita. Un volumen
clonado a otro disco no se escribe por accidente.

### La escritura, paso a paso

1. Se escriben los bloques nuevos en la punta del log. *(Nada apunta a ellos
   todavía: si se corta aquí, es basura inofensiva.)*
2. Se escriben los atributos y nodos que los referencian.
3. Se escribe el estrato nuevo, con su suma.
4. **Barrera**: se espera a que el disco confirme que todo lo anterior está
   en el plato, no en su caché (`FLUSH CACHE`).
5. Se escribe el superbloque alterno con la generación +1.

El punto de no retorno es el paso 5, y es **un solo sector**. Antes de él, el
sistema es exactamente el de antes. Después, el nuevo. No hay estado
intermedio observable — que es la definición de una transacción.

---

## 6. Nombres y rutas

Un directorio es un nodo cuyo atributo `:entradas` mapea nombre → hash de
nodo. Nada más.

- Nombres en **Latin-1**, un byte por carácter, igual que la consola y el
  teclado (ver `keyboard.rs`). Sin UTF-8 en Ring 0, sin decodificador en el
  camino. `ñ` y acentos funcionan porque el font ya los dibuja.
- Sin distinción de mayúsculas al comparar, **pero conservando** cómo se
  escribió. Es lo que espera cualquiera que venga de Windows y no cuesta nada.

---

## 7. Capabilities: el permiso ES el handle

En Unix cualquier proceso puede *nombrar* cualquier ruta y el kernel decide
con uid/gid si le deja — autoridad ambiental, justo lo que BMO-X rechaza.

En ESTRATOS, abrir no es "pedir por nombre y rezar":

```
  Un proceso recibe una capability a un NODO (típicamente un directorio).
  Desde ella puede derivar capabilities a lo que hay dentro, nunca hacia
  fuera ni hacia arriba. No existe ".." que escape del árbol concedido.
  Los derechos (leer / escribir / ejecutar / listar) viajan EN el handle y
  solo pueden reducirse al derivarlos, jamás ampliarse.
```

Consecuencia práctica: un compilador al que le das el directorio de su
proyecto **no puede tocar nada más**, y no porque se lo prohíba una lista de
permisos, sino porque el resto del disco no existe para él. No hay root que
pueda saltárselo, porque no hay root.

### El gate de ejecución

`abrir(nodo, EJECUTAR)` comprueba el atributo `:firma` contra el contenido y
lo pasa por `bmo-verify`. Si no cuadra, **no hay handle ejecutable** — el
archivo se puede leer, copiar y borrar, pero no correr. La admisión de
binarios deja de ser un paso del arranque y pasa a ser una propiedad del
suelo.

---

## 8. TimeBack: no es una capa, es la misma cosa

`platform/services/timeback` ya tiene el modelo (blobs, árboles, commits,
refs, journal, rollback, CLI: ~54 KB). Lo que cambia es dónde vive:

| Hoy (TimeBack sobre un FS) | Con ESTRATOS |
|---|---|
| `tb add` copia el archivo a `objects/` | No copia nada: el bloque **ya está** direccionado por contenido |
| `tb commit` escribe un objeto commit | Es el estrato que la escritura crea de todos modos |
| El historial ocupa el doble | El historial ocupa lo que **cambió**, y nada más |
| Hay que acordarse de commitear | Escribir es commitear |

Los mandos de TimeBack siguen teniendo sentido, pero pasan a ser vistas sobre
el disco en vez de una base de datos paralela: `tb log` recorre la cadena de
estratos, `tb diff` compara dos árboles por hash (los subárboles con el mismo
hash se saltan enteros — eso es gratis y es lo que hace a Git rápido), y
`tb restore` monta un estrato viejo.

### Un cambio obligatorio antes de nada

`timeback::hash` usa **FNV-1a**. Es rápido, determinista y perfectamente
válido para un índice… pero **no es criptográfico**. En un sistema de ficheros
direccionado por contenido, dos bloques distintos con el mismo hash significan
que uno sustituye al otro **en silencio**: pérdida de datos que ninguna suma
detecta, porque la suma es justo lo que colisionó. Y siendo FNV, provocar esa
colisión a propósito es trivial.

**ESTRATOS usa BLAKE3** (`platform/abi/bmo-abi/src/bef/blake3.rs`, ya
presente, y el mismo que usa `bmo-verify`). Un solo algoritmo de hash en todo
el sistema: contenido, firmas y verificación hablan el mismo idioma.

---

## 9. El recolector (GC)

Un FS que nunca sobreescribe llena el disco de versiones viejas. Alguien tiene
que decidir qué se puede soltar. Esta es la parte difícil de verdad — la que
todo el mundo subestima y la razón por la que a btrfs le costó una década ser
fiable.

**Decisión del dueño**: se implementa, con avisos, y el usuario manda.

### Cómo funciona

Un bloque se puede soltar cuando **ningún estrato conservado lo alcanza**. Se
recorren las raíces vivas marcando lo alcanzable, y lo demás vuelve al mapa de
espacio. Como todo está direccionado por contenido, un bloque compartido por
diez versiones se cuenta una vez.

### Política, no automatismo

```
  conservar todos los estratos de la última hora
  conservar uno por hora del último día
  conservar uno por día del último mes
  conservar los marcados a mano (los que tienen nombre) PARA SIEMPRE
```

Un estrato con nombre — *"antes de reparticionar"*, *"COBOL funcionando"* — no
se toca nunca, aunque el disco esté lleno. Los automáticos se van adelgazando
hacia atrás en el tiempo.

### Los avisos (esto es CABINA)

El FS **nunca borra en silencio ni se llena por sorpresa**:

- Al 70 % de ocupación: aviso ámbar con cuánto ocupa el historial y cuánto se
  liberaría aplicando la política.
- Al 85 %: FAULT rojo y propuesta concreta — *"soltar 47 estratos automáticos
  de más de 30 días libera 12 GiB"*.
- Al 95 %: **modo solo lectura**. Antes de perder datos por falta de sitio, el
  sistema se planta y te lo dice.
- Y un mando manual: `estratos limpiar` con lo que va a soltar **listado antes
  de hacerlo**.

Tienes razón en que en un disco enorme esto importa poco. En tus 414 GiB de
BMO-DATA importa bastante, y en un SSD hay un motivo extra: los bloques
soltados hay que devolvérselos al disco con `TRIM`, o el SSD sigue creyendo
que están ocupados y se le acaba el margen de escritura.

### ✅ La contabilidad, hecha (2026-07-31)

`bmo_estratos::espacio` — `Ocupacion` y `Nivel`, con los cuatro umbrales de
arriba, **probados en el anfitrión**. El kernel los expone en `estratos`.

Y la cuenta resultó ser **una resta**: ESTRATOS reserva con un puntero que sólo
avanza (`log_head` es el primer bloque libre), así que todo lo de debajo está
usado. Ni mapa de bits, ni listas de huecos, ni fragmentación que medir — y eso
es consecuencia directa de no sobreescribir nunca. El precio es que la cuenta
sólo sube hasta que exista el recolector, y que suba **y se vea** es justo lo
que se quiere.

### El número que zanja cuándo hace falta el GC

Con 414 GiB y bloques de 4 KiB son ~108 millones de bloques. Un `.bex` de C
ocupa cinco. Aunque cada estrato guardara uno entero **sin compartir nada**,
caben **más de veinte millones** antes de rozar el 70 % — y como todo está
direccionado por contenido, lo que no cambia no se copia, así que el número real
es mucho mayor.

Hay un test que lo comprueba (`en_414_gib_caben_millones_de_estratos`), y por eso
el orden de la §10 no se toca: **el recolector va después de escribir, no antes**.

Es la misma postura de Git y no por casualidad: `git gc` no borra tu historia,
sólo lo que ya nadie alcanza, y el reflog guarda 90 días por si acaso. Para quien
acumula a propósito, el historial **es el producto**. Lo que hace falta desde el
primer día no es recoger: es **avisar**.

---

## 10. Orden de construcción

ESTRATOS no se empieza hasta que lo de abajo esté firme. Cada paso deja algo
que funciona por sí solo:

1. **FAT32 sobre A: (leer)** — el disco ya se lee por sectores y la GPT está
   parseada. Esto desbloquea la caja negra de CABINA y sacar los `.bex` de
   dentro del kernel. *No toca ESTRATOS.*
2. **Gate de identidad** — `IDENTIFY` ya da modelo y serie; falta que sea una
   comprobación de verdad. Sin esto no se escribe nada, en ningún sitio.
3. **Capa de bloques** — el contrato único `leer / escribir / capacidad /
   identidad`, con AHCI y NVMe debajo. ESTRATOS habla con eso, no con SATA.
4. ✅ **ESTRATOS solo lectura** — formatear desde el anfitrión con una
   herramienta del toolchain, y que el kernel lo monte y lea. Sin riesgo:
   si el formato está mal, se reformatea.
5. **ESTRATOS escritura** — log, estratos, barreras. Aquí empieza lo serio.
   *(La contabilidad de espacio de la §9 ya está: sin saber cuánto queda no se
   puede decidir si se acepta una escritura.)*
6. **Recolector** — cuando haya algo que recoger.
7. **TimeBack sobre ESTRATOS** — el historial deja de ser una copia.

---

## 11. Lo que ESTRATOS NO va a ser

Mismo criterio que BMO C y BMO C++: **acotado a propósito, terminable**.

- No hay volúmenes, RAID ni espejos.
- No hay compresión ni cifrado en la v1.
- No hay cuotas, ACLs ni usuarios: hay **capabilities**.
- No hay enlaces duros. Los simbólicos, quizá.
- No es POSIX y no lo intenta.
- No hay red.

Un sistema de ficheros que hace bien seis cosas es infinitamente más útil que
uno que hace treinta a medias — y sobre todo, es uno que **se puede terminar**.

---

## 12. Riesgos, dichos antes

- **Aquí se pierden datos.** Es el componente donde un bug no da un fault
  bonito en pantalla: se lleva el trabajo de alguien. De ahí las reglas de la
  §2 y el orden de la §10.
- **El recolector es lo difícil**, no el formato.
- **Las barreras de escritura hay que respetarlas.** Un SSD que dice "ya está"
  cuando el dato sigue en su caché convierte cualquier diseño transaccional en
  decoración. `FLUSH CACHE` no es opcional.
- **Nunca dos escritores.** Mientras no haya SMP y bloqueo de verdad, ESTRATOS
  se monta desde un solo sitio.

---

*Documento de diseño. La implementación empieza cuando los pasos 1 a 3 de la
§10 estén hechos y probados en hardware real.*
