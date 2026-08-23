# LAS DOCE REGLAS -- lo que en C es indefinido, aqui esta escrito

> **F0.** Contrato. Cada fila de esta tabla **tiene una sonda** en `CENSO.md`, y
> ninguna se da por buena hasta que su sonda esta en verde.
>
> El porque esta en `docs/maestro/INTI_MAESTRO.md` sec. 6. El resumen: **el
> comportamiento indefinido de C nacio para PORTAR y hoy solo sirve para
> OPTIMIZAR** -- y la razon original caduco cuando C23 hizo obligatorio el
> complemento a dos. Comprobar cuesta **~1%**; parchear C a posteriori cuesta
> **29,1%**.

---

## La ley

> **Toda construccion de INTI tiene un resultado dicho por escrito.**
> Donde no hay resultado sensato, **atrapa** -- y atrapar es un resultado, no
> un accidente: es un error como dato (`GRAMATICA.md` sec. 12).

Eso descarta los tres modelos que no se eligieron:

| modelo | quien | por que no |
|---|---|---|
| parchear | *Friendly C* (2014) | deja fuera lo peor, y once anos despues solo hay banderas sueltas |
| detectar | Zig | vuelve a ser indefinido en `ReleaseFast`: **el binario que entregas es el que no comprueba** |
| **definir** | **WASM**, Java, Rust seguro | ✅ es este |

---

## Las doce

> **Estado al 2026-08-21.** De las cuatro que atrapan en ejecucion, **tres
> llegan a bytes y corren**: la 1, la 3 y la 12. La 2 espera a `lista de T`,
> porque un `bufer` no lleva su longitud y **no hay contra que comprobar** --
> por eso indexarlo pide `crudo`.
>
> Hasta hoy solo salia la 1. Las otras dos estaban calculadas en la IR, contadas
> y documentadas, y el emisor las descontaba sin emitir nada. El motivo escrito
> era correcto --*piden mirar un operando ANTES de la operacion*-- pero era un
> diagnostico: el arreglo estaba en la IR, que las ponia detras.

| # | en C | en INTI | error | sonda |
|---|---|---|---|---|
| **1** | desbordar un entero con signo: **indefinido** | ✅ **atrapa**, y corre. Para dar la vuelta a proposito: `suma_circular(a, b)`. **Incluye el cociente que no cabe** (`-2^63 entre -1`), que se escribe con una barra y es esta regla | `E1001` | `r01_desborde`, `r01b_cociente` |
| **2** | indice fuera del array: **indefinido** | ⏳ **espera a `lista de T`**: un `bufer` no lleva su longitud, asi que no hay contra que comprobar -- y por eso indexarlo pide `crudo` | `E1002` / `E0090` | `r02_indice` |
| **3** | dividir entre cero: **indefinido** | ✅ **atrapa**, y corre. La comprobacion mira el DIVISOR antes de dividir: despues de dividir entre cero no queda programa que mire nada | `E1003` | `r03_division` |
| **4** | leer sin inicializar: **indefinido** | **imposible de escribir**: no existe declarar sin valor | `E0031` | `r04_sin_valor` |
| **5** | puntero colgante o liberado: **indefinido** | en `pleno` no hay punteros crudos; en `llano`, prestamos con vida comprobada en compilacion | `E1005` | `r05_prestamo` |
| **6** | orden de evaluacion de argumentos: **no especificado** | **izquierda a derecha, siempre**, incluidos `y` y `o` | -- | `r06_orden` |
| **7** | desplazar mas bits que el ancho: **indefinido** | **da cero**, definido. Si el desplazamiento es constante, **avisa al compilar** | `A2007` | `r07_desplaza` |
| **8** | alias estricto (`int*` y `float*` a los mismos bytes): **indefinido** | **no existe**: dos nombres pueden ver los mismos bytes y esta definido | -- | `r08_alias` |
| **9** | `int` mide *"al menos 16 bits"* | **tamanos exactos**: `entero8/16/32/64`, `natural8..64` | `E0020` | `r09_tamanos` |
| **10** | orden de bytes: el de la maquina | **little-endian fijado** en todo lo que se serializa | -- | `r10_bytes` |
| **11** | el compilador puede reasociar flotantes y meter FMA | ✅ **IEEE-754 estricto**: el mismo programa da **el mismo bit** en cualquier maquina. Vigilado mirando los bytes emitidos | -- | `r11_flotante` |
| **12** | convertir flotante a entero fuera de rango: **indefinido** | ✅ **atrapa**, y corre. NaN e infinito incluidos, y con el ANCHO del destino dentro: 1e10 cabe en `entero64` y no en `entero32` | `E1012` | `r12_conversion` |

---

## ✅ EL AGUJERO DE LA 1 QUE VIVIA DENTRO DE LA 3 -- CERRADO (22-08)

**`-2^63 entre -1` ya atrapa con `E1001`.**

El cociente no cabe en 64 bits, asi que es la Regla 1 -- pero se escribe como una
division, y de la division solo se comprobaba **el divisor**. Estuvo abierto
menos de un dia y esto es lo que era:

```text
   antes    compilaba limpio, salia FIRMADO, y en el Ryzen moria con una
            autopsia del kernel -- `idiv` levanta `#DE`, el MISMO vector que
            dividir entre cero
   ahora    devuelve E1001, y hay sonda: `r01b_cociente`
```

★★ **Una division pide DOS reglas**, y las dos van delante:

```text
   EntreCero   el divisor es cero            -> E1003
   Cociente    el cociente no cabe           -> E1001
```

★ **La pide la IR, no la anade el emisor por su cuenta.** La diferencia no es de
estilo: hay una prueba que exige que *lo que la IR pide* y *lo que el binario
lleva* cuadren, y esa resta es la que medira al optimizador el dia que haya uno.
Un emisor que anade reglas por su cuenta rompe esa cuenta antes de que sirva.

★ Y `Comprobacion::Cociente` es **la unica de las cinco que mira DOS valores** --
el cociente solo se sale cuando el dividendo es el minimo Y el divisor es -1. Por
eso `Instr::Comprueba` gano un `contra: Option<Valor>`, y es un `Option` para que
las otras cuatro no tengan que rellenarlo con algo falso.

⚠ **Lo que costo, medido**: `cpu.ibex` pasa de 8.752 a 8.856 bytes, +104. Y eso
tiene una consecuencia que hay que decir: **ese fichero ya no es el que corrio en
el Ryzen el 22-08**. Hace lo mismo y una cosa mas, y la proxima medida se compara
contra este.

En el camino que no atrapa se paga **una comparacion y un salto que no salta**:
al segundo `cmp` solo se entra si el divisor es exactamente -1.

---

## Lo que NO se puede definir gratis, dicho por delante

Tres cosas, y se dicen aqui para que nadie las descubra como una decepcion:

1. **Bucles infinitos.** C permite suponer que todo bucle termina, y sin eso se
   pierden optimizaciones reales. **INTI no lo supone**: un bucle que no
   termina, no termina. Se paga y no se discute.

2. **La aritmetica de direcciones en `llano`.** Si INTI va a escribir un driver
   tiene que poder escribir una direccion fisica, y ahi **la comprobacion no la
   puede hacer el lenguaje**. Por eso existe `crudo`, y por eso `crudo`:
   - hay que **escribirlo** (no se hereda ni se activa con una bandera),
   - el compilador lo **cuenta**, y el numero va en el informe del `.bex`,
   - `bmo-verify` puede **exigirlo firmado**.

   Es `unsafe` de Rust y por la misma razon: **no se puede eliminar, se puede
   hacer visible y contable.**

3. **La reproducibilidad de los flotantes cuesta.** Prohibir FMA y reasociacion
   deja rendimiento en la mesa en calculo numerico. Se acepta: aqui **el mismo
   resultado en cualquier maquina vale mas**, porque el argumento de venta del
   sistema es que se puede verificar.

---

## Los codigos de error

Tres familias, y el numero **no se reutiliza nunca** aunque la regla se retire:

| familia | que es | ejemplo |
|---|---|---|
| `E0xxx` | error de compilacion **del lenguaje** (sintaxis, tipos, perfil) | `E0001` falta `perfil` |
| `E1xxx` | **atrapa en ejecucion**: llega como error, no como panico | `E1001` la suma se paso de la cuenta |
| `A2xxx` | **aviso**: compila, y el compilador dice por que no deberia | `A2007` desplazamiento constante mayor que el ancho |

Y el formato del mensaje es **contrato**, no estilo (`INTI_MAESTRO.md` sec. 8):
cuatro partes -- **que paso / donde / que habia / que hacer** --, cero jerga de
compilador, el nombre que escribio el usuario **siempre presente**, y la
sugerencia en forma de codigo que se puede pegar.

```text
   [QUE PASO]     La suma de `total` y `linea` se paso de la cuenta.
   [DONDE]        notas.inti, linea 12:   total = total + linea
   [QUE HABIA]    `total` es entero32 y aqui llego a 2.147.483.700.
   [QUE HACER]    Elige una:
                    total = total + linea               (atrapa, es lo de ahora)
                    total = suma_circular(total, linea) (da la vuelta a proposito)
                    cambia `total` a entero64
```

⚠ **Un mensaje sin las cuatro partes es un fallo de test, no un detalle de
estilo.** La evidencia dice que el **73%** de los envios de codigo llevan
errores: el mensaje **es** la interfaz principal del lenguaje.
