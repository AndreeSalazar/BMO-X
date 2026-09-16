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
