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
    monton + 0    libre    la primera direccion sin repartir
    monton + 8    fin      la primera direccion que ya NO es del monton
    monton + 16   ...      desde aqui se reparte
```

Tres lineas. **Todo lo que una pieza sabe de la otra esta aqui**, y por eso una
pieza se puede cambiar de golpe sin mirar la de al lado.

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
| `pide(monton, cuantos)` | reserva, alineado a 16. **0 si no cabe** |
| `queda_en(monton)` | cuanto queda sin repartir |
| `suelta(monton, trozo)` | hoy no hace nada -- ver abajo |

---

## 3. ** LO QUE `suelta` NO HACE, dicho antes de que alguien lo suponga

Este reparto es **de avance**: el cursor solo sube. `suelta` existe, se puede
llamar, y **no devuelve nada al monton**.

No esta escondido en un TODO. Esta aqui, en la tabla, y en el propio fichero.

Y existe desde el primer dia a proposito: un `suelta` que aparece mas tarde
obliga a repasar todo lo escrito antes; uno que ya esta y no hace nada, no.

**Donde entra el que si suelte:** en `reparto.inti`, y en ningun otro sitio. Una
lista de huecos cambia ese fichero entero y **no toca `origen.inti` ni un solo
programa que ya use `pide`**. Esa frase es toda la razon de partirlo en dos.

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

1. **`suelta` de verdad** -- lista de huecos. Solo `reparto.inti`.
2. **Que `pleno` lo use solo**: hoy un programa escribe `monton_nuevo`; un
   `texto + texto` todavia no sabe pedir memoria a nadie.
3. **Compilacion separada**, y con ella el runtime prestado.
4. **Un monton por tarea**, cuando haya tareas de verdad.
