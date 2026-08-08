# VERDAD.md -- que debe hacer BMO C, y por que

> **Quien manda, en orden:**
> 1. **BMO-X en el Ryzen.** El metal. Si el metal dice una cosa, esa es la verdad.
> 2. **Este documento.** Lo que el lenguaje *debe* hacer, escrito antes de mirar
>    ningun resultado.
> 3. **El emulador** (`bmo_lower::emu`). Una herramienta, y nada mas.
>
> Cuando el emulador y este documento no cuadran, **se arregla el emulador**.
> Ya ha pasado dos veces en un solo dia (ver *Mentiras del emulador*), y las dos
> veces el compilador estaba bien y el banco de pruebas mandaba a buscar el bug
> donde no estaba.

Este fichero se actualiza **a la vez que el codigo**. Una caracteristica sin
fila aqui es una caracteristica que nadie sabe verificar.

---

## Como se lee una fila

| Campo | Que es |
|---|---|
| **Se escribe** | El C, tal cual |
| **Debe salir** | La salida EXACTA, byte a byte |
| **Por que** | La regla del lenguaje, y el bug que evita |
| **Donde** | El test que lo ejecuta, y si esta visto en metal |

`✅ metal` = confirmado en el Ryzen con foto. `⏳ metal` = pasa en el emulador,
falta el hardware. **`⏳` no es "probablemente bien"**: es "nadie lo ha visto".

---

## 1. Lo basico -- confirmado en el Ryzen

Esto salio en la foto del 2026-07-31, ejecutando `c/holac.bex` desde el shell
de Ring 0. Es la referencia: si algun dia cambia, algo se rompio.

```
holac.bex> hola desde C en el Ryzen
holac.bex> suma 1..10 = 55
holac.bex> 42-100=-58  100/7=14  100%7=2
holac.bex> fase: calculo
holac.bex> cadena=viva hex=beef
holac.bex> C termino ok
```

| Se escribe | Debe salir | Por que | Donde |
|---|---|---|---|
| `for (i=1;i<=n;i=i+1)` | `suma 1..10 = 55` | Un salto hacia atras con desplazamiento sin parchear deja el bucle en una vuelta | `hola_example...` ✅ metal |
| `42 - 100` | `-58` | Los no conmutativos emitian `b - a` | `non_commutative...` ✅ metal |
| `100 / 7`, `100 % 7` | `14`, `2` | Igual | ✅ metal |
| `switch` con `default` | `fase: calculo` | Entraba siempre por el primer caso | ✅ metal |
| `printf("%s", "viva")` | `cadena=viva` | Las cadenas viven en otra seccion y el cargador la pone en la pagina siguiente | ✅ metal |

---

## 2. La puerta -- `<bmo/bmo.h>`

| Se escribe | Debe salir | Por que | Donde |
|---|---|---|---|
| `__syscall(0, 0xFFFFFFFFFFFFFFFE, 6, 0x616C6F68, 0, 0)` | `hola` | Los argumentos van a `rdi/rsi/rdx/r10/r8`. Si uno cae en otro registro, no sale nada | `syscall_intrinseco...` ⏳ metal |
| `0xFFFFFFFFFFFFFFFE` | `fffffffffffffffe` | No cabe en `i64`; el lexer lo hacia **cero**, o sea la capability 0 | `hex_de_64_bits...` ⏳ metal |
| `__syscall` vs `__syscall_valor` | codigo en `rax`, valor en `rdx` | La puerta contesta dos cosas y en C un par no cabe en un registro de retorno | `syscall_valor...` ⏳ metal |

**Pendiente de metal:** ningun `.bex` de C ha ejecutado todavia un `__syscall`.
`c/scrollc.bex` es el primero. Lanzarlo **desde el shell de Ring 0** (desde el
compositor dira `la entrada es de otro proceso`, que es correcto).

Debe salir, con la entrada libre:

```
---- filas 52..59 [al dia] ----
  fila 052
  ... (ocho filas)
  fila 059
```

y moverse con rueda, RePag, AvPag, Inicio y Fin. `ESC` sale con `hasta luego.`

---

## 3. La entrada -- `<bmo/entrada.h>`

| Se escribe | Debe salir | Por que | Donde |
|---|---|---|---|
| `bmo_entrada_reclamar()` con el compositor vivo | `0` | Es **exclusiva**. Un programa que no lo comprueba lee ceros y parece un raton roto | `reclamar_la_entrada...` ⏳ metal |
| `bmo_entrada_rueda(e)` dos veces sin girar | `4` y luego `0` | **Consume.** Un acumulado obliga a restar, y el primero que lo olvide tiene un scroll que se mueve solo | `la_rueda_se_vacia...` ⏳ metal |
| Girar hacia atras | `-2`, no `4294967294` | Viaja como `i32` en complemento a dos dentro de un `u64` | `la_rueda_hacia_atras...` ⏳ metal |
| `bmo_entrada_tecla(e)` sin teclas | `-1` | Convenio de `getchar`. `0` es un byte valido | `las_teclas_salen...` ⏳ metal |

---

## 4. El preprocesador

| Se escribe | Debe salir | Por que | Donde |
|---|---|---|---|
| `#define DOBLE(x) ((x)+(x))` -> `DOBLE(21)` | `42` | Las macros con parametros se guardaban y **no se expandian jamas** | `una_macro_con_parametros...` |
| `#define ANCHO (760)` -> `ANCHO` | `760` | El parentesis **pegado** hace funcion; separado, no. Era el unico sitio de C donde el espacio manda, y se ignoraba | `un_parentesis_separado...` |
| `#define ANCHO 760` -> `printf("ANCHO=%d\n", ANCHO)` | `ANCHO=760` | **Dentro de una cadena no se sustituye.** Antes imprimia `760=%d` | `una_macro_no_se_expande...` |
| Una cabecera que hace `#define` | la constante vale lo que dice | `#include` **tiraba** los `#define` del fichero incluido | `una_cabecera_incluida...` |
| `SUMA(1,2,3)` con dos parametros | **error** con el nombre | Antes ni se detectaba | `invocar_una_macro...` |

★ **La trampa que esto destapo:** dos constantes sin expandir se volvian la
*misma* variable inventada (valor 0), asi que `if (t == REPAG)` era cierto
tambien para `AVPAG`. **Un cero inventado no da un fallo local: da coherencia
falsa.** Por eso un identificador desconocido ahora es un error y no un cero.

---

## 5. Listas de inicializacion

| Se escribe | Debe salir | Por que | Donde |
|---|---|---|---|
| `int a[4] = {10,20,30,40}` | `10 30 40` | No existia ninguna lista, ni esta | `una_lista_posicional...` |
| `struct P q = {.y = 7}` | `0 7 0` | **C99 section 6.7.9/21**: lo no mencionado vale CERO. Sin borrar antes, trae basura de la pila -- distinta en cada ejecucion | `lo_no_mencionado...` |
| `int b[5] = {[2] = 30, 40}` | `0 0 30 40 0` | Un designador **reposiciona el cursor** y lo siguiente sigue desde ahi. La que mas se olvida | `tras_un_designador...` |
| `{.x=1, .y=2, .x=9}` | `9 2` | El ultimo gana, y sale de emitir en orden | `si_un_campo...` |
| `char s[8] = "hola"` | `hola` y `s[4]==0` | Una cadena llena el array **byte a byte**. Guardar el puntero deja una direccion y basura detras | `una_cadena_llena...` |
| `char s[3] = "hola"` | **error** | Escribir uno de mas pisa lo de al lado | `una_cadena_que_no_cabe...` |

---

## 6. Structs por valor

**La ABI de agregados de BMO** (ver `codegen/agregados.rs`): argumento en
`techo(tamano/8)` ranuras consecutivas de la pila; retorno por puntero oculto en
`rdi` -- *todavia no implementado*. No se copia la clasificacion por *eightbytes*
de SysV porque aqui no hay registros de argumento que repartir.

| Se escribe | Debe salir | Por que | Donde |
|---|---|---|---|
| `q = p` con `struct P{int x,y,z;}` | `1 2 3` | Copiaba **ocho bytes**: uno de 12 se copiaba a medias | `asignar_un_struct...` |
| `q = p; q.y = 99;` | `p.y` sigue en `2` | Es una copia, no un alias | `la_copia_de_un_struct...` |
| `suma(p)` con `struct P` de 12 B | `6` | Empujaba **una palabra**: la funcion recibia el primer campo y basura | `un_struct_viaja_entero...` |
| `mezcla(7, p, 5)` | `707` | Un agregado ocupa DOS ranuras y **corre** al parametro de detras. Con `16+i*8` se leia desde la mitad del anterior | `un_struct_corre...` |
| `struct P haz()` | **error** con el nombre | Devolver por valor es un tercer mecanismo y no esta. Devolver 8 bytes de 12 seria mentir | `devolver_un_struct...` |

---

## 7. La entrada -- `getchar` y `scanf`

Emitidos **en linea** como `printf`: aqui no hay libc que enlazar.

| Se escribe | Debe salir | Por que | Donde |
|---|---|---|---|
| `getchar()` en bucle sobre `"hola\n"` | `[h][o][l][a]` | Un byte cada vez, en orden | `getchar_entrega...` ⏳ metal |
| 13 bytes tecleados de golpe | `13`, no `7` | La puerta entrega **hasta 7 de una vez y los CONSUME**. Sin buffer se perderian seis de cada siete y pareceria un teclado malo | `getchar_no_pierde...` ⏳ metal |
| Dos `getchar()` en sitios distintos | `xy` | El buffer es **uno** (global oculta). Si cada sitio tuviera el suyo, el segundo empezaria de cero | `dos_getchar...` ⏳ metal |
| `scanf("%d", &x)` con `-5` | `-5` | Sin el signo, la cuenta sale al reves sin una palabra | `scanf_lee_un_entero_negativo` ⏳ metal |
| `scanf("%s", s)` con `mundo` | `<mundo>` | Lleva su **cero final**: en C una cadena sin terminador no es una cadena | `scanf_lee_una_cadena...` ⏳ metal |
| `scanf("%d %d", &a, &b)` | **error** | Un `scanf` que ignora la mitad de su formato lee mal en silencio | `scanf_con_dos...` |

★ `getchar()` **nunca devuelve `EOF`**: una consola de BMO no se acaba, se
queda esperando. `while ((c = getchar()) != EOF)` gira para siempre -- hay que
cortar con `'\n'`. Es una desviacion real del C alojado, y esta aqui porque es
justo la que hace colgarse un programa portado sin mirar.

---

## 8. Los intrinsecos y la libreria `semantic/`

62 filas en `intrinsics.toml`. Cada una es una instruccion con sus bytes
exactos; `semantic/*.h` les pone tipo, nombre y manual encima.

**Hay una prueba que compila una llamada a CADA fila**
(`cada_intrinseco_de_la_tabla_compila`). No comprueba que los bytes sean los
correctos --eso lo dice el manual de Intel y esta en la fila-- sino que la fila es
**emitible**: un nombre de registro mal escrito falla ahi y no en metal.

### Lo que el emulador SI puede contestar

| Se escribe | Debe salir | Por que |
|---|---|---|
| `atomico_xchg(&c, 42)` con `c=7` | `7 42` | Devuelve **lo que habia**, no lo que se puso. Es lo que se escribe al reves sin notarlo |
| `atomico_cas(&c, 5, 9)` con `c=5`, luego `cas(&c,5,77)` | `5 9 9 9` | Cuando no cuadra **deja el valor y devuelve el de verdad** -- por eso se puede reintentar sin releer |
| `xadd` dos veces sobre `100` | `100 101 102` | Entrega el ANTERIOR: un contador que no da el mismo numero dos veces |
| `bits_ceros_derecha(0)` | `32` | `tzcnt` esta DEFINIDO en cero; `bsf` no |
| `bytes_al_reves(0x11223344)` | `44332211` | La red habla big-endian |

★ **Un no-op no siempre es mentira.** Una barrera en un interprete de un solo
hilo que ejecuta en orden **es** un no-op de verdad: lo que ordena ya estaba
ordenado. Por eso `0F AE` se modela.

### Lo que SOLO el metal puede contestar

`cr0` `cr2` `cr3` `cr4` `rdmsr` `wrmsr` `invlpg` `lgdt` `lidt` `ltr` `xgetbv`
`rdrand` `cpuid` `rdtsc` `monitor/mwait` `wbinvd`.

El emulador **da panic** con estas a proposito. Devolver `0` como si fuera el
valor de un MSR seria inventarse un dato -- y un emulador que inventa datos es
peor que uno que no los tiene. Se compilan (la matriz lo comprueba) y se
verifican en el Ryzen o no se verifican.

---

## 9. El COMPOSITOR -- lo que no es C pero se verifica igual

Esto no es del lenguaje, pero se mira en la misma foto y sin fila aqui nadie
sabria que esperar.

| Se hace | Debe salir | Por que | Estado |
|---|---|---|---|
| **F12** | Ventana verde `ESTRATOS // centro de datos` | Las teclas de funcion no producian nada: `hid_to_ps2` las traducia y se caian por el `_ => None` de `nav_key` | ⏳ metal |
| F12 otra vez | Se cierra y **lo de debajo vuelve entero** | La consola se pinta encima de la caja; borrarla ignorandolo dejaria un agujero con el fondo del escritorio | ⏳ metal |
| **Alt+Tab** (Alt izquierdo) | La ventanita con la lista y una senalada | `Ctrl+Alt` no vale: **es AltGr** y ya tiene dueno | ⏳ metal |
| Alt+Tab dos veces | Vuelve a donde estabas | La pila MRU se reordena **al soltar Alt**, no en cada Tab. Es lo que mas se implementa mal | ⏳ metal, 17 tests en `bmo_input::foco` |
| Escribir con Datos abierta | Las teclas van a **Datos**: la linea de Ejecutar **no cambia** | Se calculaba el foco y **nadie lo leia**: `es_para` no se llamaba ni una vez, asi que todo seguia cayendo en Ejecutar | ⏳ metal |
| ESC con Datos abierta | La cierra. Con Ejecutar delante, ESC sigue **borrando la linea** | Dos ventanas, dos respuestas a la misma tecla -- eso es tener foco | ⏳ metal |
| **Alt+M** | El modo cambia y **se dice** (en la ventanita, o en la linea de estado) | Sin tecla, `Fijo` y `Puntero` eran inalcanzables: tres modos y solo uno vivo | ⏳ metal |
| Clic en una ventana | Le da el teclado, **tambien en modo Fijo** | `click-to-focus`. Fijo impide que se lo TOMEN, no que se lo des | ⏳ metal |
| Alt+Tab **a Ejecutar con Datos abierta** | La caja **se pone delante** y se escribe viendo lo que se escribe | El foco arrastra el Z-order (nunca al reves). Sin esto, enrutar bien las teclas las mandaria a una linea tapada -- el mismo fallo del reves | ⏳ metal |
| F12 en modo **Fijo** | La ventana aparece **detras** y el teclado se queda en Ejecutar | Es la prueba de que abrir != enfocar, y de que `arriba` se calcula del foco y no de "la ultima que se pinto" | ⏳ metal |
| **Mover el raton por encima de Datos** | La ventana queda **intacta**: ni un agujero | Era real y se veia: el cursor se borraba repintando `color_escena`, que no sabe de ventanas nuevas y contestaba con el fondo del escritorio. Ahora se guarda lo de debajo (*save-under*, 640 B) | ⏳ metal |
| Ctrl+Alt esconde Ejecutar y luego Alt+Tab | **No** se puede ir a la escondida | Esconder es cerrar para el foco. Si no, escribes en algo invisible | ⏳ metal |

★ La ventana de datos dice **`escritura: CERRADA`** en rojo, y tiene que decirlo:
la transaccion existe y esta probada, pero nadie la ha cableado al dispositivo.
Si algun dia aparece en verde sin que se haya cableado, eso es el bug.

★ **Lo que hay que mirar y no tiene fila propia**: que el puntero **no parpadee
ni se vea palido**. Se quita al principio del fotograma y se pone al final, asi
que se dibujan solo los fotogramas que pintan algo -- si esa cuenta estuviera mal,
el cursor estaria ausente la mitad del tiempo y se notaria antes que nada.

## 10. Lo VERIFICADO en el Ryzen el 2026-08-02

| Se hizo | Salio | Lo que demuestra |
|---|---|---|
| Arrancar | **Escritorio limpio, sin panel del kernel encima** | Los demos ya no compiten por la pantalla. `init_hello` la reclamaba y al morir el kernel repintaba su panel sobre el escritorio |
| Teclear `ls` y Enter | La lista de directorios, **al momento** | ★ El `sfence`. Antes habia que mover el raton para que apareciera lo tecleado: el write-combining retenia los pixeles hasta que algo llenaba el bufer |
| El log de arranque | `protocolo=0x0` (teclado) - `protocolo=0x1 (INFORME: el aparato ignoro el BOOT)` (raton) | ★ El `GET_PROTOCOL` contesta y **confirma** por que el raton iba corrido un byte: esta en protocolo de informe y su informe lleva Report ID |
| `apk=N:0:0` | Perdidos en **0** | El aparcadero de eventos del xHC no pierde ninguno |
| `kev` subiendo, `ep=Running` | El teclado escribe y no se muere | El bucle de re-enumeracion que lo mataba esta cerrado |
| `reboot` desde la caja | Reinicia | La operacion de Ring 3 llega al puerto de E/S |

**Lo que sigue sin verificar y por que**: el raton se mueve pero con los ejes
cruzados (`x` deriva sola) -- falta decidir si sus desplazamientos son de 8 o de
16 bits, y para eso el driver ya registra **ocho bytes** del informe crudo. Y
`KIND_MEMORIA`: esta cableada de punta a punta pero **ningun programa la ha
llamado todavia** en metal.

---

## Mentiras del emulador (historico)

Cada una hizo **fallar codigo correcto** o **pasar codigo roto**. Se apuntan
porque la proxima se parecera a estas.

| Fecha | Que mentia | Como se vio |
|---|---|---|
| 2026-07-31 | `mov [mem], eax` escribia **8 bytes** rellenando de ceros. En un registro es correcto; en memoria destruye lo de al lado | `{.x=1,.y=2,.x=9}` daba `9 0`: la ultima escritura de `x` borraba la `y` |
| 2026-07-31 | El prefijo `0x66` (16 bits) no existia | Habria reventado con el primer campo `short`. Mina, no fallo |
| 2026-07-30 | `finalizar_syscall` ponia `rax=0` y no tocaba `rdx` | `read_line` habria girado para siempre |
| -- | Concatena las secciones; el cargador las pone en paginas separadas | Un `lea [rip+disp]` roto pasaba en verde |

**Como se caza una:** si un test falla y el razonamiento sobre el codigo emitido
dice que deberia pasar, **volcar el AST antes de tocar el compilador**. La sonda
que imprimio las `Escritura` resolvio en un minuto lo que llevaba media hora de
suposiciones.

---

## Lo que falta, y que debe salir cuando este

| Que | Que se espera | Por que importa |
|---|---|---|
| **Devolver structs** | `q = haz()` con el puntero oculto en `rdi` | Hoy se rechaza con motivo |
| **`scanf` de varias conversiones** | `%d %d` de una linea | Hoy se rechaza pidiendo partirlo |
| **Floats globales** y como argumento | Ruta `xmm` en los bordes | Diferido con error honesto |
| **Invocacion de macro en varias lineas** | Juntar las lineas antes de expandir | Hoy se dice, no se adivina |

---

## Para la proxima foto del Ryzen

1. `run c/holac.bex` -> las seis lineas de la seccion 1, exactas.
2. `run c/scrollc.bex` **desde Ring 0** -> la ventana de la seccion 2, y que
   RePag/AvPag muevan. **Es el primer `__syscall` de C en silicio.**
3. `c/pregc.bex` **desde la caja del compositor** -- y luego se escribe EN LA
   CAJA y se pulsa Enter. Debe preguntar tres veces y contestar:

   ```
   como te llamas? ... hola, <lo que escribiste>
   cuantos anos tienes? ... en 10 anos tendras <n+10>
   escribe algo y cuento sus letras: ... <n> letras
   listo.
   ```

   Es el circuito entero --el terminal escribe en la consola del hijo, el hijo la
   lee-- recorrido **por primera vez desde C**. Si el contador de letras sale
   corto, el buffer de `getchar` esta perdiendo los seis que sobran de cada
   paquete.
4. La fila `raton` de CABINA: `bmb=k+r+`, `apk=...:0:0`, y `kev=` y `raton ev=`
   subiendo **en el mismo arranque**.
