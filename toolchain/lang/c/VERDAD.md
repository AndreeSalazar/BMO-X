# VERDAD.md — qué debe hacer BMO C, y por qué

> **Quién manda, en orden:**
> 1. **BMO-X en el Ryzen.** El metal. Si el metal dice una cosa, esa es la verdad.
> 2. **Este documento.** Lo que el lenguaje *debe* hacer, escrito antes de mirar
>    ningún resultado.
> 3. **El emulador** (`bmo_lower::emu`). Una herramienta, y nada más.
>
> Cuando el emulador y este documento no cuadran, **se arregla el emulador**.
> Ya ha pasado dos veces en un solo día (ver *Mentiras del emulador*), y las dos
> veces el compilador estaba bien y el banco de pruebas mandaba a buscar el bug
> donde no estaba.

Este fichero se actualiza **a la vez que el código**. Una característica sin
fila aquí es una característica que nadie sabe verificar.

---

## Cómo se lee una fila

| Campo | Qué es |
|---|---|
| **Se escribe** | El C, tal cual |
| **Debe salir** | La salida EXACTA, byte a byte |
| **Por qué** | La regla del lenguaje, y el bug que evita |
| **Dónde** | El test que lo ejecuta, y si está visto en metal |

`✅ metal` = confirmado en el Ryzen con foto. `⏳ metal` = pasa en el emulador,
falta el hardware. **`⏳` no es "probablemente bien"**: es "nadie lo ha visto".

---

## 1. Lo básico — confirmado en el Ryzen

Esto salió en la foto del 2026-07-31, ejecutando `apps/holac.bex` desde el shell
de Ring 0. Es la referencia: si algún día cambia, algo se rompió.

```
holac.bex> hola desde C en el Ryzen
holac.bex> suma 1..10 = 55
holac.bex> 42-100=-58  100/7=14  100%7=2
holac.bex> fase: calculo
holac.bex> cadena=viva hex=beef
holac.bex> C termino ok
```

| Se escribe | Debe salir | Por qué | Dónde |
|---|---|---|---|
| `for (i=1;i<=n;i=i+1)` | `suma 1..10 = 55` | Un salto hacia atrás con desplazamiento sin parchear deja el bucle en una vuelta | `hola_example…` ✅ metal |
| `42 - 100` | `-58` | Los no conmutativos emitían `b - a` | `non_commutative…` ✅ metal |
| `100 / 7`, `100 % 7` | `14`, `2` | Igual | ✅ metal |
| `switch` con `default` | `fase: calculo` | Entraba siempre por el primer caso | ✅ metal |
| `printf("%s", "viva")` | `cadena=viva` | Las cadenas viven en otra sección y el cargador la pone en la página siguiente | ✅ metal |

---

## 2. La puerta — `<bmo/bmo.h>`

| Se escribe | Debe salir | Por qué | Dónde |
|---|---|---|---|
| `__syscall(0, 0xFFFFFFFFFFFFFFFE, 6, 0x616C6F68, 0, 0)` | `hola` | Los argumentos van a `rdi/rsi/rdx/r10/r8`. Si uno cae en otro registro, no sale nada | `syscall_intrinseco…` ⏳ metal |
| `0xFFFFFFFFFFFFFFFE` | `fffffffffffffffe` | No cabe en `i64`; el lexer lo hacía **cero**, o sea la capability 0 | `hex_de_64_bits…` ⏳ metal |
| `__syscall` vs `__syscall_valor` | código en `rax`, valor en `rdx` | La puerta contesta dos cosas y en C un par no cabe en un registro de retorno | `syscall_valor…` ⏳ metal |

**Pendiente de metal:** ningún `.bex` de C ha ejecutado todavía un `__syscall`.
`apps/scrollc.bex` es el primero. Lanzarlo **desde el shell de Ring 0** (desde el
compositor dirá `la entrada es de otro proceso`, que es correcto).

Debe salir, con la entrada libre:

```
---- filas 52..59 [al dia] ----
  fila 052
  … (ocho filas)
  fila 059
```

y moverse con rueda, RePag, AvPag, Inicio y Fin. `ESC` sale con `hasta luego.`

---

## 3. La entrada — `<bmo/entrada.h>`

| Se escribe | Debe salir | Por qué | Dónde |
|---|---|---|---|
| `bmo_entrada_reclamar()` con el compositor vivo | `0` | Es **exclusiva**. Un programa que no lo comprueba lee ceros y parece un ratón roto | `reclamar_la_entrada…` ⏳ metal |
| `bmo_entrada_rueda(e)` dos veces sin girar | `4` y luego `0` | **Consume.** Un acumulado obliga a restar, y el primero que lo olvide tiene un scroll que se mueve solo | `la_rueda_se_vacia…` ⏳ metal |
| Girar hacia atrás | `-2`, no `4294967294` | Viaja como `i32` en complemento a dos dentro de un `u64` | `la_rueda_hacia_atras…` ⏳ metal |
| `bmo_entrada_tecla(e)` sin teclas | `-1` | Convenio de `getchar`. `0` es un byte válido | `las_teclas_salen…` ⏳ metal |

---

## 4. El preprocesador

| Se escribe | Debe salir | Por qué | Dónde |
|---|---|---|---|
| `#define DOBLE(x) ((x)+(x))` → `DOBLE(21)` | `42` | Las macros con parámetros se guardaban y **no se expandían jamás** | `una_macro_con_parametros…` |
| `#define ANCHO (760)` → `ANCHO` | `760` | El paréntesis **pegado** hace función; separado, no. Era el único sitio de C donde el espacio manda, y se ignoraba | `un_parentesis_separado…` |
| `#define ANCHO 760` → `printf("ANCHO=%d\n", ANCHO)` | `ANCHO=760` | **Dentro de una cadena no se sustituye.** Antes imprimía `760=%d` | `una_macro_no_se_expande…` |
| Una cabecera que hace `#define` | la constante vale lo que dice | `#include` **tiraba** los `#define` del fichero incluido | `una_cabecera_incluida…` |
| `SUMA(1,2,3)` con dos parámetros | **error** con el nombre | Antes ni se detectaba | `invocar_una_macro…` |

★ **La trampa que esto destapó:** dos constantes sin expandir se volvían la
*misma* variable inventada (valor 0), así que `if (t == REPAG)` era cierto
también para `AVPAG`. **Un cero inventado no da un fallo local: da coherencia
falsa.** Por eso un identificador desconocido ahora es un error y no un cero.

---

## 5. Listas de inicialización

| Se escribe | Debe salir | Por qué | Dónde |
|---|---|---|---|
| `int a[4] = {10,20,30,40}` | `10 30 40` | No existía ninguna lista, ni ésta | `una_lista_posicional…` |
| `struct P q = {.y = 7}` | `0 7 0` | **C99 §6.7.9/21**: lo no mencionado vale CERO. Sin borrar antes, trae basura de la pila — distinta en cada ejecución | `lo_no_mencionado…` |
| `int b[5] = {[2] = 30, 40}` | `0 0 30 40 0` | Un designador **reposiciona el cursor** y lo siguiente sigue desde ahí. La que más se olvida | `tras_un_designador…` |
| `{.x=1, .y=2, .x=9}` | `9 2` | El último gana, y sale de emitir en orden | `si_un_campo…` |
| `char s[8] = "hola"` | `hola` y `s[4]==0` | Una cadena llena el array **byte a byte**. Guardar el puntero deja una dirección y basura detrás | `una_cadena_llena…` |
| `char s[3] = "hola"` | **error** | Escribir uno de más pisa lo de al lado | `una_cadena_que_no_cabe…` |

---

## 6. Structs por valor

**La ABI de agregados de BMO** (ver `codegen/agregados.rs`): argumento en
`techo(tamaño/8)` ranuras consecutivas de la pila; retorno por puntero oculto en
`rdi` — *todavía no implementado*. No se copia la clasificación por *eightbytes*
de SysV porque aquí no hay registros de argumento que repartir.

| Se escribe | Debe salir | Por qué | Dónde |
|---|---|---|---|
| `q = p` con `struct P{int x,y,z;}` | `1 2 3` | Copiaba **ocho bytes**: uno de 12 se copiaba a medias | `asignar_un_struct…` |
| `q = p; q.y = 99;` | `p.y` sigue en `2` | Es una copia, no un alias | `la_copia_de_un_struct…` |
| `suma(p)` con `struct P` de 12 B | `6` | Empujaba **una palabra**: la función recibía el primer campo y basura | `un_struct_viaja_entero…` |
| `mezcla(7, p, 5)` | `707` | Un agregado ocupa DOS ranuras y **corre** al parámetro de detrás. Con `16+i*8` se leía desde la mitad del anterior | `un_struct_corre…` |
| `struct P haz()` | **error** con el nombre | Devolver por valor es un tercer mecanismo y no está. Devolver 8 bytes de 12 sería mentir | `devolver_un_struct…` |

---

## Mentiras del emulador (histórico)

Cada una hizo **fallar código correcto** o **pasar código roto**. Se apuntan
porque la próxima se parecerá a éstas.

| Fecha | Qué mentía | Cómo se vio |
|---|---|---|
| 2026-07-31 | `mov [mem], eax` escribía **8 bytes** rellenando de ceros. En un registro es correcto; en memoria destruye lo de al lado | `{.x=1,.y=2,.x=9}` daba `9 0`: la última escritura de `x` borraba la `y` |
| 2026-07-31 | El prefijo `0x66` (16 bits) no existía | Habría reventado con el primer campo `short`. Mina, no fallo |
| 2026-07-30 | `finalizar_syscall` ponía `rax=0` y no tocaba `rdx` | `read_line` habría girado para siempre |
| — | Concatena las secciones; el cargador las pone en páginas separadas | Un `lea [rip+disp]` roto pasaba en verde |

**Cómo se caza una:** si un test falla y el razonamiento sobre el código emitido
dice que debería pasar, **volcar el AST antes de tocar el compilador**. La sonda
que imprimió las `Escritura` resolvió en un minuto lo que llevaba media hora de
suposiciones.

---

## Lo que falta, y qué debe salir cuando esté

| Qué | Qué se espera | Por qué importa |
|---|---|---|
| **Entrada** (`getchar`/`scanf`) | Leer una línea de la consola del proceso | Es lo único que la hoja de ruta pide para cerrar C. Sin ella un programa de C no puede preguntar nada |
| **Devolver structs** | `q = haz()` con el puntero oculto en `rdi` | Hoy se rechaza con motivo |
| **Floats globales** y como argumento | Ruta `xmm` en los bordes | Diferido con error honesto |
| **Invocación de macro en varias líneas** | Juntar las líneas antes de expandir | Hoy se dice, no se adivina |

---

## Para la próxima foto del Ryzen

1. `run apps/holac.bex` → las seis líneas de la sección 1, exactas.
2. `run apps/scrollc.bex` **desde Ring 0** → la ventana de la sección 2, y que
   RePag/AvPag muevan. **Es el primer `__syscall` de C en silicio.**
3. La fila `raton` de CABINA: `bmb=k+r+`, `apk=…:0:0`, y `kev=` y `raton ev=`
   subiendo **en el mismo arranque**.
