# `lang/` -- los frontends, la esencia de cada lenguaje

Aqui vive **la esencia** de cada lenguaje: lo que solo ese lenguaje sabe hacer.
Todo lo compartido esta un piso mas abajo, en [`../forge/`](../forge/).

## Regla de arquitectura

- **El kernel no compila lenguajes.** Los frontends son herramientas offline.
- La salida canonica es **BEX** (`.bex`), codificado como BEF1.
- `bmo-abi` define la superficie, los tipos y el contrato de ejecucion.
- **Los frontends jamas se enlazan entre si.** Si dos lenguajes necesitan lo
  mismo, eso se promueve a `forge/` como libreria opcional -- nunca se importa
  `lang/cobol` desde `lang/ada`.

> ★ **Antes de anadir nada a cualquiera de estos frontends**, leer
> [`PROPOSITO.md`](PROPOSITO.md): para que existe cada lenguaje, y por que eso
> --y no el estandar-- decide que entra. Incluye el caso de Itanium, que es la
> razon de que una promesa al optimizador no cuente como trabajo pendiente.

## Los cuatro

| Directorio | Estado | Que corre |
|---|---|---|
| [`c/`](c/) | El mas completo | C de Ritchie hasta ~C11: declaradores y expresiones completos, structs y arrays con tamanos reales, punteros multinivel y a funcion, structs por valor, listas de inicializacion, macros con parametros, floats SSE, `printf` en linea, `getchar`/`scanf`, e intrinsecos de maquina desde tabla TOML |
| [`cobol/`](cobol/) | Cerrado en su alcance de banca | Decimal exacto en escala entera, `PICTURE` de edicion emitida como instrucciones, File I/O secuencial (`SELECT`/`FD`/`OPEN`/`READ ... AT END`/`WRITE`/`CLOSE`), `OCCURS` con guarda de rango, nivel 88, `IF`/`PERFORM`/`COMPUTE` |
| [`ada/`](ada/) | Primer incremento | Perfil **ZFP secuencial + Annex F**: `type ... is delta ... digits ...`, `Put_Line`, `if/else`, `while ... loop`, precedencia real. Crate propio, **sin depender de `cobol/`** |
| [`cpp/`](cpp/) | Minimo (~900 lineas) | Alcance decidido: hasta lo esencial de C++17. Fuera: concepts, coroutines, modules, ranges, STL grande |

## Por que estos tres (y no otros)

**COBOL** por el dinero: el decimal exacto no es una caracteristica que se
anade, tiene que llegar hasta la instruccion emitida. **C** por el control: es
la herramienta neutra, su trabajo es no estorbar. **Ada** por la seguridad: un
valor fuera de rango se *detecta*, no se envuelve.

Y el hallazgo que abarato Ada: el Annex F copio las reglas del `PICTURE` de
COBOL en 1985, asi que el decimal exacto ya estaba pagado.

## Como se verifica

Cada frontend tiene su **matriz de conformidad**, que compila el programa y
luego **ejecuta los bytes emitidos** comprobando la salida real -- no los compara
contra cadenas escritas a mano. Un `IF` que no bifurca se ve identico a uno que
si en un volcado de bytes.

**Al anadir una caracteristica al codegen hay que anadirle su fila.** Por eso
aqui no se dan porcentajes: un porcentaje necesita un denominador, y el estandar
de COBOL no tiene uno.

Y lo que no esta implementado **se rechaza con motivo**, nunca con un stub que
aparente funcionar.

## La autoridad

1. El Ryzen real
2. El documento de la especificacion
3. El emulador

Cuando el emulador y el hardware no cuadran, **se arregla el emulador**. Esta
fijado en [`c/VERDAD.md`](c/VERDAD.md).
