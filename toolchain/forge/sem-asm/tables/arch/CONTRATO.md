# EL CONTRATO DE UNA ARQUITECTURA

**Que tiene que traer un chip para que BMO-X corra en el.** Y nada mas: esto no
explica como se escribe un emisor, dice **que hay que declarar**.

---

## 0. POR QUE ESTE DOCUMENTO EXISTE

Lo pidio Eddi el **2026-08-24**, con la frase exacta:

> *"no intentes cerrar con Inti sino dejar hueco LUEGO para cuando llegue las
> CPU con chips, es dejar abierto para CUALQUIER chip que vengan."*

No es un plan de portar a RISC-V. Es **la forma del hueco**: cuando llegue un
chip --RISC-V, uno soberano, uno hecho a medida-- la pregunta *"que tengo que
escribir?"* debe tener una lista, no una arqueologia del emisor de x86.

### [!] Y la razon de fondo, que es de arquitectura y no de comodidad

BMO-X tiene **2 syscalls congelados**. Eso hace que la superficie que un chip
nuevo tiene que sostener sea minuscula comparada con un sistema normal:

```
   Linux      ~350 syscalls + un ABI enorme + supuestos de x86 repartidos
   BMO-X      2 syscalls, y lo demas son CAPABILITIES sobre handles
```

Pero **lo pequeno es el contrato, no el trabajo** -- y esa distincion es la que
este documento existe para mantener honesta. Ver seccion 4.

---

## 1. LO QUE YA ES AGNOSTICO, MEDIDO

No hay que creerselo: se conto el **2026-08-24**.

```
   INTI frontend (sintaxis, tipos, reglas)      15.196 lineas    agnostico
   emisor-x86_64                                  9.497
      de las cuales PRUEBAS                       5.519          no viajan
      codigo de verdad                            3.978
         que nombran un registro o un opcode        131   <- el 3,3%
```

*** **131 lineas.** Y son tan pocas porque `sem-asm` ya se lleva los bytes: el
emisor pide *"la instruccion que suma"* y no sabe que bytes son.

**El `.bex` tambien es agnostico**: describe secciones, permisos y
relocalizaciones. No describe instrucciones. El mismo formato vale para
cualquier chip.

---

## 2. LAS CUATRO TABLAS

Una arquitectura es una carpeta `tables/arch/<nombre>/` con estos cuatro
ficheros. **Los cuatro, o no es una arquitectura.**

### `instructions.toml` -- el vocabulario del ensamblador

Mnemonico -> bytes. Es lo que convierte *"suma"* en algo que el silicio ejecuta.

### `intrinsics.toml` -- el metal como vocabulario del lenguaje

Las instrucciones **invocables desde los lenguajes**. En BMO C se escriben
`__nombre()` y el compilador emite esos bytes exactos.

*** Debe traer `[meta] isa = "<nombre>"`, y ese nombre tiene que coincidir con el
de la carpeta. Una tabla que dice ser de otra arquitectura es peor que una que
falta: la que falta no compila, la que miente emite.

### `abi.toml` -- quien lleva que

```toml
[calling_convention]
arg_regs      = [...]   # el orden en que viajan los argumentos
ret_reg       = "..."   # donde vuelve el resultado
callee_saved  = [...]   # los que el llamado tiene que devolver intactos
stack_align   = 16      # en bytes

[syscall_convention]
nr_reg      = "..."     # donde va el numero de operacion
instruction = "..."     # `syscall` en x86-64, `ecall` en RISC-V
clobbered   = [...]     # lo que la puerta destruye
```

[!] **La convencion de llamada no es una preferencia: es un contrato con todo lo
ya compilado.** Cambiarla despues de que exista un solo `.bex` es romperlo.

### `inti.toml` -- la libreria del chip

Lo que aparece cuando un programa escribe `usa <nombre>` en su primera linea.

*** **Y esto es lo que hace que la ISA entre por el PRINCIPIO y no por el
final**, que es como Eddi lo pidio el 19-08:

```inti
usa x86_64
```

El nombre del modulo **es la declaracion de que el fichero no es portable**.
`usa metal` escondia la arquitectura --*el metal de que maquina?*--; `usa x86_64`
la dice, y entonces el compilador la puede **contar**, y compilando para otro
chip la linea falla con un mensaje claro en vez de con un nombre desconocido a
mitad de fichero.

---

## 3. LO QUE TODAVIA NO ES UNA TABLA, Y SE DICE

Honestidad primero: hoy hay **dos cosas** que deberian salir de estas tablas y
viven en Rust dentro de `emisor-x86_64`:

| donde | que decide | forma hoy |
|---|---|---|
| `marco.rs` | que registros se preservan | `const RESPALDO: [u8;3] = [2,6,7]` |
| `operaciones.rs` | que instruccion hace cada cuenta | 32 brazos de `match` |

[!] **`RESPALDO` y `abi.toml::callee_saved` son la misma verdad en dos sitios.**
Es exactamente el patron que este arbol persigue en todas partes, y aqui sigue
vivo. Sacarlo es trabajo mecanico --las dos ya tienen forma de tabla-- y es lo
primero que hay que hacer el dia que se quiera un segundo backend.

**No se hizo hoy a proposito**: mover eso sin un segundo chip que lo pruebe es
escribir una abstraccion contra un solo caso, y una abstraccion con un solo
usuario casi siempre esta mal cortada. Se hace **cuando llegue el chip**, con el
delante.

---

## 4. LO QUE CUESTA DE VERDAD UN CHIP NUEVO

Sin adornar, y medido contra lo que hay:

```
   sacar las dos tablas que faltan (sec. 3)            ~1 semana   mecanico
   escribir las cuatro tablas del chip nuevo       ~1 semana   lectura de manual
   la PUERTA: como se entra al kernel               dias       `puerta.rs`, 124 lineas
                                                               RISC-V usa `ecall`, otro
                                                               modelo de traps
   volver a pasar los 256 tests del emisor           ?         es la parte honesta:
                                                               contra hardware real,
                                                               sin emulador todavia
```

*** **Lo que NO hay que rehacer**: el frontend de INTI (15.196 lineas), el
formato `.bex`, las capabilities, los 2 syscalls, el sistema de ficheros, la
criptografia, el compositor. O sea, casi todo.

[!] **Lo que tampoco viaja, y es lo que la gente olvida**: el emulador de
`bmo-lower` decodifica x86 y existe para el banco de pruebas. Un chip nuevo
empieza **sin emulador**, probandose contra hardware -- que es exactamente como
empezo x86-64 aqui. No es un bloqueante; es una fase.

---

## 5. LA REGLA, EN UNA FRASE

> **Una arquitectura es una carpeta de tablas y una puerta.** Si hace falta
> tocar el frontend, el `.bex` o los syscalls para meter un chip, el corte esta
> mal y lo que hay que arreglar es el corte, no el chip.

Y mientras solo haya una carpeta en `arch/`, esa frase es una **promesa**, no un
hecho. El dia que haya dos, se convierte en algo comprobable -- y ese dia es el
unico que puede decir si este documento tenia razon.
