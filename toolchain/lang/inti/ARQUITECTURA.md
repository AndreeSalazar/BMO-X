# ARQUITECTURA DE INTI -- por que el compilador esta partido asi

> Escrito a peticion de Eddi el 2026-08-19: *"me gustaria que hagas MAS
> modularidad en ellas, el que y el porque, y pueda facilitar; porque si no va
> a ir syscall"*.
>
> La ultima frase es la que manda este documento, y merece decirse entera.

---

## 0. La frase que ordena todo

**Si el compilador no esta partido, el syscall se acaba metiendo dentro del
lenguaje.**

No es una metafora, es lo que pasa. Un compilador escrito de una pieza tiene un
sitio comodo para todo, asi que cuando alguien necesita que `guarda` funcione,
lo mas facil es que el generador de codigo emita el `syscall` ahi mismo. Al mes
siguiente `pinta` hace lo mismo. Y en cuanto dos construcciones del lenguaje
saben lo que es una capability, **el lenguaje ya esta casado con este sistema
operativo** y la portabilidad que la seccion 7 del maestro llama *"el sistema se
porta"* se ha perdido sin que nadie tomara la decision.

La modularidad no es orden ni estetica: **es lo que hace que la puerta se quede
fuera**. Un modulo que no puede nombrar a otro tampoco puede llamarlo.

---

## 1. El criterio de corte, y por que no es "las fases del compilador"

Cualquier libro parte un compilador en lexer / parser / analisis / emisor. Eso
sale igual en todas partes y **no decide nada**: dice en que orden pasa el
texto, no quien puede saber que.

Aqui el corte es otro, y viene del reparto que ya se hizo una vez en este
proyecto (`bmo-compositor-reparto`): **se corta por lo que cada pieza puede
decir sin nombrar a las demas**.

La prueba es una pregunta, y se le hace a cada modulo antes de crearlo:

> **Se puede explicar entero sin decir el nombre de otro modulo?**

Si hace falta decir *"esto es la parte del lexer que..."*, no es un modulo: es
un trozo del lexer, y va dentro.

### Lo que ese criterio produjo

| modulo | se explica asi, sin nombrar a nadie | lo que NO sabe |
|---|---|---|
| `aviso` | *"un mensaje de cuatro partes con un codigo estable"* | que existe INTI |
| `palabras` | *"49 cadenas y de que idioma son"* | que alguien las va a lexar |
| `lexico::pieza` | *"los datos que salen de leer texto"* | como se leyeron |
| `lexico::sangria` | *"una pila de margenes"* | que hay letras alrededor |
| `lexico` | *"de bytes a piezas"* | la gramatica |
| `arbol` | *"la forma de un programa"* | como se leyo |
| `sintaxis` | *"de piezas a arbol"* | si los nombres existen |
| `perfil` | *"esto cabe en el perfil declarado?"* | como se emite |

Y lo que ese criterio **rechazo**: no hay un modulo `util`, ni `comun`, ni
`helpers`. Ninguno de los tres pasa la prueba, porque los tres se explican
diciendo *"cosas que usan los demas"*.

---

## 2. Los modulos de hoy, uno a uno

### `aviso` -- el mensaje de cuatro partes

**Que es.** Un `Aviso` con codigo, que paso, donde, que habia y que hacer; y una
`Cosecha<T>`, que es un resultado **mas** todo lo que hay que decir.

**Por que existe aparte.** Porque el mensaje de error **es la interfaz principal
del lenguaje**: el 73% de los envios de codigo de estudiantes llevan errores de
sintaxis. Y siendo la interfaz principal, tiene que poder probarse sola, sin
compilar una linea de INTI. Sus pruebas comprueban cosas que no son de
compilador: que estan las cuatro partes, que el dedo cae en la columna, y **que
el texto no contiene jerga** (`token`, `AST`, `EOF`... hay una lista negra y un
test que la aplica).

**Por que `Cosecha` y no `Result`.** Un `Result` obliga a elegir entre el
resultado y los avisos, y hacen falta los dos: un fichero con tres errores tiene
que dar los tres. Parar en el primero convierte arreglar un programa en adivinar
cuantos quedan.

**Lo que hay dentro y no se ve.** `codigos.rs` reserva los numeros con una regla:
**un codigo retirado no se reutiliza jamas**. Reciclarlo haria que una busqueda
vieja diera una respuesta nueva y equivocada. Y un test comprueba que ninguno se
repite, porque dos codigos iguales es el bug de `INFO_CPU_HZ_REAL` escrito encima
de `INFO_FUGAS`: no falla, **miente**.

### `palabras` -- el vocabulario

**Que es.** Carga `tables/lang/inti/palabras.toml` y contesta una sola pregunta:
*esta palabra, es clave?*

**Por que existe aparte.** Porque es lo que hace que **el idioma sea una columna
y no un fork**. El lexer nunca compara contra `"funcion"`: pregunta. Cambiar de
idioma es cambiar el fichero, y no hay una linea del compilador que lo sepa. Hay
un test que lo demuestra: el mismo lexer lee `profile full / function f / while
x` sin tocar una linea de Rust.

**Y por que se CARGA en vez de venir incrustado.** Porque `tables/` es la raiz
que consulta `bmo-mods`, y quien deje su version en `$BMO_MODS` gana **sin
bifurcar el repo**. Si el vocabulario viviera solo dentro del binario, esa
propiedad se perderia. El incrustado existe igual, como **respaldo**, y entra por
`include_str!` del mismo fichero -- asi no pueden divergir.

**El simbolo, que es el truco entero.** El parser no ve cadenas: ve `Simbolo`.
La clave (`FUNCION`) es interna y no cambia nunca; el texto (`funcion`,
`function`) es de un idioma. **Separar esas dos cosas es lo que hace reversible
la decision del castellano.**

### `lexico::pieza` -- los datos

**Que es.** `Clase`, `Pieza`, `Numero`, `Signo`. Cero logica.

**Por que existe aparte.** Porque los va a leer todo el mundo -- el lexer los
produce, el parser los consumira, las sondas los miran. Un tipo compartido que
vive dentro del modulo que lo produce acaba arrastrando las decisiones de ese
modulo a todos los demas.

**La decision que esconde.** ★ Un numero se guarda **como texto**. No es pereza:
`numero` en INTI es decimal exacto, y convertirlo aqui a `f64` para "ya tenerlo
hecho" perderia la exactitud **en el primer paso del compilador** -- justo lo que
el lenguaje promete en la portada. El valor lo construye quien sabe en que forma
va a vivir, y ese no es el lexer.

### `lexico::sangria` -- el margen

**Que es.** Una pila de anchos. Entra un numero, salen `Sangra` / `Desangra`.

**Por que existe aparte.** Por dos motivos, y el segundo es el bueno:

1. Es **el sitio exacto donde se rompen los lenguajes con bloques por sangria**.
2. Es **la unica parte del barrido que tiene ESTADO**. Todo lo demas mira un
   caracter y decide; esto decide segun lo que paso antes. Mezclar las dos cosas
   en un bucle es como se acaba con un lexer al que nadie se atreve a tocar el
   margen.

Suelto, se prueba sin lexer: entra `[0, 4, 8, 0]` y salen tres piezas.

### `perfil` -- la frontera entre `llano` y `pleno`

**Que es.** Recorre el arbol y contesta una pregunta: *esto cabe en el perfil
que declaro el fichero?*

**Por que existe aparte.** Porque el parser **avanza** y esto **decide**. Un
`crudo` es sintacticamente igual de valido en los dos perfiles; lo que cambia es
si esta permitido, y esa es una pregunta sobre el modulo entero, no sobre la
linea.

**Lo que saca, ademas de avisos.** ★★ El numero de bloques `crudo`. Es lo que
convierte *"cuanto de mi programa esta atado a esta maquina?"* en un dato que va
al informe del `.bex`.

**Y lo que NO decide el compilador**: que crece y que pide `crudo` sale de
`tables/lang/inti/biblioteca.toml`. Son datos sobre la biblioteca, no sobre el
lenguaje -- si vivieran en el compilador, anadir una operacion de sistema
obligaria a recompilarlo.

### `lexico` -- el barrido

**Que es.** De bytes a piezas.

**Lo que NO sabe, y es lo que lo define.** No conoce la gramatica: no sabe que
`si` lleva un bloque detras. **Aqui no se puede escribir un error de gramatica.**
Un `si` suelto sin condicion no es asunto de este fichero; un `"` sin cerrar si.

---

## 3. La regla de dependencias

```text
   lexico  ---> pieza, sangria, palabras, aviso
   palabras ---> aviso(no), bmo-mods, toml
   sangria ---> aviso, pieza
   aviso   ---> nadie
```

Dos reglas, y las dos vienen de la ley del proyecto (*"contratos y formatos,
NUNCA cerebros"*):

1. **Las flechas van hacia abajo.** `aviso` no importa nada del crate. Si algun
   dia `aviso` necesitara conocer una pieza, seria senal de que el mensaje ha
   dejado de ser un mensaje.
2. **Nadie llama hacia los lados.** `sangria` no llama al barrido; el barrido le
   pregunta. Por eso `sangria` se puede probar sola y por eso el barrido puede
   cambiar sin tocarla.

Y una regla de fuera:

3. **Este crate no enlaza `bmo-abi`, `bmo-lower` ni `bmo-verify`** -- que es lo
   que enlazan los otros cuatro frontends. **F1 no emite bytes.** Atar el
   frontend a la forma del emisor antes de tener nada que emitir es el orden que
   este proyecto evita. Cuando llegue F2, se anadiran los tres y `bmo-verify`
   sera obligatorio: *ningun frontend puede escribir un ejecutable que no haya
   pasado por el gate*.

---

## 4. Donde va a caer lo que falta

La forma esta elegida para que **el syscall no tenga sitio comodo en ninguno de
estos modulos**:

| modulo futuro | que hara | por que sigue sin poder tocar la puerta |
|---|---|---|
| `arbol` | los nodos, datos puros | no llama a nadie |
| `sintaxis` | de piezas a arbol | solo conoce `pieza` y `arbol` |
| ~~`perfil`~~ | ✅ **hecho** | |
| `nombres` | quien es cada nombre y si es `cambiante` | tampoco emite |
| `emision` | de arbol a `.bex`, via `bmo-lower` | **aqui SI se emite** -- y la puerta llega como una **fila de tabla** (`intrinsics.toml`), igual que en BMO C |

★ Fijate en la ultima fila: cuando por fin haya un modulo que emite bytes, la
puerta seguira sin ser sintaxis, porque llegara **por la misma via que una
instruccion cualquiera**: una entrada en una tabla que `sem-asm` ya sabe leer.
`invoca` no sera una palabra clave nunca, sera un nombre de la biblioteca base.

---

## 5. Como se anade un modulo

1. **Pasa la prueba del nombre libre**: escribe su primera frase de
   documentacion sin mencionar otro modulo. Si no puedes, va dentro de otro.
2. **Di lo que NO sabe.** La linea mas util de un modulo es la que dice lo que
   se niega a saber.
3. **Que se pueda probar solo.** Si sus pruebas necesitan medio compilador
   montado, el corte esta mal puesto.
4. **Mira las flechas.** Si tuvieras que importar hacia arriba o hacia los
   lados, el modulo esta en la capa equivocada.

---

## 6. El censo manda

Las sondas de `censo/*.inti` **no son ejemplos**: son el corpus contra el que se
mide el lenguaje, y llevan su veredicto en la primera linea para que la sonda y
su expectativa no se puedan separar.

★ Ya se gano el sitio el primer dia. Las 38 sondas estaban escritas con **tres**
espacios de sangria y `GRAMATICA.md` dice **cuatro**. El documento y el corpus
llevaban dos dias sin estar de acuerdo, **y nadie lo habria visto leyendo**: lo
encontro el lexer en cuanto hubo uno. Estan reindentadas, y el test
`ninguna_sonda_lleva_un_fallo_de_escritura` impide que vuelva a pasar.

Es la misma propiedad que `BRECHA.md` de BMO C defiende: *leer el lexer diria que
palabras se reconocen; aqui se pregunta lo unico que decide, **compila?***
