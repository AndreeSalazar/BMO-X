# LA COMPATIBILIDAD -- las tablas de lo que no se puede romper

> Escrito el **2026-08-26**, a peticion del dueno:
>
> *"organizar en tablas cuales son los elementos MAS importantes para no romper
> la compatibilidad = ese punto es el inicio, porque con una sola cosa que
> permite compatibilidad, esa misma seria para analizar y compensar."*
>
> La segunda mitad de esa frase es la tesis de este documento y esta en la parte
> 4: **nada entra en la superficie gratis.** Lo que se concede se compensa, y lo
> que se compensa se escribe.

---

# 0. LA FRASE DE LA QUE SALE TODO LO DEMAS

> **Lo que una empresa compra no es tu codigo: es la lista de cosas que
> prometes no romper.**

Y de ahi sale la aritmetica que decide el producto entero:

| | mantener | adoptar |
|---|---|---|
| superficie **grande y estable** | caro | facil |
| superficie **pequena y estable** | barato | hay que aprender |
| superficie **grande e inestable** | -- | nadie construye |
| superficie **pequena e inestable** | -- | ni eso |

BMO-X eligio la segunda, **y esa eleccion es el producto**: dos puertas
congeladas y 93 operaciones aditivas es una promesa que cabe en una pagina, y
una promesa que cabe en una pagina se puede cumplir diez anos.

---

# 1. EL NUCLEO CONGELADO -- tocar esto rompe todo lo que ya existe

| # | que | donde vive | que rompe si se toca | como se nota |
|---|---|---|---|---|
| 1 | **`INVOKE` y `WAIT`** | `bmo-abi/syscalls/` | **todo binario que exista** | nada arranca |
| 2 | **Los numeros de operacion ya asignados** | `surface/*.rs` | los `.bex` que usen ese numero | hace otra cosa, **y no falla** |
| 3 | **Un numero RETIRADO** | -- | -- | el binario viejo falla **diciendolo** |
| 4 | **El formato `BEF1`** (`MAGIC`, cabecera 48, entrada 48) | `bmo-bex-gate` | el cargador entero | ningun programa carga |
| 5 | **El MAYOR del ABI** | `bmo-abi/lib.rs` | por diseno: declara incompatibilidad | el cargador dice `OtraVersionDelAbi` |
| 6 | **El formato del handle** (tag 63, kind 62:56, gen, indice) | `cap.rs` + `handle/opaque.rs` | toda capability viva | `handle invalido` -- ver la parte 5 |
| 7 | **Las siete `R-APP`** | `META-APP_HARD.md` | el contrato de una app | el escritorio deja de aislar |

★★ **La fila 2 es la peor de la tabla y no lo parece.** Un numero cambiado no
falla al compilar, no falla al cargar, y no falla al llamar: **hace otra cosa.**
Es el `MEM_OP_OFRECER` de agosto --0x03 en el ABI, 0x02 en el despacho-- donde
prestar memoria entraba en el brazo de *"cuantos bytes tiene"*.

★ La fila 3 es la unica de esta tabla que es una **promesa de fallar bien**. Un
tercer syscall existio y se retiro el 2026-08-10; su numero no se recicla, asi
que un binario viejo que lo llame se lleva un no con nombre y no una sorpresa.

---

# 2. LO QUE CRECE SIN ROMPER -- aqui es donde se "mejora"

| que | la regla que lo gobierna | por que no rompe |
|---|---|---|
| **operaciones nuevas** | `R-REX3`: *comodidad es cabecera, autoridad es operacion* | un binario viejo no la llama |
| **kinds de capability nuevos** | uno libre en las **dos** tablas, y `<= 0x7F` | nadie tiene un handle de un kind que no existia |
| **secciones nuevas del `.bex`** | el header ya lo dice: *"una seccion que no entiendo es data inerte"* | el cargador la ignora |
| **cabeceras de REX** | comodidad; compilan hacia dentro del `.bex` | no hay `.so` que resolver |
| **la version MENOR del ABI** | aditiva | un binario que pide menos, entra |
| **campos de `OP_INFO`** | id nuevo, nunca reusado | quien no lo pide no lo ve |

★★ **Y ese es el mecanismo entero de mejorar sin romper: lo aditivo va en el
menor, lo que rompe va en el mayor, y lo retirado no se recicla.** No hace falta
inventar nada mas.

---

# 3. LO QUE **NO** ES CONTRATO, aunque lo parezca

| regla | de quien es | se la aplica a un tercero? |
|---|---|---|
| 1.000 lineas por modulo (L6a) | **la casa** | **no** |
| todo ASCII en comentarios | la casa | no |
| la herencia de generaciones (L7) | la casa | no |
| el ambito de un commit | la casa | no |
| `R-APP1..7` | **el contrato** | **si: sin ellas su app no corre** |

```text
   las reglas del CONTRATO   viajan: sin ellas tu app NO CORRE
   las reglas de la CASA     no viajan: son como se mantiene ESTE arbol
```

★ Los guardianes leen `git ls-files` de **este** repositorio. La app de un
tercero no esta aqui, asi que no la miran nunca. Un `.bex` de 4.000 lineas de
fuente arranca igual que uno de cuarenta.

---

# 4. ★★ EL PEAJE -- lo que cuesta meter algo en la superficie

Esta es la parte que contesta *"analizar y compensar"*. **Nada entra gratis.**
Cada cosa nueva que se concede paga estas seis, y si no las paga, no entra.

| # | el peaje | por que |
|---|---|---|
| 1 | **un numero que quepa en su campo** | ver la parte 5: uno que no cabe **compila** |
| 2 | **libre en las DOS tablas** | kernel y `bmo-abi` son dos ficheros que no se hablan |
| 3 | **un NO con nombre** para cada forma de negarlo | un codigo de error no dice *"pisa la RAM que empieza en 0x100000"* |
| 4 | **se suelta al morir el dueno** | `R-APP6`. Sin esto, un proceso que revienta deja el recurso ocupado hasta reiniciar |
| 5 | **una prueba que pueda VER el fallo** | no una que pase; una que falle si la regla se rompe |
| 6 | **una linea en las tres tablas** (kernel, ABI, userland) | el guardian de `build.ps1` la exige, y por eso no se olvida |

## 4.1 -- El ejemplo trabajado: `KIND_MMIO`, metido hoy

No es un ejemplo inventado. Es lo que entro en el arbol el 2026-08-26.

| peaje | como se pago |
|---|---|
| 1. cabe en su campo | `0x74 <= 0x7F`, con un `const _: ()` que lo comprueba al compilar |
| 2. libre en las dos | libre en `cap.rs` y en `HandleKind` |
| 3. un NO con nombre | **nueve vetos** en `bmo-mmio-juicio`, cada uno con su frase y su numero a CABINA |
| 4. se suelta al morir | `mmio::process_died` cableado en `revoke_all` |
| 5. una prueba que ve el fallo | 23, y una exige que **el caso legitimo entre** -- sin ella, un juez que dijera `Err` siempre pasaria las otras 22 |
| 6. las tres tablas | `TASK_OP_APARATO_TOMAR/SOLTAR` y `APARATO_OP_BASE/BYTES` en kernel, ABI y userland |

★★ **Y el peaje mas caro no esta en la tabla, porque no es una linea: es lo que
se decidio NO conceder.** Ninguna operacion de `KIND_MMIO` acepta una direccion
fisica, porque un proceso que pudiera nombrarla estaria pidiendo ser el kernel.
Esa renuncia es lo que hace que las seis filas de arriba signifiquen algo.

---

# 5. ⚠ LO QUE PASA CUANDO EL PEAJE NO SE PAGA -- dos casos REALES de hoy

## 5.1 -- El menor del ABI que decia ser aditivo y se comprobaba exacto

`bmo-abi` declaraba *"minor versions are additive"*. El cargador --que es quien
de verdad decide-- comprobaba `abi_menor == 0`. **Exacto, no aditivo.**

```text
   el dia que el ABI subiera a 2.1  ->  el cargador rechaza un binario que el
                                        contrato dice que tiene que entrar
```

No habia hecho dano porque nadie ha subido el menor nunca. **Eso no es que
estuviera bien: es que todavia no se habia cobrado.** Peaje incumplido: el 5, y
se noto al escribirlo -- las dos primeras pruebas comparaban las dos copias entre
si y **pasaban con la regla mala**.

> Lo que separa dos reglas no es el caso que se usa: es el que todavia no.

## 5.2 -- ★★ `KIND_TAREA = 0x80`, un numero que no cabia en su campo

Encontrado hoy, escribiendo la parte 4 de este documento. El campo `kind` de un
handle son **siete bits**:

```text
   encode:   (0x80 & 0x7F) << 56    ->  el campo kind del handle vale 0
   resolve:  slot.kind (0x80) != kind (0)   ->  ERROR_INVALID_HANDLE
```

*** **No fallaba a veces: fallaba SIEMPRE.** Cada handle `KIND_TAREA` --el hijo
que un proceso lanza, o sea el paso 3 de `PLAN_DIRECTOR.md`-- se rechazaba como
invalido en cuanto alguien lo usaba. Y el mensaje decia *"handle invalido"*, que
manda a mirar al que llama.

★ La cabecera de `HANDLE_KIND_SHIFT` lo habia predicho veinte lineas mas arriba
del sitio donde estaba pasando:

> *"un desplazamiento mal puesto COMPILA, devuelve handle invalido, y se lee
> como un permiso denegado. El otro al menos mataba la maquina."*

**Arreglado a `0x55`, y con el portico**: un `const _: ()` que comprueba los
trece kinds al compilar. Comprobado por mutacion -- con `0x85` el build para con
el nombre de la constante en la linea del error.

[!] Y cambiarlo **no rompe compatibilidad con nada**, que es lo unico que
permitio arreglarlo de una vez: no habia un solo handle de ese tipo que
funcionara.

---

# 6. QUIEN COMPRUEBA CADA COSA HOY, y donde no hay nadie

| lo que se protege | quien lo vigila | corre solo? |
|---|---|---|
| operacion del kernel que falte en el ABI | `build.ps1`, contrato de Ring 0 | ✅ **99 comprobadas** |
| operacion del userland que no cuadre | `build.ps1` | ✅ **89 comprobadas** |
| dos operaciones con el mismo numero | `build.ps1` | ✅ |
| campos de `OP_INFO` en los tres lados | `build.ps1` | ✅ |
| un `kind` que no quepa en su campo | `const _: ()` en `cap.rs` | ✅ **desde hoy** |
| el ABI y el gate diciendo versiones distintas | `gate_y_validador_no_se_separan.rs` | ✅ **desde hoy** |
| una relocation fuera de su seccion | el mismo fichero | ✅ |
| **el `kind` del kernel contra el de `bmo-abi`** | **NADIE** | ⛔ **y ya han divergido** |
| **una operacion que cambia de SIGNIFICADO sin cambiar de numero** | NADIE | ⛔ no lo puede ver una herramienta |

## 6.1 -- ⚠ La deuda de la penultima fila, con nombres

Las dos tablas de kinds --`cap.rs` en el kernel y `HandleKind` en `bmo-abi`--
**ya dicen cosas distintas en seis codigos**:

```text
   0x30   kernel: CONSOLE       abi: NetSocket
   0x40   kernel: DIRECTORIO    abi: File
   0x41   kernel: ARCHIVO       abi: Directory     <- ademas CRUZADOS
   0x50   kernel: MEMORIA       abi: Process
   0x51   kernel: PRESTADO      abi: Thread
   0x70   kernel: ENDPOINT      abi: (no existe)
```

★ Hoy no hace dano porque **el `kind` del handle solo lo interpreta el kernel**:
el ABI declara la taxonomia y no la usa para resolver nada. Es deuda, no fallo --
y esta escrita aqui para que el dia que alguien de Ring 3 mire el byte del
`kind`, sepa que esa tabla no es la que manda.

[!] Por eso `KIND_MMIO` entro con `0x74`, **libre en las dos**. Anadir a una
divergencia que ya existe es la unica forma de que deje de ser deuda y pase a
ser fallo.

---

# 7. EL RESUMEN, EN CUATRO LINEAS

```text
   lo que se congela   dos puertas, los numeros dados, BEF1, el mayor, R-APP
   lo que crece        operaciones, kinds, secciones, cabeceras, el menor
   el peaje            seis cosas, y la mas cara es lo que se decide NO conceder
   quien vigila        cinco guardianes automaticos, y DOS huecos con nombre
```
