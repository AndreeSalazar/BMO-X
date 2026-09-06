# LA LEY DE VALKYRIE -- R13 a R16, una a una

> Las cuatro reglas que deciden que cruza a la superficie de los terceros.
> Cada una con lo que dice, **lo que sacrifica** (L3) y **como dice que NO**
> (L4), porque una regla que nunca ha rechazado nada es una regla que nadie ha
> probado.
>
> El codigo esta en `toolchain/tools/contrato/contrato_rex.py`. Esto es lo que
> el codigo no puede decir: por que.

---

## 0. LA FIGURA, Y POR QUE ELIGE EN VEZ DE PELEAR

En la mitologia nordica la valquiria **no pelea: elige**. Recorre el campo
despues de la batalla y decide quien cae y quien es llevado. No aporta fuerza
-- aporta **criterio**, y su criterio no se negocia con el caido.

Ninguna de estas cuatro reglas escribe una linea de codigo ni hace mas rapido
nada. Las cuatro son porteras, y lo que dejan fuera **no es "todavia no": es que
no**. Cada fila de la puerta de los terceros es una promesa que hay que mantener
despues, y por eso la puerta se cierra con criterio y no con ganas.

```text
   REDUNDANTE   R13   el espejo                 el mismo numero dicho dos veces
                R15   el ABI no se repite       dos nombres para un numero

   DEFECTUOSO   R14   ninguna app inventa       un numero que REX no publica
                R16   la cobertura solo sube    una promesa que encoge
```

---

## R13 -- EL ESPEJO

**Dice**: una constante que REX y el ABI escriben las dos veces tiene que decir
el mismo numero. Hoy son **98 parejas**, selladas en [`ESPEJO.txt`](ESPEJO.txt).

**Por que existe**: C no puede importar de Rust. `<bmo/pantalla.h>` repite a mano
los numeros que `bmo-abi` declara, y **una copia a mano es una copia que se
queda vieja sola**. El dia que el ABI mueva un opcode y la cabecera no, la app
no falla al compilar: pide otra cosa, y el kernel se la da.

**El sacrificio**: cada numero nuevo hay que sellarlo. `--sellar` reescribe el
espejo, **pero la nota se escribe a mano** -- una herramienta no sabe si
`TASK_OP_FRAMEBUFFER_CLAIM` y `BMO_OP_PANTALLA_RECLAMAR` son la misma cosa.

**Como dice que NO**: la autoprueba le da un espejo donde una pareja discrepa, y
exige que la nombre. Si se queda callada, el banco falla.

---

## R14 -- NINGUNA APP INVENTA UN NUMERO

**Dice**: una app del arbol no cruza la puerta con un numero de operacion que
REX no publique. Hoy: **0 inventados**.

**Por que existe**: un `#define MI_OP 0x1C` dentro de una app compila, corre, y
funciona -- hasta el dia que el ABI le da otro significado al `0x1C`. La app no
declaro nada; se lo aprendio. **Lo que se aprende de memoria no lo protege
ningun contrato.**

**El sacrificio**: obliga a que toda operacion util tenga cabecera antes de tener
usuario, y eso es trabajo por delante en vez de por detras. Tambien produce
ruido legitimo: `sonda_C.c` cruza con literales a proposito porque es una sonda,
y la regla **informa** de esos cuatro en vez de fallar.

**Como dice que NO**: le das una app con un literal desnudo y tiene que
nombrarla, con su fichero y su numero.

> [!] **No cubre `mods/` ni `$BMO_MODS`**, y es deliberado: un tercero **tiene
> derecho** a redefinir un numero -- es literalmente lo que promete el buscador
> de cabeceras. Esta regla vigila lo que el proyecto publica como ejemplo, que
> es lo que la gente copia.

---

## R15 -- EL ABI NO SE REPITE

**Dice**: dos operaciones de la misma familia no pueden valer lo mismo. La lista
de choques tolerados esta **vacia**, y ese es el estado correcto.

**Por que existe**: esta casa ya lo pago. El kernel lo dejo escrito al elegir
`PANTALLA_SOLTAR`:

> *"0x1D elegido tras listar los opcodes ORDENADOS, que es la regla desde que
> `MEMORIA_PEDIR` se puso en 0x12 --ya ocupado por `REINICIAR`-- y pedir memoria
> habria reiniciado la maquina."*

R5 ya vigilaba eso en el kernel. **Nadie lo vigilaba en el ABI**, que es justo la
lista que lee quien escribe una app.

**El sacrificio**: hay que declarar las familias de prefijo a mano
(`TASK_OP_`, `ARCH_OP_`, ...), y el orden importa -- `INFO_TXT_` va antes que
`INFO_` porque son dos espacios de numeracion distintos. Una familia nueva que
nadie declare pasa sin juez.

**Como dice que NO**: dos constantes de una familia con el mismo valor, y tiene
que decir cuales.

---

## R16 -- LA COBERTURA SOLO SUBE

**Dice**: cuantas constantes del ABI que son **de app** tienen cabecera en REX.
Hoy **98 de 196 (50%)**, sellado en [`COBERTURA.txt`](COBERTURA.txt). Puede
subir; no puede bajar.

**Por que el denominador es 196 y no el ABI entero**: porque un porcentaje contra
todas las constantes seria **una forma elegante de mentirse** -- contaria como
"pendiente" cosas que nunca van a estar. `DISCO_`, `CABINA_`, `AUTOPSIA_`,
`KLOG_` y los demas prefijos de [`FRONTERA.txt`](FRONTERA.txt) son instrumentos
de Ring 0 y de los paneles, y **no es que falten: es que no van**.

**El sacrificio**: la frontera hay que mantenerla a mano, y se puede cruzar en
las dos direcciones. El dia que una app de verdad necesite algo de ahi, se borra
su prefijo **y se escribe por que** -- que es exactamente el momento en que la
decision se toma a la vista y no por acumulacion.

**Como dice que NO**: le bajas el suelo y tiene que quejarse de que la cobertura
retrocedio.

> [!] **Lo que R16 NO mide**: si la cabecera es BUENA. Mide que exista.

---

## LO QUE LAS CUATRO JUNTAS NO PUEDEN

Vuelve a decirse aqui porque es la frase que impide que el sello se lea como mas
de lo que es:

> **Juzgan lo que esta ESCRITO, nunca lo que CORRE.**

Las cuatro pueden estar verdes sobre un kernel que no hace nada. Quien comprueba
que la operacion **hace** lo que su numero promete es el banco de pruebas, las
hojas de `docs/metal/` y el Ryzen. V-ABI cubre la mitad barata -- la que se
comprueba en 69 segundos sin arrancar la maquina.

---

Ver [`README.md`](README.md) (por que esta carpeta no tiene anillo),
[`FRONTERA.txt`](FRONTERA.txt) (lo que no entra, y de quien es) y
[`META-SDK_HARD.md`](../META-SDK_HARD.md) seccion 3 (las siete reglas de REX, que son
las de la libreria y no las del estandar).
