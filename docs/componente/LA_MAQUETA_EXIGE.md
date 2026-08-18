# LA MAQUETA EXIGE

> Capitulo de componente en la forma de `META-KERNEL_HARD.md`: no *"que hace
> MAQUETA"* -- eso esta en `docs/plan/PLAN_MAQUETA.md` -- sino **que exige MAQUETA
> de quien quiera escribirle, y que le devuelve a la cara**.
>
> Escrito el **2026-08-17**. Este documento **es el contrato**: lo que no esta
> aqui, no compila. Anadir algo a MAQUETA empieza por anadirlo a este fichero.

---

## 0. ★★ LA REGLA QUE ORDENA EL DOCUMENTO ENTERO

```
   UN NAVEGADOR IGNORA LO QUE NO ENTIENDE.
   UN COMPILADOR LO RECHAZA.
```

Es la unica linea que separa esto de un navegador de juguete, y es tambien la ley
de la casa: *nada que compile y no haga lo que dice*.

Una propiedad aceptada en silencio y no honrada **es la mentira que envejece sin
avisar** -- la misma clase que `INFO_ES_ESCRIBIBLE => 0`, un valor puesto por
prudencia que tres meses despues era falso y nadie se entero. Por eso:

- **toda etiqueta fuera de la seccion 2 es un error**, no un `<div>` generico;
- **toda propiedad fuera de la seccion 3 es un error**, no una linea ignorada;
- **toda unidad fuera de la seccion 4 es un error**, no un cero.

Y un error de MAQUETA **detiene la compilacion**. No hay avisos.

---

## 1. EL FICHERO

Extension `.maqueta`. **Marcado y estilo en el mismo fichero** -- es lo que pidio
el dueno (*"HTML + CSS ambos combinado"*) y es la forma de Astro: un componente es
un fichero, no tres.

```html
<maqueta>
  <style>
    .pad   { display:flex; flex-direction:column; gap:6px; padding:6px;
             background-color:#182434; border-width:2px; border-color:#0E1620 }
    .visor { height:40px; background-color:#101820; color:#DDE6F0 }
    .fila  { display:flex; flex-direction:row; gap:6px }
    .tecla { width:72px; height:72px; background-color:#2B3B52; color:#DDE6F0 }
    .op    { background-color:#3A5878 }
    .igual { background-color:#4C9BE8 }
  </style>

  <div class="pad">
    <div class="visor"></div>
    <div class="fila">
      <div class="tecla" id="k_c">C</div>
      <div class="tecla op" id="k_div">/</div>
      <div class="tecla op" id="k_mul">*</div>
      <div class="tecla op" id="k_sub">-</div>
    </div>
    ...
  </div>
</maqueta>
```

### `<maqueta>` sin tamano: el compilador lo calcula

Si la raiz no declara `ancho`/`alto`, **MAQUETA los deduce del arbol y los emite
como constantes**. Para el ejemplo de arriba salen `322 x 446`:

```
   ancho   4*72 (teclas) + 3*6 (huecos) + 2*6 (padding) + 2*2 (borde) = 322
   alto    40 + 6 + 5*72 + 4*6 (huecos) + 2*6 (padding) + 2*2 (borde) = 446
```

★ **Ese es el trabajo que hoy hace una persona y a veces mal.** Un panel se
declara con tamano; una ventana que debe ajustarse a su contenido, no.

---

## 2. LAS ETIQUETAS -- LISTA CERRADA

| etiqueta | que es | notas |
|---|---|---|
| `<maqueta>` | la raiz. Una por fichero | atributos `ancho` / `alto`, opcionales |
| `<style>` | el bloque de reglas. Uno por fichero | solo hijo directo de `<maqueta>` |
| `<div>` | caja generica | el 95% de todo |
| `<span>` | caja en linea, contiene texto | no acepta hijos |
| `<island>` | el hueco que rellena otro proceso | atributo `nombre`, obligatorio y unico |

Los **nodos de texto sueltos** son validos dentro de `<div>` y `<span>`, como en
HTML. Se miden con `len * GLIFO_ANCHO` y **no se parten en lineas**: si no caben,
es error (comprobacion B de la seccion 7).

Atributos aceptados: `class`, `id`, `nombre` (solo en `<island>`), `ancho`/`alto`
(solo en `<maqueta>`). **Cualquier otro atributo es un error.**

★ **Por que `<div>` y `<span>` y no `<caja>` y `<texto>`**: son las dos unicas
etiquetas de HTML que *no prometen semantica* -- literalmente "caja generica sin
significado", que es lo que hay aqui. Usar sus nombres es honesto y ademas
conserva la previsualizacion en navegador. `<h1>`, `<p>`, `<button>` prometen cosas
que MAQUETA no hace, y por eso **estan prohibidas**, no reinterpretadas.

`id` **no sirve para estilar**: es la clave de la tabla de golpeo (seccion 8).

---

## 3. LAS PROPIEDADES -- LISTA CERRADA

**Dieciseis.** Elegidas **contando** lo que `scene/` hace de verdad hoy, no lo
que CSS ofrece. `border-radius` entro al medir la raiz y descubrir que ya estaba
implementada; `margin` **salio** al escribir el nieto -- ver seccion 3b.

⚠ Y el numero estuvo mal dos veces mientras vivio solo en prosa: dijo
"diecisiete" con dieciseis en la tabla, y "dieciocho" con diecisiete. **Lo
destapo `value.rs`, donde son variantes de un `enum` y no se pueden contar mal.**
Un numero escrito en prosa que nadie ejecuta envejece igual que un valor puesto
por prudencia.

### La caja

| propiedad | valores | nota |
|---|---|---|
| `width` | `Npx` | |
| `height` | `Npx` | |
| `padding` | `Npx` o cuatro `Npx` | arriba derecha abajo izquierda, como CSS |

### La pintura

| propiedad | valores | nota |
|---|---|---|
| `background-color` | `#RRGGBB` | |
| `color` | `#RRGGBB` | el color del texto de ESTE nodo, no de sus hijos |
| `border-width` | `Npx` | un solo grosor, los cuatro lados |
| `border-color` | `#RRGGBB` | |
| `border-radius` | `Npx` | ★ ver abajo: **ya existe**, con su limite |

### 3b. ⚠ `margin` SALIO de la lista, al escribir el nieto

Estaba puesta y no llego a compilar nada. La razon es la misma que sostiene todo
lo demas:

> En CSS, **dos margenes verticales de hermanos se FUNDEN** -- dos de 10 px
> pegados dan 10, no 20. MAQUETA no va a implementar esa regla, y aceptar
> `margin` sin fundirlos haria que el fichero se viera distinto en el navegador
> que en el Ryzen.

Es exactamente el peligro del guardian de la seccion 5, en otro sitio. Y no
cuesta nada: `gap` dentro de un `display:flex` y `padding` en el contenedor
**cubren todos los casos contados en `scene/`**.

### ★ Y `align-items` por defecto es `stretch`, no `start`

Otra que casi se cuela. El valor por defecto de CSS es **`stretch`**, y estaba
escrito `start`: una fila flex habria ajustado sus cajas al contenido aqui y las
habria estirado al contenedor en el navegador. Misma familia -- **la
previsualizacion mintiendo** -- descubierta al escribir la maquetacion.

### ★ `border-radius` estaba rechazado por la razon equivocada (corregido 17-08)

La primera version de este contrato lo mandaba al escalon 4 del rasterizador
(mezcla alfa). **Falso, y lo destapo mirar la raiz**: `scene/mod.rs:116` tiene
`rounded_rect` desde hace tiempo -- *"Diecisiete `rect` y ya"* -- con una tabla
de curva de ocho entradas:

```rust
const RADIUS: u32 = 8;
const CURVE_TABLE: [u32; 8] = [8, 5, 3, 2, 1, 1, 0, 0];
```

O sea que **BMO-X redondea esquinas hoy, sin alfa**, apilando franjas de un pixel.
Lo que daria el escalon 4 no es la forma: es el **borde suave**. Entra ahora, con
dos avisos escritos:

- ⚠ **El borde es escalonado**, no suavizado. Aqui la previsualizacion en
  navegador se separa mas que en ningun otro sitio.
- ✅ **Y MAQUETA mejora lo que hay**: hoy existe **un solo radio** (8 px, con la
  tabla puesta a mano). El compilador calcula la tabla para el radio que se pida,
  asi que `border-radius:12px` deja de ser una tabla nueva que alguien escribe.

★ Vale la pena anotar como salio: no salio de auditar el contrato, salio de
**contar la raiz**. Es la regla de MODULAR #2 -- *medir antes de opinar* --
cobrandose una pieza el primer dia.

### La colocacion

| propiedad | valores | nota |
|---|---|---|
| `display` | `block` \| `flex` | por defecto `block` |
| `flex-direction` | `row` \| `column` | solo con `display:flex` |
| `gap` | `Npx` | solo con `display:flex` |
| `justify-content` | `start` \| `center` \| `end` \| `space-between` | eje principal |
| `align-items` | `stretch` \| `start` \| `center` \| `end` | eje cruzado. Por defecto **`stretch`**, como CSS |

### La colocacion absoluta

| propiedad | valores | nota |
|---|---|---|
| `position` | `absolute` | relativa al ancestro `<maqueta>`, no al padre |
| `left` | `Npx` | obligatoria con `position:absolute` |
| `top` | `Npx` | obligatoria con `position:absolute` |

⚠ `position:absolute` **es la unica puerta trasera del sistema** y esta aqui
porque los paneles del escritorio se colocan asi. Es tambien la unica forma de
que una caja se salga de su padre legitimamente, y por eso desactiva la
comprobacion 2 para ese nodo. **Usarla es declarar que sabes lo que haces.**

---

## 4. LAS UNIDADES

**Solo `px`, y solo enteros.** El `0` puede ir sin unidad.

No hay `%`, `auto`, `em`, `rem`, `vh`, `vw`, `fr`, `calc()`, decimales ni
negativos.

★ **Y esto no es pobreza, es L7**: `%` y `auto` exigen que una pieza conozca el
tamano de su contenedor, y en MAQUETA *un padre no sabe que tiene padre*. La
jerarquia elige el subconjunto; ver la seccion 4 de `PLAN_MAQUETA.md`.

**Los colores son `#RRGGBB`.** No hay nombres (`red`), ni `rgb()`, ni `rgba()`,
ni `transparent`. El pixel de BMO-X es `u32` en `0x00RRGGBB` y **no hay mezcla
alfa**: el rasterizador esta en el escalon 2 y la mezcla es el 4. El dia que
llegue el escalon 4, `rgba()` entra aqui -- y no antes.

---

## 5. LOS SELECTORES

Dos formas, y nada mas:

```
   etiqueta      div { ... }
   clase         .tecla { ... }
```

No hay combinadores (` `, `>`, `+`, `~`), ni `#id`, ni pseudo-clases, ni
pseudo-elementos, ni `@media`, ni `*`.

### ★ No hay especificidad: GANA EL ULTIMO

La cascada de CSS es uno de los footguns mas famosos del oficio. MAQUETA lo borra:
**las reglas se aplican en orden de fichero y la ultima que toca una propiedad
gana.** Se lee de arriba abajo y se acabo.

### ⚠ Y la trampa que eso abre, con su guardian

Un navegador **si** tiene especificidad: `.tecla` le gana a `div` aunque `div`
venga despues. Asi que un fichero con las reglas mal ordenadas se veria de una
forma en el navegador y de otra en BMO-X -- **la previsualizacion mentiria**, que
es justo lo que no se puede permitir.

**El guardian**: MAQUETA exige que las reglas esten **ordenadas de menos a mas
especificas** -- primero las de etiqueta, despues las de clase. Con ese orden,
"gana el ultimo" y "gana la mas especifica" dan **siempre** el mismo resultado, y
las dos lecturas coinciden por construccion. Una regla de etiqueta despues de una
de clase es un error.

---

## 6. LA FORMA EXACTA DE UN ERROR

Un rechazo que no ensena la salida es un muro. **Cada error lleva dos notas: por
que, y que escribir en su lugar.**

```
maqueta: calc.maqueta:14:26: propiedad no soportada -- `box-shadow`
   14 |   .tecla { width:72px; box-shadow:0 2px 4px #000 }
      |                        ^^^^^^^^^^^^^^^^^^^^^^^^^
      = por que: una sombra necesita mezcla alfa, y el rasterizador esta en el
        escalon 2 (triangulo). La mezcla es el escalon 4.
      = en su lugar: `scene/mod.rs` pinta sombras de ventana con dos capas de
        color solido. Si hace falta aqui, se declara con dos `<div>`.
```

```
maqueta: panel.maqueta:8:12: unidad no soportada -- `50%`
    8 |   .mitad { width:50% }
      |            ^^^^^^^^^^
      = por que: los porcentajes exigen conocer el contenedor, y en MAQUETA una
        pieza no sabe que tiene padre (L7).
      = en su lugar: un pixel exacto, o `display:flex` en el padre repartiendo
        con `gap`.
```

**El compilador no emite nada si hay un solo error**, y da todos los que
encuentre en la pasada, no el primero.

---

## 7. EL VEREDICTO: LAS DIEZ COMPROBACIONES

Vive en `bmo-maqueta-verdict` (bisnieto). Corre sobre los rects **ya calculados**,
o sea que no repite aritmetica: la mira.

### ⚠ La comprobacion 1 que decia este documento NO SE PUEDE FALLAR

La primera version listaba *"toda etiqueta, atributo y propiedad estan en las
listas cerradas"*. **No hay forma de que falle.** No existe `Tag::H1` ni
`Prop::BoxShadow` en `value.rs`, asi que un documento que no cumpla eso no llega
al veredicto: **muere en el padre, y por no poder ser NOMBRADO**.

Escribirla habria dado una funcion que siempre dice que si -- **un guardian de
mentira, que es peor que ninguno porque da confianza**. Se cae, y se sustituye
por las que el codigo destapo al escribirse.

### Cabe todo (`fit.rs`)

**A.** Ninguna caja se sale de su padre -- las absolutas se juzgan contra el
lienzo, no contra el padre.

**B.** ★★ **Todo texto cabe en su caja**, de ancho (`len * GLIFO_ANCHO`) y de
alto. Es la que mas vale: la fuente no parte palabras ni reajusta lineas, asi que
las letras que sobran se pintan por encima del borde. **Un navegador lo esconde y
BMO-X no puede** -- es el unico fallo de este sistema que queda *bonito* en
pantalla estando mal. El mensaje da los dos numeros.

**C.** Ninguna caja mide cero. Casi siempre es una propiedad olvidada, y como no
pinta ni ocupa sitio, no hay forma de notarlo mirando la pantalla.

### Los nombres responden (`names.rs`)

**D.** Todo `id` es unico -- es la clave de la tabla de golpeo, y con dos iguales
un clic contesta lo que no es.

**E.** Toda `<island>` tiene nombre unico y un rect no vacio.

**I.** Ninguna regla se queda sin casar con una caja.

**J.** Ninguna clase se queda sin regla que la defina. (I y J son casi siempre
los dos lados de la misma errata.)

### Hay algo escrito que no hace nada (`idle.rs`)

**F.** Ningun texto se queda sin `color`. Es el precio de no tener herencia,
cobrado aqui en vez de pintando de un color que nadie eligio.

**G.** Ningun `gap` en una caja que no es `flex`. En un navegador tampoco haria
nada; la diferencia es que alli no te lo dice nadie.

**H.** Ninguna `position:absolute` sin `left` y `top`.

### Todas son errores, y sigue sin haber avisos

Tambien F, G, I y J, que no rompen ninguna imagen. Es la regla que ordena el
proyecto: *nada que compile y no haga lo que dice*.

★ **La unica excepcion tiene su razon y no es una excepcion de verdad**: en un
fichero **sin cajas** -- una paleta como `tema/tema.maqueta` -- no se juzga nada.
Su raiz mide 0x0 y todas sus reglas salen sin usar, y **las dos cosas son la
consecuencia trivial de no tener cajas**, no defectos del fichero. Se decide una
sola vez, en `verdict::es_fragmento`.

⚠ Lo primero que se intento fue una excepcion suelta dentro de una comprobacion,
y se le escapo otra: el veredicto aprobaba las reglas del tema y acto seguido se
quejaba de que su raiz media cero. **Una excepcion repartida tapa el sintoma que
se vio, no el que viene.**

---

## 8. EL ORACULO: LOS FICHEROS DORADOS

La previsualizacion en navegador **orienta**; la verdad es esto. Mismo papel que
el rasterizador de `dibujo/` hace para la GPU: una referencia contra la que se
puede juzgar.

```
   toolchain/tools/maqueta/pruebas/
      calc.maqueta      la entrada
      calc.esperado     la salida, en texto que lee una persona
```

`.esperado` es una linea por caja -- `id  x  y  ancho  alto` -- en orden de
pintado, para que un cambio se vea como un diff y no como un fallo de test:

```
   pad        0    0  322  446
   visor      8    8  306   40
   fila_0     8   54  306   72
   k_c        8   54   72   72
   k_div     86   54   72   72
   k_mul    164   54   72   72
   k_sub    242   54   72   72
   fila_1     8  132  306   72
```

**Determinismo obligatorio**: misma entrada, mismos bytes de salida. Sin mapas
sin ordenar, sin direcciones, sin fechas.

---

## 9. LA ISLA

```html
<island nombre="vitals" class="panel_derecho"></island>
```

MAQUETA le calcula el rect y **no pinta nada dentro**. Emite la entrada en una
tabla de islas; quien rellene ese rect es cosa de Rust.

★ **Y no hay que inventar el mecanismo**: una isla es un rect con nombre que
otro proceso rellena -- que es **exactamente la superficie de `PLAN_DIRECTOR.md`**
(`BSUP`, `MEM_OP_OFRECER` / `TASK_OP_TOMAR`, la direccion por ranura). La mitad
viva del escritorio ya tiene su cableado; MAQUETA solo le dice donde va.

⚠ Una isla **no se maqueta segun su contenido**: su tamano lo pone la maqueta,
nunca el proceso que la rellena. Al reves seria dejar que una app cuelgue el
calculo del escritorio, que es lo que ya se decidio no hacer en `PLAN_DIRECTOR.md`
(decision 2: *la secuencia, no un cerrojo*).

---

## 9b. LA OTRA FRONTERA: EL NUMERO DE HIJOS

Un `.maqueta` **solo puede tener un numero de hijos conocido al compilar**. Es la
regla que impide que esto derive en un motor de maquetacion dentro del aparato, y
salio de leer `switcher.rs`: ese panel esta quieto y sin embargo su altura es
`ROW_H * lista.len()` -- las ventanas abiertas, que se saben en ejecucion.

> **La fila es un `.maqueta`. La lista es Rust.**

MAQUETA resuelve el **interior** de una fila -- lo irregular, lo que una persona
calcula mal -- y emite ademas su alto. Rust la repite con el `+=` de siempre.

Un `.maqueta` cuyo numero de hijos dependa de algo que no esta en el fichero es
un error, no una funcionalidad pendiente.

---

## 9c. EL TEMA: 62 COLORES QUE NO ESTAN EN NINGUN SITIO (medido 17-08)

```
   62   constantes de color con nombre en scene/ + desktop/
   33   de ellas usadas UNA sola vez ademas de su definicion  (53%)
   94   usos de INK_DIM      60 de INK      21 de ACCENT
```

★ **BMO-X ya tiene un tema; lo que no tiene es un sitio donde mirarlo.** Diez
colores llevan el peso y estan repartidos por quince ficheros, y los otros 33 son
la prueba del desgaste: cada panel nuevo inventa los suyos **porque no hay donde
consultarlos**.

Eso es exactamente la sensacion de Arch que motivo el proyecto -- *nada generado
por una herramienta que no puedas leer* -- y aqui se cobra sola: un fichero de
tema con esos diez nombres es mas control que cualquier panel de ajustes.

⏳ **Pendiente de decidir**: si `.maqueta` puede importar un `tema.maqueta`
compartido. Es la unica forma de que el tema exista de verdad, y es tambien la
primera vez que un fichero dependeria de otro -- con lo que eso arrastra (orden
de resolucion, y un ciclo posible). No se implementa hasta decidirlo.

---

## 10. LO QUE ESTA RECHAZADO, NOMBRADO

Que este por escrito importa: la deriva hacia navegador se hace de una propiedad
en una propiedad, y ninguna parece grave sola.

| rechazado | por que | vuelve cuando |
|---|---|---|
| herencia de propiedades | el padre no conoce a su padre (L7) | nunca en v1 |
| combinadores `.a .b`, `>` | igual | nunca en v1 |
| `%`, `auto`, `calc()` | exigen el contenedor | nunca en v1 |
| `rgba()`, `opacity` | no hay mezcla alfa | rasterizador escalon 4 |
| ~~`border-radius`~~ | **ACEPTADO el 17-08** -- ver seccion 3 | -- |
| repeticion sobre datos vivos | el numero de hijos se sabe en ejecucion | nunca: es de Rust |
| `:hover`, `:active` | es **conducta**, no maquetacion | v2, y sin tocar el layout |
| `grid` | `flex` cubre lo medido en `scene/` | cuando algo real lo pida |
| `float`, `z-index`, `overflow` | no hay caso en el arbol | cuando lo haya |
| `margin` | sus margenes se FUNDEN en CSS y aqui no | cuando se implemente la fusion |
| `@media` | una sola pantalla | cuando haya dos |
| salto de linea automatico | esconderia la comprobacion 3 | nunca |
| `<h1>`, `<p>`, `<button>`... | prometen semantica que no existe | nunca |
| script de cualquier clase | esto es un compilador | nunca |

★ `:hover` merece su linea: `calc.rs` **si** aclara la tecla bajo el puntero
(`lighten()`), asi que el caso es real. Pero el realce no cambia **ni un rect** --
solo un color. Cuando entre, entra como una segunda columna de colores en la
tabla emitida, y **el nieto no se entera**. Meterlo en la maquetacion seria el
primer paso hacia el DOM.

Ver `docs/plan/PLAN_MAQUETA.md` (como se construye) y `META-KERNEL_HARD.md` L6/L7.
