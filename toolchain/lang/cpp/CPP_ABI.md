# CPP_ABI — el ABI de BMO C++, escrito el mismo día que se implementa

> **Escrito a mano. Se actualiza A LA VEZ que el código**, igual que
> `lang/c/VERDAD.md`. Si este documento y `src/mangling.rs` no coinciden, el
> que manda es el código y este fichero está roto.
>
> Estado: **paso 4** (mangling y sobrecarga). Ver el orden en
> [`BRECHA.md`](BRECHA.md).

## ★ Por qué este fichero existe

Microsoft **nunca publicó** la especificación del ABI de C++ de MSVC. Clang
tuvo que hacerle ingeniería inversa —de ahí `MicrosoftMangle.cpp`, un fichero
entero dedicado a adivinar lo que alguien no escribió— y el ecosistema pagó
años partido en dos.

Es el mismo patrón que ya está anotado en `parser/inicializador.rs` sobre los
inicializadores designados: **lo que un frontend deja sin terminar o sin
documentar se lo cobra el ecosistema, no él.**

> **Regla, no observación: el ABI de C++ de BMO se escribe el mismo día que se
> implementa.**

## Qué NO es este ABI

**No es compatible con nadie, y es a propósito.** El *Itanium C++ ABI* —el que
usan GCC, Clang e ICC— existe para que objetos de compiladores distintos se
enlacen entre sí. BMO no tiene enlazador, no consume `.o` ajenos, no tiene
carga dinámica y compila **una sola unidad de traducción**. La compatibilidad
no compra nada y cuesta legibilidad.

Lo que sí se hereda de Itanium son las **propiedades**:

| Propiedad | Por qué |
|---|---|
| **Determinista** | el mismo nombre da siempre el mismo símbolo |
| **Sin colisiones** | dos declaraciones distintas nunca comparten símbolo, y nada que un programa escriba choca con uno generado |
| **Reversible a ojo** | `_ZN1P5dobleEv` necesita `c++filt`; `P.doble#v` no |

---

## 1. Disposición de objetos

La de un `struct` de C, sin cambios: **la calcula
`bmo_abi::types::disposicion`**, que es la misma función que usan el parser y
el codegen de BMO C. No hay una regla de C++ aparte, y por eso una clase sin
métodos es indistinguible de un `struct`.

- Los miembros van **en orden de declaración**.
- Cada uno se alinea a `min(tamaño, 8)`, mínimo 1.
- El total se redondea al alineado del miembro más grande.

### El `vptr` y la herencia (paso 5)

★ **El `vptr` va en el offset 0**, y ocupa 8 bytes. No en medio de la tabla como
en Itanium: el *offset-to-top* y la ranura de RTTI sólo hacen falta con herencia
múltiple y RTTI, y las dos están descartadas con motivo en `BRECHA.md`. Al
principio es lo que se escribiría a mano en C, y hace que el despacho sea una
indirección y no una resta.

Sólo lo llevan las clases con métodos virtuales, propios o heredados. El campo
se llama `vptr.` — con un punto, ilegal en C++, para que no choque con nada
escrito a mano.

**Un derivado empieza por la base ENTERA**, campos incluidos y en los mismos
offsets; los suyos van detrás. Ése es todo el mecanismo de la herencia simple:
**un `B*` vale como `A*` sin ajustar nada.**

```text
  class A { int x; virtual f(); }      class B : public A { int y; }
  ┌──────────┬──────────┐              ┌──────────┬──────────┬──────────┐
  │ vptr.  0 │ x      8 │              │ vptr.  0 │ x      8 │ y     12 │
  └──────────┴──────────┘              └──────────┴──────────┴──────────┘
```

### La vtabla

Una global por clase, `vtabla.<Clase>`, de `n` ranuras de 8 bytes. **El orden
es la tabla**: un derivado copia la del padre, un `override` **sustituye** su
ranura y un virtual nuevo se **añade** al final. Por eso las primeras ranuras
significan lo mismo en la base y en el derivado.

⚠ **Se rellena en ejecución, al principio de `main`**, y no con un
inicializador estático. No es una preferencia: **las globales de BMO C sólo
admiten un entero como inicializador**, y la dirección de una función no se
conoce hasta emitir el código. `main` es el único sitio por el que pasa todo
programa antes de construir nada.

El `vptr` de un objeto se apunta a su tabla **antes** de llamar al constructor:
un constructor puede llamar a un método virtual de sí mismo, y con la tabla sin
poner llamaría a la nada.

### El despacho

```text
   objeto->vptr.        la tabla del tipo REAL, no del estático
   tabla[ranura]        la ranura la fijó el compilador
   (…)(objeto, args)    llamada por puntero, con `this` de primer parámetro
```

Es exactamente lo que se escribiría a mano en C — y hay un test **en C**
(`el_despacho_virtual_entero_en_c`) que fija esa forma, porque es el suelo
sobre el que esto se apoya.

★ Y una llamada a un método propio **sin `this->`** despacha igual de
virtualmente. Es el caso que más se olvida: `int doble() { return f() * 2; }`
con `f` virtual tiene que llamar a la `f` del objeto real, no a la de la clase
donde está escrito `doble`.

## 2. Paso de parámetros

Por la pila, derecha a izquierda, en ranuras de 8 bytes; un agregado ocupa
`techo(tamaño/8)` ranuras. No hay clasificación por *eightbytes* de SysV porque
**BMO no pasa argumentos en registros**.

La regla vive en **`bmo_abi::types::disposicion::ranuras`**, con sus tests — no
en ningún frontend. Estuvo escondida en `lang/c/codegen/agregados.rs` como
`pub(super)` mientras este documento ya la llamaba ABI, y una regla que un
documento llama ABI y el árbol guarda dentro de un lenguaje es una regla que el
segundo lenguaje copia.

★ Un agregado de 8 bytes o menos **también** ocupa una ranura entera: podría
caber en un registro, pero tratarlo distinto obligaría al llamante y a la
función a ponerse de acuerdo sobre el tamaño, y ése es justo el desacuerdo que
produce basura silenciosa.

`this` es **un parámetro más**, y va **el primero**. Ahí acaba toda la magia
del puntero implícito.

⚠ **Los parámetros de coma flotante se rechazan** (paso 4). BMO C evalúa
floats por la ruta SSE pero no los **pasa** —falta la ABI de xmm— y los acepta
en silencio: `int g(double a)` compila y no hace lo que dice. Es deuda de C;
mientras exista, C++ no la emite.

## 3. Mangling

```text
  [espacio.]…[Clase.]nombre#códigos-de-parámetro
```

- `.` separa cualificadores — **ilegal en un identificador de C++**.
- `#` abre la lista de parámetros — ilegal también.
- Los parámetros van separados por `.`; sin parámetros no hay nada detrás.

Un símbolo generado **siempre lleva `#`**, y eso es lo que garantiza que nunca
choque con una función escrita a mano. Hay un test que lo comprueba.

### Códigos de tipo

Minúscula con signo, **MAYÚSCULA sin signo**. Es la única convención que hay
que recordar.

| Tipo | Código | | Tipo | Código |
|---|---|---|---|---|
| `void` | `v` | | `unsigned char` | `C` |
| `bool` | `b` | | `unsigned short` | `S` |
| `char` | `c` | | `unsigned int` | `I` |
| `short` | `s` | | `unsigned long` | `L` |
| `int` | `i` | | `unsigned long long` | `Q` |
| `long` | `l` | | `float` | `f` |
| `long long` | `q` | | `double` | `d` |

| Construcción | Código | Ejemplo |
|---|---|---|
| puntero | `P<t>` | `int*` → `Pi`, `char**` → `PPc` |
| referencia | `R<t>` | `int&` → `Ri` |
| array | `A<n><t>` | `int[4]` → `A4i` |
| clase | `{Nombre}` | `Punto` → `{Punto}` |

★ **Las llaves no son decoración**: sin ellas, una clase llamada `Pi` daría el
mismo código que un `int*` y dos funciones distintas compartirían símbolo. Hay
un test con ese nombre exacto.

### Ejemplos

| C++ | símbolo |
|---|---|
| `int f()` | `f#` |
| `int f(int, char)` | `f#i.c` |
| `int f(int*)` | `f#Pi` |
| `void f(Punto)` | `f#{Punto}` |
| `int P::doble(int)` | `P.doble#i` |
| `P::P()` | `P.P#` |
| `P::P(int)` | `P.P#i` |
| `P::~P()` | `P.~P#` |
| `n::f(int)` | `n.f#i` |

### Tres reglas que no se ven en la tabla

1. ★ **El tipo de retorno NO entra.** C++ no permite sobrecargar por retorno,
   así que meterlo generaría dos símbolos para lo que el lenguaje considera la
   misma función — y una llamada no sabría a cuál ir. Declarar `int f(int)` y
   `char f(int)` es **error**, con ese motivo escrito.
2. **`this` no entra en la firma.** Va implícito en la clase; meterlo daría a
   todos los métodos un prefijo redundante.
3. ★ **`main` no se mangla.** Es el punto de entrada que el codegen de C busca
   **por nombre**; manglarlo dejaría un `.bef` sin `main`.

### El constructor y el destructor

`P.P#…` y `P.~P#`. Ninguno puede chocar con un método: dentro de la clase `P`,
un miembro llamado `P` **es** el constructor —el lenguaje reserva el nombre— y
`~` no es legal en un identificador.

★ **Hay UN destructor, no tres.** El ABI de Itanium define D0/D1/D2 (y C1/C2/C3
para constructores), pero **D1 y D2 difieren sólo con bases virtuales**, que
están descartadas con motivo. Seis variantes se quedan en dos — y no por
recortar, sino por una decisión ya tomada por otro motivo. D0 (el que además
libera) aparecerá el día que existan `new`/`delete`.

## 4. El puente con C: `extern "C"` sin escribirlo

**Un nombre que no está en la tabla de funciones de C++ se emite tal cual, sin
manglar.** Eso es lo que hace que `printf`, `getchar` y los intrínsecos sigan
funcionando: una función de C tiene el símbolo que tiene, porque quien la
escribió no sabía que C++ existía.

Es exactamente lo que `extern "C"` nombra en el estándar, sólo que aquí es el
comportamiento por defecto para lo que C++ no declaró. Cuando entre `extern "C"`
de verdad (está como UTIL en `BRECHA.md`) será la forma **explícita** de pedir
lo mismo.

## 5. Resolución de sobrecarga

Tres escalones, y se comparan **sumando** el de cada argumento. Menos es mejor.

| Escalón | Coste | Qué es |
|---|---|---|
| **Exacto** | 0 | mismo tipo; también `T` → `T&` y `T[n]` → `T*` |
| **Promoción** | 1 | `char`/`short`/`bool` → `int`, `float` → `double`. No pierde |
| **Conversión** | 2 | cualquier aritmético a cualquier aritmético. **Puede perder** |

Un argumento que no encaje en ninguno **descarta la candidata entera**.

**El empate es un error**, con los dos símbolos escritos. Resolverlo solo
—"gana la primera declarada"— haría que añadir una sobrecarga cambiara a dónde
va una llamada existente, **en silencio**.

Fuera del alcance, con motivo en `BRECHA.md`: ADL, conversiones definidas por
el usuario, y plantillas en la resolución. Son las tres cosas que hacen de
`gcc/cp/call.cc` uno de los ficheros más grandes del frontend de GCC.

---

## Lo que este documento tendrá que decir en el paso 5

- Dónde va el `vptr` (offset 0) y cómo se ordena la vtable.
- Cómo se dispone una clase derivada (la base primero, entera).
- Qué pasa con el destructor cuando es virtual.

Y si algo de eso se implementa sin escribirse aquí el mismo día, este fichero
deja de valer y volvemos al problema de MSVC.
