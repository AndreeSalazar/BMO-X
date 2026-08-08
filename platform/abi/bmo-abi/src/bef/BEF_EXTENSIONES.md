# BEF -- reglas de extension (CONGELADAS)

> *La puerta no pregunta el idioma. Pregunta que uses la puerta.*

**Estado**: en vigor. Implementado en `kernel/src/ring0/bex.rs` y
`platform/abi/bmo-abi/src/bef/header.rs`.

Este documento existe para que BEF pueda crecer **sin que el kernel crezca
con el**, y para que nadie --ni yo a las tres de la manana-- le anada un campo
"porque hacia falta".

---

## 1. El problema que resuelve

BMO-X ya ejecuta tres lenguajes sobre la misma puerta:

```
  asm     4 288 B  ->  INVOKE  ->  Ring 0
  C      12 376 B  ->  INVOKE  ->  Ring 0
  COBOL   5 144 B  ->  INVOKE  ->  Ring 0
```

Tres origenes, tres compiladores, cero adaptadores. La pregunta natural es
que pasa cuando entren Ada, un runtime de Java o el lenguaje que sea.

La respuesta **equivocada** seria darle a BEF un encabezado por lenguaje: un
bloque "Java", uno "C#", uno para el GIL de un interprete. Eso obligaria al
kernel a saber que es Java, que es C# y que es un GIL -- y la superficie
congelada dejaria de estar congelada, porque cada lenguaje nuevo anadiria un
campo que Ring 0 tendria que entender. Seria el embudo central disfrazado de
cabecera.

La respuesta **correcta** ya estaba medio construida: BEF tiene tabla de
secciones con tipo. Solo faltaba la regla.

---

## 2. LA REGLA

> **Una seccion de tipo desconocido se SALTA. No se rechaza, no se mapea.**

Es lo que ha mantenido vivo a ELF treinta anos: la seccion que no te incumbe
no es un error, es data que no vas a abrir.

Concretamente, el kernel mapea **cuatro** tipos y nada mas:

| Tipo | Que es | Como se mapea |
|---|---|---|
| `0x01` Code | Codigo | R+X |
| `0x02` RoData | Datos constantes | R |
| `0x03` Data | Datos mutables | R+W |
| `0x04` Bss | Sin inicializar | R+W, a ceros |

Todo lo demas --`Imports`, `Exports`, `Relocs`, `Symbols`, `Manifest`,
`Shaders`, `Resources`, `Tls`, `Unwind`, `Debug`, `Signature`, **y cualquier
tipo que este kernel no conozca**-- es data para OTRO: para el enlazador, para
el verificador, para el runtime de un lenguaje del que Ring 0 no tiene por que
saber que existe.

**Se salta, pero se valida.** Sus limites tienen que caber dentro del archivo:
una seccion mal formada sigue siendo un rechazo. Lo que no hace el kernel es
gastar paginas en ella ni mapearla en el espacio del programa.

*(Antes se mapeaban todas. Un manifiesto o una tabla de depuracion acababan en
el espacio de usuario como memoria escribible: gasto y superficie de ataque a
cambio de nada.)*

### Consecuencia practica

Un lenguaje con runtime propio puede meter en el contenedor lo que necesite
--tablas de clases, metadatos de tipos, mapas de excepciones-- **sin pedirle
permiso al kernel ni anadirle un campo**. El programa lo lee el mismo, porque
sabe donde esta y que significa. Ring 0 nunca lo abre.

---

## 3. Lo que el kernel SI lee del encabezado

Siete cosas. **Ninguna nombra un lenguaje.**

| Offset | Campo | Por que lo necesita el kernel |
|---|---|---|
| 0 | `magic` | Es un BEF o no lo es |
| 4 | `version_major/minor` | Contrato del formato |
| 8 | `flags` | Es ejecutable? |
| 12 | `arch` | Es mi CPU? |
| **13** | **`endianness`** | Puedo leer sus numeros? |
| **14** | **`cpu_features`** | Se preservar su estado al cambiar de contexto? |
| 16 | `abi_version` | Habla mi ABI? |
| 24 | `entry_offset` | Donde empieza |
| 32/40 | tabla de secciones | Que mapear |

Un binario de Ada y uno de C# rellenan **los mismos campos** con numeros
distintos. Eso es un contrato. Lo otro habria sido un catalogo.

---

## 4. Los dos campos nuevos, y por que valen HOY

### `endianness` (offset 13)

`0` = little, `1` = big. Hoy el kernel solo lee little y rechaza lo demas.

Esta congelado ahora aunque no exista todavia un objetivo big-endian, porque
el dia que lo haya --PowerPC, un RISC-V configurado asi-- este byte es la
diferencia entre **anadir una comprobacion** y **reescribir todos los parsers
del sistema**. Todo el codigo de formato usa hoy `from_le_bytes`: BEF, GPT,
BPB de FAT32. El byte cuesta cero ahora.

### `cpu_features` (offset 14, mapa de bits)

| Bit | Significa |
|---|---|
| 0 | Vectores de 256 bits (AVX/AVX2, SVE) |
| 1 | Vectores de 512 bits (AVX-512) |

**Un bit desconocido se RECHAZA** -- al reves que una seccion desconocida, y por
un motivo exacto:

> Una seccion que no entiendo es data inerte.
> Una extension de CPU que no entiendo es **estado que no voy a preservar**.

Hoy `trap.rs` usa `FXSAVE`, que guarda x87 y SSE pero **no** la mitad alta de
los YMM. Un programa que use AVX se corromperia en silencio a la primera
interrupcion del temporizador -- la peor clase de fallo que hay.

Por eso el kernel **rechaza hoy cualquier `cpu_features != 0`**. Ese rechazo
no es una limitacion: es la mejora. Convierte una corrupcion silenciosa en un
"no" con nombre (`UnsupportedCpuFeature`), **antes** de que exista el XSAVE.
Cuando el kernel sepa guardar el estado ancho, esa linea se relaja. No antes.

Y el programa **puede** declararlo porque su compilador lo sabe: BMO C y BMO
COBOL son tuyos y saben perfectamente si emitieron una instruccion ancha.
Declararlo es un contrato verificable; adivinarlo en ejecucion no lo es.

---

## 5. Los otros lenguajes, segun esta regla

**Ada** -- compila a nativo AOT y tiene perfiles de runtime minimo (Ravenscar,
pensado para sistemas criticos sin sistema operativo). Es un `.bex` normal.
Encaja mejor que C: su filosofia es contratos explicitos, que es la de BMO.

**Java / C#** -- no son lenguajes compilados sino ecosistemas con maquina
virtual. Dos caminos, los dos validos, y en los dos **el kernel nunca aprende
que es Java**:

1. **Compilar AOT a nativo** (lo que ya hacen NativeAOT y native-image). Sale
   un binario que solo necesita su runtime encima de los tres syscalls. Otro
   `.bex` mas.
2. **Portar la VM como programa de Ring 3.** Entonces la JVM es una *app*, y
   los `.class` son **datos que esa app lee**. Sus metadatos viajan en
   secciones que el kernel salta.

**El GIL de un interprete** no es asunto del kernel: es un mutex *dentro* de
un interprete. Si alguien porta CPython a BMO, el GIL es problema de CPython.

---

## 6. Reglas para quien anada algo a BEF

1. **El kernel necesita este dato para MAPEAR o EJECUTAR la imagen?**
   Si la respuesta es no, va en una seccion, no en el encabezado.
2. **Nombra un lenguaje, un runtime o un producto?**
   Entonces no va en el encabezado. Nunca.
3. **Si va en el encabezado, que pasa si un productor viejo lo deja a cero?**
   El cero tiene que ser el valor seguro y por defecto.
4. **Los bytes `18..24` estan reservados y DEBEN ser cero.** Un productor que
   escriba basura ahi se encontrara con que el campo significa algo en una
   version futura.
5. **Un tipo de seccion nuevo no necesita permiso de nadie.** Se elige un
   numero libre, se documenta, y el kernel lo salta solo.

---

## 7. Relacion con ESTRATOS

En el sistema de ficheros propio (`platform/drivers/storage/estratos/`)
esta misma idea aparece un piso mas arriba: un `.bex` no es un chorro de bytes
sino un nodo con flujos con nombre --`:datos`, `:firma`, `:manifiesto`,
`:origen`-- y el kernel solo abre los que le incumben.

Es la misma regla en dos capas: **contenedores extensibles donde quien no
entiende algo, lo salta.** Por eso el manifiesto de capabilities no puede
separarse del binario, y por eso un lenguaje nuevo no obliga a tocar Ring 0.

---

*Contratos y formatos, nunca cerebros.*

---

# ★★ PUEDE MEJORAR EL BEF? -- auditado el 2026-08-04

Pregunta del dueno, y la respuesta es la contraria de la que esperaba:

> **El BEF esta POR DELANTE del sistema, no por detras.**

## El numero

3.047 lineas de formato. Y el kernel usa **la cabecera y cuatro tipos de
seccion**. Lo demas esta escrito y no lo lee nadie:

| Modulo | Lineas | Usuarios fuera de `bef/` |
|---|---|---|
| `validator.rs` | 1059 | los 4 frontends ✅ |
| `signing.rs` | 238 | 2 |
| `imports.rs` `exports.rs` `relocations.rs` `symbols.rs` `tls.rs` | 542 | **1 cada uno** -- o sea, nadie |
| `manifest.rs` | 246 | **0** |

`ImportEntry` ya lleva `library_name_off`, `symbol_hash` y `binding_offset`.
Las tablas de enlazado **existen enteras**. Lo que no existe es quien las lea.

★ **El cuello de botella no es el formato: es que nada habla el idioma que el
formato ya sabe.** Y eso es el enlazador, otra vez.

## El "truco" para correr binarios ajenos -- ya tiene nombre aqui

En `BefFlags` hay dos banderas puestas hace tiempo y sin implementar:

```rust
const PROVENANCE_PE  = 1 << 14;   // origen: PE devorado
const PROVENANCE_ELF = 1 << 15;   // origen: ELF devorado
```

**Devorar** no es Wine. Wine reimplementa la API de Windows *en ejecucion*;
devorar es **traducir al cargar** -- leer el ELF/PE, colocar sus secciones como
un BEF y dejar que corra sobre la puerta de BMO. Se parece mas a lo que hizo
Rosetta con la traduccion anticipada que a una capa de compatibilidad.

### Y el limite honesto, que es donde se decide si sirve

**Devorar te da el CODIGO, no el CONTRATO.**

Un binario de Linux hace `syscall` con el numero 1 esperando `write(2)` y la
semantica de un descriptor de fichero. BMO tiene **tres syscalls** con otros
numeros y otra semantica. El codigo traducido corre; **la primera llamada al
sistema se estrella.**

De ahi sale la unica categoria que devorar alcanza sin inventarse un POSIX:

> **Binarios estaticos y autonomos que solo COMPUTAN.**

Codecs, compresores, criptografia, solvers, un interprete que reciba y
devuelva memoria. Nada que abra un fichero, hable por red o pinte.

No es poco: es media biblioteca de algoritmos del mundo sin portar una linea.
Pero hay que decirlo entero -- **con esto no corre Steam ni Chrome, y ninguna
cantidad de trabajo en el BEF cambia eso.** Lo que los bloquea no es el
formato: es POSIX, hilos, red y GPU.

## Lo que al BEF le FALTA de verdad -- 4 huecos

Auditado campo a campo. Lo que no esta y hara falta:

### 1 - Constructores estaticos (`init` / `fini`) -- **el unico que bloquea hoy**

No hay `SectionKind` ni flag para ellos. Y los piden **dos lenguajes que ya
corren en esta maquina**:

- **C++**: los constructores de objetos globales tienen que ejecutarse antes de
  `main`. Sin esto, un `static std::string` nace sin construir.
- **Ada**: la *elaboracion* de paquetes es exactamente lo mismo con otro nombre.

Es un tipo de seccion con una lista de punteros a funcion y un bucle en el
`crt0`. Pequeno, y es la pieza que hace que C++ sea C++ y no C con clases.

### 2 - Versionado de simbolos

Para hacer crecer la libc **sin romper los programas de ayer**. Hoy un simbolo
es un nombre y un hash; el dia que `printf` cambie de firma no hay forma de que
convivan las dos.

Barato ahora, carisimo cuando ya haya binarios en el disco de alguien.

### 3 - Recursos declarados en la admision

Hoy un programa pide memoria y **descubre el tope a la quinta peticion**
(`MAX_PETICIONES = 4`). Un campo en el manifiesto --*"necesito 64 MiB y 3
ficheros"*-- deja que el kernel decida **antes de arrancarlo**.

★ Es la forma con la que este sistema piensa: un contrato negociado en la
admision, no un fallo descubierto a mitad de la ejecucion. Encaja con
capabilities mejor que con nada.

### 4 - ~~Dos banderas que prometen y no cumplen~~ -> **CERRADO el 2026-08-06**

`COMPRESSED` y `HOT_RELOADABLE` estan declaradas y **no hay implementacion
detras**. No son un fallo mientras nadie las ponga, pero una bandera que un
productor podria escribir y ningun consumidor entiende es una trampa esperando.
O se cablean, o se marcan como reservadas.

**Se marcaron como reservadas**, y por el camino se destapo que el problema era
mucho mas grande que dos banderas.

---

## ★★ EL AGUJERO QUE ESTABA DEBAJO -- auditado el 2026-08-06

La pregunta *"quien lee esta bandera?"* se hizo para las doce, no para dos. El
resultado, contado con `grep`:

| Bandera | Consumidores reales |
|---|---|
| `EXECUTABLE` / `SHARED_LIBRARY` | 1 -- el validador exige que haya una de las dos |
| `HAS_MANIFEST` - `HAS_SHADERS` - `HAS_TLS` - `SIGNED` | **0** |
| `COMPRESSED` - `HOT_RELOADABLE` - `PIE` - `USES_BAREX` | **0** |
| `PROVENANCE_PE` - `PROVENANCE_ELF` | **0** |

**De doce banderas, el validador miraba dos.** Un BEF podia declarar `SIGNED`
sin traer firma, `HAS_MANIFEST` sin manifiesto y `HAS_TLS` sin TLS -- y el
validador contestaba **valido**.

Eso no es una comprobacion pendiente: **es un campo que miente por
construccion**. Y duele mas aqui que en cualquier otro sitio del proyecto,
porque el header del BEF es *la parte congelada* -- lo que `CONTRIBUTING.md`
promete que no se mueve para que **una auditoria sirva para todos**. Un contrato
inmutable cuyos campos nadie verifica es un contrato inmutable que miente.

### Lo que se cableo: `validate_flag_coherence`

1. **Bandera puesta sin su seccion -> ERROR.** El binario afirma algo falso sobre
   si mismo. Hoy nada en BMO puede producir eso, asi que exigirlo no rompio ni
   un binario (790 tests verdes antes y despues).
2. **Seccion presente sin su bandera -> AVISO.** Es el caso que si existe: los
   frontends escriben las secciones y no ponen las banderas. No es mentira, es
   omision -- pero *la razon de ser de la bandera es que un consumidor decida sin
   recorrer la tabla*, y quien se fie de ella no mirara. **Sube a error el dia
   que los productores las pongan.**
3. **`COMPRESSED` y `HOT_RELOADABLE` -> ERROR si se ponen.** Esta es la respuesta
   a la pregunta de arriba: *reservadas*, hasta que exista la descompresion y la
   recarga.
4. **`PROVENANCE_*` -> AVISO.** Las pone el cargador al **devorar**, y devorar no
   existe todavia; el dia que exista seran legitimas y nadie tendra que volver
   aqui.

Seis pruebas nuevas fijan las cuatro reglas. La leccion general es la misma que
dejo el barrido de las agujas del mismo dia: **un campo que nadie comprueba no
es documentacion optimista, es una mentira con fecha de caducidad.**

## El veredicto

| | |
|---|---|
| Le falta al BEF para el enlazador? | **No.** Esta todo: imports, exports, relocs, symbols |
| Para hilos? | **No.** `Tls` existe |
| Para otra arquitectura? | **No.** `arch`, `endianness` y `cpu_features` estan reservados |
| Para C++ y Ada de verdad? | ★ **SI: los constructores estaticos** |
| Para crecer sin romper? | ★ **SI: el versionado de simbolos** |
| Para Chrome o Steam? | **No es cuestion del BEF.** Es POSIX, hilos, red y GPU |

**Un formato que va por delante del sistema es el problema bueno de tener.**
Significa que crecer no pide redisenar el contrato -- pide escribir los
lectores. Y en zona virgen, eso es exactamente lo que se queria.
