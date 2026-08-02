# `lang/` — los frontends, la esencia de cada lenguaje

Aquí vive **la esencia** de cada lenguaje: lo que sólo ese lenguaje sabe hacer.
Todo lo compartido está un piso más abajo, en [`../forge/`](../forge/).

## Regla de arquitectura

- **El kernel no compila lenguajes.** Los frontends son herramientas offline.
- La salida canónica es **BEX** (`.bex`), codificado como BEF1.
- `bmo-abi` define la superficie, los tipos y el contrato de ejecución.
- **Los frontends jamás se enlazan entre sí.** Si dos lenguajes necesitan lo
  mismo, eso se promueve a `forge/` como librería opcional — nunca se importa
  `lang/cobol` desde `lang/ada`.

## Los cuatro

| Directorio | Estado | Qué corre |
|---|---|---|
| [`c/`](c/) | El más completo | C de Ritchie hasta ~C11: declaradores y expresiones completos, structs y arrays con tamaños reales, punteros multinivel y a función, structs por valor, listas de inicialización, macros con parámetros, floats SSE, `printf` en línea, `getchar`/`scanf`, e intrínsecos de máquina desde tabla TOML |
| [`cobol/`](cobol/) | Cerrado en su alcance de banca | Decimal exacto en escala entera, `PICTURE` de edición emitida como instrucciones, File I/O secuencial (`SELECT`/`FD`/`OPEN`/`READ … AT END`/`WRITE`/`CLOSE`), `OCCURS` con guarda de rango, nivel 88, `IF`/`PERFORM`/`COMPUTE` |
| [`ada/`](ada/) | Primer incremento | Perfil **ZFP secuencial + Annex F**: `type … is delta … digits …`, `Put_Line`, `if/else`, `while … loop`, precedencia real. Crate propio, **sin depender de `cobol/`** |
| [`cpp/`](cpp/) | Mínimo (~900 líneas) | Alcance decidido: hasta lo esencial de C++17. Fuera: concepts, coroutines, modules, ranges, STL grande |

## Por qué estos tres (y no otros)

**COBOL** por el dinero: el decimal exacto no es una característica que se
añade, tiene que llegar hasta la instrucción emitida. **C** por el control: es
la herramienta neutra, su trabajo es no estorbar. **Ada** por la seguridad: un
valor fuera de rango se *detecta*, no se envuelve.

Y el hallazgo que abarató Ada: el Annex F copió las reglas del `PICTURE` de
COBOL en 1985, así que el decimal exacto ya estaba pagado.

## Cómo se verifica

Cada frontend tiene su **matriz de conformidad**, que compila el programa y
luego **ejecuta los bytes emitidos** comprobando la salida real — no los compara
contra cadenas escritas a mano. Un `IF` que no bifurca se ve idéntico a uno que
sí en un volcado de bytes.

**Al añadir una característica al codegen hay que añadirle su fila.** Por eso
aquí no se dan porcentajes: un porcentaje necesita un denominador, y el estándar
de COBOL no tiene uno.

Y lo que no está implementado **se rechaza con motivo**, nunca con un stub que
aparente funcionar.

## La autoridad

1. El Ryzen real
2. El documento de la especificación
3. El emulador

Cuando el emulador y el hardware no cuadran, **se arregla el emulador**. Está
fijado en [`c/VERDAD.md`](c/VERDAD.md).
