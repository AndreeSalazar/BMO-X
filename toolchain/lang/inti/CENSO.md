# CENSO DE INTI -- 42 sondas, y el veredicto escrito POR DELANTE

> **F0.** 🟡 Ninguna sonda esta medida: **hoy no existe el compilador**. Lo que
> hace este documento es fijar **que tiene que pasar**, para que el dia que el
> lexer lea el primer fichero **el desacuerdo se vea solo**.

Es el metodo de `c-gen` y de `BRECHA.md` de BMO C, con una diferencia de orden
que conviene decir: alli el censo **midio** lo que ya existia; aqui **declara**
lo que va a existir. La propiedad que importa es la misma:

> El test compara el **informe entero** contra una constante. Arreglar o romper
> una casilla **hace fallar el test hasta que se actualiza el censo**. Un
> informe deducido de las fuentes envejece el dia que alguien toca las fuentes;
> este se actualiza solo.

Las sondas viven en `censo/*.inti`. Cada fichero lleva su veredicto en la
primera linea, asi que **la sonda y su expectativa no se pueden separar**.

---

## El numero

**42 sondas. 4 medidas de verdad** (las de perfil, desde F2a), y el resto
esperando a su fase.

| familia | sondas | que fija |
|---|---|---|
| `s` sintaxis | 5 | esqueleto, sangria, textos |
| `v` valores | 5 | el numero exacto, sin nulo, sin veracidad, sin conversion |
| `c` control | 5 | fijo/cambiante, comparar, los tres bucles |
| `f` funciones | 6 | registros, capturas, defectos congelados, sin herencia |
| `e` errores | 2 | las tres formas, y que ignorar no compila |
| `p` perfiles | 8 | la frontera llano/pleno, `crudo`, `en paralelo`, **y la puerta** |
| `r` reglas | 9 | las que se pueden ver desde el fuente |

---

## Sintaxis

| sonda | que fija | espera |
|---|---|---|
| `s01_esqueleto` | `perfil` + `principal` | **COMPILA** |
| `s02_sin_perfil` | no hay perfil por defecto | `E0001` |
| `s03_tabulador` | el tabulador no es una alternativa a los 4 espacios | `E0010` |
| `s04_texto` | comillas dobles, interpolacion, los cinco escapes | **COMPILA** |
| `s05_comilla_simple` | la comilla simple no existe | `E0011` |

## Valores

| sonda | que fija | espera |
|---|---|---|
| `v01_numero_exacto` | ★ `0.1 + 0.2` da `0.3` | **COMPILA** + salida `0.3` |
| `v02_division` | `/` divide, `entre` da cociente entero | **COMPILA** + `2.5` y `2` |
| `v03_sin_nulo` | no hay nulo: `quiza T` hay que mirarlo | `E0021` |
| `v04_sin_veracidad` | `si` exige `logico`; `if lista:` no existe | `E0040` |
| `v05_sin_conversion` | `"23" + 1` no se convierte solo | `E0022` |
| `v06_mezcla_clases` | ✅ **coma flotante y entero no se mezclan** -- son los mismos ocho bytes leidos con dos alfabetos | `E0022` |
| `v07_sin_veracidad_llano` | ✅ **una condicion es una pregunta**, no "algo que no es cero" | `E0040` |

## Control y nombres

| sonda | que fija | espera |
|---|---|---|
| `c01_fija` | sin `cambiante` no se reasigna | `E0030` |
| `c02_cambiante` | con `cambiante`, si | **COMPILA** |
| `c03_comparar` | `=` compara / `no es` / `es un` | **COMPILA** |
| `c04_bucles` | las tres formas de `repite` y el rango que excluye el final | **COMPILA** |
| `c05_muta_iterando` | ★ borrar mientras se itera **no compila** | `E0050` |

## Funciones y registros

| sonda | que fija | espera |
|---|---|---|
| `f01_funcion` | funcion con tipos, `devuelve`, `de` como llamada | **COMPILA** |
| `f02_defecto_congelado` | ★ el defecto se congela: la sorpresa 1 de Python no existe | **COMPILA**, la lista no se acumula |
| `f03_sin_closures` | ★ no hay funciones anidadas ni anonimas: el *late binding* no existe **por ausencia** | `E0101` |
| `f04_parametro_fijo` | un parametro no se cambia dentro | `E0033` |
| `f05_registro` | campos, defecto, construccion posicional y por nombre | **COMPILA** |
| `f06_sin_herencia` | no hay herencia | `E0100` |

## Errores

| sonda | que fija | espera |
|---|---|---|
| `e01_error_como_dato` | las tres formas: mirar, `o si no` valor, `o si no` bloque | **COMPILA** |
| `e02_ignorar_error` | ★★ **ignorar un error es error de COMPILACION** | `E0060` |

## Perfiles -- la frontera de la seccion 1.4 del maestro

| sonda | que fija | espera |
|---|---|---|
| `p01_llano` | un driver de verdad: puertos, `bits_y`, `crudo` | **COMPILA** |
| `p02_llano_sin_lista` | en `llano` no hay lista: crece, pide monton | `E0070` |
| `p03_llano_sin_numero` | en `llano` hay que decir el tamano | `E0020` |
| `p04_crudo_en_pleno` | `crudo` no existe en `pleno` | `E0071` |
| `p05_paralelo_mutable` | ★★ lo que cruza esta congelado, o no cruza | `E0080` |
| `p06_puerta` | ★★ la puerta se llama sin `crudo`: al otro lado hay un kernel que comprueba | **COMPILA** |
| `p07_puerto_sin_crudo` | un puerto **si** lo necesita: al otro lado no hay nadie | `E0072` |
| `p08_biblioteca_no_reservada` | ★ `escribe` e `invoca` **no son palabras clave**: se pueden redefinir | **COMPILA** |

## Las reglas que se ven desde el fuente

| sonda | regla | espera |
|---|---|---|
| `r01_desborde` | 1 -- desbordar atrapa | `E1001` en ejecucion |
| `r02_indice` | 2 -- indice constante fuera de rango **no compila** | `E0090` |
| `r03_division` | 3 -- dividir entre cero atrapa | `E1003` |
| `r04_sin_valor` | 4 -- ★ leer sin inicializar **no se puede escribir** | `E0031` |
| `r07_desplaza` | 7 -- desplazar de mas da cero, con aviso | **COMPILA** + `A2007` + `0` |
| `r09_tamanos` | 9 -- tamanos exactos | **COMPILA** |
| `r11_flotante` | 11 -- las cuatro operaciones y la conversion existen | **COMPILA** |
| `r11_bits_flotante` | 11 -- los bits sobre un flotante **no compilan** | `E0123` |
| `r12_conversion` | 12 -- flotante fuera de rango atrapa | `E1012` |

### ⚠ Las cinco reglas que NO tienen sonda aqui, y por que

Honestidad antes que casillas verdes: **cinco de las doce reglas no se pueden
comprobar leyendo un fuente**, y escribirles una sonda de fuente seria fingir
que estan cubiertas.

| regla | por que no cabe en F0 | donde se comprueba |
|---|---|---|
| 5 -- punteros colgantes | necesita el analisis de prestamos | **F2**, con un caso que debe fallar al compilar |
| 6 -- orden de evaluacion izquierda a derecha | hay que **ejecutar** y ver el orden de los efectos | **F3**, sonda con dos llamadas que imprimen |
| 8 -- sin alias estricto | hay que mirar el **codigo emitido**, no el fuente | **F3**, comparando bytes |
| 10 -- little-endian fijado | se ve en los **bytes serializados** | **F3** |
| 11 -- IEEE-754 estricto, sin FMA ni reasociacion | se ve en las **instrucciones emitidas** y en el bit del resultado | **F3**, contra el mismo calculo en el anfitrion |

Por eso la fase F3 del maestro existe y se llama *"las 12 reglas, con sus
sondas en verde"*: **siete se declaran hoy y cinco nacen alli**.

---

## Como se leera esto cuando exista el compilador

```text
   cargo test -p bmo-inti-front sonda_
```

Siete ejes, 42 casillas, y el informe **entero** comparado contra la constante
del censo -- el mismo mecanismo que le encontro a BMO C cuatro defectos sin
encender la maquina (el alineado de agregados, las cuatro operaciones que
miraban mal el bit alto, `<strings.h>` ausente y `fread` a la pila).

**Un fallo aqui no es "una sonda roja": es que el lenguaje y su contrato han
dejado de estar de acuerdo**, y uno de los dos esta mal.
