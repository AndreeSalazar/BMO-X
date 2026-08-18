# PLAN MAQUETA

> El compilador de composicion del escritorio. **No es un navegador, y la
> diferencia no es de tamano: es de direccion.**
>
> Escrito el **2026-08-17**. Nace de una idea del dueno y de tres frases suyas
> que ya decidieron el diseno entero:
>
> > *"crear un compilador compositor profesional que es usar con HTML + CSS
> > exclusivo como para compositor, para construir pero no es navegador"*
>
> > *"en arch linux es como sentir que tienes control (...) pero optimizacion o
> > Gentoo no tienen sentido, BMO-X base ya cubre TODO en silicio, encima es
> > composicion para facilitar"*
>
> > *"HTML inspirado como ASTRO pero profesional"*
>
> Decidido en la misma conversacion: **cinco generaciones, sin herencia, nombre
> MAQUETA**.

---

## 0. QUE ES, EN UNA FRASE

**Un compilador que lee un arbol de cajas y unas reglas, y emite las
coordenadas ya calculadas.** La aritmetica de pixeles que hoy se escribe a mano
en `Ultra_userspace/services/gui/src/scene/` pasa a ser texto que se lee.

```
   HOY       calc.rs:       CALC_BTN = 72;  CALC_GAP = 6;  fn button(row,col)
   MANANA    calc.maqueta:  display:flex; gap:6px
```

### ⚠️ El tamano del problema, MEDIDO el 17-08 (y era menor de lo que dije)

La primera version de este documento decia *"7.780 lineas de `scene/`"*. **Es
falso y sobrevende.** Contado:

```
   13.829   el servicio gui entero
   -3.000   mouse.rs (699) + keys/* (1.374) + desktop/mod.rs (381) + gato.rs (515)
            -- CERO llamadas de dibujo entre los cuatro. MAQUETA no los toca nunca
   ~1.657   literales numericos en scene/; quitando 0/1/2 quedan ~700 magicos
      118   de las 214 lineas de calc.rs son maquetacion (el resto es la maquina
            de estados de la calculadora, y se queda en Rust para siempre)
```

**El escritorio no es sobre todo pintura: es sobre todo enrutado de entrada.**
Decirlo al reves habria hecho que la primera medida contra `calc.rs` pareciera un
fracaso cuando es un exito.

---

## 1. LA INSPIRACION, DICHA CON PRECISION

### Gentoo NO transfiere, y conviene tener escrito por que

Las USE flags existen porque **aguas arriba** alguien compilo un binario para
todo el mundo y tu estas recuperando una decision que te quitaron. **BMO-X no
tiene aguas arriba.** Eres el dueno del compilador, del kernel y del disco. No
hay nada que reclamar; el `-march=native` es un hombre peleando con un
empaquetador que aqui no existe.

### De Arch transfiere UNA cosa, y no es el rendimiento

**Nada esta generado por una herramienta que no puedas leer.** `PKGBUILD` es un
script. `/etc` es texto tuyo. Esa es la sensacion de control.

Hoy el escritorio de BMO-X es lo contrario: cambiar el hueco entre dos teclas de
6 a 8 pixeles es **editar Rust y recompilar un servicio**. Eso es mas opaco que
Arch, no menos.

### ★ Y aqui se puede ser MEJOR que Arch, no igual

El control de Arch es **en ejecucion**: editas, reinicias, y puedes arrancar a un
sistema roto. MAQUETA es control **en compilacion**: el escritorio es texto, pero
un texto que no compila **no llega a arrancar nunca**. La misma lectura, sin la
posibilidad de romperlo en caliente.

### ★★ De ASTRO transfiere la tesis entera

Astro no es un framework de navegador: es una **herramienta de construccion**. Su
rasgo definitorio es que **los componentes desaparecen al compilar** y solo queda
marcado plano; lo que necesita vivir se declara aparte, como **isla**.

Eso es exactamente esto:

| Astro | MAQUETA |
|---|---|
| el componente desaparece al compilar | la caja se vuelve un rect en una tabla |
| cero runtime por defecto | cero codigo en el aparato por defecto |
| **isla** = el trozo que si vive | **`<island>`** = el rect que rellena Rust |
| estilos con ambito por componente | igual, y **es lo que hace innecesaria la herencia** |

### ✳️ Lo que ninguna distro da, dicho sin adorno

Ninguna distro **compone** el escritorio: todas **configuran uno ya escrito**.
GNOME y KDE traen un compositor en C/C++ y te dan un panel de ajustes; Arch te da
a elegir **cual** de los escritorios ya escritos instalas. El hueco no esta al
nivel de la distro -- esta **debajo**, y es donde vive BMO-X.

### ★ El regalo ironico: el navegador como REGLA, no como destino

Si las etiquetas son las de HTML y las propiedades son las de CSS de verdad,
entonces **un `.maqueta` se abre en un navegador y se ve aproximadamente bien**.
Previsualizacion gratis mientras se escribe, en Windows, sin arrancar el Ryzen.

⚠️ **Aproximadamente**, y la palabra es literal: la fuente de BMO-X es de mapa de
bits y de ancho fijo, y la del navegador no. **La previsualizacion orienta; la
verdad son los ficheros dorados** (seccion 8 de `LA_MAQUETA_EXIGE.md`). Confundir
las dos cosas seria la primera forma de mentir de este compilador.

---

## 2. LA FRONTERA: lo QUIETO se compila, lo que VIVE se programa

Es la decision que impide la deriva hacia navegador, y hay que trazarla **antes**
de escribir una linea. Medido sobre los ficheros que existen hoy:

| Se compila (esta QUIETO) | No se compila (VIVE) |
|---|---|
| `calc.rs` -- es una rejilla | `vitals.rs` -- numeros que cambian |
| `launcher.rs`, `switcher.rs`, `splash.rs` | `cabina.rs`, `testigo.rs` |
| `chrome.rs` -- el marco de las ventanas | `data.rs` (956 lineas) -- el grafo de ESTRATOS |
| la barra, los paneles | `cursor.rs`, DOOM |

El dia que se quiera que un `<div>` muestre un numero que cambia, ya no se esta
escribiendo un compilador de maquetacion: se esta escribiendo el DOM. Y detras
del DOM viene todo lo demas.

**La costura entre las dos columnas es `<island>`**, y no es un invento nuevo:
una isla es un rect con nombre que otro proceso rellena -- que es **exactamente
la superficie de `PLAN_DIRECTOR.md`** (`BSUP`, `MEM_OP_OFRECER` / `TASK_OP_TOMAR`).
La mitad viva del escritorio ya tiene su mecanismo construido.

### ★★ Y la frontera de verdad no era "quieto contra se mueve": es EL NUMERO DE HIJOS

Leyendo `switcher.rs` (17-08) salio el caso que la tabla de arriba no describe.
El conmutador **esta quieto** -- no se anima, no cuenta nada -- y sin embargo su
altura es `ROW_H * lista.len() + ...`: **el numero de filas se sabe en ejecucion**,
porque son las ventanas que haya abiertas. Igual `launcher.rs` (`for i in
0..l.count`) y la lista de `mod.rs`.

Un compilador estatico no puede maquetar eso, y fingir que si es como se llega a
un motor de maquetacion en el aparato por la puerta de atras.

**La regla, y es la que decide el alcance real del proyecto:**

> **MAQUETA maqueta lo que tiene un numero de hijos CONOCIDO AL COMPILAR.
> La repeticion sobre datos vivos es de quien tiene los datos.**

En la practica: **la fila es un `.maqueta`, la lista es Rust.** El compilador
resuelve el interior de UNA fila -- sus rects relativos al origen de la fila, que
es lo irregular y lo que se equivoca una persona -- y Rust la coloca en un bucle
con un `+= alto_de_fila` que el propio compilador emitio. Es exactamente como esta
escrito hoy (`fy += ROW_H`), asi que no hay nada que reescribir: hay algo que
dejar de calcular a mano.

### ✅ Y un mecanismo que ya existia: `p.texto` devuelve la x siguiente

```rust
let mx = p.texto(x + 14, fy + 4, "modo: ", INK_DIM);
let mx = p.texto(mx,     fy + 4, modo,     ACCENT);
         p.texto(mx,     fy + 4, "   (Alt+M)", INK_DIM);
```

Eso es **flujo en linea**, y ya funciona. `<span>` no hay que inventarlo: hay que
enchufarlo a lo que hace `Pantalla` desde siempre.

---

## 3. LAS CINCO GENERACIONES

L7 nombra cuatro roles. Aplicada a este camino salen **cinco**, y fingir que son
cuatro es lo que produciria el monolito: la cascada relaciona dos *padres*
(nodo x regla) y la maquetacion relaciona dos *hijos* (caja x caja). Son
generaciones distintas.

```
   abuelo     TROZO      token de marcado / token de estilo
              no sabe si el documento es valido

   padre      PIEZA      Node / Rule -- nombrada y compuesta
              no sabe que tiene hermanos

   hijo       CASCADA    Node x Rule -> Box con estilo resuelto
              no sabe que es una pantalla

   nieto      MAQUETA    Box x Box -> rects enteros y definitivos
              no sabe que se hace con ellos

   bisnieto   VEREDICTO  es legal? es honesta?
              el unico con opinion
```

| crate | generacion | la pregunta que responde (L6b) |
|---|---|---|
| `bmo-maqueta-lex` | abuelo | que trozos hay en el fichero |
| `bmo-maqueta-node` | padre | que es cada pieza, con su nombre |
| `bmo-maqueta-cascade` | hijo | que regla le toca a que nodo |
| `bmo-maqueta-layout` | nieto | donde cae cada caja |
| `bmo-maqueta-verdict` | bisnieto | si esto esta bien o no |

### ★★ EL EMISOR NO ES UNA GENERACION: ES EL CONSUMIDOR

Y por la ley -- *ninguna generacion sabe quien la consume* -- **nadie de la
cadena sabe que existe**.

Consecuencia concreta, y es la que paga el reparto: la pregunta de si la salida
debe ser **codigo Rust generado** o **un recurso BEF (seccion 0x0B)** **ya no hay
que contestarla ahora**. Se empieza por Rust, que se prueba contra `calc.rs` en
una tarde, y el dia del recurso se anade un segundo emisor **sin tocar ni una de
las cinco generaciones**. La ley convirtio una decision irreversible en una
reversible.

### ⚠️ Correccion honesta a L7b

L7b dice *"el nieto siempre fuera del binario que mide"*, y su razon era el
hardware: `bmo-juicio` vive en `platform/shared/` porque el kernel es `no_main` y
no corre un test.

**Aqui esa razon no existe**: MAQUETA corre entera en el anfitrion y todo se
prueba en `cargo test`. Asi que el corte del veredicto necesita **otra** razon o
es decorativo, y la tiene:

> El veredicto se separa porque **es la unica pieza con opinion, y las opiniones
> cambian**. Cada propiedad nueva que se decida rechazar toca el veredicto. Si
> vive pegado a la aritmetica, la aritmetica se toca cada vez que cambia la
> politica -- y la aritmetica es lo que no debe moverse nunca.

---

## 4. ★★ LO QUE LA LEY DECIDE POR NOSOTROS

Esta es la parte que justifica haber discutido el reparto antes de escribir. **La
jerarquia no ordena el trabajo: elige el subconjunto de CSS.**

### El abuelo prohibe el navegador, sin que nadie lo decida

*abuelo = no sabe si el documento es valido.* La recuperacion de errores de HTML5
-- el `<p>` que se autocierra, la etiqueta mal anidada que el navegador arregla --
**exige que el tokenizador consulte el estado del arbol**, o sea que el abuelo
sepa quien lo consume. **L7a lo prohibe.**

No hace falta ser estricto por disciplina. **La ley que ya estaba escrita no deja
ser un navegador.** Es el mismo movimiento que los 246 ciclos: convierte una
intuicion en algo que se puede refutar.

### El padre prohibe tres cosas de CSS, y por eso no hay herencia

*padre = no sabe que tiene hermanos, ni que tiene padre.*

- **herencia** (`color` que baja de padre a hijo) -> exige conocer al ancestro. **Fuera.**
- **selectores de descendencia** (`.panel .boton`) -> exige conocer a los ancestros. **Fuera.**
- **`%` y `auto`** -> exigen conocer el tamano del contenedor. **Fuera.**

Y el ambito por componente de Astro es justo lo que hace que no se echen de
menos: si las reglas de un fichero solo tocan a sus cajas, la herencia era un
apano para no repetirse dentro de un documento gigante que aqui no existe.

### ✅ El texto, que es lo que hunde a los motores de maquetacion, aqui es gratis

La parte cara de CSS no son las cajas: es el texto -- metricas, kerning, shaping,
salto de linea, fallback de fuente. **BMO-X tiene fuente de mapa de bits y de
ancho fijo**, asi que medir texto es:

```
   ancho = texto.len() * bmo::GLIFO_ANCHO
```

Exacto, entero, en tiempo de compilacion y sin dependencias. **Ese es el
argumento entero de viabilidad, y no vale para nadie mas.**

---

## 5. EL SITIO

**`toolchain/tools/maqueta/`**, con los cinco crates dentro.

El razonamiento, por si hay que revisarlo: `toolchain/lang/` es para lenguajes
que producen `.bex` pasando por la base (`c`, `cobol`, `ada`). MAQUETA **no
produce codigo**: produce coordenadas. Su parentesco es con `c-gen`, `cobol-gen`,
`fontgen` y `bmo-pack` -- generadores de anfitrion. Va en `tools/`.

⚠️ **Nombres en INGLES**, identificadores y comentarios, desde la primera linea
(regla del 2026-08-08, incumplida tres veces). `maqueta` sobrevive como **nombre
de producto** -- como CABINA o DOOM -- no como identificador. **El disparador del
fallo es exactamente este**: crear ficheros nuevos en un arbol cuyos vecinos
estan en espanol.

---

## 6. LA ESCALERA

```
   [x] 0   los dos libros: este y LA_MAQUETA_EXIGE.md
   [ ] 1   abuelo   lexer de dos modos (marcado / estilo)
   [ ] 2   padre    Node y Rule
   [ ] 3   hijo     cascada por clase y etiqueta, ultimo gana
   [ ] 4   nieto    maquetacion: bloque + flex en un eje
   [ ] 5   bisnieto veredicto: las seis comprobaciones
   [x] 6   emisor A -> Rust generado, y calc.rs como primera victima
   [ ] 7   ficheros dorados como oraculo
   [ ] 8   emisor B -> recurso BEF 0x0B (arrastra "leer recursos en ejecucion")
   [ ] 9   `<island>` cableada a una superficie BSUP
```

**El escalon 6 es la prueba de que esto sirve, y tiene un numero -- afinado el
17-08 despues de contar**: de las 214 lineas de `calc.rs`, **118 son maquetacion**
(`CalcPad`, `button`, `key_at`, `contains`, las constantes y el cuerpo de
`paint_calc`). Las otras ~96 son la maquina de estados de la calculadora y **se
quedan en Rust para siempre**.

```
   118 lineas de maquetacion  ->  48 lineas de .maqueta      MEDIDO, -59%
```

### ⚠️ Lo prometido era "un tercio", y no llega. La razon esta medida

Son **48 contra 118: un 59% menos**, no un 67%. Y no es que el compilador salga
mal -- es que **la calculadora es el PEOR CASO posible para MAQUETA**:

> `calc.rs` pinta veinte teclas con `for row { for col }` sobre una tabla de
> etiquetas. **Una rejilla regular YA ES maquetacion declarativa**, y ahi un
> bucle gana en lineas a veinte `<div>` escritos uno a uno.

Donde MAQUETA gana de verdad es en lo **irregular** -- `chrome.rs` (565 lineas,
el marco de las ventanas), la barra, los paneles -- que es donde no hay bucle que
valga. Elegir la calculadora primero fue elegir el caso que peor le sienta, y eso
es lo que hace que el numero sirva.

★ **Y las lineas no son lo que mas se cobra.** Lo que desaparece son
`CalcPad::button()`, `key_at()` y `contains()`: **tres funciones que son la misma
aritmetica escrita dos veces**, una para pintar y otra para responder al raton.
Eso no se reduce, se vuelve imposible.

★ Y el sitio donde mas se cobra no es el pintado: son `button()`, `key_at()` y
`contains()`. **Hoy la misma aritmetica esta escrita dos veces** -- una para
pintar la tecla y otra para saber que tecla se pulso -- y esa duplicacion es una
clase de bug entera (el boton que se dibuja en un sitio y responde en otro). El
compilador conoce el rect final, asi que emite **la lista de pintado y la tabla
de golpeo de una sola fuente**, y las tres funciones desaparecen.

---

## 7. LO QUE ESTO NO ES

- **No es un navegador.** No hay red (cero syscalls), no hay DOM, no hay script.
- **No es un motor de maquetacion en el aparato.** Todo el calculo ocurre en el
  anfitrion; en el Ryzen solo se leen rects ya calculados.
- **No es un lenguaje de aplicaciones.** Da la CARA de una app. Los clics, el
  estado y el foco siguen siendo Rust.
- **No es HTML.** Se le parece a proposito para poder usar un navegador de regla,
  y **rechaza** todo lo que no implementa. Un navegador ignora lo que no entiende;
  esto es lo contrario, y esa inversion es la definicion del proyecto.

Ver `docs/componente/LA_MAQUETA_EXIGE.md` (que acepta y que rechaza),
`docs/plan/PLAN_DIRECTOR.md` (las superficies, que son las islas) y
`META-KERNEL_HARD.md` L6 y L7 (la ley del reparto).
