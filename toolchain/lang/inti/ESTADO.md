# ESTADO DE INTI -- las tres frases, y cual esta pagada

> Escrito el **2026-08-21**, despues de F5c y F5d, repasando el juego completo de
> documentos contra lo que existe.

Eddi define INTI con una frase de tres partes:

> *"INTI es inspiracion de Python en sintaxis, pero nivel de rendimiento de ASM,
> basicamente, pero fuera del syscall."*

Son **tres afirmaciones distintas**, se pagan por separado, y hasta hoy los
documentos las contaban juntas. Este fichero las separa y dice de cada una:
**que esta comprobado, con que, y que falta**.

La regla para entrar aqui es la del proyecto: *nada que compile y no haga lo que
dice*. Si una fila no tiene con que comprobarse, lo pone.

---

## 0. El resumen, para no tener que leer lo demas

| la frase | como se comprueba | estado |
|---|---|---|
| **sintaxis de Python** | leyendo. 40 sondas del censo, con su veredicto por delante | ✅ **hecho** |
| **rendimiento de ASM** | ⚠ **la frase esta mal planteada** -- ver 2.0. Lo que se quiere es CONTROL de ASM, y eso si | ✅ **reencuadrado** |
| **sin comportamiento indefinido** | las doce reglas, y **cuantas llegan a bytes** | ✅ **3 de 4** atrapan y corren |
| **estricto para facilitar** | que dos cosas que no se pueden operar juntas **no compilen** | ✅ **desde F6a** |
| **fuera del syscall** | contando la instruccion de la puerta en los bytes emitidos | ✅ **hecho, y es un test** |

Y el numero que resume el dia: **1.137 pruebas en verde** en INTI y en todo lo
que comparte tabla con el.

---

## 1. "Sintaxis de Python" -- hecho, y con corpus

Lo que se copio y lo que no esta en `13b` del maestro, y no se repite. Lo que
importa para este documento es **como se comprueba**, porque una afirmacion
sobre sintaxis no se mide con un cronometro:

- **40 sondas** en `censo/*.inti`, cada una con su veredicto escrito en la
  primera linea, para que la sonda y su expectativa no se puedan separar.
- El informe **entero** se compara contra una constante. No "las que pasan":
  todas, incluidas las que todavia no se pueden comprobar.

⚠ **Y lo que NO es**, dicho para que no se erosione: *inspirado en* no es
*compatible con*. La seccion 14 del maestro llama a eso *"la tentacion mas cara
del documento"*, y sigue siendo verdad.

★ La prueba de que la sintaxis no se disena por gusto: `para` era el nombre de
`hlt` en la tabla de la maquina y **no se podia escribir**, porque `para` es
palabra clave (`para cada x en xs`). Llevaba asi desde que se escribio la tabla.
Lo caza la matriz de conformidad de F5d, no una lectura.

---

## 2. "Nivel de rendimiento de ASM" -- la frase esta mal planteada

### 2.0 ⚠ Primero, la correccion, que es de Eddi (21-08)

> *"recuerda el nivel de rendimiento de ASM, que no se engane, porque fue mito:
> ya se demostro que Linux uso ASM y fue lento en el kernel."*

Tiene razon, y el maestro ya lo tenia escrito en 13.10 -- **el ensamblador no es
rendimiento, es control** -- mientras la portada seguia prometiendo lo otro.

Un `rep movsb` fue el camino lento durante una decada y hoy es el rapido: el
codigo que lo evitaba a mano se quedo atras **sin cambiar una linea**. El
ensamblador de ayer no sabe nada del silicio de manana.

★ Asi que la frase correcta no es *"rendimiento de ensamblador"* sino **"control
de ensamblador, con la sintaxis de Python"**. Y eso si se puede sostener, porque
son dos afirmaciones comprobables en vez de una que no lo es.

### 2.1 La pregunta que SI importa: hay alguien entre el fuente y la instruccion?

### 2.1 La pregunta estructural: hay alguien entre el fuente y la instruccion?

**Contestada, y es un test.** `el_bucle_de_pixeles_no_llama_a_nadie`:

```inti
funcion pinta(pantalla es bufer de natural32, cuantos es entero64, color es entero64)
    cambiante i = 0
    repite mientras i < cuantos
        crudo
            pantalla[i] = color
        i = i + 1
```

De ese bucle salen **cero llamadas y cero cruces de la puerta**. Ni despacho, ni
contador de referencias, ni una funcion por elemento.

★★ **Y ese es exactamente el techo que Python no puede levantar**, y no por
lentitud del interprete: alli `x + y` **es** una llamada, y lo seguiria siendo
compilado. Por eso el maestro dice que el AOT de PLENO daria 2-4x y no 50x.

### 2.2 La pregunta de velocidad: cuanto mas lento que ASM escrito a mano?

**Sin contestar, y ademas NO es la pregunta que decide nada** -- ver 2.0. Se deja
aqui porque alguien la va a hacer, no porque el proyecto dependa de ella.

**No se puede contestar aqui.** Pide el Ryzen y un metodo, y los
dos estan escritos en la seccion 13.5 del maestro -- pero *escrito* no es
*medido*.

Lo que si esta hoy, y es lo que hara que la medida signifique algo:

| se puede leer hoy | de donde sale |
|---|---|
| cuantas comprobaciones anti-UB lleva un programa | `ModuloIr::comprobaciones()`, y va a CABINA |
| cuantas instrucciones de maquina toca | `ModuloIr::instrucciones()`, y tambien |
| cuantos temporales viven en registro y cuantos en pila | `Emitido`, desde F3 |

⚠ **Sin esos numeros, medir el dia de manana daria un porcentaje sin contra que
compararlo.** Con ellos, *"cuesta un 1%"* se convierte en *"cuesta un 1% con
tantas comprobaciones y tantos accesos a pila"*, que es una frase que se puede
atacar.

### 2.3 Lo que se deja en la mesa a proposito, y hay que decirlo

- **La Regla 11 prohibe fundir la multiplicacion con la suma.** El silicio sabe
  hacer las dos de una vez y mas preciso; INTI no lo emite, porque el mismo
  fuente tiene que dar el mismo bit en cualquier maquina. Hay un test que lo
  vigila mirando los bytes.
- **La coma flotante vive en registros normales y solo cruza para la operacion.**
  Cuesta dos movimientos por operacion. A cambio, el asignador de registros, el
  marco y la convencion de llamada no cambian ni una linea. **No es la version
  rapida y no pretende serlo**: es la que se puede escribir entera hoy y medir
  manana.
- **Los registros preservados no se reparten.** Guardarlos cuesta dos
  instrucciones por funcion aunque no se use ninguna, y mientras haya llamadas
  el asignador se frena entero.

Las tres son decisiones, no deudas. Lo que seria deuda es no escribirlas.

---

## 3. "Fuera del syscall" -- hecho, y es la mas facil de comprobar

`un_programa_que_calcula_no_cruza_la_puerta`: se compila un programa con un
bucle y aritmetica, y **la instruccion de la puerta no aparece en los bytes**.

El numero que hay detras esta en el maestro y es el que decidio la arquitectura
entera: **969 ciclos** cuesta cruzar la puerta, contra **20** una llamada. `2+2`
no tiene ninguna autoridad que arbitrar.

★★ Y la otra mitad, que es la que hace la frase interesante: **fuera del syscall
no quiere decir sin acceso al sistema**. El mismo compilador emite un programa
sin puerta y uno con puerta, y la diferencia es **una linea del fuente**:

```inti
usa bmo
```

No una bandera del compilador, no una palabra clave. Una fila de `modulos.toml`
-- y quitar esa fila apaga la puerta sin tocar una linea de Rust. Hay un test que
comprueba las dos direcciones.

---

## 4. LA LIBRERIA DE LA MAQUINA -- lo que F5d arreglo, y lo que dejo dicho

Esta seccion existe porque era el agujero mas grande del proyecto y **no estaba
en ningun documento**, ni siquiera como pendiente.

### 4.1 Lo que estaba roto

`arch/x86_64/inti.toml` llevaba desde F2b con **61 nombres** -- los puertos, los
registros de control, las atomicas, las cuentas de bits -- y **ninguno llegaba a
un byte**:

```text
   el descenso     no generaba `Instr::Metal`  ->  bajaba a una LLAMADA
   el emisor       `Instr::Metal { .. } => {}` ->  no emitia nada
```

Asi que `lee_reloj()` compilaba, pasaba el analisis de nombres --el nombre esta
en la tabla--, pasaba el de perfiles, pasaba el gate, y **saltaba a un simbolo
que no existe**.

★★★ Es el fallo que este proyecto persigue desde el principio -- *la pieza que se
calcula bien y no la lee nadie* -- y esta vez con cuatro capas de por medio.

### 4.2 Como se comprueba ahora, para que no vuelva

**La matriz de conformidad**: se compila una llamada a **cada** nombre de la
tabla y se exige que los bytes de su instruccion aparezcan en el binario. El
comentario de `Intrinsics::names()` lo llevaba pidiendo desde que se escribio:

> *"sin poder recorrerla, una fila con el nombre de un registro mal escrito no
> falla hasta que alguien la usa -- y 'alguien la usa' en una tabla de driver
> puede ser dentro de seis meses y en metal"*.

La matriz encontro dos fallos reales el primer dia:

| fallo | por que nadie lo habia visto |
|---|---|
| `para` (=`hlt`) **no se puede escribir**: es palabra clave | nadie habia compilado nunca un nombre de esta tabla |
| `da_la_vuelta` lo promete `usa binarios` y la maquina **no lo tenia** | el modulo portable prometia algo que esta maquina no sabia traducir |

Y ademas: **lo que no se puede emitir ya no se calla**. Va a `Emitido::sin_emitir`
con su motivo, y de ahi a CABINA como fallo. Un intrinseco mudo no rompe la
compilacion -- el resto del programa esta bien -- asi que sin esa lista la unica
senal seria el binario haciendo otra cosa en metal.

### 4.3 ⚠ HASTA DONDE LLEGA EL EMULADOR, y es la cifra incomoda

De los **61 nombres**, el emulador puede ejecutar **25**. Los otros **36** solo
se pueden verificar en el Ryzen.

```text
   corren aqui        25   aritmetica, bits, barreras, las que paran
   solo en metal      36   registros de control, MSR, puertos, cpuid, azar
```

**No es un fallo del emulador: es su regla**, y esta escrita en `VERDAD.md`:

> devolver `0` como si fuera el valor de un MSR seria inventarse un dato, y un
> emulador que inventa datos es peor que uno que no los tiene

★ Y la linea no es "de bajo nivel": es **te estoy devolviendo un valor que me
acabo de inventar?** Por eso `cli` y `sti` SI se modelan --aqui no hay
interrupciones, asi que apagarlas es un no-op de verdad-- y `rdmsr` no.

Los 36 estan **escritos con nombre** en la constante `SOLO_EN_METAL`, y el test
compara la lista entera. Esa es toda la diferencia entre *pendiente* y
*olvidado*: si manana una que hoy corre deja de correr, el test no dice "algo
cambio", dice cual.

---

## 5. LA PORTABILIDAD, que dejo de ser una promesa el 21-08

`medidas.toml` llevaba escrita una tesis desde que nacio: *"dos maquinas
distintas pueden dar disposiciones distintas SIN QUE EL COMPILADOR CAMBIE"*.

Nadie la comprobaba. Ahora la comprueba `tests/segunda_maquina.rs`, dandole al
compilador la tabla de una maquina que no existe:

```text
   registro Enlace          64 bits          32 bits
      antes  es bufer         @0  (8)          @0  (4)
      luego  es bufer         @8  (8)          @4  (4)
      marca  es natural8     @16  (1)          @8  (1)
                            medida 24        medida 12
```

La mitad, y **ni una linea de Rust distinta**. Y no se queda en la disposicion:
el descenso emite lecturas de 8 bytes con una tabla y de 4 con la otra.

★ **La diferencia con `agnostico.rs`:**

```text
   agnostico.rs        nadie ESCRIBIO la linea que ataria el compilador
   segunda_maquina.rs  y ademas, cambiar la tabla CAMBIA lo que emite
```

⚠ Y lo que **no** dice: que INTI compile para 32 bits. No compila. Lo que decide
es si portar sera *escribir un emisor* o *desenterrar ochos repartidos por el
compilador*, que es la diferencia entre un mes y un ano. Salio lo primero.

---

## 6. Lo que falta, en orden y con el motivo

| | que | por que va ahi |
|---|---|---|
| ~~1~~ | ~~**La foto del Ryzen**~~ | ✅ **HECHA (22-08).** M1, M2 y M3 corrieron en el Ryzen 5 5600X: `reglas`, `bits` y `atomicas` salieron a CERO y el informe se guardo en disco. Ver `PLAN_DE_PRUEBAS.md` §4 |
| 2 | **PLENO** | ⏳ **en marcha (23-08)**: `texto` mide, `numero` tiene disposicion --16 bytes, coeficiente `entero64` + escala-- y la puerta de `disposicion` esta abierta. Falta lo que vive en EJECUCION: monton, contador de referencias, y que `[bytes] llegan` admita `pleno` |
| 2b | **Los tipos de retorno** | `si hay_algo()` no se comprueba porque el tipo que devuelve una funcion no se resuelve todavia. ⭐ **Y desde el 23-08 es la fila que mas desbloquea**: la deduccion ya cubre el constructor, la copia y el literal de texto, y `x = f()` es el siguiente escalon |
| 3 | **La Regla 2** | ⏳ **no es deuda del emisor**: un `bufer` es una direccion y no lleva su longitud, asi que no hay contra que comprobar. Nace con `lista de T`, que si la lleva -- o sea, con el 2 |
| 4 | **Congelar y tareas** | el eslabon que rompe el GIL |
| 5 | **El REPL** | el ultimo a proposito: *un interprete no puede escribir un driver* |

★★★ **La Regla 12 y la Regla 3 ya no estan en esta lista** (21-08). De las cuatro
comprobaciones, **tres llegan a bytes y corren**.

Y el arreglo no estaba donde el diagnostico decia. El emisor tenia escrito el
motivo correcto --*"piden mirar un operando ANTES de la operacion"*-- y por eso
sobrevivio: era un diagnostico, no una causa. **La causa estaba en la IR**, que
las colocaba detras de la operacion. Movidas delante y con el operando que hay
que mirar, el emisor las emite en cuatro instrucciones.

⚠ Y salieron **dos fallos del emulador** que ningun lenguaje habia notado, los
dos de la misma familia: el banco decia que si y el Ryzen habria dicho que no.

| lo que hacia | lo que hace el silicio |
|---|---|
| `cvttsd2si` **saturaba** (1e30 daba el entero mayor, NaN daba cero) | devuelve el mas negativo como centinela, para los dos |
| `imul` **no ponia banderas** | enciende `cf` y `of` si el producto no cabe |

El segundo es hermano exacto del hueco de `add` que se encontro el 19-08, con
las mismas palabras: *ningun lenguaje de BMO lo habia notado porque ninguno
emitia un `jo`*. BMO C no comprueba el desbordamiento; INTI si, y su Regla 1
salia verde aqui y habria atrapado en metal.

★★★ **Y el 22-08 el silicio lo confirmo.** La sonda pregunto las tres reglas en
el Ryzen y la linea `reglas` salio a **0x00**: desborde devolvio 1001, entre cero
1003 y conversion 1012. **Emulador arreglado y metal dicen lo mismo**, que es la
unica forma de que *"INTI no tiene comportamiento indefinido"* deje de ser una
frase sobre un emulador que escribimos nosotros.

---

## 7. Los documentos, y cual miente

Repaso del juego completo, que era la peticion:

| documento | estado |
|---|---|
| `README.md` | ✅ al dia (la tabla de fases llegaba hasta F3b) |
| `GRAMATICA.md` | ✅ al dia -- 14d nuevo: `flotante64`, el NaN, la Regla 11 |
| `ARQUITECTURA.md` | ✅ corregido -- la seccion 4 daba por futuro `nombres`, `ir` y el emisor |
| `CENSO.md` | ✅ 40 sondas (eran 38) |
| `REGLAS.md` | ✅ sigue siendo verdad; la 11 y la 12 ya tienen sonda |
| `LINAJE.md` | ✅ sigue siendo verdad |
| `MONTON.md` | ✅ sigue siendo verdad |
| `docs/maestro/INTI_MAESTRO.md` | ✅ al dia -- F5a-d marcadas, y la seccion 7 lleva la sonda de la segunda maquina |
| **este** | el que faltaba: **las tres frases, separadas** |
