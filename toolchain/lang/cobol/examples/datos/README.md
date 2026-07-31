# Los datos de los ejemplos

Lo que los programas de `4-ficheros/` y `5-tablas/` LEEN. El build los copia a
`datos/` del volumen de datos, y los `SELECT … ASSIGN TO "datos/…"` de los
ejemplos apuntan ahí.

★ **Estaban sólo en `staging/`, que está en el `.gitignore`.** O sea: no eran
del repositorio. Un `build.ps1 -Clean` o un disco nuevo los borraba y **no había
forma de regenerarlos** — los ejemplos de ficheros quedaban sin entrada y sin
nadie que supiera qué debían contener. Ahora viven aquí y el build los despliega
como despliega los `.bex`.

| Fichero | Quién lo lee | Qué es |
|---|---|---|
| `movim.txt` | `batch.cob`, `cartera.cob` | Importes con decimales, uno por línea, **con negativos**: sin uno negativo el `IF` del signo no se ejercita |
| `concs.txt` | `conceptos.cob` | Números de concepto (1..3), el subíndice de la tabla |
| `imps.txt` | `conceptos.cob` | El importe de cada movimiento, en paralelo con `concs.txt` |
| `grande.txt` | nadie automáticamente | 900 líneas. Para `lee` y para el scroll: llena el historial de golpe |

Los cuatro acaban **con** salto de línea. El caso que falta —un fichero cuyo
último renglón NO lo lleva— es el que se comía el registro más reciente, y
`leer_linea` ya lo arregló; que no haya aquí un fichero así significa que ese
arreglo **no está cubierto por ningún ejemplo**. Anotado como hueco, no como
decisión.
