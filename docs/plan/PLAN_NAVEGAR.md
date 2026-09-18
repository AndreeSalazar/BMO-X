# PLAN NAVEGAR -- la propuesta maestra de la app que navega sin ser navegador

> Escrito el **2026-09-16**, el mismo dia que la LAMINA paso su banco
> (`platform/shared/bmo-antena/src/lamina.rs`) y que un articulo real salio
> maquetado de `toolchain/tools/antena/lamina.js`. Lo pidio el dueno asi:
>
> > *"primero con mi BMO-X una app simple pero que tenga mensaje que se
> > necesita conectar con ANTENA (da igual cual) pero ANTENA SIEMPRE, y eso es
> > como que 'Navegar', eso es el titulo, pero encapsula TODO en INTI para poder
> > viajar; en carpeta que vivira TODOS los elementos necesarios; que lo que
> > MASTICA la ANTENA mi BMO-X lo refleje en tiempo real; que tendra su
> > propuesta maestra"*.
>
> Esta es la propuesta maestra. Todo lo que dice se puede comprobar, y lo que
> todavia no existe lo dice con ese nombre.

---

# 0. QUE ES, EN UNA FRASE

**NAVEGAR es la ventana por la que se ve lo que la ANTENA mastica.** No tiene
motor de navegador, ni JavaScript, ni TLS, ni codecs: recibe la pagina YA
MAQUETADA (la LAMINA), la pinta, y devuelve clics y teclas. El navegador vive
en la antena (`docs/plan/PLAN_CLOUD_LOCAL.md`, seccion 11), y esta app es
**la cara** de ese reparto en el escritorio de BMO-X.

```text
   la ANTENA (movil, Arch)          NAVEGAR (BMO-X)
   --------------------------       -----------------------------------
   la web, JS, CSS, fuentes,        pinta cajas, tiras e imagenes con lo
   layout, sesiones, codecs         que BMO-X ya tiene (rasterizador,
   -> LAMINA, QOI, MPEG-1           fuente.h, imagen.h)
   olvida                           RECUERDA: cada lamina va a ESTRATOS
   va delante                       decide: pide, niega, guarda
```

** El nombre dice lo que HACE la persona --navegar--, no lo que ES el programa.
No es un navegador, y la diferencia no es de tamano: es de direccion (la misma
frase que abre `docs/plan/PLAN_MAQUETA.md`).

---

# 1. LA REGLA QUE NO CAMBIA CON LAS VERSIONES

```text
   sin antena no hay pagina, y NAVEGAR NO LO TAPA:
     ni cache, ni "ultima vista", ni una pantalla en blanco que parezca cargar.
     Dice que falta la antena, con ese nombre, y espera.

   da igual CUAL antena, pero ANTENA SIEMPRE:
     un movil con Termux, un Arch desmontado, otro BMO-X con red. Lo que se
     exige es el PROTOCOLO (ANTENA/1 + LAMINA), no el aparato.

   lo que llega son DATOS, nunca codigo:
     el lector de `lamina.rs` rechaza con nombre todo lo que no es del
     formato. Un sistema que no puede ejecutar lo que le llega no necesita
     antivirus: necesita un lector estricto.
```

---

# 2. EL SITIO: DONDE VIVE CADA PIEZA, Y POR QUE

Eddi: *"en carpeta que vivira TODOS los elementos necesarios"*. La carpeta
existe, pero **no todo cabe en ella**, y decir por que es la regla MODULAR:

```text
   Ultra_userspace/apps/navegar/         LA APP. `navegar.inti` y lo que sea
                                         SOLO suyo (el recorrido de la lamina
                                         para pintar, el scroll, el foco).
                                         Aqui vive Ring 3; DOOM ya va a `apps/`
   tables/lang/inti/runtime/superficie/  lo que TODA app INTI con ventana va a
   tables/lang/inti/runtime/entrada/     necesitar: NO es de Navegar, es de
   tables/lang/inti/runtime/fuente/      INTI. Vive donde vive el runtime de
                                         INTI (`objetos/`, `monton/`), en REX
   platform/shared/bmo-antena/           el PROTOCOLO y el lector de la lamina:
                                         Rust, puro, con banco. Lo comparten el
                                         ANTENISTA y `cliente.py`
   Ultra_userspace/services/antenista/   EL QUE HABLA con la antena (N3):
                                         Rust, TCP por `bmo-pila`, cuarentena.
                                         No existe todavia
   toolchain/tools/antena/               EL OTRO LADO: `antena.py`, `lamina.js`,
                                         `cliente.py`, y la app Android (AA0)
```

** Por que Navegar NO habla TCP: INTI no tiene modulo `red` ("bloqueada por el
sistema", `modulos.toml`), y no lo va a tener por esto. La red es de
`bmo-pila` (Rust, Ring 3). Asi que son DOS procesos y UN bloque:

```text
   ANTENISTA (Rust)  --TCP-->  antena
        |  ofrece un bloque con la LAMINA (MEM_OFRECER, como una superficie)
        v
   NAVEGAR (INTI)    pinta el bloque; deja CLIC/TECLA en un buzon
        |
        v
   DIRECTOR          compone la ventana de Navegar como cualquier otra
```

El antenista JUZGA (rechaza las faltas de la antena, aplica la cuarentena) y
Navegar PINTA. Navegar vuelve a leer cada linea al pintar --INTI atrapa la
aritmetica que se pase-- pero el juez es uno y esta en Rust.

---

# 3. EL NUMERO INCOMODO: INTI HOY NO ABRE UNA VENTANA

Comprobado el 2026-09-16 con nueve lineas:

```text
   perfil llano
   usa superficie
   usa entrada
   funcion principal
       limpia(mi_superficie(), 2105376)
       texto_en(mi_superficie(), 8, 8, "hola", 16777215)
       pinta(mi_superficie())
       espera_tecla()

   E0075 7 cosa(s) se pidieron y no llegaron a un byte.
     - mi_superficie: la llamada no tiene destino -- no esta en este modulo
       y no hay enlazado
     ... no se ha escrito nada.
```

`[superficie]` y `[entrada]` estan en `tables/lang/inti/modulos.toml` como
NOMBRES, con la nota de que *"los cuerpos estan en REX"*. En REX estan los
cuerpos de C (`tables/bmo/superficie/roja.h` 237 lineas, `amarilla.h` 340,
`fuente.h` + `fuente/datos.h`, `entrada.h` 372); en INTI no hay ninguno. El
compilador hace lo correcto --se niega en vez de escribir un binario al que le
falta algo-- y por eso la version 0 de Navegar escribe por consola.

** Lo que hay que escribir, y cuanto es: el PORT de esas cabeceras a INTI, en
`tables/lang/inti/runtime/`. Pedir memoria, escribir la cabecera de 32 bytes
(`BSUP`, ancho, alto, formato, secuencia), `MEM_OFRECER` al padre, pixeles en
`escribe_natural32` dentro de `crudo` (que el `.ibx` CUENTA), y el buzon de
eventos con el bit 62 (la letra) y el bit 63 (el raton). `bico.inti` ya hace la
mitad (pide bloque, escribe bytes). Y la fuente: `toolchain/tools/fontgen`
escribe hoy dos salidas del mismo arte (Ring 0 y `fuente/datos.h`); hace falta
la TERCERA, en INTI, para que no haya dos fuentes.

---

# 4. "REFLEJA EN TIEMPO REAL": LA LAMINA VIVA, CON NUMEROS

Eddi: *"que lo que MASTICA la ANTENA mi BMO-X lo refleje en tiempo real"*. Lo
que eso puede significar con la LAN medida (~10 Mbit, 16 ms), dicho por
escalones:

```text
   una pagina           89,9 KB (Wikipedia a 640 px) = 72 ms + 16 de ping.
                        Clic -> pagina en ~0,1 s. Esto es Opera Mini, y basta
   la pagina que CAMBIA la antena vigila el DOM (MutationObserver) y reemite
                        la lamina ENTERA cuando cambia, con un suelo de 250 ms:
                        4 laminas/s = 360 KB/s = 2,9 Mbit. Cabe, justo. Un
                        chat que anade un mensaje se ve en un cuarto de segundo
   animacion            NO por lamina: 30 laminas/s serian 22 Mbit. Lo que se
                        mueve dentro de un rectangulo es VIDEO (S4, MPEG-1 a
                        1,5 Mbit) o es ESPEJO (S6). La lamina dice DONDE esta
                        el rectangulo; el video viaja aparte
   scroll, hover        en LOCAL, sin red: la lamina llega entera
```

** "Tiempo real" aqui es **un cuarto de segundo para lo que cambia y cero para
lo que se mueve en local**. Prometer 60 fps por lamina seria mentir con la LAN
de esta casa; prometer 60 fps por ESPEJO es S6 y cuesta lo que dice la seccion
5 de `PLAN_CLOUD_LOCAL.md`.

---

# 5. LO QUE NAVEGAR ENSENA Y LO QUE NO

```text
   ENSENA    cajas, texto (8x16, escala 1..4, Latin-1), imagenes (QOI/BICO),
             enlaces (el cursor cambia encima), campos (se teclea dentro),
             el nombre de la antena y su estado (conectada / cuarentena /
             desterrada), y de que fecha es la lamina
   NO        JavaScript, canvas, WebGL, fuentes proporcionales, sombras,
             degradados, z-index que reordene, animaciones. Google Docs y
             parecidos son ESPEJO o nada
   NUNCA     una pagina en blanco que finja cargar. Sin antena: el mensaje
```

---

# 6. LOS ESCALONES

- [x] **N1a -- Navegar v0: el mensaje, en INTI y con icono.** HECHO el
      2026-09-16: `Ultra_userspace/apps/navegar/navegar.inti` escribe por
      consola que hace falta una ANTENA, que no hay ninguna y que no finge;
      `Ultra_kernel_x86-64/build/ejemplos.ps1` lo compila a `apps/navegar.ibx`
      y le mete su icono. **Como se sabe:** `cargo test -p bmo-inti-x86-64
      --test navegar` lo corre en el emulador y exige las tres palabras
      (ANTENA, Ninguna conectada, no finge); y el icono sale en la rejilla del
      escritorio del Ryzen.

- [x] **N0 -- INTI abre una ventana.** HECHO el 2026-09-16 (ver 8.7): el
      port de `roja.h` y `amarilla.h` a `tables/lang/inti/runtime/superficie/`
      (`roja.inti`, `amarilla.inti`, `dibujo.inti`) y de las teclas por el
      buzon a `runtime/entrada/buzon.inti`; los nombres en `modulos.toml`
      `[superficie]` y `[entrada]` con cuerpo. **Como se sabe:** el programa
      de nueve lineas de la seccion 3 compila sin E0075 (17.072 bytes de
      `.ibx`); `tests/gemelos_inti.rs` de `bmo-c-front` corre el mismo dibujo
      en C y en INTI y exige el mismo bloque byte a byte. Lo que queda para
      el Ryzen es N1.

- [x] **N1 -- Navegar v1: la ventana con el mensaje, en el METAL.** HECHO el
      2026-09-18: la foto del Ryzen ensena la ventana `tid 5 482x141` con el
      fondo oscuro, el titulo en blanco y el aviso de la ANTENA en naranja,
      y `save` no acusa ninguna oferta negada. La "ventana blanca" del 17-09
      no volvio tras los arreglos del USB de esa noche (el escritorio sin
      turno); no se aislo la causa exacta, y se dice. Lo que la foto tambien
      ensena: el texto se corta por la derecha (482 px para 8 palabras de 8
      letras) y el titulo dice `tid 5`, no NAVEGAR -- dos cosas del DIRECTOR,
      no de la app. El original:
      El
      `navegar.inti` v1 ya esta escrito (`usa superficie`: 480x112, 64
      ranuras, el mensaje en la fuente de BMO-X, Esc o `q` cierran, R-APP8)
      y probado en el emulador por los dos caminos
      (`emisor-x86_64/tests/navegar.rs`). **Como se sabe:** clic en el icono
      del escritorio del Ryzen, sale la ventana con el mensaje, `q` la
      cierra, y el DIRECTOR no acusa nada en `cabina fallos`.

- [x] **N2 -- Navegar pinta una lamina DE FICHERO.** HECHO el 2026-09-18
      en el emulador, esperando el Ryzen. Dos cambios sobre lo escrito, dichos:
      la lamina viene de DATOS (`datos/ejemplo.lamina`, la copia el build)
      y no del `.ibx`, porque `paquete.recurso` no tiene cuerpo en INTI; y el
      recorrido vive en `tables/lang/inti/runtime/lamina/lamina.inti` (`usa
      lamina`) y no en la carpeta de la app, porque `usa` solo trae piezas
      del runtime -- y es un FORMATO, que es lo que el runtime guarda. Scroll
      con flechas y AvPag/RePag (la rueda no viaja por el buzon todavia).
      ** Y destapo un fallo del emisor de INTI: del septimo argumento en
      adelante se PERDIAN en silencio (`take(6)`); ahora van por la pila
      (`emisor-x86_64/src/funcion.rs`, prueba `argumentos_altos.rs`).
      **Como se sabe:** `cargo test -p bmo-inti-x86-64 --test navegar`: la
      lamina de example.com en 640x400 con el fondo `eeeeee`, el titulo a
      escala 2 en 121..153, el enlace en su azul; la flecha abajo la sube 32
      px; una caja fuera y un color malo se niegan con su nombre y su linea
      en naranja, y lo de fuera no se pinta. En el Ryzen: `run
      apps/navegar.ibx` con `datos/ejemplo.lamina` en el disco.

- [ ] **N3 -- el ANTENISTA.** `Ultra_userspace/services/antenista`: Rust, Ring
      3, habla ANTENA/1 por `bmo-pila` (G5), aplica `cuarentena.rs`, pide
      `PAGINA <url>`, valida la lamina con `lamina.rs` y la OFRECE a Navegar en
      un bloque; recoge `CLIC`/`TECLA` del buzon de Navegar. **Como se sabe:**
      una pagina real de la antena del movil se pinta en Navegar, y una antena
      que manda una linea basura aparece en CABINA con su falta y Navegar
      ensena "antena en cuarentena, N s".

- [ ] **N4 -- la lamina VIVA.** La antena reemite al cambiar el DOM (suelo 250
      ms) y Navegar repinta; un rectangulo de video se pide por S4 y se pinta
      dentro. **Como se sabe:** un mensaje nuevo en una pagina de chat aparece
      en Navegar en menos de medio segundo, medido con `cabina`.

- [ ] **N5 -- el HISTORIAL.** Cada lamina que entra se guarda en ESTRATOS con
      el nombre de la antena y la hora (`Ultra_userspace/userland/src/estratos.rs`,
      `crear_desde`), y Navegar la reabre sin antena marcada como "de ayer, de
      la antena X" -- que no es fingir: es decir de cuando es. **Como se sabe:**
      `historial` lista las laminas, y con la antena apagada Navegar ensena una
      con su fecha y sin quitar el mensaje de que no hay antena.

---

# 7. EL OTRO LADO: QUE HACE FALTA EN EL MOVIL

Eddi: *"y la ANTENA para educar: que necesito en mi CELULAR para empezar? o
construimos APP pero en carpeta fuera de BMO-X para mi celular o cualquier
Android?"*.

Dos respuestas, en orden de coste:

```text
   HOY, SIN APP   Chrome en el movil + Opciones de desarrollador + depuracion
                  USB + `chrome://inspect` en el Chrome del PC. Se abre la
                  pagina en el movil, se pega `lamina.js` en la consola REMOTA
                  y la lamina sale del HONOR de verdad, con sus milisegundos.
                  Es L0 de `PLAN_CLOUD_LOCAL.md` sin escribir una linea de
                  Android. Paso a paso: `toolchain/tools/antena/GUIA_MOVIL.md`
   DESPUES, LA    una app Android propia (Kotlin): un WebView al ancho que
   APP            BMO-X pide, `lamina.js` inyectado, y un servidor TCP que
                  habla ANTENA/1. Es LA MISMA app que S6 (espejo) va a
                  necesitar. Su FUENTE va en el repo, `toolchain/tools/antena/
                  android/` (es nuestro, Apache-2.0, y son pocos ficheros);
                  lo que va FUERA es el SDK, Gradle y lo compilado -- un
                  `gradle-wrapper.jar` en el repo es un binario que nadie lee
                  en un diff, y ya se dijo que no
```

** "Cualquier Android": si. La app no pide root, ni un fabricante, ni una
version rara: WebView y un socket TCP los tiene cualquier Android de esta
decada. El HONOR es el primero porque es el que hay.

- [ ] **AA0 -- la app Android, en el repo.** `toolchain/tools/antena/android/`
      con el fuente Kotlin (WebView + `lamina.js` + servidor ANTENA/1 con
      `PAGINA`), sin binarios; `LEEME.md` dice como se compila fuera. **Como se
      sabe:** el APK instalado en el HONOR contesta `HOLA ANTENA/1` y a
      `PAGINA <url>` con una lamina que `cliente.py --lamina` acepta.

---

# 8. INTI Y C: COOPERAR, NO FUNDIR (analisis del 2026-09-16, antes de N0)

Eddi: *"primero analiza INTI con C para combinar, pero no literalmente sino
que cooperen"*. Se miro el codigo, no el recuerdo. Lo que hay:

## 8.1 Lo que INTI y C COMPARTEN hoy

```text
   bmo-lower (L1)            los ayudantes genericos: escribir por consola,
                             pedir memoria, salir, los codificadores x86. Los
                             dos frontends lo enlazan
   sem-asm/tables/           las instrucciones (`instructions.toml`), los
                             intrinsecos, `arch/x86_64/abi.toml`. Los CINCO
                             frontends leen las mismas tablas
   el BEF                    `.bex` y `.ibx` son el mismo formato; el mismo
                             cargador, el mismo gate, el mismo escritorio
   el emulador               `bmo_lower::emu::Machine` corre el codigo de los
                             dos en el anfitrion; el banco de C y el de INTI
                             lo usan
   el ABI (con dos jueces)   C: las constantes de `bmo.h` las juzga `contrato`
                             contra el ABI (R13, el espejo sellado). INTI:
                             `[constantes]` de `modulos.toml` las juzga
                             `toolchain/lang/inti/tests/espejo_del_kernel.rs`
                             contra el FUENTE DEL KERNEL, fila a fila.
                             [!] La primera version de esta seccion dijo que
                             "ningun guardian las compara": era FALSO, y se
                             corrigio al leer el fichero. Lo que si faltaba
                             (8.5): la prueba no era exhaustiva --40 filas
                             para 41 constantes-- y el contrato de la
                             SUPERFICIE no estaba en el ABI en ningun sitio
```

## 8.2 Lo que NO comparten, y por que no se pueden enlazar

```text
   el IR                     C baja de su AST a x86 en `lang/c/src/codegen/`;
                             INTI baja de su IR propio en `emisor-x86_64/`. No
                             hay un IR comun por el que pasar un cuerpo de C a
                             un programa de INTI
   el enlazado               no existe entre frontends. `usa monton` es
                             INCLUSION textual (`lib.rs::armar`: "no es
                             enlazado, y la diferencia se paga"); `usa archivo`
                             y `usa superficie` traen nombres de REX "sin
                             destino: hace falta enlazado, y no lo hay".
                             `bmo-linker` es otra cosa: la tabla de simbolos
                             de los `.elf` de Rust
   la convencion de llamada  ** Y ESTA ES LA QUE MAS PESA. C pasa los
                             parametros POR LA PILA (`frame.rs`: empiezan en
                             `[rbp+16]`, por ranuras); INTI los pasa EN
                             REGISTROS (`funcion.rs`: `ARGUMENTOS` = rdi, rsi,
                             rdx, r10, r8, r9, la fila `argumentos` de
                             `arch/x86_64/inti.toml`). Aunque hubiera enlazado,
                             una funcion de C llamada desde INTI leeria basura
                             de la pila. Haria falta un PUENTE por cada llamada
```

** Conclusion de 8.2: "combinar literalmente" (que Navegar en INTI llame a
`bmo_texto` de `fuente.h`) pide tres cosas que no existen --enlazado entre
frontends, un formato de objeto, y un puente de convencion-- y la primera es
la compilacion separada, que `docs/maestro/PYTHON_MAESTRO.md` ya tiene como uno de sus tres
bloqueantes y como el desbloqueo mas valioso del toolchain. No se
compra para abrir una ventana.

## 8.3 Las cuatro formas de cooperar, con su precio

```text
   A  ENLAZAR (compilacion separada)   el destino de verdad: cada cuerpo de
                                       REX escrito UNA vez y usado por cinco
                                       frontends. Semanas. No es de Navegar
   B  POR EL CONTRATO                  el codigo se escribe dos veces (C en
      (lo que la casa ya hace          `roja.h`, INTI en `superficie.inti`) y
      entre C y Rust)                  cada NUMERO vive una vez: la cabecera
                                       de 32 bytes, el indice 5 (secuencia),
                                       el buzon (16 + 8n, bits 62/63),
                                       MEM_OFRECER 0x03, MI_PADRE 0x26. Un
                                       guardian los compara; si C y INTI
                                       discrepan, el build se pone rojo.
                                       Y la FUENTE: fontgen escribe la tercera
                                       salida (INTI) del MISMO arte: un arte,
                                       tres tablas, cero copias a mano
   C  C COMO ORACULO                   el banco corre la MISMA secuencia de
      (como el rasterizador para       dibujo en C (`texto.bex`) y en INTI en
      la GPU)                          el emulador y exige los MISMOS bytes en
                                       el bloque de la superficie. La version
                                       de C, que ya corre en el Ryzen, juzga a
                                       la de INTI antes de que toque el metal
   D  POR PROCESO                      dos programas, uno en cada lenguaje,
      (MEM_OFRECER)                    cooperando por bloques. Es lo que hace
                                       el ANTENISTA (Rust) con Navegar. Para
                                       la ventana no: "todo en INTI" es la
                                       peticion, y una app partida en dos
                                       procesos para pintar texto seria
                                       esconder que INTI no sabe pintar
```

## 8.4 La decision: B + C, y A queda escrita como destino

N0 se hace en INTI, con C de ORACULO y el CONTRATO de juez:

```text
   1. N0a  la FORMA de la superficie entra en el ABI UNA vez, y las tres
           copias --C, Rust, INTI-- pasan a tener juez (hecho, ver 8.5)
   2. N0b  fontgen, cuarta salida: `runtime/fuente/datos.inti` del mismo
           arte que `fuente/datos.h` y la tabla de Ring 0 (hecho, ver 8.6)
   3. N0   el port: roja.h y amarilla.h enteros, fuente.h como dibujo, y
           las teclas por el BUZON (no la entrada exclusiva de entrada.h).
           Los `crudo` se cuentan en el informe del .ibx; la aritmetica que
           en C dio dos #PF en `raycaster_C.c` aqui ATRAPA (hecho, ver 8.7)
   4. la prueba de GEMELOS en el emulador: C e INTI dibujan lo mismo, los
      bytes del bloque coinciden. Si no coinciden, gana C (ya corre en el
      Ryzen) hasta que se demuestre lo contrario en el metal (hecho, 8.7)
```

** Lo que INTI gana y C no puede dar: cero comportamiento indefinido, los
sitios sin comprobacion CONTADOS, y una app --Navegar-- que viaja entera en un
lenguaje. Lo que cuesta: escribirlo dos veces hasta que exista A. Se dice, y
se acepta con los ojos abiertos.

## 8.5 N0a, HECHO el 2026-09-16: el contrato de la superficie, una vez

Lo que se encontro al ir a hacerlo, que no era lo que decia el analisis:

```text
   el contrato de la superficie (BSUP, la cabecera de 32 bytes, el buzon de
   16 + 8n, los bits 63/62/61, las VISTAS) vivia en DOS copias a mano:
      C      tables/bmo/superficie/roja.h y amarilla.h   (BMO_SUP_*)
      Rust   director/scene/surface.rs (MAGIC, HEADER_TAG, BUZON_TAG) y
             desktop/keys/app.rs (CARACTER = 1 << 62)
   cada una con un comentario "el mismo numero que...". Un comentario no es
   un juez. INTI iba a ser la TERCERA copia
```

Lo que se hizo, en el orden en que se hizo:

```text
   ABI     platform/abi/bmo-abi/src/syscalls/surface/superficie.rs: 18
           constantes SUP_*, con la cabecera y el buzon dibujados
   C       contrato_rex: familia ("SUP_", "BMO_SUP_"); el guardian encontro
           17 parejas SIN SELLAR y paro el build hasta que una persona las
           mirara (R13); selladas con nota en VALKYRIE-ABI/ESPEJO.txt.
           100 -> 117 parejas. (SUP_CAMPO_SECUENCIA no tiene gemelo en C:
           roja.h escribe el 5 en linea, y queda dicho)
   Rust    bmo-userland lleva SUP_* con nombre; el DIRECTOR lee ESOS y ya
           no tiene literales propios. RE_OPS y RE_OPS_USER de contrato
           cubren SUP_*: 97 -> 115 constantes del userland juzgadas (R4)
   INTI    modulos.toml [constantes]: 18 nombres (sup_*, evento_*, estado_*,
           vista_*). espejo_del_kernel.rs lee ahora de DOS fuentes (el kernel
           para las puertas, el ABI para la forma), gana la fila de mi_tarea
           (la 41, que no la miraba nadie) y es EXHAUSTIVO: una constante sin
           fila hace fallar la prueba con su nombre
```

Y se comprobo que los tres jueces MUERDEN, no que existan: `SUP_CABECERA`
a 36 en el userland -> R4 rojo con el nombre; `BMO_SUP_BUZON_RANURA` a 16 en
C -> R13 rojo citando el sello; `sup_cabecera` a `0x24` en INTI -> la prueba
dice `sup_cabecera = 0x24, y SUP_CABECERA dice 0x20`; y una constante nueva
sin fila -> `sin fila en ESPEJO: sin_juez`.

** Esto es "mejorar C e INTI para que cooperen" en su forma concreta: no
comparten codigo (no pueden), comparten un contrato con juez en cada copia.
El port de N0 escribe `sup_magic` y `evento_letra`, nunca `0x50555342`.

- [x] **N0a -- el contrato de la superficie, una vez.** HECHO el 2026-09-16:
      `platform/abi/bmo-abi/src/syscalls/surface/superficie.rs`, las tres
      copias juzgadas (R13 para C, R4 para el userland de Rust,
      `toolchain/lang/inti/tests/espejo_del_kernel.rs` exhaustivo para INTI).
      **Como se sabe:** cambiar cualquiera de los tres numeros pone el build
      en rojo con el nombre de la constante, y una constante nueva en
      `modulos.toml` sin fila en el espejo tambien.

## 8.6 N0b, HECHO el 2026-09-16: la fuente en INTI, y dos fallos del compilador debajo

`toolchain/tools/fontgen` escribe la CUARTA salida del mismo arte:
`tables/lang/inti/runtime/fuente/datos.inti`, lo que trae `usa fuente`. Cada
glifo son dos palabras de 64 bits (filas 0..7 con la fila 0 en el byte bajo, y
8..15), asi que `GLIFOS + g * 16 + f` es el byte de la fila `f`: la misma
cuenta que en C y en el kernel, y la tabla pesa lo mismo (1.920 B). Trae
`glifo_fila(g, f)` (un glifo que no existe da el HUECO `?`; el unico `crudo`
solo lee dentro de la tabla) y `glifo_de(byte)` (ASCII directo, los 25 extras
Latin-1 por su byte, el HUECO para lo demas).

** Y el juez no compara ficheros: `lang/inti/emisor-x86_64/tests/fuente.rs`
CORRE el `.ibx` en el emulador, le pregunta las 1.920 filas y los 256 bytes, y
los compara con `fuente/datos.h` leido como texto. Al hacerlo salieron DOS
fallos del compilador que ningun programa habia pisado:

```text
   la quinta familia    `imul` para TODO producto. Sus banderas dicen si cabe
   del fallo del signo  CON SIGNO: `255 * 2^56` sobre natural64 --que cabe--
                        atrapaba por la Regla 1, y `255 * 2^48` no. Ahora un
                        natural se multiplica con `mul` (F7 /4, `rdx:rax`),
                        cuyo CF es el acarreo que el `jc` sin signo espera. El
                        emulador del banco no tenia `mul`: se le anadio con las
                        mismas banderas que el silicio. Apuntado en
                        `medidas.toml`, seccion `sin_signo`, como la quinta
   el tipo de una       `Plano::tipo_de` contestaba `None` para una LLAMADA, y
   llamada              `sin_signo` daba `false`: `glifo_fila(g, f) *
                        potencia(f)`, dos natural64 declarados, se comprobaba
                        con signo aunque el emisor ya multiplicara bien.
                        `deduccion.rs` SI lo sabia (para `x = f()`): eran dos
                        criterios para la misma pregunta, justo lo que la
                        cabecera de `tipo_de` dice que no puede pasar. Ahora
                        una llamada tiene el tipo que la funcion DIJO y una
                        operacion aritmetica hereda el de su lado tipado
```

Y una tercera cosa que no era un fallo sino una ausencia: ninguna prueba de
`tests/` habia corrido un programa con una tabla congelada, asi que la tabla
se leia de la direccion cero -- del CODIGO -- y `glifo_fila` devolvia opcodes
(`0x48, 0x8a, 0x8b`). `pruebas.rs::ejecuta_en` ya lo contaba del 23-08; ahora
`tests/fuente.rs` rearma `RoData` con el mismo `rodata_de` que usa el `.ibx`.

** Lo que esto ensena de "cooperar": el ORACULO (C) cazo tres cosas en INTI el
primer dia, y ninguna estaba en la tabla de glifos.

- [x] **N0b -- fontgen, la cuarta salida.** HECHO el 2026-09-16:
      `toolchain/tools/fontgen` escribe `tables/lang/inti/runtime/fuente/datos.inti`
      del mismo arte; `tables/bmo/fuente/datos.h` y la tabla del kernel salen
      byte a byte iguales que antes. **Como se sabe:** `cargo test -p
      bmo-inti-x86-64 --test fuente` corre `usa fuente` en el emulador y exige
      los 1.920 bytes y los 256 indices de `datos.h`; y `pruebas/signo.rs`
      exige que `255 * 2^56` en natural64 no atrape y `2^32 * 2^32` si.

## 8.7 N0 y N0c, HECHOS el 2026-09-16: INTI abre una ventana, y C lo juzga

```text
   runtime/superficie/roja.inti      pedir el bloque, la cabecera BSUP por
                                     NOMBRE (sup_magic, sup_campo_secuencia...),
                                     el buzon, la COLA PRIVADA (el handle del
                                     bloque y los bytes ofrecidos, DETRAS de lo
                                     ofrecido: un handle a la vista de otro
                                     proceso seria una capability regalada),
                                     MEM_OFRECER al padre; 0 si nadie compone o
                                     la oferta se rechaza
   runtime/superficie/amarilla.inti  el buzon: superficie_evento (anillo con
                                     mascara), evento_es_raton / es_letra /
                                     es_configurar y sus campos, el puntero,
                                     la VISTA, tomada
   runtime/superficie/dibujo.inti    limpia, pixel, rectangulo y texto_en con
                                     RECORTE: el unico crudo que escribe un
                                     pixel solo lo llaman funciones que ya
                                     recortaron. texto_en pinta OCHO letras por
                                     palabra (`ocho_bytes`): en llano no hay
                                     `texto`, y no es temporal
   runtime/entrada/buzon.inti        espera_tecla (duerme 4 ms entre miradas),
                                     hay_tecla, raton -- por el BUZON, no por
                                     la entrada exclusiva de entrada.h, que es
                                     la del shell de Ring 0
```

Lo que INTI no tiene y el port tuvo que respetar, dicho:

```text
   sin globales      no hay `mi_superficie()`: la superficie es un natural64
                     que la app guarda y pasa. El nombre se fue de la tabla
   sin `texto`       en llano un literal es una palabra de ocho bytes
   `y` es operador   una coordenada no puede llamarse `y`: son px/py, x0/y0
   `o` no cortocircuita   `si s = 0 o campo(s) = 0` LEE el bloque 0. Cada
                     pregunta en su `si`, la de `s` primero
   sin liberar       el kernel no tiene hoy operacion para devolver un bloque
                     (MEM_OP_*: base, bytes, ofrecer, fisica): reconfigurar
                     pide otro y el viejo se queda, igual que en C sin el
                     `free` que alli solo devuelve al monton de la app
   `usa` transitivo  una pieza del runtime con su propio `usa` (dibujo pide
                     `fuente`) no traia nada: `armar` es ahora una cola y
                     cada modulo entra una vez
```

** N0c, los gemelos: `toolchain/lang/c/src/tests/gemelos_inti.rs` compila el
MISMO dibujo (64x32, ocho ranuras, fondo, un rectangulo, "Hola") con los dos
frontends, lo corre en el emulador con un DIRECTOR de mentira (`padre = 7`,
`emu/director.rs`: contesta MI_PADRE, acepta la oferta y reparte eventos al
buzon mientras la app duerme) y compara el bloque OFRECIDO byte a byte:
cabecera, pixeles y buzon. Se comprobo que muerde: un pixel de mas en el
rectangulo de INTI son 60 bytes distintos con su coordenada; un glifo
equivocado, 312. Y sin padre, ninguno de los dos ofrece nada y los dos se
van con codigo 1.

- [x] **N0c -- los gemelos.** HECHO el 2026-09-16:
      `toolchain/lang/c/src/tests/gemelos_inti.rs` corre la misma secuencia
      (limpia, rectangulo, texto) en C y en INTI y compara el bloque de la
      superficie byte a byte. **Como se sabe:** la prueba pasa, y mover el
      rectangulo un pixel o cambiar un glifo la pone en rojo con la
      coordenada del primer byte distinto.
