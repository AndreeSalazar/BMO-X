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

- [ ] **N0 -- INTI abre una ventana.** El port de `tables/bmo/superficie/*.h`,
      `fuente.h` y `entrada.h` a `tables/lang/inti/runtime/superficie/`,
      `fuente/` y `entrada/`, y la tercera salida de `toolchain/tools/fontgen`
      (los glifos en INTI). **Como se sabe:** el programa de nueve lineas de la
      seccion 3 compila sin E0075 y pinta "hola" en una ventana del Ryzen que el
      DIRECTOR compone; el informe del `.ibx` cuenta los `crudo` del runtime.

- [ ] **N1 -- Navegar v1: la ventana con el mensaje.** `navegar.inti` con
      `usa superficie` y `usa entrada`: el mensaje de v0 en su ventana, cierra
      con Esc. **Como se sabe:** clic en el icono del escritorio, sale la
      ventana con el mensaje, Esc la cierra, y el DIRECTOR no acusa nada en
      `cabina fallos`.

- [ ] **N2 -- Navegar pinta una lamina DE FICHERO.** `ejemplo.lamina` como
      recurso del `.ibx` (`paquete.recurso`), el recorrido de la lamina en INTI
      (`Ultra_userspace/apps/navegar/lamina.inti`: las mismas reglas que
      `lamina.rs`, con la aritmetica atrapada), scroll con rueda y flechas en
      local. **Como se sabe:** la lamina de example.com se ve en el Ryzen igual
      que en el navegador (el parrafo de tres lineas a 16 px), y una lamina con
      una caja fuera se niega con su nombre en la ventana.

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
   2. N0b  fontgen, tercera salida: `runtime/fuente/datos.inti` del mismo
           arte que `fuente/datos.h` y la tabla de Ring 0
   3. N0   el port: 39 funciones de C (roja.h 8, amarilla.h 17, fuente.h 3,
           entrada.h 11; ~526 lineas de codigo sin comentarios) a
           `runtime/superficie/`, `runtime/fuente/`, `runtime/entrada/`.
           Los `crudo` se cuentan en el informe del .ibx; la aritmetica que
           en C dio dos #PF en `raycaster_C.c` aqui ATRAPA
   4. la prueba de GEMELOS en el emulador: C e INTI dibujan lo mismo, los
      bytes del bloque coinciden. Si no coinciden, gana C (ya corre en el
      Ryzen) hasta que se demuestre lo contrario en el metal
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

- [ ] **N0b -- fontgen, la tercera salida.** `toolchain/tools/fontgen` escribe
      `tables/lang/inti/runtime/fuente/datos.inti` del mismo arte. **Como se
      sabe:** los 120 glifos de INTI y los de `fuente/datos.h` son los mismos
      bytes, comprobado por una prueba de fontgen.

- [ ] **N0c -- los gemelos.** Una prueba en `emisor-x86_64/tests/` corre la
      misma secuencia (limpia, rectangulo, texto) en C y en INTI y compara el
      bloque de la superficie byte a byte. **Como se sabe:** la prueba existe,
      pasa, y una tilde movida en la fuente de INTI la pone en rojo.
