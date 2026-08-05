# BEF — reglas de extensión (CONGELADAS)

> *La puerta no pregunta el idioma. Pregunta que uses la puerta.*

**Estado**: en vigor. Implementado en `kernel/src/ring0/bex.rs` y
`platform/abi/bmo-abi/src/bef/header.rs`.

Este documento existe para que BEF pueda crecer **sin que el kernel crezca
con él**, y para que nadie —ni yo a las tres de la mañana— le añada un campo
"porque hacía falta".

---

## 1. El problema que resuelve

BMO-X ya ejecuta tres lenguajes sobre la misma puerta:

```
  asm     4 288 B  →  INVOKE  →  Ring 0
  C      12 376 B  →  INVOKE  →  Ring 0
  COBOL   5 144 B  →  INVOKE  →  Ring 0
```

Tres orígenes, tres compiladores, cero adaptadores. La pregunta natural es
qué pasa cuando entren Ada, un runtime de Java o el lenguaje que sea.

La respuesta **equivocada** sería darle a BEF un encabezado por lenguaje: un
bloque "Java", uno "C#", uno para el GIL de un intérprete. Eso obligaría al
kernel a saber qué es Java, qué es C# y qué es un GIL — y la superficie
congelada dejaría de estar congelada, porque cada lenguaje nuevo añadiría un
campo que Ring 0 tendría que entender. Sería el embudo central disfrazado de
cabecera.

La respuesta **correcta** ya estaba medio construida: BEF tiene tabla de
secciones con tipo. Solo faltaba la regla.

---

## 2. LA REGLA

> **Una sección de tipo desconocido se SALTA. No se rechaza, no se mapea.**

Es lo que ha mantenido vivo a ELF treinta años: la sección que no te incumbe
no es un error, es data que no vas a abrir.

Concretamente, el kernel mapea **cuatro** tipos y nada más:

| Tipo | Qué es | Cómo se mapea |
|---|---|---|
| `0x01` Code | Código | R+X |
| `0x02` RoData | Datos constantes | R |
| `0x03` Data | Datos mutables | R+W |
| `0x04` Bss | Sin inicializar | R+W, a ceros |

Todo lo demás —`Imports`, `Exports`, `Relocs`, `Symbols`, `Manifest`,
`Shaders`, `Resources`, `Tls`, `Unwind`, `Debug`, `Signature`, **y cualquier
tipo que este kernel no conozca**— es data para OTRO: para el enlazador, para
el verificador, para el runtime de un lenguaje del que Ring 0 no tiene por qué
saber que existe.

**Se salta, pero se valida.** Sus límites tienen que caber dentro del archivo:
una sección mal formada sigue siendo un rechazo. Lo que no hace el kernel es
gastar páginas en ella ni mapearla en el espacio del programa.

*(Antes se mapeaban todas. Un manifiesto o una tabla de depuración acababan en
el espacio de usuario como memoria escribible: gasto y superficie de ataque a
cambio de nada.)*

### Consecuencia práctica

Un lenguaje con runtime propio puede meter en el contenedor lo que necesite
—tablas de clases, metadatos de tipos, mapas de excepciones— **sin pedirle
permiso al kernel ni añadirle un campo**. El programa lo lee él mismo, porque
sabe dónde está y qué significa. Ring 0 nunca lo abre.

---

## 3. Lo que el kernel SÍ lee del encabezado

Siete cosas. **Ninguna nombra un lenguaje.**

| Offset | Campo | Por qué lo necesita el kernel |
|---|---|---|
| 0 | `magic` | Es un BEF o no lo es |
| 4 | `version_major/minor` | Contrato del formato |
| 8 | `flags` | ¿Es ejecutable? |
| 12 | `arch` | ¿Es mi CPU? |
| **13** | **`endianness`** | ¿Puedo leer sus números? |
| **14** | **`cpu_features`** | ¿Sé preservar su estado al cambiar de contexto? |
| 16 | `abi_version` | ¿Habla mi ABI? |
| 24 | `entry_offset` | Dónde empieza |
| 32/40 | tabla de secciones | Qué mapear |

Un binario de Ada y uno de C# rellenan **los mismos campos** con números
distintos. Eso es un contrato. Lo otro habría sido un catálogo.

---

## 4. Los dos campos nuevos, y por qué valen HOY

### `endianness` (offset 13)

`0` = little, `1` = big. Hoy el kernel solo lee little y rechaza lo demás.

Está congelado ahora aunque no exista todavía un objetivo big-endian, porque
el día que lo haya —PowerPC, un RISC-V configurado así— este byte es la
diferencia entre **añadir una comprobación** y **reescribir todos los parsers
del sistema**. Todo el código de formato usa hoy `from_le_bytes`: BEF, GPT,
BPB de FAT32. El byte cuesta cero ahora.

### `cpu_features` (offset 14, mapa de bits)

| Bit | Significa |
|---|---|
| 0 | Vectores de 256 bits (AVX/AVX2, SVE) |
| 1 | Vectores de 512 bits (AVX-512) |

**Un bit desconocido se RECHAZA** — al revés que una sección desconocida, y por
un motivo exacto:

> Una sección que no entiendo es data inerte.
> Una extensión de CPU que no entiendo es **estado que no voy a preservar**.

Hoy `trap.rs` usa `FXSAVE`, que guarda x87 y SSE pero **no** la mitad alta de
los YMM. Un programa que use AVX se corrompería en silencio a la primera
interrupción del temporizador — la peor clase de fallo que hay.

Por eso el kernel **rechaza hoy cualquier `cpu_features != 0`**. Ese rechazo
no es una limitación: es la mejora. Convierte una corrupción silenciosa en un
"no" con nombre (`UnsupportedCpuFeature`), **antes** de que exista el XSAVE.
Cuando el kernel sepa guardar el estado ancho, esa línea se relaja. No antes.

Y el programa **puede** declararlo porque su compilador lo sabe: BMO C y BMO
COBOL son tuyos y saben perfectamente si emitieron una instrucción ancha.
Declararlo es un contrato verificable; adivinarlo en ejecución no lo es.

---

## 5. Los otros lenguajes, según esta regla

**Ada** — compila a nativo AOT y tiene perfiles de runtime mínimo (Ravenscar,
pensado para sistemas críticos sin sistema operativo). Es un `.bex` normal.
Encaja mejor que C: su filosofía es contratos explícitos, que es la de BMO.

**Java / C#** — no son lenguajes compilados sino ecosistemas con máquina
virtual. Dos caminos, los dos válidos, y en los dos **el kernel nunca aprende
qué es Java**:

1. **Compilar AOT a nativo** (lo que ya hacen NativeAOT y native-image). Sale
   un binario que solo necesita su runtime encima de los tres syscalls. Otro
   `.bex` más.
2. **Portar la VM como programa de Ring 3.** Entonces la JVM es una *app*, y
   los `.class` son **datos que esa app lee**. Sus metadatos viajan en
   secciones que el kernel salta.

**El GIL de un intérprete** no es asunto del kernel: es un mutex *dentro* de
un intérprete. Si alguien porta CPython a BMO, el GIL es problema de CPython.

---

## 6. Reglas para quien añada algo a BEF

1. **¿El kernel necesita este dato para MAPEAR o EJECUTAR la imagen?**
   Si la respuesta es no, va en una sección, no en el encabezado.
2. **¿Nombra un lenguaje, un runtime o un producto?**
   Entonces no va en el encabezado. Nunca.
3. **Si va en el encabezado, ¿qué pasa si un productor viejo lo deja a cero?**
   El cero tiene que ser el valor seguro y por defecto.
4. **Los bytes `18..24` están reservados y DEBEN ser cero.** Un productor que
   escriba basura ahí se encontrará con que el campo significa algo en una
   versión futura.
5. **Un tipo de sección nuevo no necesita permiso de nadie.** Se elige un
   número libre, se documenta, y el kernel lo salta solo.

---

## 7. Relación con ESTRATOS

En el sistema de ficheros propio (`platform/drivers/storage/estratos/`)
esta misma idea aparece un piso más arriba: un `.bex` no es un chorro de bytes
sino un nodo con flujos con nombre —`:datos`, `:firma`, `:manifiesto`,
`:origen`— y el kernel solo abre los que le incumben.

Es la misma regla en dos capas: **contenedores extensibles donde quien no
entiende algo, lo salta.** Por eso el manifiesto de capabilities no puede
separarse del binario, y por eso un lenguaje nuevo no obliga a tocar Ring 0.

---

*Contratos y formatos, nunca cerebros.*

---

# ★★ ¿PUEDE MEJORAR EL BEF? — auditado el 2026-08-04

Pregunta del dueño, y la respuesta es la contraria de la que esperaba:

> **El BEF está POR DELANTE del sistema, no por detrás.**

## El número

3.047 líneas de formato. Y el kernel usa **la cabecera y cuatro tipos de
sección**. Lo demás está escrito y no lo lee nadie:

| Módulo | Líneas | Usuarios fuera de `bef/` |
|---|---|---|
| `validator.rs` | 1059 | los 4 frontends ✅ |
| `signing.rs` | 238 | 2 |
| `imports.rs` `exports.rs` `relocations.rs` `symbols.rs` `tls.rs` | 542 | **1 cada uno** — o sea, nadie |
| `manifest.rs` | 246 | **0** |

`ImportEntry` ya lleva `library_name_off`, `symbol_hash` y `binding_offset`.
Las tablas de enlazado **existen enteras**. Lo que no existe es quien las lea.

★ **El cuello de botella no es el formato: es que nada habla el idioma que el
formato ya sabe.** Y eso es el enlazador, otra vez.

## El "truco" para correr binarios ajenos — ya tiene nombre aquí

En `BefFlags` hay dos banderas puestas hace tiempo y sin implementar:

```rust
const PROVENANCE_PE  = 1 << 14;   // origen: PE devorado
const PROVENANCE_ELF = 1 << 15;   // origen: ELF devorado
```

**Devorar** no es Wine. Wine reimplementa la API de Windows *en ejecución*;
devorar es **traducir al cargar** — leer el ELF/PE, colocar sus secciones como
un BEF y dejar que corra sobre la puerta de BMO. Se parece más a lo que hizo
Rosetta con la traducción anticipada que a una capa de compatibilidad.

### Y el límite honesto, que es donde se decide si sirve

**Devorar te da el CÓDIGO, no el CONTRATO.**

Un binario de Linux hace `syscall` con el número 1 esperando `write(2)` y la
semántica de un descriptor de fichero. BMO tiene **tres syscalls** con otros
números y otra semántica. El código traducido corre; **la primera llamada al
sistema se estrella.**

De ahí sale la única categoría que devorar alcanza sin inventarse un POSIX:

> **Binarios estáticos y autónomos que sólo COMPUTAN.**

Códecs, compresores, criptografía, solvers, un intérprete que reciba y
devuelva memoria. Nada que abra un fichero, hable por red o pinte.

No es poco: es media biblioteca de algoritmos del mundo sin portar una línea.
Pero hay que decirlo entero — **con esto no corre Steam ni Chrome, y ninguna
cantidad de trabajo en el BEF cambia eso.** Lo que los bloquea no es el
formato: es POSIX, hilos, red y GPU.

## Lo que al BEF le FALTA de verdad — 4 huecos

Auditado campo a campo. Lo que no está y hará falta:

### 1 · Constructores estáticos (`init` / `fini`) — **el único que bloquea hoy**

No hay `SectionKind` ni flag para ellos. Y los piden **dos lenguajes que ya
corren en esta máquina**:

- **C++**: los constructores de objetos globales tienen que ejecutarse antes de
  `main`. Sin esto, un `static std::string` nace sin construir.
- **Ada**: la *elaboración* de paquetes es exactamente lo mismo con otro nombre.

Es un tipo de sección con una lista de punteros a función y un bucle en el
`crt0`. Pequeño, y es la pieza que hace que C++ sea C++ y no C con clases.

### 2 · Versionado de símbolos

Para hacer crecer la libc **sin romper los programas de ayer**. Hoy un símbolo
es un nombre y un hash; el día que `printf` cambie de firma no hay forma de que
convivan las dos.

Barato ahora, carísimo cuando ya haya binarios en el disco de alguien.

### 3 · Recursos declarados en la admisión

Hoy un programa pide memoria y **descubre el tope a la quinta petición**
(`MAX_PETICIONES = 4`). Un campo en el manifiesto —*"necesito 64 MiB y 3
ficheros"*— deja que el kernel decida **antes de arrancarlo**.

★ Es la forma con la que este sistema piensa: un contrato negociado en la
admisión, no un fallo descubierto a mitad de la ejecución. Encaja con
capabilities mejor que con nada.

### 4 · Dos banderas que prometen y no cumplen

`COMPRESSED` y `HOT_RELOADABLE` están declaradas y **no hay implementación
detrás**. No son un fallo mientras nadie las ponga, pero una bandera que un
productor podría escribir y ningún consumidor entiende es una trampa esperando.
O se cablean, o se marcan como reservadas.

## El veredicto

| | |
|---|---|
| ¿Le falta al BEF para el enlazador? | **No.** Está todo: imports, exports, relocs, symbols |
| ¿Para hilos? | **No.** `Tls` existe |
| ¿Para otra arquitectura? | **No.** `arch`, `endianness` y `cpu_features` están reservados |
| ¿Para C++ y Ada de verdad? | ★ **SÍ: los constructores estáticos** |
| ¿Para crecer sin romper? | ★ **SÍ: el versionado de símbolos** |
| ¿Para Chrome o Steam? | **No es cuestión del BEF.** Es POSIX, hilos, red y GPU |

**Un formato que va por delante del sistema es el problema bueno de tener.**
Significa que crecer no pide rediseñar el contrato — pide escribir los
lectores. Y en zona virgen, eso es exactamente lo que se quería.
