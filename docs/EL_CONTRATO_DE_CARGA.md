# EL CONTRATO DE CARGA

> *"El kernel ese es tonto, no sabe a donde quiere."* -- Eddi, 2026-08-10
>
> Y tiene razon, con una correccion: el kernel **debe** ser tonto. El problema
> no es que no sepa a donde va -- es que le pedimos que lo AVERIGUARA.

## La frase que ordena el documento

**Hoy el kernel DEDUCE lo que un programa necesita. Manana el programa DECLARA,
el sistema CONCEDE, y el kernel solo COMPRUEBA.**

Es la misma linea que ya ordena el resto de BMO-X y que aqui se rompio sin que
nadie lo notara: **contratos y formatos, nunca cerebros**.

---

# PARTE 0 -- El experimento que dio permiso a esto

El arranque del 2026-08-10 en el Ryzen:

```
   46 t00026 FAULT proc:   cabecera invalida (magic, version o 0 secciones) =4B3D8
   47 t00026 WARN  proc:   el .bex de disco no paso la admision =1
   48 t00038 WARN  gui:    el .bex no paso la admision
```

Ningun `.bex` entra. **Ni siquiera el compositor.** Y la maquina sigue viva: el
disco lee, el shell lista `c\`, el teclado USB entrega teclas, `task=1/1`.

> **Fallo el cargador y no se llevo nada por delante.**

Eso no es una anecdota, es el resultado del experimento: **el cargador es una
pieza y fallo sola**. Nadie depende de COMO consigue sus bytes, solo de que los
consiga. Por eso lo que sigue no es un parche: es cambiar la pieza.

## Lo que esta probado, y lo que no

**Probado.** `inspect` solo devuelve `cabecera invalida` en una linea
(`bex.rs:397`): `magic != BEF1 || version_major != 1 || section_count == 0`. Los
ficheros de `staging` estan perfectos -- `BEF1`, version 1, 4 secciones,
`total_size` correcto en los cuatro que se miraron. **Los bytes que llegan al
bufer no son los del fichero.** Es transporte, no formato.

**Probado.** Uno de los fallos trae `=18300` = 99.072 bytes. No es multiplo de
512 y no es el tamano de ningun `.bex`. `read_file` solo puede devolver
multiplos de 512 o el tamano exacto del fichero, asi que ese numero **solo puede
ser `bex::necesita()`** -- o sea que la FASE 1 leyo un `BEF1` valido y una tabla
legible. **El prologo llega bien; la lectura grande rompe la cabecera.**

**NO probado, y no se va a suponer:** cual de los cinco commits del camino de
disco del 08-10 lo hace. Los cinco entraron entre las 14:40 y las 19:30 y
**ninguno ha visto un CPU**.

---

# PARTE 1 -- Por que la pieza esta mal, no rota

## El historial de la constante

| cuando | que paso | que se hizo |
|---|---|---|
| 08-07 | `c/read.bex` mide 1,1 MB y `MAX_BEX` es 1 MiB | subir la constante |
| 08-09 | el paquete de DOOM mide 6,3 MB contra 4 MiB | escalon 2: leer menos |
| 08-10 | escalon 2+3 para esquivar la constante | el bufer compartido se piso |

Tres episodios, **una sola constante**. Y el comentario de `lanzar.rs:130` ya lo
tenia escrito:

> *"Lo que este numero no arregla, dicho para que nadie lo suponga: el bufer
> sigue siendo uno y estatico... y sigue siendo una pagina de rebote."*

Estaba documentado como deuda y se estaba tratando como diseno.

## El cerebro que hay en Ring 0

```
   ring0/task/bex.rs:213     fn necesita(prologo: &[u8]) -> Result<usize, BexError>
```

El kernel abre el fichero, le recorre la tabla de secciones, suma offsets y
**deduce** cuanta memoria hace falta. Setenta lineas de politica dentro del
anillo cero para contestar una pregunta que el fichero deberia traer contestada.

## Y el hueco donde iba la respuesta lleva meses en el formato

`platform/abi/bmo-abi/src/bef/sections.rs:30`:

```rust
    /// Manifest TOML (capabilities, version, dependencies).
    Manifest = 0x09,
```

Declarada. **Nadie la escribe y nadie la lee.** Es exactamente lo que ya paso
con `Resources = 0x0B`: el sitio donde el programa dice lo que requiere estaba
en el contrato, vacio, mientras el kernel lo adivinaba.

---

# PARTE 2 -- Las cinco piezas

No hay orden de prioridad: **es un solo cambio con cinco caras**. El orden que
sigue es de DEPENDENCIA -- lo que hace falta para que lo siguiente pueda
existir -- y nada mas.

## A -- La verificacion AL ATERRIZAR

`verificar_hashes` (`bex.rs:305`) ya existe: cada `.bex` trae un BLAKE3 por
seccion. Corre **tarde y en el sitio equivocado**: sobre un bufer entero que
para entonces ya puede estar pisado, y despues de acotar la tabla.

**Cambia a:** cada seccion se cierra con su hash **cuando termina de aterrizar**,
antes de mapearse. Un transporte malo deja de ser `cabecera invalida` y pasa a
ser:

```
   FAULT proc: la seccion Code no cuadra con su hash  =<bytes que llegaron>
```

Esto es lo que convierte la medida en una **propiedad del diseno**. Hoy, para
saber que llego de verdad, hace falta anadir una linea de depuracion, mirarla en
una foto y quitarla. Despues, el sistema lo dice siempre, en el sitio exacto,
sin que nadie lo pida.

> **Una medida que hay que anadir para diagnosticar es una medida que no existe
> el dia que hace falta.**

## B -- Muere el bufer

Fuera `IMAGE` (4 MiB de `.bss`), fuera `MAX_BEX`, fuera `EN_USO` (el candado que
serializa **toda la maquina** por lanzamiento).

```
   HOY       disco --> bufer estatico de 4 MiB --> copia --> paginas del proceso
   MANANA    disco ------------------------------------> paginas del proceso
```

Y aqui esta lo que hace que esto no sea solo mas limpio, sino **correcto por
construccion**:

> El escalon 3 tiene que PREGUNTARLE a las tablas de pagina donde vive
> fisicamente un bufer virtual, pagina a pagina (`tramo_dma` -> `fisica_exacta`),
> y fiarse de la respuesta.
>
> Un marco recien pedido al asignador **no hay que preguntarselo a nadie**: mide
> exactamente una pagina, es fisicamente contiguo por definicion, y el asignador
> acaba de devolver su direccion fisica.

El destino de DMA perfecto ya lo teniamos y lo estabamos rodeando.

**Lo que sigue necesitando memoria provisional, dicho para que no parezca magia:**

| que | cuanto | de donde |
|---|---|---|
| el prologo | 2 KiB, acotado por el formato | estatico, y se queda |
| la tabla de relocations | lo que mida la seccion | marcos, y se sueltan |
| la seccion de firma | 8 + 40 por hash | marcos, y se sueltan |

Ninguna de las tres es una constante que haya que subir dentro de una semana.

**Y el techo del sistema cambia de sitio:** el tamano maximo de un programa deja
de ser un numero escrito en el kernel y pasa a ser la RAM que hay. Que es
justo lo que dice `LA_RAM.md`.

## C -- El fichero DECLARA lo que requiere, y el PORQUE

Seccion nueva: **`Requisitos = 0x15`**, tabla binaria de registros de tamano
fijo. Ring 0 la lee sin parser -- sin cerebro.

```
   registro de requisito (32 bytes)
   +--------+--------+------------------+------------------+
   | clase  | unidad |     cantidad     |  motivo (indice) |
   |  u16   |  u16   |       u64        |       u32        |
   +--------+--------+------------------+------------------+
   |                    reservado (20 B)                    |
   +--------------------------------------------------------+
```

`motivo` no es decoracion: es el indice de una cadena en la propia seccion, y es
**lo que sale por el "no"**. Hoy un rechazo dice `=1`. Despues dice el renglon
que el programa escribio.

```
   run apps/doom.bex
   no: pide 6,3 MB de recursos residentes  --  "el WAD se lee a demanda,
       pero la tabla de niveles vive en RAM mientras el juego corre"
```

**El TOML no muere:** se escribe en `Manifest 0x09` para humanos y para el
toolchain, y `bmo-pack` lo compila a la tabla binaria. Dos vistas del mismo
hecho, una sola fuente de verdad. Ring 0 lee la compilada.

Y no rompe nada: **un tipo de seccion desconocido se salta, no se rechaza** --
esa regla ya esta escrita en `bex::is_loadable` y es lo que ha mantenido vivo a
ELF treinta anos.

## D -- La admision devuelve el REQUISITO que fallo

```rust
   //  hoy
   fn admit_from_disk(..) -> Option<(u32, u32)>       // trece motivos -> None
   //  manana
   fn admit_from_disk(..) -> Result<(u32, u32), Requisito>
```

CABINA ya sabe la verdad desde el 08-09. Lo que no la sabe es **quien llamo**:
el `run` de Ring 3 sigue diciendo *"no paso la admision"* aunque el motivo este
en pantalla dos lineas mas arriba. Un codigo de error por la puerta del syscall
y el renglon completo por la consola.

## E -- Y lo otro, completado

La misma clase de numero, todos:

| donde | que | que lo sustituye |
|---|---|---|
| `bex.rs:54` | `MAX_BEX_SECTIONS = 16` | lo que declare la cabecera, acotado por el fichero |
| `proc.rs:578` | `MAX_PROGRAMS = 8` | el registro crece o se recicla |
| `proc.rs:230` | `MAX_DISK_NAMES = 6` | idem |
| `sonda.bex` | el renglon de ruta de 128 | el tamano que el llamante declare |
| `MI_PAQUETE` | los recursos, hoy sin lector a trozos | `fs::leer_tramo`, que ya existe |

Lo ultimo es lo que le falta a DOOM para sacar su WAD **sin que nadie reserve
6,3 MB de nada**.

---

# PARTE 2b -- LA FIRMA (apuntado el 2026-08-10, no hecho)

No entra hoy y no bloquea a nadie. Se escribe porque las dos decisiones de abajo
son baratas ahora y caras despues, y porque la tercera --de quien es la llave--
no es tecnica y conviene que este contestada antes de que alguien la conteste
sin darse cuenta.

## 1. La firma es del INDICE, no del bulto

Firmar "el fichero" obliga a **leer el fichero entero** para poder comprobar
nada. Para `doom.bex` son 814.664 bytes antes de decidir; con el WAD dentro,
cinco megas. Eso es el modelo bodega que `LA_RAM.md` prohibe, y en su peor
version: te lo traes todo, lo hasheas, y **despues** todavia hay que cargarlo.

Pero el fichero ya trae la respuesta:

```text
   doom.bex                     814.664 B
     seccion Signature              248 B    el BLAKE3 de las otras SEIS
```

Se firma la tabla de huellas. Esos 248 bytes responden por todo lo demas, asi
que la puerta queda:

```text
   1. prologo             2 KB    donde esta cada cosa
   2. tabla de huellas     248 B  y AQUI se comprueba la firma. La puerta.
   3. no cuadra -> no entra, y no se leyo ni el 0,03% del fichero
   4. cuadra    -> cada seccion contra su huella AL ATERRIZAR (pieza A)
```

> **La firma antes de entrar y el quirofano no se pelean: la firma es del
> indice, y el indice es diminuto.**

Y el dia que el WAD viaje dentro del paquete, su huella esta en esa misma tabla:
se comprueba **a trozos segun se lee**, sin tener nunca los 4,2 MB en RAM para
decidir si son buenos.

## 2. El escalon NO va dentro del fichero

`Ed25519Signature` ya existe en `signing.rs` --64 bytes de firma y **32 de clave
publica**-- y nadie la escribe. Es la tercera seccion del formato que lleva meses
declarada y vacia, detras de `Resources 0x0B` y `Manifest 0x09`.

Con esa clave dentro, la jerarquia **no necesita ni un campo nuevo**:

- el fichero lleva **quien lo firmo** (la clave publica),
- el sistema tiene una tabla de claves conocidas y **que autoriza cada una**.

Un campo "nivel" en el fichero seria un fichero declarando su propia autoridad.
Cualquiera escribiria el suyo. **El nivel lo pone quien reconoce la llave, no
quien la usa.**

## 3. Y una jerarquia solo es una jerarquia si los escalones PUEDEN cosas distintas

Si los tres niveles significan "arranca", son tres pegatinas. Se ganan el sitio
cuando enganchan con la pieza C, que es donde ya esta escrito lo que un programa
pide:

```text
   el .bex DECLARA      lo que requiere      (Requisitos 0x15)
   la llave DECIDE      lo que se le concede
```

La pantalla en exclusiva, la ventana de escritura del disco, la RAM sin tope: no
se conceden por lo que un programa pida, sino por **quien responde de el**. Y el
"no" sigue trayendo el renglon: *"pides la ventana de escritura y tu firma no
llega a eso"*.

** Los nombres importan mas de lo que parece. `publica / privada / especial`
dicen como te sientes con cada una; delante de un banco hace falta la otra
pregunta contestada -- **quien responde si esto rompe algo**: nadie, un socio con
pacto, o el dueno del sistema. Eso si se puede poner en un contrato.

## 4. [!] Y DOOM va SIN FIRMA, a proposito

Es la tentacion obvia --darle la llave especial y quitarselo de encima-- y es
donde peor cae:

- Es codigo de terceros bajo GPL, compilado desde una sonda que vive **fuera del
  repo precisamente por la licencia**. La llave especial es la que va delante de
  un banco; ponerla a responder por codigo ajeno es prestar el aval para una
  demo.
- Una llave que se le da a todo no distingue nada.
- Y el que de verdad importa: **el valor de DOOM como demostracion es que corre
  SIN firma**. Eso es lo que prueba que BMO-X no es un jardin cerrado. Firmarlo
  demuestra justo lo contrario de lo que se quiere ensenar.

DOOM es el caso "sin firma, corre igual, y el sistema dice de quien es y de quien
no" -- que es la misma regla que el resto del sistema ya aplica en todas partes:
no rechazar, **decirlo con su nombre**.

---

# PARTE 3 -- Lo que esto NO arregla

Dicho aqui para que nadie lo suponga leyendo lo de arriba:

**El fallo de transporte del 08-10 no desaparece solo.** Si el disco esta
escribiendo donde no debe, va a seguir haciendolo despues de las cinco piezas.

Lo que cambia es **lo que ese fallo puede hacer**:

| | hoy | despues |
|---|---|---|
| donde cae | un bufer compartido por toda la maquina | una pagina de UN proceso |
| que rompe | la cabecera de la imagen en curso | los datos de esa pagina |
| como se entera uno | `cabecera invalida`, que apunta al formato | `la seccion X no cuadra con su hash` |
| cuando | despues de leerlo todo | al aterrizar esa seccion |

De un fallo silencioso que miente sobre su causa, a uno que se nombra a si mismo
en el sitio donde ocurre. **Eso es lo que se compra, y es distinto de arreglarlo.**

---

# PARTE 4 -- Orden de dependencia

```
   C  (el formato declara)     ----+
                                   |
   A  (verificar al aterrizar)  ---+---> B (muere el bufer) ---> D (el motivo sale)
                                   |
   E  (las otras constantes)    ----+
```

- **C** puede entrar hoy: es aditivo, no rompe ningun `.bex` existente, y hasta
  que alguien la escriba la seccion simplemente no esta.
- **A** no depende de nada y es lo que hace que **B** se pueda depurar cuando
  falle -- por eso va antes, no por prioridad.
- **B** es el cambio grande y necesita que **A** ya este puesta.
- **D** cierra el circulo: el motivo llega a quien lanzo.
- **E** es trabajo suelto que no bloquea a nadie y no se aparca por eso.

---

*Escrito el 2026-08-10, con `gui.bex` sin cargar y el kernel corriendo
perfectamente. Las dos cosas a la vez son el argumento entero.*
