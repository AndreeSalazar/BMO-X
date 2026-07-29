# bmo-mods — extender BMO sin pedirle permiso a nadie

Un comité decide qué entra en un lenguaje y cuándo. Da estabilidad y cuesta
años por cada cambio. Aquí no hay comité: **el que quiere una extensión la
declara y la usa.**

Esta librería es el contrato de ese mecanismo. Como todo en `forge/`, se
**elige**; quien no la quiera sigue leyendo sus ficheros a mano.

## Las tres posturas

No hay que elegir bando. El mismo mecanismo da tres, y son tres de verdad:

| Quiero… | Escribo | Qué pasa cuando BMO corrige algo |
|---|---|---|
| **el estándar de BMO** tal cual | nada | me llega |
| **mi propio estándar** | una tabla SIN `parent` | no me llega, y es lo que pedí |
| **añadir cosas** a uno que ya hay | una tabla CON `parent` y sólo el delta | me llega, encima de lo mío |

La tercera es la que impide que esto sea anarquía. Un mod de cinco líneas
sobre `c11` **no puede bifurcar el resto**: hereda lo que no toca. Copiar la
tabla entera para cambiar tres claves sí es una bifurcación — y es lo único
que se podía hacer antes de que la herencia existiera.

```toml
# mi-mod/standards/C/miempresa.toml — cinco lineas, C11 entero debajo
[standard]
short_name = "MIEMPRESA"

[features]
saturating_math = true    # lo mio
trigraphs        = false  # y puedo APAGAR algo del padre

[based_on]
parent = "c11"
```

`lineage()` enseña la cadena (`miempresa → c11 → c99 → c89`) y `origin()` dice
**qué fichero** puso cada valor. En un sistema donde cualquiera puede tapar
una tabla, "¿de dónde ha salido esto?" es la primera pregunta de todo el
mundo.

Una cadena que se muerde la cola se caza y se enseña entera, en vez de colgar
el compilador.

## Escribir un mod en un minuto

Un mod es un directorio con tablas. No es código, no se ejecuta, y por eso no
puede robar nada.

```
mi-mod/
└── standards/
    └── C/
        └── c99-mio.toml
```

```toml
[standard]
short_name = "C99-MIO"
year = 2026

[features]
line_comments = true
mi_extension  = true      # ← ningún Rust de este repo conoce esta clave

[type_rules]
implicit_int = false
```

```bash
export BMO_MODS=/ruta/a/mi-mod
```

Y ya está cargado. `bmo-c-front --std c99-mio` lo encuentra, y
`feats.on("mi_extension")` lo lee.

## Las tres reglas

**1. Tu raíz va primero.** `$BMO_MODS` se busca antes que las tablas del
sistema. Puedes **tapar** `c89.toml` con el tuyo sin editar el repo. Corregir
el sistema no debería obligarte a bifurcarlo.

**2. Ausente no es error.** Un estándar viejo simplemente no menciona lo que
aún no existía. `on()` devuelve `false` y no pasa nada. Donde la diferencia
importa —las reglas de tipos— `rule()` devuelve `Option`, porque "C89 permite
`int` implícito" y "esta tabla no dice nada" llevan a compiladores distintos.

**3. Las secciones son de verdad.** `[features]` y `[type_rules]` no se
mezclan. Antes sí: los dos lectores que había partían líneas por `=` e
ignoraban las secciones. Funcionaba por suerte, porque ninguna clave se
repetía todavía.

## La frontera honesta

Esto quita el Rust de **DECLARAR** una extensión, no de implementarla.

Añadir `mi_extension = true` es gratis. Que el compilador haga algo distinto
sigue siendo código. Es la misma frontera que la fábrica de COBOL: lo tabular
se genera, la semántica de cada verbo se escribe. Prometer más sería vender
compatibilidad que no existe — que es justo el fallo del que este proyecto
huye.

Lo que sí desaparece: antes, añadir una característica exigía tocar **tres
sitios de Rust** (el campo del struct, su `Default` y el `match` del lector).
Ése era el trámite de comité en miniatura, y ya no está.

## Qué cubre y qué no

| | |
|---|---|
| `standards/<LENG>/<x>.toml` | Qué tiene y qué permite un lenguaje. C, C++ y COBOL entran por la misma puerta. |
| `<modulo>/BMO.toml` | Qué ofrece un módulo: `[exports]`, `[sources]`, y `provides`/`requires` — los mismos nombres de capacidad que `BMO_SYMBOLS.toml`. |
| `arch/<isa>/*.toml` | Instrucciones. Las lee `sem-asm`, que ya tenía parser propio. |

**Mods de CÓDIGO (un `.bex` que ofrece servicios): todavía no.** Eso son datos
que se ejecutan, y necesita el gate de firma y `bmo-verify` — que hoy está
escrito y **no tiene un solo usuario** en el workspace. Ése es el orden
correcto, y por eso se empieza por los datos.

## Por qué existía el problema

Había tres lectores de TOML para el mismo formato, y dos llevaban **copiada**
la misma lista de cinco rutas candidatas. Cuando el repo se reorganizó, una de
las copias apuntó durante meses a un directorio muerto: el gating de
estándares caía al default **en silencio**, y `//` pasaba a estar permitido en
C89 sin que nadie se enterara.

Un formato con tres lectores tiene tres formas de mentir.
