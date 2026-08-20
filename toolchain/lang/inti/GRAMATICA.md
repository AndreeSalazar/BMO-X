# GRAMATICA DE INTI -- la sintaxis, fijada

> **F0.** Esto es un CONTRATO, no una descripcion de lo que hay: hoy no existe
> ni el lexer. Se escribe primero porque **una decision de sintaxis mal puesta
> se paga en cada programa que se escriba nunca**.
>
> El porque de cada decision esta en `docs/maestro/INTI_MAESTRO.md`. Aqui esta
> **lo que se escribe**, y al lado el numero de la seccion que lo justifica.

Fichero: `.inti`. Codificacion: **UTF-8**; las palabras clave son **ASCII**.

---

## 0. Las diez decisiones que se ven a simple vista

| # | decision | por que |
|---|---|---|
| 1 | **Bloques por indentacion**, sin `:` al final | 9.1: la sintaxis tipo C no bate a palabras clave elegidas al azar |
| 2 | **Sin `;`, sin `{}`, sin parentesis en las condiciones** | menos puntuacion muda |
| 3 | **`=` significa igual en los dos sitios**: `x = 5` asigna, `si x = 5` compara. Sin ambiguedad **porque asignar no es una expresion** | es la solucion de Quorum, el lenguaje disenado con evidencia |
| 4 | **Palabras, no simbolos**, para lo que no se aprende en el colegio: `y`, `o`, `no`, `entre`, `resto` | `&&`, `\|\|`, `!`, `%` no comunican nada a quien empieza |
| 5 | **Simbolos donde SI se aprenden en el colegio**: `+ - * / < > <= >=` | son notacion matematica, no convencion de programador |
| 6 | **`cambiante` para poder cambiar** | 10.7: mata cuatro sorpresas de golpe |
| 7 | **`de` es azucar de llamada de un argumento**: `cuenta de lista` = `cuenta(lista)` | se lee como una frase y no anade gramatica |
| 8 | **Los errores se escriben o no compila**: `o si no` | 10.6 |
| 9 | **`perfil` en la primera linea**: `llano` o `pleno` | 1.4: un solo lenguaje, dos perfiles |
| 10 | **`crudo` es la unica ventana sin comprobar, y se ve** | 6.6 |

---

## 1. El esqueleto de un fichero

```text
perfil pleno              # obligatorio, primera linea util
usa entrada               # lo que se importa, uno por linea
usa superficie

# ---- de aqui abajo, declaraciones de nivel superior ----

MAXIMO = 100              # constante: se CONGELA al cargar el modulo

registro Punto
   x es numero
   y es numero

funcion principal
   escribe "hola"
```

Reglas del esqueleto:

- `perfil` es **obligatorio**. Sin el, `E0001`.
- Todo lo de nivel superior **se congela** cuando el modulo termina de cargarse
  (`INTI_MAESTRO.md` sec. 4). Por eso un modulo se puede prestar a otra tarea
  sin cerrojos, y por eso **`cambiante` no vale en el nivel superior** (`E0002`).
- `principal` es el punto de entrada. Un modulo sin `principal` es una libreria.

---

## 2. Indentacion

- **Cuatro espacios** por nivel. **Los tabuladores son un error** (`E0010`), no
  una alternativa: la sorpresa de mezclar los dos no vale lo que cuesta.
- Un bloque abre con la linea que lo introduce y cierra al volver la sangria.
- **Una linea, una sentencia.** No hay continuacion implicita salvo dentro de
  `(` `[` `{` sin cerrar.

---

## 3. Nombres, comentarios y textos

```text
# comentario hasta el final de la linea. No hay comentario de bloque.

nombre_de_variable          # minusculas y guion bajo
NombreDeRegistro            # los tipos empiezan por mayuscula
CONSTANTE                   # todo mayusculas: convencion, no regla
```

Texto:

```text
saludo = "hola"
con_hueco = "hola {nombre}, tienes {edad} anos"     # interpolacion
en_bruto = crudo_texto "C:\ruta\sin\escapes"
multi = """
   varias lineas, y la sangria del margen se quita
   """
```

- Las comillas son **dobles**. Las simples **no existen** (una forma menos).
- Escapes: `\n`, `\t`, `\"`, `\\`, `\{`. Se acaba la lista.
- ⚠ Un texto es **UTF-8 y se valida al construirlo**. La conversion a la consola
  Latin-1 la hace `escribe`, **no el usuario** (`INTI_MAESTRO.md` 10.10).

---

## 4. Valores y tipos

| tipo | que es | ejemplo |
|---|---|---|
| `numero` | el numero de todos los dias: **decimal exacto** | `1`, `2.5`, `0.1` |
| `entero8` `entero16` `entero32` `entero64` | tamano **exacto**, con signo | `entero32` |
| `natural8` .. `natural64` | sin signo | `natural8` |
| `decimal` | la forma exacta de `numero`, si se quiere nombrar | |
| `flotante32` `flotante64` | IEEE-754 **estricto**, cuando se pide | |
| `logico` | `cierto` / `falso` | |
| `letra` | un punto de codigo Unicode | `'a'` no: se escribe `letra "a"` |
| `texto` | cadena UTF-8 inmutable | `"hola"` |
| `lista de T` | secuencia | `[1, 2, 3]` |
| `tabla de T a U` | asociativa | `{"a": 1, "b": 2}` |
| `quiza T` | puede no haber valor | |
| `nada` | lo que devuelve una funcion que no devuelve | |

★★ **No hay nulo.** Un valor que puede faltar se declara `quiza T` y **no se
puede usar sin mirarlo**. El "error de los mil millones de dolares" es una
sorpresa evitable, y evitarla es gratis en un lenguaje nuevo.

★ **`numero` es decimal exacto por defecto** (`INTI_MAESTRO.md` 10.3):

```text
escribe 0.1 + 0.2          # 0.3     y no 0.30000000000000004
```

En **perfil llano** `numero` **no existe**: hay que decir el tamano (`E0020`).
Sin tamanos no hay perfil sin monton, y esa obligacion sale del perfil, no del
gusto.

---

## 5. Variables

```text
x = 5                       # fija. Volver a asignarla es E0030
cambiante y = 5             # puede cambiar
y = y + 1

cambiante z es entero32 = 0 # con tipo explicito
```

- **No existe declarar sin valor.** Por eso "leer una variable sin inicializar"
  --que en C es comportamiento indefinido-- **no se puede escribir** (regla 4 de
  `REGLAS.md`).
- El tipo es **opcional en `pleno`** y **obligatorio en `llano`**.

---

## 6. Operadores, en orden de precedencia (de mas fuerte a mas debil)

| nivel | operadores | nota |
|---|---|---|
| 1 | `(...)`, `f(...)`, `a[i]`, `a.campo` | |
| 2 | `-x`, `no x` | unarios |
| 3 | `elevado` | `2 elevado 8` |
| 4 | `*`, `/`, `entre`, `resto` | `entre` = cociente entero; `/` divide de verdad |
| 5 | `+`, `-` | |
| 6 | `desplaza ... izquierda/derecha`, `bits_y`, `bits_o`, `bits_xor` | sobre todo en `llano` |
| 7 | `<`, `>`, `<=`, `>=` | |
| 8 | `=`, `no es`, `es`, `es un` | comparacion |
| 9 | `y` | |
| 10 | `o` | |

```text
si edad >= 18 y tiene_carnet
   ...

si nombre = "eddi"          # comparar: `=`
   ...

si respuesta no es "si"
   ...

si x es un numero           # preguntar el TIPO: `es un`
   ...
```

★ **`5 / 2` da `2.5`; `5 entre 2` da `2`.** Nombres distintos, no dos simbolos
que se parecen (`/` y `//` de Python son la sorpresa numero 10).

⚠ **`y` y `o` NO cortocircuitan a medias:** evaluan de izquierda a derecha y
paran en cuanto saben la respuesta. Es lo mismo que hace todo el mundo, pero
aqui esta **escrito** en vez de suponerse (regla 6 de `REGLAS.md`).

---

## 7. Condicionales

```text
si temperatura > 30
   escribe "hace calor"
sino si temperatura > 15
   escribe "se esta bien"
sino
   escribe "hace frio"
```

- La condicion tiene que ser **`logico`**. `si 1` o `si "hola"` es `E0040`.
  La "veracidad" de Python (`if lista:`) **no existe**: se escribe
  `si no esta vacia lista`. Una regla que hay que aprender menos.

---

## 8. Bucles: uno, con tres formas

```text
para cada alumno en alumnos
   escribe alumno.nombre

para cada i en 0 hasta 10          # 0,1,...,9 -- el final NO entra
   escribe i

repite 10 veces
   escribe "."

repite mientras quedan_datos
   ...

repite                              # infinito a proposito
   si terminado
      corta
```

- `corta` sale del bucle, `continua` salta a la vuelta siguiente.
- **`para cada` no puede modificar la coleccion que recorre** (`E0050`). Es el
  bug clasico de borrar mientras se itera, y aqui no compila.
- ⚠ `0 hasta 10` **excluye el 10**. Se elige el mismo convenio que el indice
  base 0 (sec. 9) para no tener dos reglas distintas en la cabeza.

---

## 9. Listas, tablas e indices

```text
notas = [8, 6, 9]
notas[0]                       # 8   -- LA BASE ES 0
primero de notas               # 8
ultimo de notas                # 9
cuenta de notas                # 3

edades = {"ana": 30, "luis": 25}
edades["ana"]                  # 30
```

★ **La base es 0, y es una decision incomoda que se explica:** para un novato
el 1 seria mas facil (COBOL y Lua lo hacen). Pero **INTI escribe sistema**, y
un desfase de uno entre lo que dice el lenguaje y lo que dice el hardware es
peor bug que la sorpresa del primer dia. **Se elige la verdad sobre la
comodidad**, y a cambio existen `primero de` y `ultimo de` para no tener que
contar casi nunca.

**Un indice fuera de rango ATRAPA** (regla 2). No lee memoria de al lado, no da
un valor cualquiera: devuelve un error, y si el compilador ve el rango, **ni
siquiera compila**.

---

## 10. Funciones

```text
funcion saluda(nombre es texto)                  # sin `devuelve` -> devuelve nada
   escribe "hola {nombre}"

funcion media(numeros es lista de numero) devuelve numero
   si esta vacia numeros
      devuelve 0
   ...

funcion divide(a, b) devuelve numero o error     # puede fallar
   si b = 0
      falla "no se puede dividir entre cero"
   devuelve a / b
```

- Los parametros **no se pueden cambiar dentro** salvo que se declaren
  `cambiante` (P3).
- **El valor por defecto de un parametro se congela** al declarar la funcion,
  asi que la sorpresa numero 1 de Python (`def f(lista=[])`) **no puede
  existir**.
- Las funciones son valores: `f = media` guarda la funcion, `f(x)` la llama.
  **`f` y `f()` se ven distintos a proposito.**
- ★ **No hay funciones anidadas ni anonimas** (`E0101`), y el motivo es del
  perfil y no del gusto: **una captura hay que guardarla en algun sitio, y en
  `llano` no hay monton.** Tenerlas solo en `pleno` serian dos lenguajes con
  una gramatica. Consecuencia buena: sin capturas, la sorpresa 2 de Python
  (*late binding*) **no existe por ausencia**, sin necesidad de ninguna regla.
- Una llamada se escribe de tres maneras y **las tres dan el mismo arbol**:
  `escribe("hola")`, `escribe "hola"` (sec. 10b) y `cuenta de notas` para un
  solo argumento.

---

## 10b. ★★ La forma de sentencia: una llamada sin parentesis

Peticion de Eddi: *"la otra libreria es para quitar las `()` y otras cosas, para
ACERCAR o estar igual que la sintaxis de Python pero ULTRA simplificado"*.

```text
   escribe "hola"
   escribe "media:", m
   guarda "notas.txt", texto(m)
   anade notas, 5
```

Y **no rompe la regla de que `f` y `f()` se ven distintos**, porque solo vale
**al principio de una sentencia y con argumentos**:

```text
   escribe            <- el VALOR de la funcion, no una llamada
   escribe()          <- la llamada sin argumentos
   escribe "hola"     <- la llamada con uno
```

### La pieza que decide, y es una sola

Detras del nombre tiene que venir algo que **empieza un valor y no puede
continuar una expresion**: un texto, un numero, otro nombre, un tipo,
`cierto`/`falso`/`nada`, o una tabla. Lo que queda fuera:

| se escribe | se lee como | por que |
|---|---|---|
| `x = 5` | asignacion | `=` continua la sentencia |
| `p.x = 3` | asignacion | `.` continua el nombre |
| `notas[0] = 5` | asignacion | `[` es un indice, no un argumento |
| `total - 1` | expresion | ⚠ `-` es ambiguo y se queda fuera |
| `f(1)` | la llamada de siempre | `(` ya tiene su forma |

⚠ **El `-` se queda fuera a proposito.** `escribe -1` podria ser `escribe(-1)` o
una resta, y **una regla que hay que pensar no simplifica nada**. Para pasar un
negativo: `escribe(-1)`.

★ Y sale **el mismo nodo del arbol** que con parentesis: no hay dos formas de
llamar, hay una escrita de dos maneras. Hay un test que lo comprueba.

---

## 11. Registros

```text
registro Alumno
   nombre es texto
   nota   es numero
   activo es logico = cierto        # valor por defecto

a = Alumno("ana", 9)                # posicional
b = Alumno(nombre: "luis", nota: 7) # por nombre

a.nombre                            # leer
```

- **No hay herencia, ni clases, ni `self`, ni metodos magicos**
  (`INTI_MAESTRO.md` 10.5). Un registro son datos; el comportamiento son
  funciones.
- Un registro **pequeno y sin partes que crecen es un VALOR**: se copia, no
  tiene identidad. Uno que contiene `texto` o `lista` es una **COSA** y se
  cuenta por referencias -- pero **se comporta igual**, porque lo que se pasa
  no se puede cambiar.

Para dar comportamiento propio a un registro (sumarlo, compararlo, medirlo) se
rellenan **operaciones numeradas**, que es la tabla que ya existe en
`bmo_abi::dynobj::slots`:

```text
operacion Punto suma(a, b) devuelve Punto
   devuelve Punto(a.x + b.x, a.y + b.y)
```

---

## 12. Errores: se escriben o no compila

Tres formas, y no hay una cuarta:

```text
# 1. mirar el resultado
resultado = abrir("notas.txt")
si fallo resultado
   escribe "no pude abrir:", motivo de resultado
   devuelve
texto = valor de resultado

# 2. la salida corta, con un valor
texto = abrir("notas.txt") o si no ""

# 3. la salida corta, con un bloque
texto = abrir("notas.txt") o si no
   escribe "no pude abrir:", motivo
   devuelve
```

- Una funcion que puede fallar lo declara: `devuelve texto o error`.
- **Ignorar un resultado que puede fallar es `E0060`**, error de compilacion.
  No hay `except:` pelado que se trague nada porque no hay nada que tragarse.
- `falla "motivo"` produce un error; **no hay excepciones que salten**.
- ★ Por eso el interprete no necesita tablas de desenrollado y el AOT devuelve
  el error **en un registro**.

---

## 13. Los dos perfiles

```text
perfil llano
```

Prohibido, y **el compilador lo dice con nombre y sitio** (`E0070`):

| en `llano` NO hay | motivo |
|---|---|
| `numero` sin tamano, `texto`, `lista`, `tabla` | crecen: piden monton |
| contador de referencias, congelado, tareas | piden runtime |
| interpolacion de texto que reserva | reserva |

Y **si** hay, y sirve para escribir un driver:

```text
perfil llano

funcion lee_tecla devuelve natural8
   crudo
      repite mientras (entrada_puerto(0x64) bits_y 1) = 0
         espera()

      devuelve entrada_puerto(0x60)
```

- `crudo` es la unica construccion que apaga las comprobaciones. **Se escribe,
  el compilador la CUENTA, y el numero sale en el informe del `.bex`** para que
  `bmo-verify` pueda exigirlo firmado. Igual que `unsafe` de Rust y por la misma
  razon: no se puede eliminar, se puede hacer **visible y contable**.
- `crudo` **no existe en `pleno`** (`E0071`).

```text
perfil pleno
```

Todo lo anterior mas texto, listas, tablas, `numero`, contador de referencias,
congelado y tareas:

```text
en paralelo
   procesa(trozo_a)
   procesa(trozo_b)
```

- Lo que entra en `en paralelo` **tiene que estar congelado o copiarse**
  (`E0080`). Por eso no hay GIL: no hay nada compartido y mutable a la vez.

---

## 14. La gramatica, en EBNF

```ebnf
modulo        = perfil , { usa } , { declaracion } ;
perfil        = "perfil" , ( "llano" | "pleno" ) , NL ;
usa           = "usa" , NOMBRE , NL ;

declaracion   = constante | registro | funcion | operacion ;
constante     = NOMBRE , "=" , expr , NL ;
registro      = "registro" , TIPO , NL , SANGRA , { campo } , DESANGRA ;
campo         = NOMBRE , [ "es" , tipo ] , [ "=" , expr ] , NL ;
funcion       = "funcion" , NOMBRE , [ "(" , [ params ] , ")" ] ,
                [ "devuelve" , tipo_ret ] , NL , bloque ;
operacion     = "operacion" , TIPO , NOMBRE , "(" , [ params ] , ")" ,
                [ "devuelve" , tipo_ret ] , NL , bloque ;
params        = param , { "," , param } ;
param         = [ "cambiante" ] , NOMBRE , [ "es" , tipo ] , [ "=" , expr ] ;
tipo_ret      = tipo , [ "o" , "error" ] ;

tipo          = TIPO
              | "lista" , "de" , tipo
              | "tabla" , "de" , tipo , "a" , tipo
              | "quiza" , tipo ;

bloque        = SANGRA , sentencia , { sentencia } , DESANGRA ;

sentencia     = asigna | si | para | repite | devuelve | falla
              | corta | continua | crudo | paralelo | expr_sent ;

asigna        = [ "cambiante" ] , destino , [ "es" , tipo ] , "=" , expr , NL ;
destino       = NOMBRE , { "." , NOMBRE | "[" , expr , "]" } ;

si            = "si" , expr , NL , bloque ,
                { "sino" , "si" , expr , NL , bloque } ,
                [ "sino" , NL , bloque ] ;

para          = "para" , "cada" , NOMBRE , "en" , rango_o_expr , NL , bloque ;
rango_o_expr  = expr , [ "hasta" , expr ] ;

repite        = "repite" , NL , bloque
              | "repite" , expr , "veces" , NL , bloque
              | "repite" , "mientras" , expr , NL , bloque ;

devuelve      = "devuelve" , [ expr ] , NL ;
falla         = "falla" , expr , NL ;
corta         = "corta" , NL ;
continua      = "continua" , NL ;
crudo         = "crudo" , NL , bloque ;                 (* solo perfil llano *)
paralelo      = "en" , "paralelo" , NL , bloque ;       (* solo perfil pleno *)

expr          = o_expr , [ "o" , "si" , "no" , ( expr | NL , bloque ) ] ;
o_expr        = y_expr , { "o" , y_expr } ;
y_expr        = comp , { "y" , comp } ;
comp          = suma , { ( "=" | "no" "es" | "es" "un" | "es"
                         | "<" | ">" | "<=" | ">=" ) , suma } ;
suma          = bits , { ( "+" | "-" ) , bits } ;
bits          = prod , { ( "bits_y" | "bits_o" | "bits_xor"
                         | "desplaza" ( "izquierda" | "derecha" ) ) , prod } ;
prod          = pot , { ( "*" | "/" | "entre" | "resto" ) , pot } ;
pot           = unario , [ "elevado" , pot ] ;
unario        = [ "-" | "no" ] , sufijo ;
sufijo        = primario , { "(" , [ args ] , ")" | "[" , expr , "]"
                           | "." , NOMBRE } ;
primario      = NUMERO | TEXTO | "cierto" | "falso" | NOMBRE
              | lista | tabla | "(" , expr , ")"
              | NOMBRE , "de" , sufijo                  (* cuenta de x *)
              | ( "valor" | "motivo" ) , "de" , sufijo
              | "fallo" , sufijo ;
lista         = "[" , [ expr , { "," , expr } ] , "]" ;
tabla         = "{" , [ par , { "," , par } ] , "}" ;
par           = expr , ":" , expr ;
```

★ Nota sobre `NOMBRE de sufijo`: **`de` no es un operador**, es la forma de
llamar con un argumento. `cuenta de notas` y `cuenta(notas)` son el **mismo
arbol**. Existe porque se lee como una frase, y **cuesta cero gramatica**.

---

## 15. Las palabras clave -- 49, y ya viven en una TABLA

```text
perfil  llano  pleno  usa
funcion  devuelve  registro  operacion
cambiante  es  un  de  a
si  sino  para  cada  en  hasta  repite  veces  mientras
corta  continua  falla  crudo  paralelo
y  o  no  entre  resto  elevado  desplaza  izquierda  derecha
bits_y  bits_o  bits_xor
cierto  falso  nada  quiza  error  fallo  valor  motivo
lista  tabla
```

### ★ Las seis palabras que TAMBIEN son nombres

Descubierto escribiendo el parser, el 2026-08-19, y es una correccion de
verdad: **`y` y `o` son operadores... y son los nombres de variable mas usados
del mundo despues de `x`.** Un lenguaje en el que no se puede escribir `x, y`
--ni `p.y`-- tiene un problema.

La salida no es quitar los operadores. Es que la palabra signifique una cosa
**en posicion de operador** y otra **en posicion de valor**:

```text
   x = y            # `y` es un NOMBRE: aqui toca un valor
   si a y b         # `y` es el OPERADOR: aqui toca un operador
   p.y = 3          # detras de un punto, cualquier palabra es un campo
```

Son seis: **`y`, `o`, `a`, `un`, `en`, `de`**. No hay ambiguedad porque el
parser siempre sabe cual de las dos posiciones espera -- no se elige
adivinando, se elige por el sitio. Es lo mismo que hacen `await` en JavaScript
o `record` en Java.

Y de paso se quito el `a` de `elevado a`, que ahora es solo `elevado`: **una
palabra clave de una letra que ademas es el nombre mas comun de una variable es
una trampa, y la mas barata de quitar es la que no hacia falta.**

⚠★ **Estas palabras NO se escriben en el parser: viven en
[`tables/lang/inti/palabras.toml`](../../forge/sem-asm/tables/lang/inti/palabras.toml)**,
que ya existe -- el mismo patron que `intrinsics.toml` (*"anadir una
instruccion = 1 entrada TOML, CERO Rust"*). El fichero **trae ya la columna en
ingles**, no para activarla, sino para que la frase de abajo se pueda comprobar
en vez de creer.

Y vive en `tables/` y no en `lang/inti/` por un motivo concreto: **`tables/` es
la raiz que consulta `bmo-mods`**. Quien deje su version en `$BMO_MODS` gana,
**sin bifurcar el repo**. Un dialecto de INTI es un fichero, no un fork.

**Motivo, y es una decision de hoy que se paga o se cobra hoy:** palabras clave
en espanol significa que nadie fuera de tu idioma contribuye. Con la tabla,
**un fichero mas y INTI habla ingles sin tocar el compilador**. Hacerlo asi
ahora no cuesta nada; convertirlo despues cuesta el parser entero.

Y **las tildes son alias**: la version acentuada de una palabra clave lexa
igual que la ASCII. El fichero canonico sigue siendo ASCII, y quien escribe con
tildes no tropieza.

---

## 16. Lo que NO tiene la sintaxis, y es a proposito

| no hay | en su lugar |
|---|---|
| `:` de bloque, `;`, `{}` de bloque | la sangria |
| ternario `a ? b : c` | `si` |
| `lambda` y funciones anidadas | una `funcion` en el margen, y `f = media` para pasarla como valor |
| comprensiones anidadas | `para cada` |
| decoradores | una funcion que llama a otra |
| `global` / `nonlocal` | no hacen falta: un nombre es del bloque donde nacio |
| `is` / identidad | **no existe** (P2) |
| operadores definidos por el usuario | `operacion` sobre ranuras numeradas |
| `++`, `--`, `+=` como expresion | `x = x + 1` |
| nulo | `quiza T` |
| conversion implicita entre tipos | `numero(texto)`, `texto(numero)` |

---

## 17. Como llega INTI al sistema -- ★ LA PUERTA NO ES SINTAXIS

> Pregunta de Eddi, 2026-08-19: *"como es lenguaje de sistema, no viven los
> syscall, no? aunque suene extrano, pero si es para poder tener control en
> ellas, para uso."*

**No suena extrano: es la pregunta correcta, y la respuesta es que NO.** Ni una
palabra clave de INTI habla de `INVOKE`, de `WAIT` ni de capabilities. Y aun
asi se tiene control absoluto sobre ellas. Las dos cosas a la vez, y este es
el motivo.

### Por que no, con el precedente delante

**C nunca tuvo `read()` como palabra clave.** Era una funcion de biblioteca, y
la instruccion de trampa vivia en unas lineas de ensamblador dentro de libc.
Por eso C pudo pasar del PDP-11 al Interdata: **el lenguaje no sabia en que
sistema estaba corriendo**.

★★★ Si `invoca` fuera sintaxis de INTI, **el lenguaje quedaria casado con este
sistema operativo** y se perderia justo la mitad de la portabilidad que la
seccion 7 del maestro llama *"el SISTEMA se porta"* -- que es la unica razon
historica por la que existio C.

Y hay un segundo motivo, del propio arbol: **la superficie son DOS syscalls
congelados**. Meterlos en la gramatica seria congelar la gramatica al ritmo del
kernel, cuando la decision de BMO-X fue la contraria: *la API crece por dentro,
en la pareja (tipo de objeto, operacion), y el ABI no se toca*.

### Los tres escalones, y en cual esta cada cosa

```text
   usa superficie / archivo / entrada / paquete      <- REX: lo que se escribe
      guarda "notas.txt", texto                          normalmente
      pinta rectangulo(10, 10, 100, 50)

   usa bmo                                           <- la puerta, envuelta
      codigo = invoca(cap, operacion, a0, a1, a2)
      valor  = invoca_valor(cap, operacion, a0, a1, a2)
      espera(esperable, visto, tiempo)

   usa x86_64                                        <- los intrinsecos
      entrada_puerto(0x60)      escribe_puerto(0x60, x)
      lee_reloj()               para()
```

**Ninguno de esos nombres es palabra clave.** Los tres escalones son **tablas**:

| escalon | donde vive | quien lo puede tapar |
|---|---|---|
| REX | `tables/bmo/*.h` y su equivalente para INTI | `$BMO_MODS`, sin bifurcar el repo |
| la puerta | `tables/bmo/` sobre el intrinseco | igual |
| los intrinsecos | `tables/arch/<maquina>/inti.toml` (los nombres) sobre `intrinsics.toml` (los bytes) | igual |

★ Y eso ya funciona asi para C: `__syscall(...)` **es una fila de
`intrinsics.toml`** con sus bytes (`0F 05`) y el registro de cada argumento
escrito ahi. Hay dos filas y no una porque **la puerta contesta dos cosas**:
codigo en `rax` (`[syscall]`) y valor en `rdx` (`[syscall_valor]`). *Se lee
como C, se comporta como ASM, y ninguna de las dos mitades esconde nada de la
otra.* INTI hereda ese mecanismo entero: **anadir una operacion del sistema =
una entrada de tabla, CERO lineas del compilador.**

### ★★ Y la distincion que decide donde hace falta `crudo`

```text
   invoca(cap, op, ...)        NO necesita `crudo`
   entrada_puerto(0x60)        SI necesita `crudo`
```

No es una inconsistencia, es la regla del sistema aplicada al lenguaje:

> **Una capability existe para arbitrar AUTORIDAD.** Al otro lado de `invoca`
> hay un kernel que comprueba quien eres y que puedes hacer. Al otro lado de
> un puerto de E/S **no hay nadie**.

O sea: `crudo` no marca *"esto es de bajo nivel"* -- marca **"aqui nadie
comprueba por ti"**. La puerta es de bajo nivel y esta comprobada; un `outb`
no lo esta. Por eso uno se escribe y el otro no.

### Lo que esto te da, que es lo que preguntabas

- **Control total**: desde `perfil llano` se puede llamar a la puerta desnuda
  con los seis argumentos, sin nada en medio. Es el equivalente exacto de lo
  que hoy hace `bmo.h`.
- **Sin casarse con el sistema**: el dia que INTI compile para otra maquina, lo
  que cambia es una carpeta de tablas. La gramatica no se entera.
- **Y sin dos mundos**: `guarda "x", texto` y `invoca(cap, 7, ...)` son el mismo
  lenguaje, en el mismo fichero si hace falta. Es lo segundo que C le dio a
  Unix -- *el kernel y las herramientas se escribian igual*.

⚠ **Correccion a como se dijo antes:** en los ejemplos del maestro se escribio
que *"`guarda` es del lenguaje, no de una libreria que hay que instalar"*. Lo
segundo es cierto y lo primero no: **`guarda` es de la biblioteca base, y la
biblioteca base viaja DENTRO** -- que es la propiedad de REX (*"una cabecera
trae el cuerpo: no hay `libbmo.so` que alguien tenga que resolver despues"*).
No hay nada que instalar **y** no es sintaxis. Las dos cosas.

---

## 18. Como se comprueba que esto es verdad

`CENSO.md` -- una sonda por construccion, cada una con su veredicto **escrito
por delante**. Cuando exista el lexer (F1), el test compara el informe **entero**
contra la constante del censo, asi que **el censo no se puede quedar viejo**:
arreglar o romper una casilla hace fallar el test hasta que se actualiza.

Es el metodo de `c-gen`, y la razon esta en `BRECHA.md` de BMO C: *leer el lexer
diria que palabras se reconocen; aqui se pregunta lo unico que decide,
**compila?***
