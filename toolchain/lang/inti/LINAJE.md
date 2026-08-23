# EL LINAJE DE INTI -- abuelo, padre, hijo, nieto

> Peticion de Eddi, 2026-08-19: *"vamos a hacer la jerarquia de abuelo, padre,
> hijo, nieto, cada uno modular y por que... INTI si o si tiene que tener piezas
> MUY indestructibles. Necesito saber de que materiales es capaz por si sola,
> para no cambiar la pieza sino cambiarla de GOLPE cuando un dia falle."*

---

## 0. Que quiere decir "indestructible" aqui

No lo que suena. Una pieza que no se rompe nunca no existe, y prometerlo seria
mentir.

> ★★★ **Una pieza es indestructible cuando romperla no rompe nada mas.**

Esa es la definicion operativa, y tiene la ventaja de que **se puede medir**:

```text
   cuantos ficheros hay que tocar para sustituirla?
      uno       -> es modular
      cinco     -> no lo es, aunque el documento diga que si
```

Y por eso este documento **no viaja solo**: `tests/linaje.rs` mide las flechas y
falla si alguien las cruza. Un documento no impide un `use`.

### Las cuatro propiedades que hacen sustituible una pieza

| # | propiedad | como se comprueba |
|---|---|---|
| 1 | **Se prueba sola**, sin montar el resto | sus tests no construyen medio compilador |
| 2 | **No nombra a nadie de su generacion ni de las siguientes** | `linaje.rs`, test 1 |
| 3 | **Tiene un contrato escrito que sobrevive a su codigo** | hay un `.md` que se puede leer sin abrir el `.rs` |
| 4 | **Se puede tirar entera y reescribir** | `linaje.rs`, test 4: cuantos la miran |

---

## 1. Las cuatro generaciones

```text
   ABUELO     los CONTRATOS y las TABLAS          no son codigo
      |
   PADRE      el frontend                         no nombra ninguna maquina
      |
   HIJO       el emisor de UNA maquina            la nombra en cada linea
      |
   NIETO      los programas y sus .bex            lo usan todo
```

★ La regla que las une: **cada generacion se puede sustituir sin tocar las de
arriba**. Y las de abajo se enteran, pero no se rompen -- porque lo que heredan
es un contrato, no una implementacion.

---

## 2. ABUELO -- los contratos y las tablas

**De que esta hecho:** texto. Ni una linea de codigo.

| pieza | que fija | si se cambia |
|---|---|---|
| `GRAMATICA.md` | la sintaxis y la EBNF | hay que rehacer `sintaxis` |
| `REGLAS.md` | las doce reglas anti-UB | hay que rehacer `ir` y el emisor |
| `CENSO.md` + `censo/*.inti` | el corpus con su veredicto | **nada se rompe: se ven las diferencias** |
| `palabras.toml` | las 49 palabras, en 3 idiomas | **nada**: se relee al arrancar |
| `comun.toml` | la biblioteca que esta sin pedirla | **nada** |
| `modulos.toml` | lo que trae cada `usa` | **nada** |
| `arch/x86_64/inti.toml` | los nombres y el perfil de la maquina | **nada**: otra carpeta es otra maquina |

★★ **Por que el abuelo no es codigo, y es lo que lo hace indestructible:** una
tabla no se puede romper de la forma en que se rompe un programa. Se puede
equivocar --y entonces el compilador dice algo raro-- pero **no arrastra a
nadie**, porque quien la lee ya sabia que podia no estar.

Y la prueba de que esto no es teoria: **cuatro peticiones tuyas se resolvieron
sin tocar el compilador** -- el ingles, el modo Python, `usa x86_64` y la
biblioteca comun. Todas fueron filas de una tabla.

### ⚠ El unico abuelo delicado

`GRAMATICA.md`. Cambiarlo **si** obliga a rehacer cosas, y por eso F0 se escribio
antes que una sola linea de codigo. Es la pieza que no se puede cambiar de
golpe, y la unica.

---

## 3. PADRE -- el frontend, y sus propias generaciones

Vive en `src/`. **No nombra ninguna maquina** y hay un test que lo vigila.

Por dentro tiene cinco generaciones, y `tests/linaje.rs` comprueba que ninguna
mira hacia arriba **ni hacia los lados**:

```text
   gen 0   aviso        palabras          <- no miran a nadie
   gen 1   lexico       arquitectura      cabina
   gen 2   arbol
   gen 3   sintaxis
   gen 4   perfil       nombres      ir   <- y NO se miran entre ellos
```

★★ **Y `tablas` se llevo su SEGUNDA inquilina el 2026-08-23**, por el mismo test
y con la misma frase. `disposicion` (gen 3) necesito saber si un tipo **crece**
--para poder decir que un campo de `texto` mide una referencia-- y la lista
vivia dentro de `perfil`, que es su hermano MAYOR:

```text
   disposicion (gen 3) mira a perfil (gen 4) en mod.rs
```

El `Catalogo` entero se mudo a `tablas`. Y lo que **no** se mudo es la linea que
importa: el recorrido que decide a quien acusar sigue en `perfil`. **Aqui va el
dato; alli, la decision.**

> La regla no cambio, se aplico: *una tabla vive en la generacion mas baja que
> la necesita*. La primera vez fue `Modulos`. **La segunda vez es la que dice si
> una regla era una regla o una excusa.**

★ De paso salio gratis lo que siempre sale al mudar algo: `perfil` metia la mano
en cuatro campos privados del catalogo. Ahora pregunta --`crece`, `cuesta`,
`sin_medida`, `llega_a_bytes`--, que es una API en vez de una intimidad.

### Pieza por pieza: de que es capaz sola

| pieza | gen | de que esta hecha | que aguanta sola | si falla |
|---|---|---|---|---|
| `aviso` | 0 | cuatro campos y un formato | **todo**: sus tests no compilan una linea de INTI | los mensajes salen feos. Nada deja de compilar |
| `palabras` | 0 | un TOML y un `HashMap` | **todo** | el compilador no arranca. Es el unico con respaldo incrustado, y por eso |
| `lexico` | 1 | un bucle sobre caracteres y una pila de margenes | **casi todo**: entra texto, salen piezas | no hay piezas. Se cambia entero sin tocar la gramatica |
| `arquitectura` | 1 | un TOML por maquina | **todo** | `usa x86_64` no encuentra nada, **y eso es una respuesta correcta** |
| `cabina` | 1 | una traduccion de avisos y numeros a eventos | **todo** | el sistema deja de enterarse. **Todo lo demas compila igual** |
| `arbol` | 2 | datos puros, cero decisiones | **todo**: no tiene logica que falle | nada: si el arbol esta mal, esta mal quien lo construyo |
| `sintaxis` | 3 | descenso recursivo y una tabla de precedencia | **casi todo** | no hay arbol. Se reescribe leyendo solo `GRAMATICA.md` |
| `disposicion` | 3 | dos tablas leidas y una suma de desplazamientos | **todo** | no hay plano: `p.x` y `a[i]` dejan de resolverse. La gramatica sigue en pie |
| `tablas` | 1 | TOML y `HashMap`. **Cero decisiones** | **todo**: no compila una linea de INTI | el compilador no sabe que trae `usa`, ni que crece. Con respaldo incrustado |
| `perfil` | 4 | **un recorrido**, y nada mas: sus listas se mudaron a `tablas` el 23-08 | **todo** | `llano` deja de vigilarse. **El resto compila igual** |
| `nombres` | 4 | una pila de ambitos | **todo** | no se avisa de nombres. El resto compila igual |
| `ir` | 4 | un descenso a instrucciones | **todo** | no hay bytes. La gramatica sigue en pie |

★★ Fijate en la ultima columna de los tres analisis: **si uno falla, los otros
dos siguen**. Eso no salio solo -- es lo que el test `los_tres_analisis_no_se_miran_entre_ellos`
impide que se pierda. El caso que mas facil se cuela es que `nombres` necesite
algo que `perfil` ya calculo y le llame: parece gratis y no lo es, porque el dia
que uno se reescriba, el otro se va con el.

---

## 4. HIJO -- el emisor de una maquina

`emisor-x86_64/`, **un crate aparte**. Y no por orden: porque el test de
agnosticismo prohibe en el padre lo que aqui se hace en cada linea.

| pieza | de que esta hecha | si falla |
|---|---|---|
| `marco` | un reparto de ranuras, el ancho de palabra **y el asignador de registros** | los valores caen mal. ★★ **Y fue el UNICO fichero que cambio el dia de los registros -- la promesa se cumplio** |
| `lib` (la seleccion) | de `Instr` a bytes, con `bmo_lower::x86` | no hay `.bex`. El frontend entero sigue en pie |

★★★ **Esta es la generacion mas sustituible de todas, y a proposito.** El dia de
ARM se escribe `emisor-aarch64/` al lado y **no se toca ni una linea del
frontend**. Es la mitad B de la portabilidad convertida en dos carpetas.

★★★ **Y la promesa se cobro el mismo dia.** F3 --los temporales en registros--
cambio `marco.rs` **y tres lineas de `lib.rs`**. Nada del frontend, nada de la
IR, nada de los contratos. Se pudo porque la IR ya traia los temporales, que es
lo unico que un asignador necesita.

Es la primera vez que este linaje se pone a prueba de verdad, y aguanto.

---

## 5. NIETO -- los programas

Los `.inti` y sus `.bex`. Dependen de todo lo anterior y **no los mira nadie**,
que es la posicion mas comoda del arbol: un programa puede fallar sin llevarse
nada por delante.

★ Y hay una excepcion que merece la pena: `censo/*.inti` son nietos que **miden
a sus abuelos**. Un programa de treinta lineas que caza un desacuerdo entre la
gramatica y el compilador -- han sido siete en tres dias.

---

## 6. Como se cambia una pieza de golpe

El procedimiento, para cuando llegue el dia:

1. **Lee su contrato**, no su codigo. Si no hay contrato, la pieza no era
   sustituible y el trabajo empieza escribiendolo.
2. **Corre `cargo test` y anota el numero.** Ese es el liston.
3. **Borra el modulo entero** y escribe uno nuevo con la misma superficie
   publica.
4. **Corre los tests otra vez.** Si vuelve el mismo numero, la pieza estaba bien
   cortada. Si aparecen fallos en OTROS modulos, no lo estaba -- y eso es un
   dato sobre el corte, no sobre la pieza nueva.
5. Y si el paso 4 falla, **`linaje.rs` dice por donde**: alguien la habia
   agarrado por dentro.

★★ El paso 3 es el que hace falta poder hacer sin miedo, y es la razon de las
cuatro propiedades de la seccion 0. Todo lo demas de este documento existe para
que ese borrado sea posible.

---

## 7. Lo que NO es modular, y hay que saberlo

Honestidad antes que casillas verdes:

| pieza | por que no se puede cambiar de golpe |
|---|---|
| `GRAMATICA.md` | cambiar la sintaxis obliga a rehacer `sintaxis`, y a reescribir todo programa que exista |
| El formato de `Aviso` | lo usan todos: cambiar los cuatro campos toca cada mensaje del compilador. **Es a proposito** -- un contrato compartido es lo contrario de un acoplamiento |
| El ABI de BEF | no es de INTI: es del sistema, y lo comparten cinco lenguajes |

Y una que **parece** modular y no lo es del todo: la IR. Cambiar una
instruccion toca `ir` **y** el emisor, porque el emisor la traduce. Son dos
ficheros, no uno -- sigue siendo barato, pero no es de golpe.

---

Ver [`ARQUITECTURA.md`](ARQUITECTURA.md) (por que el compilador esta partido
asi), [`GRAMATICA.md`](GRAMATICA.md) (el contrato del abuelo) y
`tests/linaje.rs` (la ley, ejecutandose).
