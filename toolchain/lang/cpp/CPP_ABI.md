# CPP_ABI -- el ABI de BMO C++, escrito el mismo dia que se implementa

> **Escrito a mano. Se actualiza A LA VEZ que el codigo**, igual que
> `toolchain/lang/c/VERDAD.md`. Si este documento y `src/mangling.rs` no coinciden, el
> que manda es el codigo y este fichero esta roto.
>
> Estado: **paso 4** (mangling y sobrecarga). Ver el orden en
> [`BRECHA.md`](BRECHA.md).

## ★ Por que este fichero existe

Microsoft **nunca publico** la especificacion del ABI de C++ de MSVC. Clang
tuvo que hacerle ingenieria inversa --de ahi `MicrosoftMangle.cpp`, un fichero
entero dedicado a adivinar lo que alguien no escribio-- y el ecosistema pago
anos partido en dos.

Es el mismo patron que ya esta anotado en `parser/inicializador.rs` sobre los
inicializadores designados: **lo que un frontend deja sin terminar o sin
documentar se lo cobra el ecosistema, no el.**

> **Regla, no observacion: el ABI de C++ de BMO se escribe el mismo dia que se
> implementa.**

## Que NO es este ABI

**No es compatible con nadie, y es a proposito.** El *Itanium C++ ABI* --el que
usan GCC, Clang e ICC-- existe para que objetos de compiladores distintos se
enlacen entre si. BMO no tiene enlazador, no consume `.o` ajenos, no tiene
carga dinamica y compila **una sola unidad de traduccion**. La compatibilidad
no compra nada y cuesta legibilidad.

Lo que si se hereda de Itanium son las **propiedades**:

| Propiedad | Por que |
|---|---|
| **Determinista** | el mismo nombre da siempre el mismo simbolo |
| **Sin colisiones** | dos declaraciones distintas nunca comparten simbolo, y nada que un programa escriba choca con uno generado |
| **Reversible a ojo** | `_ZN1P5dobleEv` necesita `c++filt`; `P.doble#v` no |

---

## 1. Disposicion de objetos

La de un `struct` de C, sin cambios: **la calcula
`bmo_abi::types::disposicion`**, que es la misma funcion que usan el parser y
el codegen de BMO C. No hay una regla de C++ aparte, y por eso una clase sin
metodos es indistinguible de un `struct`.

- Los miembros van **en orden de declaracion**.
- Cada uno se alinea a `min(tamano, 8)`, minimo 1.
- El total se redondea al alineado del miembro mas grande.

### El `vptr` y la herencia (paso 5)

★ **El `vptr` va en el offset 0**, y ocupa 8 bytes. No en medio de la tabla como
en Itanium: el *offset-to-top* y la ranura de RTTI solo hacen falta con herencia
multiple y RTTI, y las dos estan descartadas con motivo en `BRECHA.md`. Al
principio es lo que se escribiria a mano en C, y hace que el despacho sea una
indireccion y no una resta.

Solo lo llevan las clases con metodos virtuales, propios o heredados. El campo
se llama `vptr.` -- con un punto, ilegal en C++, para que no choque con nada
escrito a mano.

**Un derivado empieza por la base ENTERA**, campos incluidos y en los mismos
offsets; los suyos van detras. Ese es todo el mecanismo de la herencia simple:
**un `B*` vale como `A*` sin ajustar nada.**

```text
  class A { int x; virtual f(); }      class B : public A { int y; }
  +----------+----------+              +----------+----------+----------+
  | vptr.  0 | x      8 |              | vptr.  0 | x      8 | y     12 |
  +----------+----------+              +----------+----------+----------+
```

### La vtabla

Una global por clase, `vtabla.<Clase>`, de `n` ranuras de 8 bytes. **El orden
es la tabla**: un derivado copia la del padre, un `override` **sustituye** su
ranura y un virtual nuevo se **anade** al final. Por eso las primeras ranuras
significan lo mismo en la base y en el derivado.

⚠ **Se rellena en ejecucion, al principio de `main`**, y no con un
inicializador estatico. No es una preferencia: **las globales de BMO C solo
admiten un entero como inicializador**, y la direccion de una funcion no se
conoce hasta emitir el codigo. `main` es el unico sitio por el que pasa todo
programa antes de construir nada.

El `vptr` de un objeto se apunta a su tabla **antes** de llamar al constructor:
un constructor puede llamar a un metodo virtual de si mismo, y con la tabla sin
poner llamaria a la nada.

### El despacho

```text
   objeto->vptr.        la tabla del tipo REAL, no del estatico
   tabla[ranura]        la ranura la fijo el compilador
   (...)(objeto, args)    llamada por puntero, con `this` de primer parametro
```

Es exactamente lo que se escribiria a mano en C -- y hay un test **en C**
(`el_despacho_virtual_entero_en_c`) que fija esa forma, porque es el suelo
sobre el que esto se apoya.

★ Y una llamada a un metodo propio **sin `this->`** despacha igual de
virtualmente. Es el caso que mas se olvida: `int doble() { return f() * 2; }`
con `f` virtual tiene que llamar a la `f` del objeto real, no a la de la clase
donde esta escrito `doble`.

## 2. Paso de parametros

Por la pila, derecha a izquierda, en ranuras de 8 bytes; un agregado ocupa
`techo(tamano/8)` ranuras. No hay clasificacion por *eightbytes* de SysV porque
**BMO no pasa argumentos en registros**.

La regla vive en **`bmo_abi::types::disposicion::ranuras`**, con sus tests -- no
en ningun frontend. Estuvo escondida en `lang/c/codegen/agregados.rs` como
`pub(super)` mientras este documento ya la llamaba ABI, y una regla que un
documento llama ABI y el arbol guarda dentro de un lenguaje es una regla que el
segundo lenguaje copia.

★ Un agregado de 8 bytes o menos **tambien** ocupa una ranura entera: podria
caber en un registro, pero tratarlo distinto obligaria al llamante y a la
funcion a ponerse de acuerdo sobre el tamano, y ese es justo el desacuerdo que
produce basura silenciosa.

`this` es **un parametro mas**, y va **el primero**. Ahi acaba toda la magia
del puntero implicito.

⚠ **Los parametros de coma flotante se rechazan** (paso 4). BMO C evalua
floats por la ruta SSE pero no los **pasa** --falta la ABI de xmm-- y los acepta
en silencio: `int g(double a)` compila y no hace lo que dice. Es deuda de C;
mientras exista, C++ no la emite.

## 3. Mangling

```text
  [espacio.]...[Clase.]nombre#codigos-de-parametro
```

- `.` separa cualificadores -- **ilegal en un identificador de C++**.
- `#` abre la lista de parametros -- ilegal tambien.
- Los parametros van separados por `.`; sin parametros no hay nada detras.

Un simbolo generado **siempre lleva `#`**, y eso es lo que garantiza que nunca
choque con una funcion escrita a mano. Hay un test que lo comprueba.

### Codigos de tipo

Minuscula con signo, **MAYUSCULA sin signo**. Es la unica convencion que hay
que recordar.

| Tipo | Codigo | | Tipo | Codigo |
|---|---|---|---|---|
| `void` | `v` | | `unsigned char` | `C` |
| `bool` | `b` | | `unsigned short` | `S` |
| `char` | `c` | | `unsigned int` | `I` |
| `short` | `s` | | `unsigned long` | `L` |
| `int` | `i` | | `unsigned long long` | `Q` |
| `long` | `l` | | `float` | `f` |
| `long long` | `q` | | `double` | `d` |

| Construccion | Codigo | Ejemplo |
|---|---|---|
| puntero | `P<t>` | `int*` -> `Pi`, `char**` -> `PPc` |
| referencia | `R<t>` | `int&` -> `Ri` |
| array | `A<n><t>` | `int[4]` -> `A4i` |
| clase | `{Nombre}` | `Punto` -> `{Punto}` |

★ **Las llaves no son decoracion**: sin ellas, una clase llamada `Pi` daria el
mismo codigo que un `int*` y dos funciones distintas compartirian simbolo. Hay
un test con ese nombre exacto.

### Ejemplos

| C++ | simbolo |
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
   asi que meterlo generaria dos simbolos para lo que el lenguaje considera la
   misma funcion -- y una llamada no sabria a cual ir. Declarar `int f(int)` y
   `char f(int)` es **error**, con ese motivo escrito.
2. **`this` no entra en la firma.** Va implicito en la clase; meterlo daria a
   todos los metodos un prefijo redundante.
3. ★ **`main` no se mangla.** Es el punto de entrada que el codegen de C busca
   **por nombre**; manglarlo dejaria un `.bef` sin `main`.

### El constructor y el destructor

`P.P#...` y `P.~P#`. Ninguno puede chocar con un metodo: dentro de la clase `P`,
un miembro llamado `P` **es** el constructor --el lenguaje reserva el nombre-- y
`~` no es legal en un identificador.

★ **Hay UN destructor, no tres.** El ABI de Itanium define D0/D1/D2 (y C1/C2/C3
para constructores), pero **D1 y D2 difieren solo con bases virtuales**, que
estan descartadas con motivo. Seis variantes se quedan en dos -- y no por
recortar, sino por una decision ya tomada por otro motivo. D0 (el que ademas
libera) aparecera el dia que existan `new`/`delete`.

## 4. El puente con C: `extern "C"` sin escribirlo

**Un nombre que no esta en la tabla de funciones de C++ se emite tal cual, sin
manglar.** Eso es lo que hace que `printf`, `getchar` y los intrinsecos sigan
funcionando: una funcion de C tiene el simbolo que tiene, porque quien la
escribio no sabia que C++ existia.

Es exactamente lo que `extern "C"` nombra en el estandar, solo que aqui es el
comportamiento por defecto para lo que C++ no declaro. Cuando entre `extern "C"`
de verdad (esta como UTIL en `BRECHA.md`) sera la forma **explicita** de pedir
lo mismo.

## 5. Resolucion de sobrecarga

Tres escalones, y se comparan **sumando** el de cada argumento. Menos es mejor.

| Escalon | Coste | Que es |
|---|---|---|
| **Exacto** | 0 | mismo tipo; tambien `T` -> `T&` y `T[n]` -> `T*` |
| **Promocion** | 1 | `char`/`short`/`bool` -> `int`, `float` -> `double`. No pierde |
| **Conversion** | 2 | cualquier aritmetico a cualquier aritmetico. **Puede perder** |

Un argumento que no encaje en ninguno **descarta la candidata entera**.

**El empate es un error**, con los dos simbolos escritos. Resolverlo solo
--"gana la primera declarada"-- haria que anadir una sobrecarga cambiara a donde
va una llamada existente, **en silencio**.

Fuera del alcance, con motivo en `BRECHA.md`: ADL, conversiones definidas por
el usuario, y plantillas en la resolucion. Son las tres cosas que hacen de
`gcc/cp/call.cc` uno de los ficheros mas grandes del frontend de GCC.

---

## Lo que este documento tendra que decir en el paso 5

- Donde va el `vptr` (offset 0) y como se ordena la vtable.
- Como se dispone una clase derivada (la base primero, entera).
- Que pasa con el destructor cuando es virtual.

Y si algo de eso se implementa sin escribirse aqui el mismo dia, este fichero
deja de valer y volvemos al problema de MSVC.
