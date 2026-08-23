# EL MONTON de INTI PLENO

> Peticion de Eddi, 2026-08-20: *"si MONTON es monolitico = modular, para poder
> evitar problemas o choques. INTI como siempre modular."*

Este documento es **el abuelo** de la jerarquia: lo que las piezas prometen. Las
piezas se pueden tirar y reescribir; esto no, y por eso esta escrito aparte.

---

## 0. La correccion que hay que leer primero

Al cerrar F4b dije que el monton estaba bloqueado por **las variables de
modulo** (globales), porque un asignador necesita estado que sobreviva a la
funcion.

**Eso era verdad de un monton MONOLITICO, y solo de ese.** El `malloc` de C
guarda su cursor en una global escondida, y por eso necesita globales.

Uno modular no las necesita:

```text
    ** EL ESTADO DEL MONTON VIVE DENTRO DEL MONTON
```

Y no es un apano para esquivar una funcionalidad que falta -- **es mejor**. Un
`malloc` con estado global es *autoridad ambiente*: cualquiera reparte de lo
mismo sin haberlo pedido, y dos partes de un programa se pisan sin haberse visto
nunca. `pide(monton, n)` tiene la forma de una capability: **para repartir de un
monton hay que tenerlo en la mano**.

Que es exactamente como funciona todo lo demas en BMO-X.

---

## 1. LA DISPOSICION -- la unica frontera entre las piezas

```text
    monton + 0    libre     la primera direccion sin repartir
    monton + 8    fin       la primera direccion que ya NO es del monton
    monton + 16   huecos    la cabeza de la lista de huecos, 0 = ninguno
    monton + 24   ...       reservado, para que el reparto empiece alineado a 16
    monton + 32   ...       desde aqui se reparte
```

Y **cada trozo repartido lleva su medida delante** (2026-08-23):

```text
    trozo - 16    medida      bytes del payload, siempre multiplo de 16
    trozo -  8    siguiente   solo mientras esta suelto. 0 si esta en uso
    trozo         el payload, alineado a 16
```

**Todo lo que una pieza sabe de la otra esta aqui**, y por eso una pieza se
puede cambiar de golpe sin mirar la de al lado.

★ **Lo que cuesta, dicho con el numero delante: 16 bytes por trozo.** Lo que
compra es que `suelta` se pueda escribir -- hasta hoy recibia una direccion y
nada mas, y una direccion sola no dice cuanto devolver.

⚠ **La alternativa era `suelta(monton, trozo, cuantos)`**, que sale gratis en
memoria. Se descarto por lo que cuesta en la otra moneda: **un numero equivocado
ahi no falla, corrompe el monton en silencio**. Una cabecera no se puede
mentir.

---

## 2. LAS PIEZAS

```text
    abuelo    ESTE DOCUMENTO           la disposicion y las promesas
    padre     runtime/monton/origen    habla con el kernel, NO sabe repartir
    hijo      runtime/monton/reparto   sabe repartir, NO habla con el kernel
    nieto     los programas            piden y usan
```

### `origen.inti` -- de donde viene la memoria

`monton_nuevo(cuantos)` cruza la puerta dos veces --pedir el bloque, preguntar
por su base--, escribe la cabecera, y devuelve la direccion del monton.

**Devuelve 0 si el kernel dice que no.** No inventa un monton mas pequeno ni
reintenta: quien pide 4 KiB y recibe 0 tiene que enterarse ahi, y no dos
funciones mas adelante.

### `reparto.inti` -- como se parte

| operacion | que hace |
|---|---|
| `pide(monton, cuantos)` | reserva, alineado a 16. **Mira los huecos ANTES de avanzar**. 0 si no cabe |
| `queda_en(monton)` | cuanto queda sin repartir, contando solo el cursor |
| `queda_suelto(monton)` | cuantos bytes hay sueltos y reutilizables |
| `suelta(monton, trozo)` | ✅ **SUELTA DE VERDAD**. Devuelve los bytes que vuelven |

---

## 3. ✅ `suelta` YA SUELTA (2026-08-23) -- y lo que ESTA seccion decia antes

Esta seccion se llamaba *"LO QUE `suelta` NO HACE, dicho antes de que alguien lo
suponga"* y decia:

> Este reparto es **de avance**: el cursor solo sube. `suelta` existe, se puede
> llamar, y **no devuelve nada al monton**.

Ya no. Un trozo soltado entra en una lista de huecos y **el siguiente `pide` lo
reutiliza** -- comprobado ejecutando, que es lo unico que lo decide:

```text
    a = pide(m, 100)
    suelta(m, a)
    c = pide(m, 100)        ->   c ES LA MISMA DIRECCION QUE a
```

★ Y el monton se puede **auditar**: `queda_suelto` recorre la lista y suma. Sin
ese numero, *"suelta de verdad"* seria una afirmacion sin forma de comprobarla
desde fuera.

### ⚠ La prediccion que esta seccion hizo, y la mitad que fallo

Decia que la lista de huecos *"cambia `reparto.inti` entero y **no toca
`origen.inti`** ni un solo programa que ya use `pide`"*.

```text
    ningun programa cambia      ✅ se cumplio
    no toca `origen.inti`       ❌ NO se cumplio
```

**Una lista de huecos necesita una CABEZA**, y una cabeza es estado *del monton*,
no del repartidor: tiene que sobrevivir entre llamadas y vivir donde vive el
resto de lo que un monton sabe de si mismo. O sea, en la cabecera -- que la
escribe `origen`.

★★ Se deja escrito porque la frase que fallo era exactamente la clase de frase
que este proyecto se cree: **una prediccion sobre un limite de modulo, escrita
antes de intentarlo.** La mitad buena --que ningun programa cambia-- es la que de
verdad justificaba partir el fichero en dos, y esa aguanto.

### Lo que sigue sin hacer, y hay que decirlo

- **Los huecos no se juntan.** Dos trozos sueltos y contiguos siguen siendo dos
  huecos, asi que pedir uno del doble avanza el cursor aunque el sitio estuviera
  ahi. Es fragmentacion, y se paga.
- **El cursor no baja**, ni siquiera al soltar el ultimo. Solo se podria con el
  ultimo, y una regla que funciona a veces es peor que una que no funciona nunca.

---

## 4. Por que esta escrito en INTI, y en `llano`

Porque `llano` presume de poder escribir el sistema, y la forma de demostrarlo
no es repetirlo:

```text
    ** LA PIEZA QUE HACE POSIBLE `pleno` ESTA ESCRITA EN `llano`
```

Si el monton hubiera que escribirlo en Rust, *"INTI puede escribir el sistema"*
seria publicidad. Y trae dos cosas que no se pueden fingir:

- sus bloques `crudo` **se cuentan**, porque pasan por el mismo analisis de
  perfiles que los de cualquier programa. El sitio donde nadie comprueba por ti
  sale en el informe con un numero;
- vive en `tables/`, asi que **`$BMO_MODS` lo sustituye sin bifurcar el
  compilador**. Cambiar el repartidor de memoria del lenguaje es dejar otro
  fichero delante.

---

## 5. Lo que hoy se paga, y se dice

`usa monton` es **inclusion**, no enlazado: las declaraciones se meten en el
mismo `.bex`. Diez programas que usen el monton llevan diez copias -- que es
literalmente lo que la seccion 13c del maestro le critica a Go.

La respuesta ya esta escrita alli y no es "enlazar mejor": el runtime es codigo
que no cambia, o sea **congelado**, y lo congelado en BMO-X **se presta en vez de
copiarse** (`MEM_OP_OFRECER`). El dia que exista compilacion separada, el segundo
programa que arranque no paga el monton otra vez.

Se hace asi hoy porque la alternativa era tener el monton escrito, probado y
**sin forma de usarlo**, que en este proyecto cuenta como no tenerlo.

Y el otro precio, mas pequeno: una pieza traida se compila con SUS `usa`, asi
que `usa monton` deja a mano los nombres de `memoria`, que el fichero no pidio.
Es una fuga, esta marcada, y se va con lo mismo.

---

## 6. Lo que falta, en orden

1. ~~**`suelta` de verdad**~~ -- ✅ **HECHO (23-08)**, con seis pruebas que
   ejecutan. Lo que queda de ese frente es **juntar huecos contiguos**.
1b. ~~**El contador de referencias**~~ -- ✅ **HECHO (23-08)**, y en un modulo
   APARTE: `usa objetos`, no `usa monton`. El motivo es el numero de la seccion
   5 -- meterlo dentro engordaria a todo programa que solo quiera memoria cruda.
   Cinco pruebas que ejecutan, y una que exige que diga **lo mismo que
   `bmo_abi::dynobj::header`**, porque dos escrituras de la misma regla se
   separan el dia que alguien toca una.
2. **Que `pleno` lo use solo**: hoy un programa escribe `monton_nuevo`; un
   `texto + texto` todavia no sabe pedir memoria a nadie.
3. **Compilacion separada**, y con ella el runtime prestado.
4. **Un monton por tarea**, cuando haya tareas de verdad.
