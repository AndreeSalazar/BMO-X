# PLAN -- NUNCA ADIVINA: lo que el compilador no puede saber, no lo supone

> La regla, con las palabras del dueno, **2026-09-10**:
>
> *"si encuentra lo que es adivinar, no compila hasta que lo aclares. No me
> gustaria que la CPU tenga que perder tiempo en adivinar. NUNCA ADIVINA."*
>
> Y la exageracion que la resume: **adivinar es malo y mata.**

---

# 1. POR QUE ESTO NO ERA UN CAPRICHO, Y SE SUPO MIDIENDO

Se fue a escribir una regla sobre el **UB de C** --lo que el estandar deja sin
definir-- y se barrieron las doce formas clasicas para medir que hace hoy BMO C
con cada una. Salio esto:

```text
   12 de 12 EMITEN                    ni una se rechaza, ni una avisa
   10 de 12 CONTESTAN UN NUMERO       y el programa sigue
    2 de 12 el numero que dan el METAL NO LO DARIA NUNCA
```

Las dos ultimas son las peores, porque no son un desacuerdo con el estandar:
son un desacuerdo con la maquina.

```text
   INT_MIN / -1   ->  el emulador contesta 2147483648
                      el Ryzen levanta #DE: `idiv` no tiene ese resultado
   *NULL          ->  el emulador contesta un numero cualquiera
                      el Ryzen levanta #PF
```

** Con eso, **el banco sale verde y la maquina falla**. Que es exactamente el
peor sitio donde puede vivir una diferencia.

## *** Pero el hallazgo no estuvo en el UB: estuvo en las FILAS DE CONTROL

Las filas de control existen para que, si todo sale mal, se sepa que el problema
no es lo que se estaba mirando. **Tres salieron rojas**, y ninguna es UB: es C
perfectamente ordinario dando un numero equivocado sin un solo error.

```text
   float *p = &f;  (int)*p        ->  1080033280   los BITS de 3.5
   struct s { float f; };  v.f    ->  DESBORDA LA PILA DEL COMPILADOR
   1UL << 63                      ->  0
   18446744073709551615UL         ->  0
```

*** **El compilador ya adivinaba donde el estandar NO le daba permiso.** Y eso
cambia el orden del trabajo: primero se cierran los sitios donde supone por su
cuenta, y despues se declara que hace donde el estandar si le deja elegir.

  > Antes de escribir la regla contra lo indefinido, hay que dejar de inventar
  > en lo que esta perfectamente definido.

---

# 2. LO QUE YA SE HIZO (2026-09-10)

- [x] **A1 -- LOS TRES FALLOS CALLADOS.** Un flotante fuera de una variable
      suelta (leer daba los bits, escribir guardaba cero, y un `struct` con un
      campo `float` mataba al compilador); el sufijo del literal que se leia y
      se tiraba, con lo que **toda mascara de 64 bits salia CERO**; y un
      `unwrap_or(0)` que convertia en cero el literal decimal que no cabia.

  Se verifica: `tests/flotante_por_lugar.rs` y `tests/literales_y_sufijos.rs`,
  14 filas.

- [x] **A2 -- EL EMBUDO: `exige_tipo`.** Nueve sitios del emisor decian *"no se
      de que tipo es esto, asumo ocho bytes"* y con ese tipo elegian **el ancho
      de la instruccion**. Ahora no eligen: apuntan un error, y
      `emit_program` se niega a entregar el `.bex` si hay uno solo.

  ** `void*` cuenta como no saberlo: el tipo SI se sabe --`Some(Void)`-- pero su
  ancho no existe, y `void*` es el tipo con el que se pasa memoria de un lado a
  otro en C. Si eso se permite, la suposicion entra por la puerta mas usada.

  [!] Y **no rompio nada, porque se midio antes de escribirla**: los nueve
  sitios se instrumentaron con un contador y se compilo el arbol entero --DOOM
  incluido, 56.976 lineas-- con resultado **CERO**.

    > Una regla que nadie incumple hoy no es una regla inutil: es la unica que
    > se puede poner sin negociar.

  Se verifica: `tests/nunca_adivina.rs`, y su fila de control --`*(int*)p`
  compila-- que prueba que es un embudo y no un muro.

- [x] **A3 -- EL CENSO, Y SU LIMITE.** 61 sitios del compilador escriben
      `unwrap_or`. No todos adivinan: un valor por defecto es una eleccion sobre
      lo tuyo, y esto es una suposicion sobre la memoria de otro.

  ** Y el censo por sintaxis **no los encontro todos**: ocho estaban escritos
  `unwrap_or(TypeSpec::Long)` y el noveno era un brazo `_ => TypeSpec::Long`.

    > Buscar una forma de escribir encuentra una forma de escribir. Adivinar
    > tiene mas de una.

---

# 3. LO QUE FALTA

## [ ] A4 -- LAS SUPOSICIONES DE DISPOSICION

Quedan cuatro que eligen un TAMANO en vez de un tipo, y viven en el parser, que
no tiene donde apuntar un error de emision:

```text
   parser/types.rs:123      struct_sizes.get(n).unwrap_or(8)
   parser/types.rs:137      struct_aligns.get(n).unwrap_or(8)
   parser/inicializador.rs  dos mas de la misma forma
```

** Un tamano de struct supuesto no falla: **coloca mal una tabla entera**. Cada
elemento cae donde no es, y lo que se lee despues es el campo del vecino.

Se verifica: las cuatro pasan por un embudo con nombre, y una fila que le pide
el tamano de un struct que no existe **no compila**.

## [ ] A5 -- LA TABLA DEL UB, que era el encargo original

Ahora si, y sobre un suelo limpio. Un vocabulario **cerrado** de clases de
comportamiento indefinido, y para cada una **una de dos** -- nunca una tercera:

```text
   DEFINIDO    BMO-X elige, y la eleccion esta ESCRITA y probada con una fila
   RECHAZADO   no se emite el .bex, y se dice la clase y la linea
```

*** Lo que no puede existir es la tercera: elegir en silencio. Que es lo que
hacen hoy las doce.

Primer reparto propuesto, por lo que se midio:

| clase | hoy | propuesta | por que |
|---|---|---|---|
| desbordamiento con signo | envuelve | **DEFINIDO**: envuelve | el metal hace eso, y DOOM depende de ello |
| desplazar >= el ancho | para la maquina | **RECHAZADO** si el contador es constante | se sabe al compilar |
| division por cero | para la maquina | **RECHAZADO** si el divisor es constante | idem |
| `INT_MIN / -1` | da un numero imposible | **RECHAZADO** si es constante, y si no, DEFINIDO = lo que haga `idiv` | hoy el emulador y el metal NO coinciden |
| leer sin inicializar | da 0 | **RECHAZADO** | es estatico y es el bug mas comun de C |
| caer por el final de una funcion | devuelve 0 | **RECHAZADO** | es estatico y certero |
| `i = i++` | elige un orden | **RECHAZADO** | es sintactico |
| `f(i++, i++)` | evalua de derecha a izquierda | **DEFINIDO**: de derecha a izquierda | hay que elegir uno, y ya esta elegido |
| fuera de rango | escribe donde sea | **RECHAZADO** con indice constante | lo demas pide comprobaciones en ejecucion |
| `*NULL` | da basura | queda para el kernel | en el metal es un #PF, y eso SI avisa |

[!] **Y el orden importa**: las de "constante" son las que se pueden hacer YA,
porque el plegador ya existe (`codegen/decidir/plegado.rs`) y sabe decir si algo
es constante. Las de analisis --leer sin inicializar-- piden un recorrido nuevo.

- [ ] **A5a -- las cinco que ya se pueden decidir al compilar** (contador,
      divisor, `INT_MIN/-1`, indice, caer por el final)
- [ ] **A5b -- las dos sintacticas** (`i = i++`, dos efectos sin punto de secuencia)
- [ ] **A5c -- leer sin inicializar**, que pide recorrido
- [ ] **A5d -- el documento de lo DEFINIDO**, que es la mitad que un tercero
      necesita para escribir C para BMO-X sin sorpresas

## [ ] A6 -- EL EMULADOR Y EL METAL TIENEN QUE DECIR LO MISMO

Dos de las doce formas dan en el emulador un numero que el Ryzen no daria.
Mientras eso siga asi, **una fila verde del banco no prueba lo que parece**.

Se verifica: `INT_MIN / -1` y `*NULL` paran el emulador como paran la maquina --
o el banco declara que ahi no juzga.

---

# 4. LO QUE ESTA REGLA **NO** ES

## 4.1 No es "C estricto"

No se rechaza C valido. La fila de control del banco existe para eso: `*(int*)p`
compila, y es exactamente la aclaracion que se pedia. **La regla pide que lo
digas, no que no lo uses.**

## 4.2 No es un analisis de flujo

No se persigue lo que solo se sabe ejecutando. Lo que se cierra es lo que el
compilador **ya sabe que no sabe** -- el sitio donde hoy escribe un `unwrap_or`.

  > La diferencia entre un compilador estricto y uno honesto: el estricto te
  > dice que no a cosas que funcionan; el honesto te dice que no sabe.

## 4.3 Y no alarga el tiempo de compilacion

`exige_tipo` corre donde ya corria un `unwrap_or`: es un `match` mas. El coste
de la regla es **cero instrucciones en la maquina** -- que era el punto: lo que
el dueno no quiere es que **la CPU** pierda tiempo adivinando, y una CPU que
ejecuta la suposicion equivocada no pierde tiempo: pierde el dato.
