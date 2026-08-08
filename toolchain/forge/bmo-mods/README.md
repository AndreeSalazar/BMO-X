# bmo-mods -- extender BMO sin pedirle permiso a nadie

Un comite decide que entra en un lenguaje y cuando. Da estabilidad y cuesta
anos por cada cambio. Aqui no hay comite: **el que quiere una extension la
declara y la usa.**

Esta libreria es el contrato de ese mecanismo. Como todo en `forge/`, se
**elige**; quien no la quiera sigue leyendo sus ficheros a mano.

## Las tres posturas

No hay que elegir bando. El mismo mecanismo da tres, y son tres de verdad:

| Quiero... | Escribo | Que pasa cuando BMO corrige algo |
|---|---|---|
| **el estandar de BMO** tal cual | nada | me llega |
| **mi propio estandar** | una tabla SIN `parent` | no me llega, y es lo que pedi |
| **anadir cosas** a uno que ya hay | una tabla CON `parent` y solo el delta | me llega, encima de lo mio |

La tercera es la que impide que esto sea anarquia. Un mod de cinco lineas
sobre `c11` **no puede bifurcar el resto**: hereda lo que no toca. Copiar la
tabla entera para cambiar tres claves si es una bifurcacion -- y es lo unico
que se podia hacer antes de que la herencia existiera.

```toml
# mi-mod/standards/C/miempresa.toml -- cinco lineas, C11 entero debajo
[standard]
short_name = "MIEMPRESA"

[features]
saturating_math = true    # lo mio
trigraphs        = false  # y puedo APAGAR algo del padre

[based_on]
parent = "c11"
```

`lineage()` ensena la cadena (`miempresa -> c11 -> c99 -> c89`) y `origin()` dice
**que fichero** puso cada valor. En un sistema donde cualquiera puede tapar
una tabla, "de donde ha salido esto?" es la primera pregunta de todo el
mundo.

Una cadena que se muerde la cola se caza y se ensena entera, en vez de colgar
el compilador.

## Escribir un mod en un minuto

Un mod es un directorio con tablas. No es codigo, no se ejecuta, y por eso no
puede robar nada.

```
mi-mod/
+-- standards/
    +-- C/
        +-- c99-mio.toml
```

```toml
[standard]
short_name = "C99-MIO"
year = 2026

[features]
line_comments = true
mi_extension  = true      # <- ningun Rust de este repo conoce esta clave

[type_rules]
implicit_int = false
```

```bash
export BMO_MODS=/ruta/a/mi-mod
```

Y ya esta cargado. `bmo-c-front --std c99-mio` lo encuentra, y
`feats.on("mi_extension")` lo lee.

## Las tres reglas

**1. Tu raiz va primero.** `$BMO_MODS` se busca antes que las tablas del
sistema. Puedes **tapar** `c89.toml` con el tuyo sin editar el repo. Corregir
el sistema no deberia obligarte a bifurcarlo.

**2. Ausente no es error.** Un estandar viejo simplemente no menciona lo que
aun no existia. `on()` devuelve `false` y no pasa nada. Donde la diferencia
importa --las reglas de tipos-- `rule()` devuelve `Option`, porque "C89 permite
`int` implicito" y "esta tabla no dice nada" llevan a compiladores distintos.

**3. Las secciones son de verdad.** `[features]` y `[type_rules]` no se
mezclan. Antes si: los dos lectores que habia partian lineas por `=` e
ignoraban las secciones. Funcionaba por suerte, porque ninguna clave se
repetia todavia.

## La frontera honesta

Esto quita el Rust de **DECLARAR** una extension, no de implementarla.

Anadir `mi_extension = true` es gratis. Que el compilador haga algo distinto
sigue siendo codigo. Es la misma frontera que la fabrica de COBOL: lo tabular
se genera, la semantica de cada verbo se escribe. Prometer mas seria vender
compatibilidad que no existe -- que es justo el fallo del que este proyecto
huye.

Lo que si desaparece: antes, anadir una caracteristica exigia tocar **tres
sitios de Rust** (el campo del struct, su `Default` y el `match` del lector).
Ese era el tramite de comite en miniatura, y ya no esta.

## Auditar lo que sale de aqui

Si cualquiera puede escribir un dialecto, **el binario ya no basta**: dos
`.bex` iguales por fuera pueden venir de tablas distintas. La respuesta no es
prohibir mods, es que el binario lleve encima de que tablas salio.

Eso es lo que necesita un banco, y esta planteado --no hecho-- en
[AUDITORIA.md](AUDITORIA.md).

## Que cubre y que no

| | |
|---|---|
| `standards/<LENG>/<x>.toml` | Que tiene y que permite un lenguaje. C, C++ y COBOL entran por la misma puerta. |
| `<modulo>/BMO.toml` | Que ofrece un modulo: `[exports]`, `[sources]`, y `provides`/`requires` -- los mismos nombres de capacidad que `BMO_SYMBOLS.toml`. |
| `arch/<isa>/*.toml` | Instrucciones. Las lee `sem-asm`, que ya tenia parser propio. |

**Mods de CODIGO (un `.bex` que ofrece servicios): todavia no.** Eso son datos
que se ejecutan, y necesita el gate de firma y `bmo-verify` -- que hoy esta
escrito y **no tiene un solo usuario** en el workspace. Ese es el orden
correcto, y por eso se empieza por los datos.

## Por que existia el problema

Habia tres lectores de TOML para el mismo formato, y dos llevaban **copiada**
la misma lista de cinco rutas candidatas. Cuando el repo se reorganizo, una de
las copias apunto durante meses a un directorio muerto: el gating de
estandares caia al default **en silencio**, y `//` pasaba a estar permitido en
C89 sin que nadie se enterara.

Un formato con tres lectores tiene tres formas de mentir.
