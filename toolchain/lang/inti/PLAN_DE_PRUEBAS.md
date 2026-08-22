# EL TEST TOTAL DE INTI -- en orden, y sin dulce

> Peticion de Eddi, 2026-08-21: *"solo necesito TEST total en orden que se
> necesitarian... me gustaria estar MAS cerca de lo que creo, sin dulce claro."*

Este documento no dice lo que INTI hace bien. Dice **que hay que comprobar para
poder afirmar lo que se afirma**, en el orden en que hay que hacerlo, y **que
falta hoy** en cada peldano.

La regla para entrar aqui: cada fila tiene un **criterio de aprobado que se
puede ejecutar**. Una fila sin criterio es una intencion, y las intenciones van
en el maestro, no aqui.

---

## 0. LA CIFRA INCOMODA, primero

**1.138 pruebas en verde** suena a mucho. Repartidas dicen otra cosa:

| lo que se prueba | como | veredicto |
|---|---|---|
| que el compilador no se rompe | 1.138 pruebas | ✅ solido |
| que el codigo emitido **hace lo que dice** | el emulador | ⚠ **25 de 61** nombres de la maquina |
| que corre **en un CPU de verdad** | ninguna | ⛔ **CERO** |
| que aguanta un programa **grande** | ninguna | ⛔ **CERO** |
| que aguanta un programa **hostil** | ninguna | ⛔ **CERO** |

★★★ **Ninguna linea de INTI ha corrido nunca en un procesador.** Todo lo verde
de arriba es un emulador que escribimos nosotros, comprobando bytes que
escribimos nosotros. Eso no es poco --caza muchisimo-- pero **no es lo mismo**, y
este documento existe para no confundirlos.

Y el programa mas grande que INTI ha compilado tiene **once lineas**.

---

## 1. LOS SIETE PELDANOS, en orden

El orden no es de importancia: es de **dependencia**. Cada uno necesita que el
anterior este, y saltarse uno hace que el siguiente mida otra cosa.

```text
   1  EL CORPUS       las sondas dicen la verdad          -> ya
   2  EL EMULADOR     lo que compila, corre y acierta     -> ya, con limite
   3  EL METAL        lo que el emulador no puede dar     -> AQUI ESTAMOS
   4  EL TAMANO       un programa de mil lineas
   5  EL HOSTIL       lo que rompe a proposito
   6  EL TIEMPO       cuanto cuesta, medido
   7  EL AJENO        alguien que no lo escribio
```

---

## 2. PELDANO 1 -- EL CORPUS: que las sondas digan la verdad

**Estado: ✅ hecho el 21-08, y hasta hoy estaba a medias.**

| criterio | como se ejecuta | estado |
|---|---|---|
| toda sonda se puede leer | `ninguna_sonda_lleva_un_fallo_de_escritura` | ✅ |
| **toda sonda se PARSEA** salvo las que buscan un error de sintaxis | `ninguna_sonda_muere_en_el_parser_salvo_las_que_lo_buscan` | ✅ **nuevo** |
| toda sonda cumple el veredicto que declara | `cada_sonda_cumple_el_veredicto_que_declara` | ✅ **nuevo**: 37 de 42 |
| las 5 exentas tienen su motivo, y **la exencion caduca sola** | el mismo test, en la otra direccion | ✅ **nuevo** |

⚠ **Lo que este peldano costo descubrir, y es la leccion del dia:** el test
comprobaba **diez** sondas contra su veredicto, escritas en una lista a mano. Las
otras treinta y dos declaraban un veredicto que no miraba nadie.

Y una lista a mano **no crece sola**. Consecuencias reales encontradas al
recorrer las 42:

| sonda | llevaba asi | que pasaba |
|---|---|---|
| `v04_sin_veracidad` | desde **F0** | **no compilaba**: usaba `lista` como variable, y `lista` es palabra clave. Tres errores de sintaxis. Es el mismo fallo que `para` en la tabla de la maquina |
| `r12_conversion` | desde F0 | escribia `escribe(...)` en `perfil llano`, donde no existe: moria al compilar y **nunca llegaba a la regla que probaba** |
| `f04_parametro_fijo` | desde F2b | daba `E0030` y declara `E0033`. El aviso generico manda a *"la linea donde nace, sin `cambiante`"*, y la de un parametro es la firma: **un consejo correcto que no se puede seguir** |

★ Y el tercero tenia una prueba propia **exigiendo el codigo equivocado**, o sea
que el banco congelaba el fallo en su sitio. Eso solo se ve comparando contra lo
que el corpus DECLARA, no contra lo que el compilador hace.

**Las 5 que quedan exentas**, con su fase:

```text
   p05_paralelo_mutable   E0080 pide el analisis de tareas       -> F7
   r02_indice             E0090 pide la LONGITUD                 -> lista de T
   v03_sin_nulo           E0021 pide `quiza T`                   -> PLENO
   v04_sin_veracidad      E0040 funciona en llano (v07). Es pleno -> PLENO
   v05_sin_conversion     E0022 funciona en llano (v06). Es pleno -> PLENO
```

---

## 3. PELDANO 2 -- EL EMULADOR: que lo que compila corra y acierte

**Estado: ✅ hecho, con un limite que hay que decir.**

| criterio | estado |
|---|---|
| la aritmetica da el numero | ✅ |
| las tres reglas que llegan a bytes atrapan **con su codigo** | ✅ (F5e) |
| el NaN pierde cinco comparaciones y gana la sexta | ✅ (F5c) |
| **cada nombre de la tabla de la maquina emite sus bytes** | ✅ (F5d): 61 de 61 |
| **cada nombre se EJECUTA** | ⚠ **25 de 61** |

⚠ Los otros **36 solo se pueden verificar en metal**, y no por un fallo del
emulador: por su regla, escrita en `VERDAD.md`. Devolver `0` como si fuera el
valor de un MSR seria inventarse un dato.

★★ **Y el emulador ha fallado tres veces este mes**, las tres de la misma
familia --el banco decia que si y el Ryzen habria dicho que no--:

```text
   `add` no ponia `of`         (19-08)   ningun lenguaje emitia un `jo`
   `imul` no ponia banderas    (21-08)   el desbordamiento al multiplicar
                                         NUNCA se atrapaba aqui
   `cvttsd2si` saturaba        (21-08)   1e30 daba el entero mayor y NaN daba
                                         cero; el silicio da el centinela
```

⚠ **Eso es lo que mide de verdad la fiabilidad de este peldano**: el oraculo se
equivoca, y solo se descubre cuando un lenguaje nuevo le pregunta algo que
ninguno le habia preguntado. **Quedan preguntas sin hacer.**

---

## 4. PELDANO 3 -- EL METAL: donde estamos, y que hace falta

**Estado: ⛔ CERO. Es el peldano que bloquea todo lo de abajo.**

Ya no falta nada de mi lado: desde F6b `inti fichero.inti` produce un `.bex` que
pasa el gate. **Lo que falta es la maquina.**

### 3.1 Las tres pruebas, en orden

| # | prueba | criterio de aprobado |
|---|---|---|
| **M1** | **el hola mundo del metal**: un `.bex` de INTI arranca y sale por la puerta con un codigo elegido | el kernel recoge exactamente ese codigo |
| **M2** | **las 36 instrucciones que el emulador no puede dar** | cada una devuelve algo, y las que se puedan comparar contra un valor conocido cuadran |
| **M3** | **las tres reglas atrapan en metal**: desborde, entre cero, conversion | los codigos 1001, 1003 y 1012 salen por la puerta |

### 3.2 ⚠ Por que M2 no es una lista de 36 sondas sueltas

Porque cada arranque cuesta un reinicio. **Un programa que las ejecute todas y
saque los resultados por la puerta vale por 36**; treinta y seis programas valen
por uno y cuestan treinta y seis reinicios.

★ Y hay una trampa que hay que evitar por delante: de las 36, **muchas no
devuelven un valor comparable** (`cli`, `lgdt`, `wbinvd`). Para esas el criterio
no puede ser *"da el numero correcto"* sino *"no tumba la maquina y el programa
sigue"*, que es lo unico que se puede afirmar. Decirlo antes evita una tabla de
resultados que parece decir mas de lo que dice.

### 3.3 Lo que M1 puede destapar, y no seria culpa de INTI

El `.bex` de INTI no ha pasado nunca por el cargador de verdad. Lo que se prueba
ahi no es solo el compilador: es **el contrato entero** -- secciones, `entry_offset`,
alineacion, la convencion de la puerta. Si M1 falla, el primer sospechoso no es
el emisor.

---

## 5. PELDANO 4 -- EL TAMANO: un programa grande

**Estado: ⛔ CERO. El programa mas grande que INTI ha compilado tiene ONCE
lineas.**

⚠ Esta es la parte que mas se subestima, y la que mas cosas rompe:

| criterio | por que rompe algo |
|---|---|
| **mil lineas** de INTI compilan | los limites que nadie puso: profundidad de bloques, numero de temporales, saltos que no caben en un byte |
| **cien funciones** que se llaman entre si | el parcheo de llamadas es de un solo modulo; el reparto de registros se frena con la primera llamada |
| un `registro` con **cincuenta campos** | desplazamientos que pasan del rango de un `disp8` |
| un bucle con **diez mil vueltas** | el emulador tiene un tope de pasos; el `.bex` no |

★ **El candidato natural es el propio MONTON**, que ya esta escrito en INTI y en
`llano`. Hoy se compila por piezas dentro de las pruebas; compilarlo entero desde
la linea de ordenes es el primer programa de verdad que INTI puede intentar.

**Criterio de aprobado**: compila, pasa el gate, y **el reparto de memoria da los
mismos resultados que el de las pruebas de F4c**.

---

## 6. PELDANO 5 -- EL HOSTIL: lo que rompe a proposito

**Estado: ⛔ CERO.**

Todo lo que se ha probado hasta hoy son programas **que alguien escribio para
que funcionaran**. Eso es la mitad facil.

| criterio | que busca |
|---|---|
| **fuzz del lexer y el parser**: bytes al azar, ficheros cortados a la mitad, sangrias imposibles | que **no haya panico**. Un compilador que revienta con un fichero raro no es utilizable |
| todo mensaje de error tiene sus **cuatro partes** | el contrato de la seccion 8 se prueba en unos pocos avisos, no en todos |
| ningun aviso **miente sobre su causa** | es lo que casi pasa con el destino de trampa unico, y lo que si pasaba con `E0030` en los parametros |
| un fuente de **cero bytes**, uno de un solo salto de linea, uno sin salto final | los tres bordes clasicos |

⚠ **Y el criterio de este peldano no es "no falla": es "falla bien".** Un
compilador que rechaza un fichero raro con un mensaje claro esta aprobado. Uno
que revienta, no -- aunque el fichero fuera basura.

---

## 7. PELDANO 6 -- EL TIEMPO: cuanto cuesta, medido

**Estado: ⛔ CERO, y es el que mas se presta a mentir.**

⚠ **Este peldano NO puede empezar antes del 3.** Medir en un emulador da un
numero que no significa nada: el emulador no tiene cache, ni prediccion de
saltos, ni ejecucion fuera de orden -- y las comprobaciones anti-UB son baratas
**exactamente por eso**.

| criterio | contra que se compara |
|---|---|
| lo que cuestan las reglas | el mismo programa con `suma_circular` (sin comprobar) |
| lo que cuesta la coma flotante en registros normales | la version que reparta registros de coma flotante, el dia que exista |
| lo que cuesta un `.bex` de INTI | el mismo programa en BMO C |

★★ Y la frase que este peldano tiene que poder decir **no es** *"va como
ensamblador"* -- eso es el mito de 13e. Es: **"comprobar cuesta X%, medido en
este CPU, contra este programa"**. Un porcentaje sin las tres cosas no vale nada.

---

## 8. PELDANO 7 -- EL AJENO: alguien que no lo escribio

**Estado: ⛔ CERO, y es el unico que no puedo hacer yo.**

| criterio | que mide |
|---|---|
| alguien escribe un programa **sin preguntar** | si los mensajes bastan |
| alguien porta un programa de C a INTI | si la biblioteca cubre lo que hace falta |
| alguien lee un `.inti` que no escribio y **dice que hace** | la promesa de la sintaxis |

⚠ **Sin este peldano, "estricto para facilitar" es una opinion.** Los otros seis
los puede aprobar el que escribio el lenguaje; este no.

---

## 9. EL ORDEN, y por que no se puede reordenar

```text
   1 CORPUS   ->  sin el, los demas miden contra un veredicto que miente
   2 EMULADOR ->  sin el, el metal no dice si el fallo es del CPU o del emisor
   3 METAL    ->  sin el, medir tiempo da numeros de un emulador
   4 TAMANO   ->  sin el, lo hostil prueba programas de once lineas
   5 HOSTIL   ->  sin el, el ajeno se estrella con el primer fichero raro
   6 TIEMPO   ->  sin el, no se puede decir lo que cuesta
   7 AJENO    ->  y sin el, todo lo anterior lo aprobo quien lo escribio
```

★ **El 3 es el que bloquea de verdad**, y es el unico que no depende de escribir
mas codigo: depende de arrancar una maquina.

---

## 10. LO QUE HOY SE PUEDE AFIRMAR, y lo que no

Sin dulce, que era la peticion:

| se puede decir | ✅/⛔ |
|---|---|
| *"el compilador emite lo que dice emitir"* | ✅ 61 de 61 nombres, comprobado |
| *"no hay comportamiento indefinido en las reglas que llegan a bytes"* | ✅ 3 de 4, corriendo |
| *"el mismo fuente da el mismo bit"* | ✅ y hay un test |
| *"entre el fuente y la instruccion no hay nadie"* | ✅ y hay un test |
| *"INTI corre en BMO-X"* | ⛔ **nunca ha corrido en un CPU** |
| *"INTI sirve para un programa de verdad"* | ⛔ **once lineas es el record** |
| *"INTI cuesta un 1% por no tener UB"* | ⛔ **no esta medido en ningun sitio** |
| *"INTI es facil"* | ⛔ **nadie de fuera lo ha escrito nunca** |

★★★ **Los cuatro primeros son mas de lo que la mayoria de los lenguajes nuevos
puede decir a esta altura. Los cuatro ultimos son los que deciden si esto es un
lenguaje o un ejercicio.**
