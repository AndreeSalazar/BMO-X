# Auditoría: que el binario diga con qué reglas se hizo

> **Esto es un plan, no una descripción.** Nada de lo que dice "PENDIENTE"
> existe todavía. Lo que dice "YA ESTÁ" se puede comprobar hoy en el repo.
> Un documento que confunde las dos cosas es peor que no tenerlo.

## El problema, dicho como lo diría un banco

Un auditor no pregunta *"¿el número está bien?"*. Pregunta **"¿con qué reglas
se calculó ese número, y cómo lo pruebo?"**.

Ahí `bmo-mods` abre una puerta y un agujero a la vez. Si cualquiera puede
escribir un dialecto y compilar con él, entonces **el binario ya no basta**:
dos `.bex` idénticos por fuera pueden venir de tablas distintas. La respuesta
no es prohibir los mods — es que el binario **lleve encima** de qué tablas
salió.

## La pieza incómoda: dónde corre el compilador

`bmo-cobol-front.exe` es un binario de **Windows**. La cadena de hoy es
cruzada: se compila en el PC de desarrollo y el `.bex` viaja al Ryzen. Las
tablas viven en el disco del PC y se consumen al compilar.

Eso obliga a separar dos ideas que suenan igual:

| | Qué es | Tamaño |
|---|---|---|
| **Compilar en BMO** | que el compilador CORRA sobre BMO-X y lea las tablas de su propio disco | autoalojamiento; otro proyecto entero |
| **Auditar en BMO** | que las tablas y la procedencia VIAJEN al disco y se puedan inspeccionar allí | acotado, y es lo que pide un banco |

Un auditor no necesita recompilar. Necesita **leer** y **comparar**. Por eso
la segunda columna se puede hacer ya y la primera no hace falta para esto.

## Las tres piezas

### 1. Las tablas viajan al disco — `BMO-DATA/`

**YA ESTÁ:** `staging/BMO-DATA/` es el espejo del volumen de datos FAT, y
`build.ps1` ya copia `BMO-DATA/apps/` a la máquina.

**PENDIENTE:** que copie también las tablas —`standards/`, los mods usados—
a `BMO-DATA/tablas/`. Son TOML: se leen con el `ls` que ya existe y con
cualquier visor de texto, **sin Rust y sin el repositorio delante**. Ésa es
la parte de *"no en vista de Rust, sino otra vista"*: la vista es el fichero,
y el fichero es legible por una persona.

### 2. El binario lleva su procedencia — sección `Manifest`

**YA ESTÁ:** el contenedor BEF tiene `SectionKind::Manifest = 0x09`
(*"Manifest TOML (capabilities, version, dependencies)"*), con su constructor
`BefSection::manifest_toml()` y su validador.

**PENDIENTE:** que algún frontend la escriba. Hoy tiene **cero escritores**.

Lo que debería llevar dentro:

```toml
[procedencia]
lenguaje  = "COBOL"
estandar  = "cobol85"
cadena    = ["miempresa", "cobol85"]   # lo que devuelve lineage()

[tablas]
"cobol85"   = "blake3:9f2c…"
"miempresa" = "blake3:41ab…"
```

Con eso, auditar un `.bex` deja de ser un acto de fe: se abre, se leen los
hashes, y se comparan con las tablas de `BMO-DATA/tablas/`. Si no cuadran, el
binario no salió de lo que dice.

BLAKE3 no hay que traerlo: es el mismo que ya usan las firmas del BEF,
ESTRATOS y `bmo-hash`. **Uno solo en todo el sistema**, a propósito.

### 3. Aprobar un mod = meter su hash en un conjunto

La idea de Eddi: *"cuando estén aprobados por ellos, ya se convierten en
estándar"*. Técnicamente eso no es una ceremonia, es una lista.

**PENDIENTE:** un `aprobados.toml` en `BMO-DATA/` con los hashes que la
organización acepta. Un `.bex` cuya procedencia cite una tabla que no está en
la lista **no es inválido** — es *no aprobado*, que es distinto y hay que
poder distinguirlo. Un banco necesita las dos cosas: probar cosas nuevas y
saber cuáles pasaron revisión.

Y aquí encaja `bmo-verify`: **está escrito, es miembro del workspace y no
tiene un solo usuario.** Es el sitio natural de esta comprobación, y hasta que
se cablee, cualquier promesa de aprobación es decorativa.

## Por qué esto es federación y no anarquía

El ciclo se cierra solo:

1. Cualquiera escribe un mod (`parent = "cobol85"` + su delta).
2. Compila; el `.bex` sale diciendo de qué salió.
3. La organización lo audita leyendo tablas y hashes.
4. Si pasa, su hash entra en la lista de aprobados.
5. A partir de ahí es estándar **para esa organización** — sin comité, sin
   esperar años, y sin que nadie más tenga que aceptarlo.

Lo que impide la anarquía no es prohibir: es que **todo lo que se hace queda
declarado**. Un mod dice de quién hereda; un binario dice de qué mod salió.

## Lo que sigue sin resolver, y hay que decirlo

- **Dos mods no tienen por qué ser compatibles entre sí.** Un comité da esa
  promesa; esto no. Es un intercambio real.
- **Nada de esto detiene a un compilador mentiroso.** La procedencia la
  escribe el propio compilador; un compilador modificado escribe lo que
  quiera. Contra eso sólo sirve firmar el compilador, y eso es el gate de
  identidad — otra conversación.
- **La cadena cruzada sigue siendo cruzada.** Auditar en BMO no es compilar en
  BMO, y confundirlas sería prometer autoalojamiento sin tenerlo.
