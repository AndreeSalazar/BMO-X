# PLAN DEL AUTOHOSPEDAJE -- que BMO-X compile SOBRE SI MISMO

> **Lo que afirma**: que un compilador de esta casa puede correr en Ring 3, leer
> un fuente del disco de BMO-X, y escribir un `.bex` que el escritorio lanza.
>
> **Como se cae**: el `.bex` del compilador no arranca, o arranca y se queda sin
> monton, o compila y produce un `.bex` que el cargador rechaza.
>
> Escrito el **2026-09-06**. Es el primer plan de esto en el arbol: se busco
> `autohospeda`, `self-host` y `compilar en la maquina` y no habia ninguno.

---

## 0. LO QUE YA ES VERDAD, PARA NO RECONSTRUIRLO

```text
   escribir desde Ring 3      ESTRATOS 1.0, y relee TRAS REINICIAR    19-08
   leer ficheros              FAT32 + `<bmo/archivo.h>` de REX
   Rust -> .bex               `bex-link`, que ya produce d.bex y precio.bex
   un monton que CRECE        bmo-rt/heap/freelist.rs + SyscallBackend
   el gate del .bex           bmo-verify, por el que pasa todo lo que se escribe
```

** El cuarto es el que cambia el plan entero, y estaba mal recordado en
`PLAN_PYTHON`: **el tope de 1 MiB es el monton de C** (`BMO_MONTON_BYTES` en
`tables/bmo/monton/roja.h`, un bloque fijo). El de **Rust** pide arenas de 1 MiB
al kernel por `NR_MEM_ALLOC` y **crece**, y una reserva grande se lleva su propio
trozo. Un compilador de Rust en Ring 3 **no esta capado en 1 MiB**.

---

## 1. ** EL HUECO, MEDIDO -- y la medida elige el orden sola

No es una opinion sobre cual es mas facil. Son dos columnas, y las dos apuntan
al mismo sitio:

| frontend | lineas | `std::fs` | `std::path` | `HashMap` | **dependencias externas** |
|---|---:|---:|---:|---:|---|
| **Ada** | **1.608** | 3 | 1 | 7 | **NINGUNA** |
| COBOL | 13.668 | 1 | 24 | 40 | `toml`, `serde` (via `sem-asm`) |
| C | 24.627 | 4 | 39 | 57 | `toml`, `serde` (via `sem-asm` y `bmo-mods`) |
| INTI | 27.794 | 62 | 14 | 64 | `toml` directo, y `bmo-mods` |

### Lo que estas columnas dicen, y no es lo que parece

**El acoplamiento a `std` esta en la CASCARA, no en las tripas.** `std::fs`,
`std::path`, `std::env` y `std::process` viven en `main.rs` -- leer un fichero y
parsear la linea de comandos. El parser y el codegen ya tienen forma de `alloc`,
y `HashMap` -> `BTreeMap` es un cambio mecanico porque `BTreeMap` vive en `alloc`.

### ★★ Y la columna que decide es la ultima, no la primera

La cadena de dependencias de **Ada** es, entera:

```text
   bmo-ada-front
     +-- bmo-verify   -> bmo-bex-gate, bmo-abi
     +-- bmo-lower    -> bmo-abi
     +-- bmo-abi      #![no_std] YA, + bitflags, bmo-hash, bmo-carga-juicio
```

**Todo en el arbol, y con la raiz ya en `no_std`.** Ni `toml` ni `serde` ni una
sola caja de fuera. Los otros tres arrastran `toml` --que es un parser de
configuracion con su propio arbol de dependencias-- y portarlo es un proyecto
aparte que no tiene nada que ver con compilar.

> **Por eso Ada va primero, y no es sentimental**: es un orden de magnitud menos
> de codigo *y* el unico cuya cadena entera ya esta donde tiene que estar.

---

## 2. ⚠ LA FORMA QUE OBLIGA EL CELO -- leer esto antes de disenar nada

`EJECUTAR` pide **autoridad**, se fija al nacer y **solo la puede fijar Ring 0**
(`Ultra_kernel_x86-64/kernel/src/ring0/task/autoridad.rs`). La cabecera de ese
fichero ya dice quien la tiene:

```text
   el escritorio         lo arranca el KERNEL      SI
   `run` del shell 0     lo teclea el dueno        SI
   un hijo de Ring 3     lo lanza otro proceso     NO
```

Asi que un compilador de a bordo **compila, y no lanza lo que compilo**:

```text
   ada.bex          lee el fuente, escribe hola.bex en ESTRATOS   <- sin autoridad
   el ESCRITORIO    lo lanza                                      <- tiene autoridad
```

** Y esto **no es un obstaculo que rodear: es el diseno cobrandose en su primer
caso de uso real.** En Linux `gcc` puede lanzar lo que produce porque hereda tu
uid; aqui no hereda nada. El dueno no pierde nada --el escritorio es suyo y lanza
lo que quiera-- y lo que se impide es que **un programa lance otro sin que el
dueno lo pida**. Es exactamente lo que `EL_ORQUESTAL.md` llama el celo.

> [!] **La tentacion que hay que rechazar**: darle autoridad al compilador
> "porque es una herramienta del sistema". Ese es el tercer bit de autoridad, y
> la cabecera de `autoridad.rs` ya dejo escrita la pregunta que hay que hacerse
> antes: *si esa operacion de verdad no tiene objeto, o es que todavia no se ha
> encontrado cual es*.

---

## 3. LOS ESCALONES

Ordenados por la regla de la casa: **lo que no toca nada va primero, lo que no se
deshace va al final.** Del 1 al 3 el anfitrion tiene que seguir compilando igual;
si algo se rompe ahi, se rompio en el sitio barato.

```text
   [ ] 1  la sonda del hueco     quitar `std` de `toolchain/lang/ada` y CONTAR
                                 los errores, en vez de estimarlos. No arregla
                                 nada: produce el numero con el que se juzga
                                 todo lo demas

   [ ] 2  BTreeMap en Ada        los 7 `HashMap` de `toolchain/lang/ada/src`.
                                 Mecanico, y el banco de `bmo-ada-front`
                                 (20 filas) tiene que seguir en verde

   [ ] 3  la capa de FUENTE      un trait que sustituya los 3 `std::fs` de
                                 `toolchain/lang/ada/src`: una cara para el
                                 anfitrion y otra para `<bmo/archivo.h>`.
                                 Se prueba entera en el anfitrion

   [ ] 4  ada como lib no_std    `toolchain/lang/ada/src/lib.rs` con
                                 `#![no_std] + alloc`; `main.rs` se queda en
                                 std y pasa a ser SOLO el envoltorio de CLI

   [ ] 5  ada.bex ARRANCA        `bex-link` sobre el frontend, y en el Ryzen
                                 dice su version SIN compilar nada. Es el
                                 primer .bex de Rust de este tamano en Ring 3

   [ ] 6  compila EN LA MAQUINA  un `.ada` trivial leido del disco de BMO-X,
                                 y el `.bex` escrito en ESTRATOS. Pasa por
                                 `bmo-verify` como cualquier otro

   [ ] 7  el ESCRITORIO lo lanza el ciclo cerrado, y el dueno lo cierra con un
                                 clic porque la autoridad no viaja (seccion 2)

   [ ] 8  ESPEJO.txt VIAJA       `VALKYRIE-ABI/ESPEJO.txt` al stick, y el
                                 compilador de a bordo lee de ahi los numeros
                                 en vez de traerlos incrustados
```

### Como se cae cada uno, que es la mitad que sirve

| escalon | si esta bien | si falla |
|---|---|---|
| 1 | un numero de errores, y una lista de los sitios | -- |
| 2 | `cargo test -p bmo-ada-front` sigue en 20 filas | una fila roja: el orden de iteracion importaba |
| 3 | el `cierre.bex` de hoy sale byte a byte igual | sale distinto: la capa cambio algo |
| 4 | compila para `x86_64-unknown-none` | falta un `alloc::` que era `std::` |
| 5 | `ada` en el escritorio imprime su version | no arranca: monton, pila o reloc |
| 6 | aparece `hola.bex` en la ventana de ESTRATOS | se queda sin monton, o `bmo-verify` lo rechaza |
| 7 | el icono lanza y el programa imprime | el cargador lo rechaza: el `.bex` de a bordo no es igual que el del anfitrion |
| 8 | los numeros salen del fichero y el sello sigue verde | -- |

---

## 4. ★★ EL ESCALON 8, Y LO QUE LE HACE A V-ABI

Hoy los numeros de las operaciones viven **incrustados** en las cabeceras de REX,
compilados en el anfitrion. `VALKYRIE-ABI/ESPEJO.txt` es la tabla sellada de esas
**98 parejas** y hoy solo la lee el juez, en Windows.

El dia que el compilador corra a bordo, esa misma tabla es **el manifiesto que
lee para saber que operaciones existen y con que numero**. Y eso convierte a
V-ABI de documental en **portante**: la misma fuente de verdad, leida por los dos
lados (`R-REX4`).

### ⚠ Y obliga a enmendar una frase que se commiteo el 06-09

`VALKYRIE-ABI/README.md` dice hoy: *"nada de esta carpeta viaja dentro de
`BOOTX64.EFI`"*. Con el escalon 8 eso deja de ser cierto -- `ESPEJO.txt` viajaria
en `BMO-DATA/`.

**La enmienda no debilita el argumento: lo afila.** V-ABI sigue sin tener anillo,
porque lo que viaja son **datos, no instrucciones**, y un dato no tiene nivel de
privilegio. Lo que cambia es que pasa de *dato que solo lee el anfitrion* a
*dato que leen los dos lados*, y eso es exactamente lo que se le pide a una
fuente unica de verdad.

> El dia que se marque la casilla 8, esa frase del README se reescribe **en el
> mismo commit**. Dejarla es la clase de mentira comoda que el arbol lleva la
> semana entera cazando.

---

## 5. LO QUE **NO** BLOQUEA ESTO, Y SE DICE PARA QUE NO SE CUELE

* **La compilacion separada.** Es el techo de lo que el compilador *acepta*, no
  de que el compilador *corra*. Un fuente de un fichero se compila igual.
* **Los hilos.** Un compilador no los necesita, y `toolchain/lang/c/BRECHA.md`
  los pide para otra cosa.
* **La criptografia.** Un `.bex` de a bordo sale con `sig_algo = 0` igual que los
  del anfitrion. No mejora, pero tampoco empeora.
* **La GPU, la red y el audio.** Nada de esto toca la pantalla mas alla de
  imprimir.

---

## 6. LO QUE SERIA UN ERROR

* **Empezar por C o por INTI** porque son los que mas se usan. Son los dos que
  arrastran `toml`, y portar un parser de configuracion de terceros no ensena
  nada sobre autohospedar un compilador.
* **Darle autoridad al compilador.** Ver la seccion 2.
* **Que el `.bex` de a bordo salga distinto al del anfitrion.** El escalon 3
  existe para cazar eso temprano: mismo fuente, mismos bytes.
* **Llamarlo "como Linux".** Lo que se parece es el gesto --tecleas y compila--;
  lo que hay debajo no se parece en nada, y prometer lo segundo por haber
  conseguido lo primero es vender una compatibilidad que no existe.

---

Ver [`EL_ORQUESTAL.md`](../identidad/EL_ORQUESTAL.md) (por que la autoridad no
viaja, y los otros siete sitios del celo),
[`VALKYRIE-ABI/README.md`](../../VALKYRIE-ABI/README.md) (que se promete, y la
frase que el escalon 8 enmienda), [`BRECHA.md`](../../toolchain/lang/c/BRECHA.md)
(lo que le falta a C, que es otro frente) y
[`PLAN_REX.md`](PLAN_REX.md) (las cabeceras con las que se escribe una app).
