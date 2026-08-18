# PLAN: LA CARA VIAJA

> **Que pasa cuando la maquetacion deja de ser codigo y pasa a ser un DATO.**
>
> Escrito el **2026-08-17**, despues de cablear la calculadora al escritorio con
> el emisor A (codigo Rust generado). Este documento **no se implementa todavia**:
> se escribe ahora porque el POR QUE es largo y porque decidirlo tarde seria
> decidirlo mal.
>
> Nace de una pregunta del dueno el mismo dia:
>
> > *"podria construir un CRATE especial para recibir datos de navegador? Se que
> > suena extrano pero ha habido casos como Gemini, Plan 9, Tiny Core, QNX..."*
>
> La respuesta corta es **si, y el camino corto no pasa por la red**. La larga es
> este fichero.

---

## 0. QUE ES, EN UNA FRASE

**El emisor B**: en vez de generar Rust que se compila dentro del servicio,
escribir la maquetacion como un **recurso** que se lee en ejecucion.

```
   HOY (emisor A)   .maqueta -> calc_gen.rs -> se compila con el compositor
   MANANA (B)       .maqueta -> calc.recurso -> se LEE al arrancar la app
```

Ninguna de las cinco generaciones cambia. **Ninguna sabe que existe un emisor**,
asi que B es un modulo mas en `emit/` -- eso ya esta demostrado, no prometido.

---

## 1. EL POR QUE, EN CUATRO RAZONES

### 1.1 Hoy la cara solo cambia RECOMPILANDO EL SISTEMA

Cambiar el hueco entre dos teclas de 6 a 8 pixeles cuesta hoy:

```
   editar calc.maqueta
   -> regenerar calc_gen.rs
   -> compilar bmo-service-gui
   -> enlazar el .bex
   -> grabar el disco
   -> REINICIAR EL RYZEN
```

Y esa frase ya esta escrita en este arbol, en `vista-ciudad`, sobre otra cosa:

> *"Un arranque animado que solo se puede juzgar reiniciando la maquina es un
> arranque que nadie va a ajustar nunca."*

**Un escritorio que solo se puede ajustar reiniciando es un escritorio que nadie
va a ajustar nunca.** El emisor A no arregla eso; lo mueve de sitio.

### 1.2 Una app deberia traer su propia cara, y el sitio YA EXISTE

El paquete BEF tiene desde hace tiempo la seccion **`Resources 0x0B`**, con su
indice, y `bmo-pack` sabe meter datos dentro de un `.bex`. **Nadie la lee en
ejecucion.** Una app es UN fichero, y hoy su cara no viaja dentro de el: viaja
compilada dentro del compositor, que es un sitio que no le corresponde.

★ Esto invierte quien manda: hoy **el escritorio sabe pintar la calculadora**.
Con el recurso, **la calculadora trae su cara** y el escritorio solo la aloja.
Es la misma frase que ya sostiene `PLAN_DIRECTOR.md` para los pixeles vivos, un
escalon mas abajo.

### 1.3 Es lo que convierte el escritorio en texto DE VERDAD

`PLAN_MAQUETA.md` seccion 1 dice que MAQUETA puede ser **mejor que Arch**: el
control de Arch es en ejecucion y puede arrancar a un sistema roto; MAQUETA es
control en compilacion y un texto que no compila no llega a arrancar.

El recurso da **las dos cosas a la vez**, y esa es la parte que merece un
documento:

```
   el .maqueta se compila en el ANFITRION   -> el veredicto ya corrio
   el recurso se lee en el APARATO          -> se cambia sin recompilar
```

Se edita en caliente **lo que ya paso por el juez**. Arch no puede ofrecer eso
porque no tiene juez; un fichero de configuracion se lee tal cual.

### 1.4 Y es lo unico que hace falta para que la cara venga DE FUERA

Aqui entra la pregunta del navegador, y conviene decirla al derecho:

> ★★ **Si el otro extremo es un navegador, pagas precios de navegador.** Para que
> un navegador te mande algo hay que hablar lo que el habla: TCP + TLS + HTTP, y
> WebSocket si se quiere vivo. Por la medida de este mismo arbol, *el driver de
> red es el 5% y la pila es el 95%*. **La factura no la trae la idea: la trae el
> interlocutor.**

Gemini hizo exactamente eso: **cambiar de interlocutor**. Y ahi el caso de BMO-X
es mas fuerte que el suyo, por algo que ya esta construido:

```
   Gemtext manda UN DOCUMENTO PARA MAQUETAR
   MAQUETA mandaria UNA MAQUETACION YA RESUELTA
```

El aparato no maqueta: **pega**. El motor de maquetacion **no viaja**.

**Y lo importante para el orden del trabajo: nada de esto necesita red para
demostrarse.** Un recurso puede llegar por **fichero**, y BMO-X ya sabe leer
ficheros. La red solo cambia **por donde llega**.

---

## 2. LO QUE YA ESTA CONSTRUIDO, Y NO HAY QUE INVENTAR

| pieza | estado | de donde sale |
|---|---|---|
| las cinco generaciones | ✅ hechas, 145 pruebas | `toolchain/tools/maqueta/` |
| el veredicto (10 comprobaciones) | ✅ hecho | `verdict/` |
| el emisor como consumidor | ✅ demostrado con A | `emit/` |
| la seccion `Resources 0x0B` del BEF | ✅ formato + indice + `bmo-pack` | [[bmo-paquete-bef]] |
| leer ficheros en ejecucion | ✅ FAT32 y ESTRATOS montan | `dev/disk/` |
| la isla = superficie | ✅ `BSUP`, y ya usada por el visor | `PLAN_DIRECTOR.md` |
| **leer un recurso en ejecucion** | ❌ **esto es lo que falta** | -- |

**Una sola pieza.** Todo lo demas esta puesto y probado.

---

## 3. EL FORMATO, Y LO QUE CUESTA

Medido sobre la calculadora ya compilada: **28 cajas, 17 golpes, 1 isla**.

```
   cabecera        magico 4 + version 2 + ancho 2 + alto 2 + cuentas 6   =   16 B
   28 cajas        x,y,w,h (8) + fondo,borde (8) + grosor,radio (2)      =  504 B
   17 textos       x,y (4) + color (4) + largo (1) + letras (~1)         =  170 B
   17 golpes       rect (8) + indice de nombre (2)                       =  170 B
   nombres         ~5 B x 18                                             =   90 B
                                                                          -------
   LA CARA ENTERA DE LA CALCULADORA                                       ~ 950 B
```

★ **Menos de un kilobyte.** Y no es que salga pequeno por apretarlo: sale pequeno
porque **lo caro se quedo en el anfitrion**. Un navegador tendria que mandar el
documento *y* traer el motor que lo maqueta.

Para comparar, en el mismo arbol: el `.bex` de una app pequena son decenas de
KiB, y un solo fotograma de 1920x1080 son 8,3 MB.

---

## 4. LOS TRES TRANSPORTES, Y QUE DEMUESTRA CADA UNO

```
   1. FICHERO EN EL DISCO      ya se puede   ->  demuestra la IDEA ENTERA
   2. DENTRO DEL .bex (0x0B)   falta leerlo  ->  la app trae su cara
   3. POR LA RED               falta la pila ->  solo cambia POR DONDE llega
```

★★ **El 1 y el 2 demuestran todo lo que hay que demostrar.** Que la cara sea un
dato, que se cambie sin recompilar, que una app la traiga consigo, que el aparato
no maquete. El 3 no anade ni una idea: anade **distancia**.

Por eso la red no esta en la escalera de este documento. Cuando llegue --por
`smoltcp`, o por lo que sea-- este trabajo estara hecho y sera un cambio de
`fread` por `recv`.

---

## 5. LOS CUATRO PRECEDENTES, LEIDOS CON HONESTIDAD

### Gemini / Gemtext -- **transfiere la tesis, no el formato**

Acertaron en el diagnostico (la complejidad de la web es un error) y pagaron por
ello: renunciaron a la estetica entera. MAQUETA no tiene que renunciar, **porque
compila**: los bordes, los colores y el flex se resuelven antes de viajar.

⚠ Y lo que hay que copiarles de verdad no es el formato: es **haber cambiado de
interlocutor**. Un `maqueta://` que hable con BMO-X en los dos extremos cuesta
casi nada; hablar con un navegador cuesta la pila entera.

### Plan 9 / 9P -- **transfiere el modelo**

*"Una ventana puede vivir en un servidor y dibujarse localmente."* Eso es
exactamente un recurso de maquetacion mas una isla: **el sitio viaja, los pixeles
se pintan donde estan**. La diferencia es que 9P mandaba operaciones de dibujo y
aqui viaja **la geometria ya resuelta**, que es menos aun.

### QNX / Neutrino -- **transfiere el limite**

*Mensajes compactos, no capas de abstraccion.* Los ~950 bytes de arriba son
literalmente eso. Y su leccion util es la que duele: **lo que hunde a un sistema
grafico pequeno no es el dibujo, son las capas** -- que es la misma razon por la
que aqui el motor de maquetacion no viaja.

### Tiny Core / SliTaz -- **transfiere el AVISO, y es el mas util de los cuatro**

Tenian el nucleo en 10 MB y en cuanto habia que maquetar una ventana entraban
X11/Wayland y GTK/Qt y el consumo se iba a mas de 1 GB. **El nucleo pequeno no
les sirvio de nada porque estaban encadenados a la capa de dibujo de otro.**

★ Ese es el aviso para BMO-X y no es teorico: **el dia que se quiera "recibir
datos de un navegador" en serio, la tentacion sera portar algo que ya sepa
hablarle** -- y eso trae la capa entera. La forma de no repetir su historia es
tener **cara propia**, que es este documento.

---

## 6. ⚠ EL PELIGRO, Y ES UNO SOLO PERO GORDO

**Si la cara viaja, el veredicto NO viajo con ella.**

Hoy las diez comprobaciones corren en el anfitrion sobre el `.maqueta`. Un
recurso que llega de fuera --de un fichero editado a mano, de un `.bex` de otro,
de la red-- **no ha pasado por ningun juez**, y el que lo lee es el compositor.

Y esto ya tiene precedente escrito, en `PLAN_DIRECTOR.md`:

> `Cabecera::leer` valida ancho/alto/stride **contra los bytes que dijo el
> kernel**, en `u64` (en 32 bits el producto se desborda y da un total pequeno).
> Sin eso, una app que declare 4000x4000 en 1 MiB hace que el compositor lea
> fuera del prestamo.

Lo mismo, palabra por palabra, para el recurso. **Lo que el lector tiene que
comprobar** -- y no es el veredicto, es la ESTRUCTURA:

1. El magico y la version.
2. Que las cuentas declaradas **caben en los bytes que hay**, en `u64`.
3. Que todo desplazamiento de nombre **cae dentro** del buffer.
4. Que ningun rect se sale del lienzo declarado.
5. Que el lienzo declarado cabe en la pantalla.

★ **La diferencia entre las dos listas importa**: el veredicto juzga si la
maquetacion es BUENA (el texto cabe, la caja no se sale de su padre) y eso se
puede seguir haciendo solo en el anfitrion. El lector comprueba si el fichero es
**seguro de leer**, y eso hay que hacerlo siempre, aunque el fichero lo haya
hecho uno mismo.

Un recurso corrupto no debe dar un `#PF` en el compositor. **Una app rota no se
lleva el escritorio** -- que es la misma ley que ya rige las superficies.

---

## 7. LO QUE ESTO **NO** DA

- **No da maquetacion en el aparato.** El recurso trae rects; nadie los
  recalcula. Si la ventana cambia de tamano, el recurso no se adapta -- eso
  seguiria pidiendo un motor dentro, que es la linea que no se cruza.
- **No da repeticion sobre datos vivos.** Sigue en pie la regla de
  `LA_MAQUETA_EXIGE.md` 9b: *la fila es un `.maqueta`, la lista es Rust*.
- **No da animacion.** `:hover` viaja como segunda tabla de colores, y nada mas.
  Lo que se mueve es Rust en el bucle de fotograma.
- **No da red.** Y no la necesita para nada de lo de arriba.

---

## 8. LA ESCALERA

```
   [ ] 1  el FORMATO en un crate sin E/S, probado entero en el anfitrion
          (como `estratos` y `trim`: un formato mal empaquetado no da un fallo,
          da algo peor -- se lee mal y nadie se entera)
   [ ] 2  emit/bef.rs -- el emisor B. NO toca ninguna generacion
   [ ] 3  el LECTOR, con las cinco comprobaciones de la seccion 6
   [ ] 4  leerlo desde un FICHERO suelto, y que la calculadora se pinte con el
   [ ] 5  meterlo en la seccion 0x0B del .bex y leerlo desde ahi
   [ ] 6  vigia de fichero: guardar el .maqueta y verlo cambiar sin reiniciar
   [ ] ~  la red. Otro dia, y ya no cambia nada de lo de arriba
```

**El escalon 4 es donde la idea queda demostrada**, y no hace falta llegar al 5
para saber si funciona.

Ver `docs/plan/PLAN_MAQUETA.md` (como se construye MAQUETA),
`docs/componente/LA_MAQUETA_EXIGE.md` (el contrato),
`docs/plan/PLAN_DIRECTOR.md` (las superficies y la frontera de confianza) y
`toolchain/tools/maqueta/README.md` (el mapa de carpetas).
