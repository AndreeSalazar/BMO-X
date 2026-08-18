# IPC MAESTRO -- como se hablan dos programas, y donde acaba un mensaje

> Escrito el **2026-08-18**, el dia que la calculadora dejo de leer mal sus
> operandos. Pregunta del dueno: *"entonces que estrategias mejores? o que
> crates podemos inspirar aunque JSON es ayuda pero recuerdo como que se tienen
> que comunicarse, no?"*.
>
> Este documento contesta **de que copiar**. No tiene plan hermano porque hoy no
> hay ninguna casilla abierta: lo que hacia falta se hizo el mismo dia. El dia
> que la haya, su plan va en `plan/` y con el nombre que manda el indice.

---

# ★★ 0. LA FRASE QUE ORDENA EL DOCUMENTO ENTERO

```
   SERIALIZACION   como se ESCRIBE un dato        <- JSON resuelve esto
   ENMARCADO       donde ACABA un mensaje         <- aqui estaba el fallo
```

Son dos problemas distintos y BMO-X solo tenia uno. **JSON no lo habria
arreglado: lo habria heredado**, porque un JSON mandado por un rio de bytes
sigue necesitando que alguien diga donde termina. Por eso HTTP lleva
`Content-Length` y por eso existe un formato con nombre propio que se llama
*JSON delimitado por saltos de linea*.

> **JSON no te ahorra el enmarcado. Te lo anade encima.**

---

# 1. EL FALLO QUE OBLIGO A ESCRIBIR ESTO, medido

La calculadora del escritorio manda tres lineas y lee dos. El compositor las
escribe **de golpe** y despues lanza el motor, asi que los diez bytes ya estan
en el anillo antes de la primera lectura. `CONSOLE_READ` entrega **hasta siete**.

```text
   paquete 1 = "12.50\n3"

   el ACCEPT cierra la linea al ver el \n, se queda "12.50"
   y TIRA el resto del paquete -- que era el `3`, la operacion que se pedia
```

El motor contestaba `0.00` **con estado correcto**: una cuenta que nadie habia
pedido, sin un solo error por ningun lado. Es la clase de fallo que esta casa
persigue -- compila, corre, y dice algo que no es.

★ Y estuvo ahi desde que la calculadora se cablea. No se vio porque **`ACCEPT`
era la unica sentencia del lenguaje que ninguna prueba habia ejecutado**: los
demas ejemplos o no leen nada o leen de un fichero. El banco no tenia como
sembrar lo que un terminal habria tecleado.

---

# ★ 2. LAS CUATRO ESTRATEGIAS DE ENMARCADO

| estrategia | quien la usa | que cuesta |
|---|---|---|
| **delimitador** (`\n`) | casi todo Unix | el lector necesita **estado entre llamadas** |
| **longitud por delante** | 9P, D-Bus, `Content-Length` | cuatro bytes, y se acabo el escaneo |
| **registro de ancho fijo** | **COBOL, desde 1959** | cero parsing, cero delimitadores |
| **canal de mensajes** | `bmo-channel`, QNX, seL4 | el nucleo lo hace UNA vez, para todos |

La primera es la que habia, y **le faltaba justo la mitad que la hace
correcta**: un lector que no pierda el sobrante tiene que guardarlo hasta la
llamada siguiente. El codigo que emite el compilador no tiene donde -- cada
`ACCEPT` es una emision independiente sin estado que sobreviva.

---

# ★★ 3. Y LA RESPUESTA YA ESTABA EN EL ARBOL, DOS VECES

Esto es lo que hace innecesario inventar nada, y es el hallazgo de la
investigacion:

| mecanismo | como delimita un mensaje | podia tener el fallo? |
|---|---|---|
| `platform/shared/bmo-channel` | **entradas de 32 bytes** en un anillo de 62 | **no**, es imposible |
| Endpoint RPC (`TASK_OP_ENDPOINT_*`) | un handle, una llamada, una respuesta | no |
| **la consola** | un `\n` en un rio de bytes | **si, y era la unica** |

En `bmo-channel` el mensaje **no se delimita: se cuenta**. Una ranura *es* un
mensaje, asi que no hay nada que buscar y no puede sobrar nada. El fallo de la
calculadora no se puede cometer ahi ni escribiendolo mal a proposito.

La consola es la excepcion: su anillo de entrada son 256 bytes sueltos y el
terminal mete ocho cada vez. **Ese paquete no es un mensaje: es un trozo
arbitrario del rio.**

★ Y la calculadora hablaba por ahi por una razon que no es tecnica: **estaba
ahi**. Es la misma forma de error que meter la maquetacion en el bucle de
fotograma -- no se elige, se hereda.

## Lo que se hizo, y por que en el nucleo

`CONSOLE_READ` **no cruza nunca un salto de linea**: si entre los bytes
disponibles hay un `\n`, el paquete acaba ahi. Es el enmarcado hecho por el
nucleo, en el sitio mas barato que existe -- una comparacion.

Lo importante es que **el que lee bytes en crudo no pierde nada**: recibe lo
mismo, solo que en paquetes que acaban donde acaba una linea. Y el que lee
lineas deja de necesitar memoria. Esta escrito en el contrato
(`TASK_OP_CONSOLE_READ` de la superficie del ABI) porque lo cumplen dos
implementaciones --kernel y emulador-- y tendra que cumplirlo la tercera.

---

# 4. DE QUE COPIAR -- **ideas, no dependencias**

⚠ Antes de la lista: enlazar cualquiera de estos crates contradiria el proyecto
entero. BMO-X no toma dependencias; lo que se toma es **como estan pensados**.

- ★★ **COBS** (*Consistent Overhead Byte Stuffing*). El arreglo de libro si te
  quedas en un rio de bytes: reserva el `0` como delimitador y **garantiza que
  no aparece dentro** del mensaje, con un byte de coste por cada 254. Se
  implementa en unas treinta lineas. Es lo que usa `postcard`, el serializador
  `no_std` de embebidos y el mas cercano a este mundo.
- ★ **`tokio-util::codec`** -- no el codigo: **la forma**. Un `Decoder` posee un
  bufer y tiene derecho a contestar *"todavia no"*. Eso es exactamente lo que a
  `read_line` le faltaba: hoy no podia decir "no", solo podia tirar.
- **SLIP** (RFC 1055, 1988). El abuelo de todo esto, dos bytes de escape. Vale
  la pena por lo corto que es de leer.
- **9P / Plan 9**. Cada mensaje empieza por su tamano. Ya estaba citado en las
  notas de MAQUETA -- *"eso es 9P y QNX dicho de otra forma"*.

---

# 5. LO QUE SERIA UN ERROR COPIAR

## 5.1 JSON, y no por el formato

Un parser de JSON **dentro del motor** es lo que la ley de MAQUETA prohibe en su
propio terreno: un aparato que interpreta en ejecucion lo que se pudo resolver
antes. Y en COBOL es peor -- `ACCEPT` lee una linea; escribirle un parser de
llaves y comillas seria construir la mitad de un lenguaje dentro de un programa
de 120 lineas.

★ Ademas pediria **tipos en los dos lados**: la segunda copia de un contrato,
que es el fallo de `bmo.h` repetido.

## 5.2 Un bus de mensajes general antes de tener dos clientes

D-Bus, protobuf sobre sockets, un broker. Hoy hay **una** app con motor. La
regla de la casa --seis cosas bien y terminable-- dice que el bus se escribe
cuando duela no tenerlo, no antes.

## 5.3 Hacer del terminal el bus

Es lo que estaba pasando sin que nadie lo decidiera. La consola es **para
personas**: tiene eco, tiene historial, tiene un cursor. Que dos programas se
hablen por ahi funciona hasta que uno escribe rapido.

---

# ★★ 6. LA IRONIA, Y ES DE 1959

**COBOL llevaba razon desde el principio.** El registro de ancho fijo --`PIC
X(20)`, sin delimitadores-- no tiene este fallo porque no tiene donde tenerlo:
el mensaje mide lo que dice su PICTURE y no hay nada que buscar. El rio de bytes
con `\n` es la costumbre de Unix, y es la que se rompio.

Y las ranuras de 32 bytes de `bmo-channel` son **la misma idea** dicha en 2026.
Un anillo de entradas de tamano fijo y una `FILE SECTION` con registros de
tamano fijo resuelven el mismo problema por el mismo camino.

---

# 7. LA DECISION QUE QUEDA ABIERTA

Lo de arriba desbloquea la calculadora y deja el contrato sano. Lo que **no**
resuelve es de quien es el canal:

> ★ La consola es un terminal para personas, no un bus para programas.

El dia que una segunda app necesite un motor, ese motor va detras de un
**endpoint** -- que ya existe, ya tiene sus tres guardias probados en hardware
(`toolchain/tools/rpc-demo`) y no puede perder un byte porque no maneja bytes,
maneja llamadas.

Cuando eso pase, el ancho fijo de COBOL y las ranuras de `bmo-channel` seran la
misma frase dicha desde los dos lados de la maquina.

---

Ver `platform/abi/bmo-abi/src/syscalls/surface/tarea.rs` (el contrato de
`CONSOLE_READ`), `platform/shared/bmo-channel` (el anillo de ranuras) y
`docs/componente/LA_PUERTA_POR_DENTRO.md` (la superficie de los syscalls).
