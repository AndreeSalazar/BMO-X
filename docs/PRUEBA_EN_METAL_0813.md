# PRUEBA EN METAL -- el arranque del 2026-08-13

Tres commits, y **ninguno ha visto un CPU**. La guia anterior queda en
`PRUEBA_EN_METAL.md`; de ahi siguen abiertas las seis preguntas de su segunda
vuelta, que **no se repiten aqui** -- se pueden contestar en el mismo arranque.

> Lo de hoy son tres preguntas, y una de ellas se contesta con una sola linea de
> texto en pantalla.

```powershell
Ultra_kernel_x86-64\build.ps1 -Flash -Drive A -Data A
```

`-BuildOnly` ya paso entero: ASCII limpio, contrato de syscalls con 49
operaciones y 42 campos de INFO, y los tres binarios enlazados.

---

# 1 -- DOOM: LA LINEA QUE LO CAMBIA TODO

`run apps/doom.bex` **desde el escritorio** (el icono, o `Ejecutar`), no desde el
shell de Ring 0. Ver el punto 4 sobre por que.

Lo ultimo que se supo: DOOM moria imprimiendo

```text
   Unknown configuration variable: 'use_joystick'
```

que **no era un aviso sino la causa de muerte** -- `I_Error` imprime y llama a
`exit`. La causa estaba en el compilador: `&c->defaults[i]` valia CERO.

## Lo que tiene que salir ahora

```text
   M_LoadDefaults: Load system defaults.
   W_Init: Init WADfiles.            <- ESTA. Es toda la prueba.
```

**Si aparece `W_Init`, el arreglo del codegen funciono en metal**: DOOM paso la
tabla de configuracion, encontro su WAD y esta abriendolo. Lo que falle DESPUES
es territorio nuevo -- y se sabra, porque ya no hay muertes mudas.

| Sintoma | Que significa |
|---|---|
| sale `W_Init` y sigue | **el arreglo funciono.** Apuntar donde muere ahora |
| vuelve `use_joystick` | el `.bex` desplegado es el viejo: el flash no llego |
| muere antes, en otra linea | otro defecto delante. La linea nueva ES el dato |
| `IWAD file ... not found` | ahora si es el WAD. Estaba en `A:\apps\` con 4.196.020 B |

★ **Y si arranca**: `A:\apps\doom1.wad` esta en el disco y la partida deberia
empezar. Eso ya no es una prueba de sistema, es jugar.

---

# 2 -- EL GRAFO DE ESTRATOS, CON CURVAS DE VERDAD

`F12` -> `TAB` -> pestana `nodos`.

Antes: una espina vertical con codos, todo rectangulos de un pixel. Ahora **una
curva Bezier por hijo**, del punto de salida del padre a la entrada de cada caja,
con punta de flecha.

## Que mirar, y es todo a ojo

1. **Que las curvas se vean curvas y no poligonales.** El numero de tramos se
   estima del tamano; si se ven las esquinas del troceado, el sospechoso es
   `tramos()` en `dibujo/curva.rs` y se sube el divisor.
2. **Que la punta de flecha toque la caja** y no se quede a un pixel ni la pise.
3. **Que las curvas no se crucen con las cajas.** Viven en el canal de 44 px
   entre las dos columnas; si alguna pasa por encima de un nombre, el tirante es
   demasiado largo.
4. ★ **ENCOGER LA VENTANA por la esquina.** Es la prueba del recorte, que hasta
   hoy no existia en esta ventana: **ni un pixel de arista puede salirse del
   marco**. Con codos no podia pasar por construccion; con curvas si.
5. Y que **arrastrar la ventana no deje rastro** de curvas viejas.

Vuelta atras: `git revert 82bb94ea`.

[!] `gui.bex` paso de 319.528 a **337.192 B**. Si el escritorio no arranca, es
esto antes que nada.

---

# 3 -- LA REGRESION QUE HAY QUE DESCARTAR: TODOS LOS `.bex` DE C

El commit del codegen (`1a48cbd2`) **toca todos los programas de C**, no solo
DOOM. Tres ordenes y se descarta entero:

```text
   run c/caja.bex        sus cuatro lineas y sus dos recursos
   run c/ray.bex         el laberinto, con paredes
   run cobol/1/hola.bex  que COBOL no se entero de nada
```

Si alguno deja de arrancar o pinta basura donde antes pintaba bien:
`git revert 1a48cbd2`.

★ Lo que **no** puede pasar es que un programa compile y haga otra cosa en
silencio: el brazo que rellenaba de ceros ahora es un error de compilacion. Si el
build hubiera encontrado un caso sin cubrir, `-BuildOnly` habria parado -- y no
paro, con los 12 ejemplos y las 56.465 lineas de DOOM delante.

---

# 4 -- ESTRATOS: `generacion 2`, Y LA PARTE BONITA ES DESPUES DE REINICIAR

En la caja de `Ejecutar`:

```text
   estratos sellar
```

Son **dos palabras a proposito**: `sellar` a secas se teclea sin querer, y esto
**escribe en el disco**.

Lo que hace: una transaccion **sin datos**. No reserva un bloque ni toca un
objeto -- commitea apuntando al mismo estrato que ya habia, y lo unico que cambia
es el numero de generacion, escrito en **la copia del superbloque que NO manda**.

★ Por eso es la primera: recorre el camino ENTERO --reservar, cerrar, `FLUSH
CACHE`, barrera, commit, superbloque alterno-- y **no puede perder un dato aunque
salga mal**. Si falla antes del commit, el volumen es el de antes; si falla
escribiendo el superbloque nuevo, se estropea la copia que no manda y el volumen
monta igual.

```text
   COMMIT. generacion 2
```

Y luego `F12` tiene que decir `generacion 2`.

★★ **LA QUE DE VERDAD PRUEBA ALGO: REINICIAR Y VOLVER A MIRAR.** Si sigue
diciendo `2`, llego al plato y no se quedo en la cache del SSD. **Eso es lo unico
que separa una barrera que funciona de una que se cree.**

Si sale `el sellado NO se hizo`, el motivo esta en F11 con nombre:
`SinVolumen`, `SinBarrera` (el `FLUSH CACHE` fallo, y entonces NO se commiteo) o
`Rechazada`.

---

# QUE TRAER DE VUELTA

1. **`A:\datos\salida.txt`** -- se llena solo con lo que se lanza desde
   `Ejecutar`, y `guarda` vuelca el historial entero. Vale mas que cualquier
   foto. **Y por eso DOOM va lanzado desde el escritorio**: desde el shell de
   Ring 0 su salida no la recoge nadie.
2. **La foto del grafo con curvas**, que es lo unico de esta tanda que no se
   puede contar con texto.
3. La linea de DOOM donde muera, si muere.
4. `generacion` antes y despues del reinicio.

---

# LOS TRES COMMITS

| commit | que toca | si algo falla |
|---|---|---|
| `1a48cbd2` | **el codegen de C**: todos los `.bex` | un programa de C deja de andar |
| `073d10f8` | solo documentos | nada |
| `82bb94ea` | **el pintado del grafo** + `userland` | el escritorio no arranca |
