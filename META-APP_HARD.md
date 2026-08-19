# META-APP HARD

> **La ley de una app.** No la firma un framework: la firma **la superficie del
> sistema** -- que a su vez la firmo el silicio.
>
> Escrito el **2026-08-18**, el dia que la primera app completa de BMO-X
> --cara compilada, motor en COBOL-- se ejecuto en el Ryzen y quedo claro que le
> faltaba una sola pieza para poder vivir sola.
>
> Hermano de [`META-KERNEL_HARD.md`](META-KERNEL_HARD.md) y con la misma forma:
> aqui una regla solo existe si al lado tiene **de donde sale** y **que se
> rompe** si no se cumple.

---

## 0. ★★ LA CADENA QUE ORDENA EL DOCUMENTO ENTERO

```
   el SILICIO      firma la ley del kernel      META-KERNEL_HARD.md
   la SUPERFICIE   firma la ley de una app      este documento
```

El Meta-Kernel dice: *una regla solo existe si trae el componente que la exige y
el numero con el que la exige.* La Meta-App es la misma frase un anillo mas
arriba: **una app no puede hacer nada que la superficie no conceda, y la
superficie son dos syscalls.**

### Y la consecuencia que la gente no espera: NO HAY FRAMEWORK

No es una carencia, es la propiedad. Una app de BMO-X no depende de que
sobreviva la libreria de nadie, ni de que una version mayor no rompa la anterior,
ni de un runtime que haya que embarcar. **Depende de dos syscalls congelados.**

Es lo contrario del error de Tiny Core: nucleo de 10 MB y, en cuanto habia que
maquetar una ventana, entraban X11 y GTK.

---

## 1. LO QUE YA ESTABA DECIDIDO Y NO SE RE-DISCUTE

De `docs/identidad/EL_CONTRATO_DE_CARGA.md`, y es la frase de la que cuelga todo
lo demas:

> **El programa DECLARA, el sistema CONCEDE, el kernel solo COMPRUEBA.**

Una app no PIDE permiso en ejecucion: dice de antemano lo que necesita, y lo que
no declaro no existe para ella. No hay `root` al que escalar porque no hay `root`.

---

## 2. QUE EXIGE BMO-X DE ALGO QUE QUIERA SER UNA APP

Cada regla trae de donde sale y que se rompe sin ella.

### R-APP1 -- Es UN fichero

Un `.bex` con sus recursos dentro (seccion `Resources` 0x0B): icono, datos,
lo que haga falta. **Sin esto**: una app son varios ficheros que se pueden
separar, y el dia que uno falte el fallo aparece a mitad de ejecucion.

*De donde sale*: el formato BEF ya lo llevaba y `bmo-pack` lo escribe.

### R-APP2 -- Declara lo que necesita, y no puede mentir

El manifiesto de capabilities viaja **dentro** del binario, y en ESTRATOS ni
siquiera se puede separar de el (`:manifiesto` es un atributo del mismo nodo).
**Sin esto**: los permisos viven fuera del programa y alguien los edita.

### R-APP3 -- Dibuja en SU memoria y la OFRECE. No toma la pantalla

`MEM_OP_OFRECER` con el tid que da `TASK_OP_MI_PADRE`. El DIRECTOR la mapea una
vez y a partir de ahi lee pixeles con un `mov`.

**Sin esto**: el modelo viejo, `lend_screen` -- la app se lleva la pantalla
entera y mientras vive no hay escritorio. Es lo que hace DOOM hoy, y es la razon
por la que DOOM no puede compartir sitio con nada.

### R-APP4 -- Sube su `sequence` cuando el dibujo esta ENTERO

No hay cerrojo y no debe haberlo. **Sin esto**: se compone un fotograma a
medias, y el peor caso deja de ser "se ve el anterior una vuelta mas" para pasar
a ser "se ve medio dibujo".

### R-APP5 -- Nadie se cree sus numeros

La cabecera `BSUP` la escribe la app, o sea otro proceso. El DIRECTOR comprueba
que **lo que declara cabe en los bytes que el kernel dijo que presto**, en `u64`.
**Sin esto**: una app que declare `4000 x 4000` en un bloque de 1 MiB se lleva el
escritorio por delante, y el fallo de pagina lo cobra el DIRECTOR.

★ La regla en una frase: **una app rota no se lleva el escritorio.**

### R-APP6 -- Muere sin llevarse a nadie

Se pregunta cada fotograma con `PRESTADO_OP_DUENO`. Una app muerta deja su
`sequence` congelada, que es **indistinguible de una app pensando**. **Sin
esto**: la ventana de un programa que ya no existe se queda en pantalla para
siempre.

### R-APP7 -- Su cara es dato o codigo generado, nunca un motor

MAQUETA compila la maquetacion en el anfitrion y emite coordenadas. **Sin
esto**: cada app lleva dentro un motor de composicion, que es como un escritorio
de 10 MB acaba pesando 400.

*De donde sale*: `docs/componente/LA_MAQUETA_EXIGE.md`.

---

## 3. Y QUE LE DEVUELVE EL SISTEMA -- la otra mitad del trato

Una ley que solo exige es un peaje. Esto es lo que se recibe **sin escribir una
linea**:

| lo que da | de donde sale |
|---|---|
| marco, arrastre, estirar, minimizar/maximizar/cerrar | `scene/chrome.rs` |
| composicion **sin una sola copia** por fotograma | el prestamo de memoria |
| icono en el escritorio y lanzamiento por clic | `BICO` + `bmo-pack` + `scene/launcher.rs` |
| quien tiene el teclado, decidido por el usuario | `bmo_input::foco` |
| decimal exacto en centavos | el COBOL de la casa |
| una cara declarativa, con su tabla de golpeo | MAQUETA |
| que un fallo suyo no mate el escritorio | R-APP5 y R-APP6 |

★★ **Eso ultimo es lo que ningun framework da**: aqui el aislamiento no es una
promesa de la libreria, es la frontera del proceso.

---

## 4. LA CASILLA QUE FALTA, Y ES UNA SOLA

```
   HECHO   la app dibuja  ->  el DIRECTOR compone   ->  sale en su marco
   FALTA   el dedo        ->  la app
```

Las superficies de hoy son **de salida**. Una app puede ensenar; no la puedes
tocar. El camino, las dos opciones y su precio estan en
`docs/plan/PLAN_DIRECTOR.md`, paso 2c -- y la conclusion medida es que **no hay
transporte que construir**: `bmo-channel` ya entrega llamadas de un Ring 3 a
otro, y es lo que usa Endpoint RPC desde que `rpc-demo` paso por hardware.

---

## 5. ★★ QUE POTENCIAL TIENE ESTO -- la escalera, sin humo

La pregunta del dueno fue *"hasta quizas juegos y apps famosas"*. La respuesta
honesta es una escalera, y lo util es saber en que escalon esta cada cosa.

### 5.1 Lo que cabe HOY, con lo que ya existe

Cualquier cosa cuya cara sea **fija y su dato pequeno**: la calculadora, un
reloj, un monitor, un panel de estado. Lo unico que las separa de ser apps
sueltas es la casilla 4.

### 5.2 Lo que cabe EN CUANTO entre la entrada

Todo lo que sea **mirar y senalar**: un explorador de ficheros, un editor de
texto, un visor de imagenes, un juego por turnos. Y lo que ya esta portado y hoy
se lleva la pantalla entera --`ray.bex`, DOOM-- pasa a vivir en una ventana.

★ **DOOM ya corre**, y eso no es una anecdota: es la prueba de que la superficie
aguanta un programa real de decenas de miles de lineas. Lo que le falta a DOOM no
es potencia, es **sitio compartido**.

### 5.3 Lo que necesita mas, y se sabe QUE mas

| lo que falta | que desbloquea |
|---|---|
| **SDL** | ★★ la palanca mas grande que existe: miles de juegos apuntan ahi |
| compilacion separada | proyectos de mas de un fichero sin dolor |
| `libm` completa | todo lo que haga trigonometria o punto flotante serio |
| monton de verdad | cualquier cosa que no sepa su tamano al compilar |

★★ **SDL es una capa fina sobre "dame una superficie, dame teclas, dame
sonido"** -- y BMO-X tendra las tres. Portar SDL no es portar un juego: es
portar el catalogo.

### 5.4 Y lo que NO va a caber, dicho para que nadie se lo prometa

Un navegador. Una suite ofimatica moderna. Nada que arrastre millones de lineas
de dependencias ajenas, porque **eso no es un problema de sistema operativo: es
un problema de plantilla**.

★ Decirlo no es rendirse. Es la misma regla que hizo terminable a ESTRATOS y a
BMO C: **seis cosas bien y acotado**. Un sistema que promete Photoshop es un
sistema que no termina la calculadora.

---

## 6. LO QUE SERIA UN ERROR

- **Un framework.** En cuanto una app dependa de una capa que no sea la
  superficie, la superficie deja de ser el contrato y el framework pasa a serlo.
  Ese es el dia en que BMO-X hereda las deudas de otro.
- **Un runtime obligatorio.** Si una app necesita que algo este vivo para
  arrancar, ese algo es un punto unico de fallo que la ley del kernel no
  contempla.
- **Una API general antes de tener clientes.** Una API es una promesa al codigo
  de otros; sin otros, es coste sin comprador. Ver `docs/maestro/IPC_MAESTRO.md`,
  seccion 5.
- **Copiar la forma de una app de escritorio ajena** cuando el sistema pide otra
  cosa. Ya esta dicho para el explorador: *se copia que se entienda sin
  explicacion, no la forma*.

---

Ver `META-KERNEL_HARD.md` (la ley de la maquina), `docs/plan/PLAN_DIRECTOR.md`
(la casilla que falta), `docs/identidad/EL_CONTRATO_DE_CARGA.md` (declarar y
conceder) y `docs/componente/LA_MAQUETA_EXIGE.md` (la cara).
