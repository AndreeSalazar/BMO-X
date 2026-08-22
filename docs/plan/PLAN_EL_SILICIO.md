# PLAN EL SILICIO

> **Que INTI hable el idioma de este procesador, y que el perfil diga cual de
> los dos dialectos habla.**
>
> Escrito el **2026-08-22**, el mismo dia que la sonda contesto `reglas = 0` en
> el Ryzen. Nace de tres frases del dueno:
>
> > *"que mi Inti aprenda hablar el idioma de CPU a base de que tipo de perfil
> > para respetar y asi poder hacer que la CPU ejecute las katanas"*
>
> > *"investigue datos en C y ignora por completo lo que la CPU intenta decir
> > sus red flag, es como si un atleta profesional corriera en atajos, eso no
> > cuenta"*
>
> > *"el samurai NO EXISTE la duda: o cortas o seras cortado. Eso es todo, el 0
> > y el 1"*

---

## 0. QUE ES, EN UNA FRASE

**Las doce reglas dejan de ser codigo que el compilador escribe y pasan a ser un
contrato que el binario declara y el procesador ejecuta** -- y **el perfil es
quien decide cual de las dos cosas es**, porque no todo codigo puede permitirse
que corte otro.

```text
   HOY      el compilador emite la comprobacion    llano y pleno, igual
   MANANA   `llano`  la lleva el programa          porque puede SER el manejador
            `pleno`  la lleva el silicio           porque tiene a quien volver
```

---

## 1. LA RAIZ: quien ejecuta el corte

Toda regla anti-UB tiene tres partes, y **solo la tercera se discute**:

```text
   1. el silicio LEVANTA la bandera    `of` tras un `imul`, `#DE` tras un `idiv`
   2. alguien la MIRA                  o no
   3. alguien EJECUTA el corte         el programa, el sistema, o nadie
```

| quien corta | quien es | coste en el camino normal | si el numero no cabe |
|---|---|---|---|
| **nadie** | C | 0 | el programa sigue con un numero inventado |
| **el compilador** | INTI hoy | 1 a 6 instrucciones | atrapa |
| **el silicio** | INTI manana, en `pleno` | **0** | atrapa |

Y ahi esta el argumento entero: la columna del coste de `pleno` es **la misma
que la de C**. Salir del comportamiento indefinido sin pagar nada es alcanzable
en este procesador, y **C no lo cogio porque C tiene que correr en maquinas que
no tienen estas trampas**. BMO-X corre en una.

Hay un cuarto participante que no sale en la tabla y decide mas que los tres:
**el sistema, que le manda callar al silicio**. Ver 2.2.

### 1.1 Lo que C hace, dicho con justicia

*"C ignora las red flags"* es verdad en el resultado e injusto en el motivo.

En 1978 el desbordamiento con signo era indefinido **porque las maquinas no se
ponian de acuerdo**: complemento a uno, signo-magnitud, aritmetica que atrapaba.
El comportamiento indefinido era **el precio de la portabilidad entre maquinas
que se portaban distinto**, y era un precio honrado.

**C23 hizo obligatorio el complemento a dos.** El desacuerdo se acabo. La razon
caduco y el comportamiento indefinido se quedo, porque para entonces los
compiladores lo usaban para optimizar.

En BMO-X: **una arquitectura, un toolchain, todas las fuentes**. La portabilidad
que compraba ese precio no compra aqui absolutamente nada. Pagar en 2026 un
precio de 1978 por un problema que no se tiene es lo que no tiene sentido -- y
esa es la frase justa, mas fuerte que *"C miente"* porque se puede verificar.

### 1.2 Y por eso un UB no es un fallo: son DOS funciones

**Cada comportamiento indefinido de C es, en INTI, dos nombres.**

```text
   C        a + b   desborda -> indefinido. Una cosa, sin nombre.
   INTI     a + b               atrapa, E1001
            suma_circular(a, b) da la vuelta A PROPOSITO, y esta definido
```

El UB no se elimina: **se parte en dos y las dos se nombran**. La que atrapa es
para quien no lo esperaba; la que da la vuelta es para quien la queria. Eso es
lo que quiere decir *"si sale UB no es fallo, se puede resolver y aprovechar"*,
y ya esta hecho en la Regla 1. **Es la plantilla para las que falten.**

---

## 2. LO QUE HAY HOY, MEDIDO

### 2.1 Lo que cuesta cada katana, en instrucciones

Contadas en `emisor-x86_64/src/reglas.rs` y en `emitir_funcion`, sobre el camino
que **no** atrapa:

| regla | como se comprueba hoy | instr. | lo que el silicio ofrecia |
|---|---|---|---|
| **1** desborde | `jo` detras del `imul`/`add` | **1** | `of`, ya puesta. **esto ya es optimo** |
| **3** entre cero | `carga` + `test` + `jz`, ANTES del `idiv` | **3** | `#DE`, vector 0, **gratis** |
| **12** conversion | centinela + `comisd` + `jp` + `jne` (+ ancho) | **3 a 6** | `#XM`, vector 19, **gratis** |

La Regla 1 no tiene nada que ganar: el `jo` **ya es** leerle la bandera al
silicio. Es la prueba de que el modelo funciona, y por eso es la que corre desde
F0.

Las otras dos **pagan en software por una senal que el hardware regala**. Y no
por ignorancia: el propio emisor lo tiene escrito.

> *"mirar el resultado no sirve de nada porque dividir entre cero no deja
> resultado, deja una excepcion del procesador"*
> -- `emisor-x86_64/src/lib.rs`, Regla 3

**El motivo real es otro, y es la raiz de este plan: el camino de la trampa es
una lapida, no un camino de vuelta.** `kernel/src/ring0/core/autopsy.rs`
convierte cualquier excepcion de Ring 3 en un informe y se lleva la tarea por
delante. No hay forma de que un `#DE` vuelva al programa como un error que es un
dato. **INTI no comprueba antes porque no sepa: comprueba antes porque no tiene
a donde volver.**

### 2.2 El sistema le manda callar al silicio, y luego INTI paga por escucharlo

```text
   faggin/s1_cpu/src/cpu/mod.rs:241    let mxcsr: u32 = 0x1F80;  ldmxcsr
   kernel/src/ring0/plat/trap.rs:362   ((xsave_base + 24) ...).write(0x1F80)
```

`0x1F80` son **las seis excepciones de coma flotante enmascaradas**. O sea: el
sistema le dice al procesador *"no levantes la mano, devuelve el centinela"*.

Y despues INTI gasta hasta seis instrucciones en `regla_doce` **reconstruyendo
en software el veredicto que el procesador estaba dispuesto a dar en cero**, con
el agravante de que el centinela es ambiguo --tambien es el resultado legitimo
de convertir `-2^63`-- y por eso hacen falta dos preguntas en vez de una.

**Enmascarar es la eleccion correcta HOY** (nadie recogeria el `#XM`) y es
exactamente la eleccion que este plan tiene que poder revertir **por tarea**.

### 2.3 LA KATANA QUE HOY SUSURRA

Asi atrapa INTI, en bytes:

```asm
   atrapa:  mov  rax, 1003
            mov  rsp, rbp
            pop  rbp
            ret
```

**Atrapar es devolver un numero.** Y un numero devuelto es indistinguible de un
resultado: una funcion `devuelve natural64` que atrapa devuelve `1003`, que es
un `natural64` perfectamente legitimo. **Quien la llama no tiene forma de saber
cual de las dos cosas le llego.**

La sonda `cpu.inti` funciona porque pregunta `si desborda() no es 1001` y nada
mas de ese programa devuelve 1001. **En un programa de verdad eso es una mentira
silenciosa** -- justo lo que este proyecto existe para eliminar, y con la firma
detras dandole la razon.

> **Un corte que deja al programa corriendo con un numero que parece bueno no es
> un corte.** El samurai no susurra.

El propio emisor lo dice: *"cuando haya errores como datos de verdad, esto
construira el valor de error"*. **Es el peldano P4 de la seccion 5, y es el mas
importante de todo el documento.**

### 2.4 LA KATANA QUE EL SILICIO YA BLANDE Y NADIE SABE QUE EXISTE

Medido hoy. Este fuente:

```text
perfil llano

funcion peligro devuelve entero64
    cambiante a es entero64 = -9223372036854775808
    cambiante b es entero64 = -1
    devuelve a entre b
```

```text
   reglas pedidas          1
   reglas emitidas         1
   ok: 904 bytes -> minmenos1.bex
```

**UNA regla: la del divisor cero.** Pero `-2^63 / -1` no cabe en 64 bits, y el
emisor emite `cqo; idiv` -- que ante eso levanta **`#DE`, el mismo vector que
dividir entre cero**.

O sea: el programa compila limpio, pasa el gate, sale firmado, y en el Ryzen
**muere con una autopsia del kernel** en vez de atrapar con `E1001`. No es
comportamiento indefinido --la muerte esta definida-- pero **tampoco es lo que
`REGLAS.md` promete**, y la distancia entre esas dos cosas es la que este
proyecto no se puede permitir.

Y fijate en la forma del fallo, porque es el espejo de todo lo demas: en todos
los otros sitios INTI comprueba en software lo que el silicio ya sabia. **Aqui
el silicio corta y era INTI quien no sabia que eso era una regla.**

> **Sonda que falta: `r01b_cociente`.** Criterio de aprobado: `-2^63 entre -1`
> devuelve `E1001`, y no una autopsia.

---

## 3. POR QUE ESTO ES UNA PREGUNTA DE PERFIL, Y NO DE OPTIMIZACION

La bisagra ya estaba escrita, en el mensaje de error de `llano`:

> *"Lo que crece pide memoria, y `llano` no tiene monton: **por eso puede
> escribir un manejador de interrupciones**."*

**Codigo `llano` puede SER el manejador.** Y entonces no puede depender de un
manejador para hacer cumplir sus propias reglas, porque eso es circular:

```text
   un #DE dentro del manejador de #DE   -> #DF
   un #DF dentro del manejador de #DF   -> triple falta, la maquina reinicia
```

De ahi salen los dos dialectos, y **no son una preferencia: son un teorema**.

| | `llano` | `pleno` |
|---|---|---|
| quien lleva la katana | **el propio programa** | **el silicio** |
| como | comprobacion en linea | trampa + mesa de aterrizaje |
| coste normal | 1 a 6 instrucciones | **0** |
| donde vive | Ring 0, manejadores, drivers | Ring 3, con runtime |
| por que asi | **puede ser el que atiende la trampa** | tiene a quien volver |
| si el sistema cae | sigue cortando | tambien cae |

Y esto es lo que le faltaba al perfil para ser lo que el dueno pedia. Hasta hoy
`perfil` contestaba *"que puedo ESCRIBIR"*. A partir de aqui contesta **"quien
ejecuta mis reglas"**, que es una afirmacion sobre el binario y no sobre el
fuente -- y por eso tiene que viajar dentro del `.bex`.

---

## 4. EL REPERTORIO DEL SILICIO

Lo que este procesador ofrece, lo que ya se le pide, y lo que **no se le puede
pedir** -- dicho por delante para que nadie lo proponga.

### 4.1 Lo que se puede pedir

| senal | vector | para que regla | como se enciende | estado |
|---|---|---|---|---|
| `of` / `cf` | -- | **1** desborde | ya viene puesta | **en uso** |
| `#DE` | 0 | **3** entre cero, y el cociente de 2.4 | ya viene puesta | falta P4 |
| `#XM` | 19 | **12** conversion | desenmascarar `IE` en `MXCSR` (Ring 3, `ldmxcsr`) | falta P4 |
| `#AC` | 17 | una regla que **INTI no tiene**: acceso desalineado | `EFLAGS.AC` (Ring 3) + `CR0.AM` (Ring 0) | propuesta |
| `#UD` (`ud2`) | 6 | cualquiera: **un corte que no se puede confundir con un valor** | emitir dos bytes | ver P4 |
| pila en sombra (CET) | -- | integridad de la direccion de retorno | por preguntar | ver 4.3 |

### 4.2 Lo que NO se puede pedir, y hay que decirlo

```text
   into    "interrumpe si desbordo" -- INVALIDA en modo 64 bits
   bound   comprobar un indice contra un par de limites -- INVALIDA en 64 bits
   MPX     los registros de limites de Intel -- retirados del silicio
```

Las tres son exactamente lo que este plan querria y **las tres estan muertas**.
Escribirlo aqui vale mas que descubrirlo dentro de tres meses: la **Regla 2
(indice) no va a tener katana de hardware**, y por eso nace en software con
`lista de T` y ahi se queda.

### 4.3 Y una que hay que PREGUNTARLE a la sonda

La pila en sombra de CET valdria para una regla que no existe todavia --*"nadie
ha pisado mi direccion de retorno"*-- y **no sabemos si este Ryzen la tiene**,
porque la sonda hoy trae **un solo registro de los cuatro** de la hoja de
identificacion, y esa bandera vive en otro.

> **Dependencia concreta:** completar `que_cpu_eres` para que devuelva los
> cuatro registros. Esta escrito en la tabla de intrinsecos con su motivo. Sin
> eso, la mitad del repertorio de este procesador no se puede ni preguntar.

---

## 5. EL PLAN, PELDANO A PELDANO

> La ley de EL FUERO: **una regla solo existe si trae su componente y su
> numero.** Cada peldano de abajo tiene criterio de aprobado ejecutable. Un
> peldano sin criterio es una intencion, y las intenciones van en el maestro.

### P0 -- LA COSTURA. HECHO (2026-08-22)

Cada trozo del modulo fusionado se lleva escrito de que fichero salio y con que
perfil venia. `arbol::Pieza`, `Modulo::pieza_de`, `Aviso::con_pieza`.

**Por que va primero:** no se puede declarar honradamente el perfil de un
binario sin saber de que esta hecho. Y arreglaba un fallo vivo: un `texto` en la
linea 3 de una pieza salia como *"en tu_fichero.inti, linea 3"*, que puede estar
en blanco.

```text
   aprobado:  el aviso nombra la pieza y quien la trajo, y un fallo propio
              sigue acusando al fichero propio.  4 pruebas, verdes.
```

### P1 -- EL PERFIL VIAJA DENTRO DEL `.bex`. HECHO (2026-08-22)

Hasta hoy `empaquetar()` escribia **una** seccion: `Code`. El perfil, los bloques
`crudo` y las arquitecturas salen por la consola con `-i` y **se mueren ahi**. El
sitio ya existe y esta vacio: **`SectionKind::Manifest = 0x09`**, con su escritor
`BefSection::manifest_toml()` y su validador.

Y de paso deja de ser mentira un comentario que hoy afirma algo que no existe:
`perfil/mod.rs` dice *"va al informe del `.bex` para que `bmo-verify` pueda
exigirlo firmado"*. **No va.** `bmo-verify` no tiene ni la palabra.

```text
   aprobado:  leer un `.bex` de INTI sin ver el fuente y saber su perfil, sus
              bloques `crudo` y sus piezas.  Y `bmo-verify` puede exigirlo.

   ✅ las dos mitades.  15 pruebas nuevas.
```

**Como quedo:**

```text
   frontend   `manifiesto/`     escribe el TOML y lo vuelve a leer
   emisor     `empaquetar()`    lo recibe HECHO y lo mete en Manifest 0x09
                                -- no sabe que es un perfil, y no tiene por que
   escritor   `BefBuilder`      enciende `HAS_MANIFEST` SOLO, al ver la seccion
   gate       `exige_manifiesto`  y el compilador de INTI se lo exige a SI MISMO
```

★★★ **La ultima linea es la que impide que esto se deshaga.** Si manana alguien
rompe el cableado, el compilador **se niega a escribir el fichero** en vez de
sacar un `.bex` correcto por dentro y mudo por fuera. No se anadio una
comprobacion nueva: se uso el gate que ya se llamaba, con la politica puesta.

⚠ Y no se metio en `verify()`: exigir el manifiesto a todo el mundo rechazaria
hoy cada `.bex` de BMO C, COBOL y Ada. Eso no seria un gate mas estricto, seria
el toolchain dejando de compilar. **Es una politica que el productor elige, y
INTI es el primero que la elige sobre si mismo.**

#### P1 -- LA PREDICCION, escrita ANTES de construirlo

> Eddi: *"el samurai, aunque este preparado para la guerra, tiene que estar
> fortalecido: siempre predice TODO, y la CPU es el motivo que predice."*
>
> Escrita el 2026-08-22 antes de la primera linea de codigo. Una prediccion que
> se escribe despues de ver el resultado no vale nada.

| lo que se pregunta | prediccion | si sale otra cosa | salio |
|---|---|---|---|
| **el codigo emitido** | **identico byte a byte.** El manifiesto no toca `Code` | algo del manifiesto se colo en la emision | ✅ **acertada.** Byte a byte, y el `.bex` sin manifiesto sigue midiendo **8.752** exactos |
| **el tamano de `cpu.bex`** | crece de **8.752** a algo entre 9.000 y 9.800: el TOML son ~400 bytes, mas una entrada de tabla, mas el relleno hasta la frontera de sector | no crece = no se escribio; crece mucho mas = el TOML lleva algo que no toca | ✅ **acertada. 9.183** (predije 9.000-9.800) |
| **el gate** | **sigue diciendo que si.** La seccion `Manifest` ya tiene validador (UTF-8 no vacio) desde antes de este trabajo | rechaza = el header miente sobre si mismo, y hay que mirar `validate_flag_coherence` | ✅ **acertada** |
| **el aviso de coherencia** | **no sale.** `HAS_MANIFEST` se pone SOLA al anadir la seccion | *"hay seccion manifest y el header no lo anuncia"* = se olvido, y se va a olvidar siempre | ✅ **acertada**, y mejor de lo previsto: la bandera la pone ahora `BefBuilder::build()` |
| **la frontera de sector** | **se conserva.** Lo que se carga tiene que empezar en multiplo de 512 o el disco no puede escribir en los marcos del proceso | si se rompe, el `.bex` no se carga desde disco y el sintoma aparece lejos de aqui | ✅ **acertada.** `Code` en el 512 |
| **la carga en el Ryzen** | **no cambia nada.** El cargador solo mapea `Code`, `RoData`, `Data` y `Bss`: `Manifest` es inerte | no arranca = la seccion no era inerte | ⏳ **ABIERTA.** No se puede cerrar en esta maquina |
| **leer el perfil sin el fuente** | `perfil = "llano"`, `crudo = 1`, `arquitecturas = ["x86_64"]`, y **las piezas del monton con SU perfil declarado** | ❌ **FALLO EN EL NUMERO.** Perfil, arquitecturas y piezas acertados; `crudo` NO es 1: es **10** |

⚠ **La fila de la carga en el Ryzen no se puede cerrar aqui.** Se puede
argumentar leyendo `ram.rs` --y se hizo-- pero **argumentar no es medir**. Queda
abierta hasta el proximo arranque, y va a la lista de pendientes de hardware.

El argumento, para que se pueda juzgar: el cargador solo mapea `Code`, `RoData`,
`Data` y `Bss`, y el header lo dice con todas las letras --*"un bit desconocido
se RECHAZA, al reves que una seccion desconocida: **una seccion que no entiendo
es data inerte**"*--. Es un argumento bueno. Sigue sin ser una medida.

#### P1 -- LO QUE SALIO (2026-08-22)

**Cinco acertadas, una FALLADA y una abierta.**

*** **La fallada, dicha entera: predije `crudo = 1` para la sonda y son 10.**

```text
   [modulo]
   perfil = "llano"

   [metal]
   crudo = 10
   arquitecturas = ["x86_64"]

   [[pieza]] monton/origen.inti    usa = "monton"   perfil = "llano"
   [[pieza]] monton/reparto.inti   usa = "monton"   perfil = "llano"
```

Siete son de la propia sonda --`cuantas_hojas`, `firma_del_cpu`, `una_medida`,
`estado_extendido`, `azar_dos_veces`, `prueba_bits`, `prueba_atomicas`-- y tres
vienen dentro de las piezas del monton.

★★ **Y el fallo vale mas que el acierto**, porque es exactamente lo que este
peldano existe para arreglar: yo, que acababa de leer ese fichero entero, dije
un numero equivocado sobre el. **El medidor no dice cuantas ventanas abriste:
dice cuantas trae el binario.** Nadie lo sabia de memoria porque hasta hoy no se
podia leer sin abrir el fuente -- que es la frase con la que empieza este
peldano.

★ Y una cosa que no se buscaba: el `.bex` **ya trae una seccion
`Requisitos = 0x15`** de 93 bytes, escrita sola por el constructor. Es el sitio
que P4 necesita para la mesa de aterrizaje, y **ya esta ahi**.

---

### P2 -- LA REGLA DEL MEZCLADO

Con costuras, la regla se puede **decir**:

> **El perfil de un binario es el MAS ESTRICTO de los que lo componen**, y una
> pieza que se declara mas laxa que quien la trae es una **decision**, no un
> silencio.

Hoy es silencio: medido el 22-08, un fichero `llano` que trae una pieza `pleno`
sale como un `.bex` firmado de 880 bytes sin una palabra. Lo que se compila **si**
se juzga contra el estricto --la garantia aguanta-- pero la contradiccion no se
dice.

```text
   aprobado:  una pieza mas laxa que quien la trae da su codigo con las cuatro
              partes, y el manifiesto declara el perfil RESULTANTE, no el
              declarado.
```

### P3 -- INTI ENTRA EN `FrontendKind`

`bmo_abi::profile::FrontendKind` lista `Bmo, C, Cpp, Rust, JavaBmo, PythonBmo,
Ada, Cobol, Custom`. **INTI no esta.** `ALL_PROFILES` tiene cinco perfiles y
ninguno es el suyo. Un `.bex` de INTI llega al kernel indistinguible de
cualquier otra cosa.

Aqui es donde `llano` empieza a significar algo **fuera del compilador**: *"esto
puede correr en Ring 0 / dentro de un manejador"* pasa de ser un comentario a
algo comprobable **al cargar**.

```text
   aprobado:  el cargador distingue un `.bex` de INTI `llano` de uno `pleno`,
              y `ring0_capable` sale de un dato y no de una suposicion.
```

### P4 -- EL CAMINO DE VUELTA: atrapar deja de ser devolver un numero

**El peldano que sostiene todo lo demas.** Tres mitades, y la tercera es la que
importa:

**(a) La mesa de aterrizaje.** El binario declara, por regla, **su codigo y la
direccion de su bloque de trampa**. Y eso ya existe en los bytes: `emitir_funcion`
emite hoy un bloque `atrapa:` por codigo, al final de cada funcion. **No hay que
inventar el mecanismo: hay que declarar donde esta.**

La forma de la tabla ya esta elegida por el formato: `SectionKind::Requisitos =
0x15`, cuyo motivo escrito es exactamente este --*"una deduccion en Ring 0 es un
cerebro donde tendria que haber un contrato"*.

**(b) Que el kernel aterrice en vez de enterrar.** Ante un `#DE` o un `#XM` de
una tarea que trae mesa, el kernel **no hace autopsia**: pone el codigo en el
registro de retorno, mueve el `rip` al bloque declarado, y sigue. Es lo mismo
que hace hoy el salto del compilador, ejecutado desde el marco de la excepcion.

**(c) Y el corte deja de susurrar.** Un codigo devuelto es ambiguo (2.3). La
salida honrada es la que ya usa el lenguaje para lo demas: **el error como
dato**, y donde eso no quepa --`llano` sin sitio donde construirlo-- un `ud2`,
que es un corte que **no se puede confundir con un valor**.

Una arruga concreta que hay que resolver aqui: **`#DE` sirve a dos reglas a la
vez** -- divisor cero (E1003) y cociente que no cabe (E1001, la de 2.4). El
vector es el mismo; distinguirlas pide mirar el operando desde el marco de la
excepcion, o dejar una de las dos en software.

```text
   aprobado:  un programa `pleno` divide entre cero, NO muere, y quien llamo
              recibe `E1003` como un dato que no se puede confundir con un
              resultado.  Y `-2^63 entre -1` da `E1001`.
```

### P5 -- LA KATANA DEL SILICIO

Con P4 puesto, en `pleno`:

```text
   quitar   `carga` + `test` + `jz` delante de cada division      -3 instr.
   quitar   el centinela y el `comisd` de la conversion           -3 a -6 instr.
   poner    `ldmxcsr` con `IE` desenmascarada al arrancar la tarea   1 vez
```

En `llano` **no se toca nada**, y esa es la mitad importante del peldano.

```text
   aprobado:  el mismo fuente compilado en los dos perfiles da el mismo
              resultado en el Ryzen, y el de `pleno` no lleva las
              comprobaciones en sus bytes.
```

### P6 -- LA MEDIDA, contra el umbral que ya existe

Sin esto el peldano 5 es una opinion. El instrumento ya esta calibrado y el
umbral ya esta puesto: **una mejora tiene que mover el minimo mas de ~1%** (mejor
de ocho, con la dispersion al lado; establecido el 22-08 en el Ryzen).

Y la prediccion honrada, escrita por delante: **puede que no se note.** Las
reglas cuestan ~1% entero y este trabajo se lleva una parte de ese 1%. La razon
para hacerlo **no es la velocidad**: es 2.3, que atrapar deje de ser ambiguo. Si
al final el numero no se mueve, el peldano sigue valiendo y el numero se publica
igual.

```text
   aprobado:  la linea `reglas` sigue en CERO y el minimo se publica con su
              dispersion, se haya movido o no.
```

### El orden, y por que es ese

```text
   P0 costura ----> P1 el perfil viaja ----> P2 la regla del mezclado
                            |
                            +----> P3 FrontendKind
                            |
                            +----> P4 el camino de vuelta
                                          |
                                          v
                                   P5 la katana ----> P6 la medida
```

**P1 es el cuello de botella de todo.** P3, P4 y P5 necesitan que el binario
declare algo, y hoy el binario no declara nada.

### Y el reloj que corre por debajo

Hoy `armar` **pega fuentes**: el perfil del conjunto se sabe compilando. El dia
que exista **compilacion separada** --que ya es bloqueante para Python-- un
programa `llano` va a enlazar un objeto `pleno` **y no habra nadie mirando**.

Hacer P1 y P2 **antes** de que llegue el enlazado es la diferencia entre un
contrato y una excavacion.

---

## 6. C, Y QUE SITIO LE TOCA

La pregunta del dueno, contestada sin adornos.

**Lo que NO se sostiene:** C como *el lenguaje de sistema* de BMO-X. En una
maquina sola, con un toolchain propio y todas las fuentes a mano, el
comportamiento indefinido no compra nada y cuesta todo. Y hay una prueba de
casa: los **dos huecos del emulador** que INTI destapo este mes --`imul` sin
banderas, `cvttsd2si` saturando-- llevaban ahi sin que nadie los notara **porque
BMO C no emite un `jo`**. No habia quien preguntara.

**Lo que SI se sostiene:** C como *lenguaje de compatibilidad*. Existe, compila,
esta probado y sirve para lo unico que ningun lenguaje propio puede dar: **correr
el codigo C que ya existe en el mundo.** Eso no es poco y no se tira.

El movimiento no es *quitar C*: es **dejar de escribir sistema nuevo en C**. Y
hay una deuda concreta que sale de aqui, porque es de la misma familia:

> `bmo-c-front` compilo durante meses contra un emulador que mentia en dos
> instrucciones. **Nadie ha vuelto a mirar que le paso a lo que se compilo
> entonces.**

---

## 7. LO QUE ESTE PLAN NO PUEDE DAR, DICHO POR DELANTE

1. **La Regla 2 no tendra katana de hardware.** `bound` y MPX estan muertos
   (4.2). Nace en software con `lista de T` y ahi se queda.

2. **En `llano` no se ahorra ni una instruccion**, y no es una limitacion que se
   arreglara luego: es la definicion del perfil (seccion 3). Codigo que puede ser
   el manejador no puede delegar en el manejador.

3. **Puede que el peldano 6 no mida nada.** Esta escrito arriba y se acepta antes
   de empezar, para que el resultado no se pueda reinterpretar despues.

4. **Nada de esto arregla el `#DE` de 2.4 por si solo.** Ese es un agujero de HOY,
   en `llano`, que se tapa con una comprobacion en software y una sonda -- no hace
   falta esperar a P4 para eso, y no se debe.
