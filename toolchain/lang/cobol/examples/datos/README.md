# Los datos de los ejemplos

Lo que los programas de `4-ficheros/` y `5-tablas/` LEEN. El build los copia a
`datos/` del volumen de datos, y los `SELECT ... ASSIGN TO "datos/..."` de los
ejemplos apuntan ahi.

★ **Estaban solo en `staging/`, que esta en el `.gitignore`.** O sea: no eran
del repositorio. Un `build.ps1 -Clean` o un disco nuevo los borraba y **no habia
forma de regenerarlos** -- los ejemplos de ficheros quedaban sin entrada y sin
nadie que supiera que debian contener. Ahora viven aqui y el build los despliega
como despliega los `.bex`.

| Fichero | Quien lo lee | Que es |
|---|---|---|
| `movim.txt` | `batch.cob`, `cartera.cob` | Importes con decimales, uno por linea, **con negativos**: sin uno negativo el `IF` del signo no se ejercita |
| `concs.txt` | `conceptos.cob` | Numeros de concepto (1..3), el subindice de la tabla |
| `imps.txt` | `conceptos.cob` | El importe de cada movimiento, en paralelo con `concs.txt` |
| `grande.txt` | nadie automaticamente | 900 lineas. Para `lee` y para el scroll: llena el historial de golpe |

Los cuatro acaban **con** salto de linea. El caso que falta --un fichero cuyo
ultimo renglon NO lo lleva-- es el que se comia el registro mas reciente, y
`leer_linea` ya lo arreglo; que no haya aqui un fichero asi significa que ese
arreglo **no esta cubierto por ningun ejemplo**. Anotado como hueco, no como
decision.
