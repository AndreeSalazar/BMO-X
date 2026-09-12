# PLAN DEL DIRECTOR -- el censo, lo que gasta, y por que

> Pedido por el dueno el 2026-09-12: *"modular el DIRECTOR por completo, que
> cumplen sus funciones y consumo de wats y por que razones"*.
>
> Se empieza por el CENSO y no por el codigo, que es la regla de la casa:
> **preguntar por el SITIO antes de escribir**. Es el hermano de
> `PLAN_CODEGEN.md`, escrito el mismo dia para el otro monolito.

---

## 0. LA PRIMERA CONCLUSION CORRIGE A QUIEN LA BUSCABA

Empece contando con `wc -l` y salieron **dos ficheros sobre las 1.000 lineas**:
`commands/reports.rs` (1.410) y `main.rs` (1.261). Parecia el trabajo obvio.

**Y con el metro de la casa es falso.** L6a cuenta lineas **DE CODIGO** desde el
2026-08-24, y el motivo esta escrito en la propia ley: la regla mordio a quien la
cumplia, porque *"un comentario no comparte estado con nadie"*.

```text
                      CODIGO   TOTAL   %doc
   commands/reports.rs   968    1410    32%   <- el unico cerca del limite
   commands/system.rs    604     900    33%
   scene/consola.rs      600     979    39%
   main.rs               506    1261    60%   <- no es un monolito: es un ENSAYO
   ---------------------------------------
   LOS 72 FICHEROS    12.661  24.269    48%
```

* **48% documentacion medida**, contra el 36% del arbol entero. El DIRECTOR es la
parte mejor explicada del repo, y `wc -l` lo castigaba por eso.

*** O sea que **el DIRECTOR no tiene un problema de tamano**. Lo que tiene es lo
que no declaraba.

---

## 1. LO QUE NO DECLARABA: 69 de 71 ficheros callaban lo que gastan

R21 (L6h) exige `[consumo]` a los 180 ficheros de Ring 0. Fuera del kernel
**solo validaba a quien quisiera declarar**, y el porque estaba escrito en
`contrato_consumo.py`:

> *"Solo se juzga a quien declara: exigirlo a todo el compositor seria un muro,
> no un letrero."*

Era verdad el 11-09 --el DIRECTOR tenia DOS sellos de 71-- y es lo que la
excepcion tapaba: **no que exigirlo fuera malo, sino que el trabajo no se habia
hecho**. Hecho el trabajo, la excepcion caduca.

```text
   [x] los 72 ficheros declaran [consumo], con su razon y quien los llama
   [x] R21 lo EXIGE en el arbol del DIRECTOR (CONSUMO_EXIGIDO_FUERA)
   [x] la autoprueba tiene la fila que prueba que la excepcion esta CERRADA
   [x] y cada build lo IMPRIME, que es la mitad que no se comprueba sola
```

### *** EL RESULTADO, y es la respuesta a la pregunta

```text
   R21 L6h: en reposo gastan 6 de 72 ficheros de director
           LATE     main.rs
           APARATO  commands/disco.rs, commands/red.rs, commands/system.rs,
                    scene/sound.rs
           APAGA    desktop/tick/roja.rs
```

**SEIS de 72.** Es la misma forma que Ring 0 --nueve de 180-- y dice lo mismo: la
eficiencia del escritorio empieza por esos seis, no por los otros 66.

Y el **por que** de cada uno, que es lo que se pidio:

| fichero | clase | por que gasta |
|---|---|---|
| `main.rs` | LATE | ARMA el bucle: mil vueltas por segundo sobre el LATIDO, y en cada una sondea la entrada |
| `desktop/tick/roja.rs` | APAGA | es **donde duerme**: el latido, o 8 ms por vuelta en reposo. Si se equivoca, se queda la maquina |
| `commands/system.rs` | APARATO | abre el tubo del audio, PARA nucleos, reinicia. Tres cosas que dejan el hardware distinto |
| `commands/disco.rs` | APARATO | `trim` le MANDA al SSD borrar bloques |
| `commands/red.rs` | APARATO | `red rx` ARMA el anillo de recepcion y lo deja armado |
| `scene/sound.rs` | APARATO | reclama `KIND_AUDIO`, que es EXCLUSIVO, mientras la ventana este abierta |

* **Y los otros 66 son NADA con una razon comprobable, no con una frase hecha.**
Cada sello dice QUIEN lo llama, que es lo que convierte "NADA" en una afirmacion
que se puede desmentir: una escena pinta cuando el compositor se lo pide; un
comando, cuando el dueno lo escribe; un fichero de `keys/` o `mouse/`, solo si
hubo una tecla o el raton se movio.

### Las TRES que son MEZCLA, dicho por delante

L6h manda que un fichero declare UNA clase, y que hasta que se parta declare **la
PEOR** y lo diga con `[!] MEZCLA`. Tres lo hacen ahora, y nadie habia usado esa
salida en todo el arbol -- los tres cortes del primer dia en Ring 0 se partieron
de verdad:

```text
   commands/system.rs   los informes (se piden) | audio_tubo, smp_parar, reiniciar
   commands/disco.rs    el informe del disco    | `trim`
   commands/red.rs      los contadores          | `red rx`, que ARMA la tarjeta
```

*** Los tres tienen la misma forma: **un informe que solo lee, y una orden que
cambia el aparato**. La costura esta a la vista y el corte va por ahi.

---

## 2. EL NUMERO DE LOS VATIOS, QUE NADIE HABIA MEDIDO

El presupuesto de este bucle esta escrito en `main.rs`, y todo el se apoya en una
sola cifra:

```text
   "una vuelta en vacio cruza NUEVE puertas"
   9 x 969 ciclos = 8.721 = 2,36 us por vuelta
      a    250 vueltas/s ->  0,06 % del CPU
      a  60000 vueltas/s -> 14,14 %
```

**Esas nueve eran una cuenta a mano, hecha leyendo el bucle.** Y el instrumento
para medirlas existia, y ya llegaba a Ring 3:

```text
   meter.rs             cuenta `DOORS` desde el 2026-08-16
   INFO_SYSCALL_CUENTA  lo sirve a Ring 3 (campo 0x2F)
   medida/coste         lo lee desde entonces, y le dice `trafico_total`
   el DIRECTOR          nunca pregunto
```

*** Es el mismo patron que la seccion `Resources` del BEF: **estaba en el formato
y nadie la escribia**. No faltaba el instrumento, faltaba llamarlo.

```text
   [x] `Tick::trafico_x10` -- puertas por vuelta, por diez
   [x] se lee UNA VEZ POR SEGUNDO y no por vuelta: en cada vuelta seria subir
       un 11% las puertas para poder contarlas, o sea medir el termometro. Una
       vez por segundo sobre ~9.000 puertas es el 0,01%
   [x] sale en la tabla `consumo`, en una subregla `escritorio` propia, con
       las vueltas, las que pintan, los ciclos/vuelta y la BARRA de % de CPU
```

### Y lo que ese numero NO es, dicho donde se lee

`meter::doors()` es un contador **global del kernel**. Asi que la cifra es el
trafico de TODA la maquina repartido entre las vueltas de ESTE bucle:

```text
   escritorio solo    es suyo  -> se compara con las 9 contadas a mano
   con una app        son las suyas TAMBIEN -> sube sin que el compositor
                      haya cambiado nada
```

Por eso se llama **trafico** y no "mis puertas", que es el vocabulario que ya
puso `medida/coste`. Un nombre que promete menos de lo que mide es como se lee
mal un instrumento bueno.

---

## 3. EL PRIMER CORTE, y por que NO es de tamano

`commands/reports.rs` tenia **968 lineas de codigo contra el limite de 1.000**, o
sea 32 de margen. Y dentro llevaba DOS clases de coste:

```text
   la tipografia   section, label, campo, fila, fila_de, fila_mili,
                   fila_barra, fila_cero, subregla, envolver
                   -> recibe un numero y lo coloca.               [cuesta] NADA
   los informes    nueve report_*, que PREGUNTAN A LA MAQUINA
                   -> 91 puertas, y alguien decide con lo que salga.
                                                                  [cuesta] DATO
```

** Dos clases en un fichero es exactamente lo que L6e llama mal cortado, y el
corte va por donde cambia el coste. Que devuelva margen es la **consecuencia**, no
el motivo -- igual que en `codegen/bex.rs` el mismo dia.

```text
   [x] commands/tabla.rs     nace con [carril] VERDE, [consumo] NADA,
                             [cuesta] NADA, [riesgo] ESPEJO
   [x] commands/reports.rs   968 -> 803 lineas de codigo (197 de margen)
   [x] y gana sus sellos     [carril] AMARILLO, [cuesta] DATO,
                             [riesgo] AJENO SILENCIO
   [x] cuatro modulos que pedian la tipografia a `reports` ahora se la piden
       a quien la tiene: `cabina`, `guia`, `disco`, `red`
```

---

## 4. LOS CORTES QUE QUEDAN, por el mismo criterio

No por tamano: por lo que cada trozo cuesta si se equivoca (L6e) y por la clase
de consumo que lleva dentro (L6h).

```text
   [ ] `commands/system.rs`, 604 lineas de codigo y [!] MEZCLA. Los informes
       --`cpu`, `mem`, `info`, la autopsia-- contra las TRES ordenes que cambian
       la maquina. Es el MAYOR de los tres mezclados y el unico que PARA nucleos
       -- commands/system.rs

   [ ] `commands/disco.rs` y `commands/red.rs`, la misma forma y mas pequena:
       el informe se queda, la orden sale. Salen los dos o ninguno -- partir uno
       y no el otro deja dos formas de decir lo mismo
       -- commands/disco.rs, commands/red.rs

   [ ] `scene/consola.rs`, 600 lineas de codigo y 39% de documentacion. Es el
       segundo mas grande y nadie lo ha examinado por clase todavia
       -- scene/consola.rs

   [ ] `scene/surface.rs`, 437 lineas. Cruza `tomar_prestado_de` en CADA vuelta
       del bucle, y por L6h es NADA --late el que ARMA, no el que es llamado--.
       La etiqueta es correcta y pierde un dato: **cuesta una puerta por vuelta
       en reposo**. Ver el apartado 5
       -- scene/surface.rs
```

---

## 5. EL HUECO DE LA LEY QUE ESTE TRABAJO ENCONTRO

L6h pregunta UNA cosa: *"en reposo, este codigo corre?"*. Y su regla corta la
lista para que se pueda leer: **late el que ARMA, no el que es llamado**. Sin
ella, medio arbol saldria LATE por vivir dentro de un bucle.

*** En Ring 0 eso funciona porque hay muchos bucles. **En el DIRECTOR hay UNO**, y
entonces la regla esconde justo lo que mueve los vatios:

```text
   scene/surface.rs     NADA, correcto -- y cruza 1 puerta en cada vuelta
   desktop/keys/mod.rs  NADA, correcto -- y sondea la entrada en cada vuelta
   main.rs (autopsias)  NADA, correcto -- y pregunta `autopsia_total` siempre
```

** Los tres son NADA de verdad: si el bucle no los llama, no corren. Y los tres
suman puertas en una vuelta en la que no ha pasado nada.

> **El eje del consumo contesta "corre en reposo?" y no contesta "cuanto cuesta
> cada vuelta".** En Ring 0 esas dos preguntas casi coinciden; en Ring 3, con un
> solo bucle, no.

* **Y no se inventa una clase nueva.** El vocabulario de L6h es cerrado a
proposito, y ampliarlo por un caso es como una lista de cien deja de leerse. Lo
que falta no es una etiqueta: es **la medida**, y esa es `trafico_x10` del
apartado 2. La etiqueta dice quien corre; el numero dice cuanto cuesta.

---

## 6. LO QUE **NO** HAY QUE HACER, dicho por delante

```text
   [!] NO optimizar el bucle todavia. `OPTIMIZACION_MAESTRO.md` es tajante: es
       LO ULTIMO. Y aqui hay un motivo extra: el presupuesto se apoyaba en una
       cuenta a mano, asi que primero se LEE el trafico en el metal y despues
       se decide. Optimizar contra una cifra no medida es adivinar con numeros

   [!] NO tocar `desktop/tick/roja.rs` sin leer su cabecera entera. Es el unico
       APAGA, cuesta MAQUINA, y sus DOS averias conocidas salieron de ahi: una
       se quedo el nucleo entero y la otra tumbo el kernel. Ninguna vino de
       contar mal

   [!] NO sellar `userland/src` a la vez. Una regla que se estrena en rojo se
       desactiva en una semana. Entra cuando este sellado, no antes

   [!] Y los tres ficheros GENERADOS --`scene/gato.rs`, `scene/calc_gen.rs`,
       `scene/tema_gen.rs`-- llevan su sello puesto en el GENERADOR, no en la
       salida. Editarlos a mano es escribir una verdad que la siguiente
       compilacion borra, y eso ya casi paso el 12-09
```

---

## 7. EL DATO DEL METAL QUE CIERRA ESTO

Nada de lo de arriba es una medida: es el cableado para poder tomarla. El
arranque que la toma:

```text
   escribir `consumo`       la subregla `escritorio`, con trafico y % de CPU
   sin nada corriendo       el trafico ES del escritorio -> se compara con las
                            NUEVE contadas a mano
   con DOOM corriendo       sube, y la subida es de DOOM: es la prueba de que
                            el contador es de la MAQUINA y no de aqui
```

* Si el trafico en reposo sale **~9**, la cuenta a mano era buena y el
presupuesto de `main.rs` se sostiene. Si sale muy por encima, hay puertas en el
camino de reposo que nadie habia visto -- y entonces el apartado 5 deja de ser
una nota y pasa a ser el trabajo siguiente.
