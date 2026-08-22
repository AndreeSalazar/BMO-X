# `cpu.inti` -- la prediccion, escrita antes de correrla

```text
   run inti/cpu.bex
```

El `.bex` lo produce **`build.ps1`**, en `staging\BMO-DATA\inti\cpu.bex`, que es
el espejo de lo que va al volumen de datos. Sale del mismo paso que los ejemplos
de C, COBOL y Ada.

OJO: la primera version se dejo en una carpeta `inti/` del repo y **nunca llego
al FAT32**. Es el mismo fallo que este bloque del build ya tenia escrito de
cuando le paso a C: *un lenguaje que compila y cuyo binario no se despliega esta
a medias*.

El programa deja su informe en **`/inti/cpu.txt`** ademas de sacarlo por
pantalla.

---

## ⚠ LO QUE PASO, Y LO QUE CAMBIO DESPUES (22-08)

**Corrio. Siete de las ocho lineas acertaron la prediccion**, y las que dan cero
--`bits` y `atomicas`, diecisiete comprobaciones-- salieron a cero. El programa
llego a `-- fin -`. La sonda escribio ella misma `/inti/cpu.txt`, o sea que
guardar por la puerta funciona.

★ Y en tres ejecuciones **las seis lineas de hechos salieron bit por bit
identicas**. Solo se movio `tsc`. Eso es lo que tiene que pasar: una medida
varia, un hecho no.

### La que fallo, y lo que enseno

`tsc` salio 6 veces por encima del techo predicho. La causa se midio: el
asignador de registros se apagaba entero en cuanto una funcion tenia una
instruccion de maquina, asi que el bucle vivia en la pila.

Se arreglo --acotando el freno con lo que las tablas ya declaran-- y el resultado
fue **0,6% por debajo de la mejor medida anterior, con un ruido del 11,6% entre
dos ejecuciones sin tocar nada**.

*** O sea: no se puede afirmar ninguna mejora. Y el motivo de fondo es que el
arreglo ataco la mitad equivocada -- `cambiante i` es una LOCAL, y las locales no
van nunca a registro: solo los temporales. El contador seguia yendo a memoria.

### Por eso esta sonda mide distinto desde el 22-08

```text
   antes    una medida
   ahora    la MEJOR DE OCHO, y una linea `ruido` con la peor menos la mejor
```

Lo que contamina una medida --una interrupcion, un cambio de frecuencia, un fallo
de cache-- solo puede SUMAR tiempo. Asi que el minimo es la muestra menos
contaminada, y no un promedio de contaminaciones.

** Y la linea `ruido` es la que decide si el numero de al lado sirve: **una
optimizacion tiene que bajar mas que el ruido para que se pueda decir que bajo**.
Sin ese margen, un numero es un numero suelto.

---

## LA PREDICCION, escrita ANTES de correrlo

★★★ **Esto es lo que espero que salga.** Va escrito por delante a proposito: una
prediccion que se escribe despues de ver el resultado no vale nada, y este
proyecto ya tiene el metodo -- *medir en vez de opinar*, y **decir antes lo que
se espera medir**.

| linea | prediccion | si sale otra cosa | ✅ salio (22-08, Ryzen) |
|---|---|---|---|
| `-- cpu -` (1) | **entre 0x0D y 0x10**. Un Ryzen moderno entiende hasta la hoja 13-16 | **0x00000000** = `cpuid` no llego a ejecutarse | **0x10** ✅ acertada |
| `-- cpu -` (2) | **0x00A20F1x** o parecido: familia 0x19 (Zen 3/4) codificada | 0 = lo mismo de arriba | **0x00A20F12** ✅ acertada. ⚠ Es Zen **3** (Vermeer, 19h/21h), no Zen 4: Raphael es 19h/61h |
| `tsc` | ~~0x400-0x2000~~ **FALLO: salio 0xB68B**. Con la medida nueva (mejor de ocho) deberia bajar de ahi | **0** = el contador no avanza, y toda medida futura vale cero | **0xB588** (46.472). Bajo, si: la mejor de ocho |
| `xcr0` | **0x00000007** (x87 + SSE + AVX) o **0x00000207** con AVX-512 | **0** = el estado extendido no esta encendido, y eso explicaria el `#GP` de `xrstor` | **0x07** ✅ acertada. Sin AVX-512, como toca en Zen 3 |
| `azar` | **0x00000001** -- dos tiradas distintas | **0** = `rdrand` devuelve siempre lo mismo, o no ejecuto | **0x01** ✅ acertada |
| `bits` | ★ **0x00000000** | **cualquier otro numero**: cada bit dice que cuenta fallo. Ver la tabla de abajo | **0x00** ★ APROBADO |
| `atomicas` | ★ **0x00000000** | idem, y ahi el sospechoso es la memoria, no el CPU | **0x00** ★ APROBADO |
| **`reglas`** | ★★★ **0x00000000.** Las tres reglas anti-UB atrapando **en silicio**: desborde (bit 0), entre cero (bit 1), conversion (bit 2). Es la linea que decide si *"INTI no tiene comportamiento indefinido"* es verdad o es una frase | cualquier bit encendido = esa regla NO atrapo, y el programa siguio con un numero inventado | ★★★ **0x00** APROBADO. Las tres atraparon: 1001, 1003, 1012 |
| `ruido` | **la peor menos la mejor de ocho.** Cuanto mas bajo, mas se puede afirmar | si es mayor que `tsc`, la medida no vale para comparar nada | **0x17B4** (6.068) = 13,1%. Menor que `tsc`, o sea que la medida vale |
| `-- fin -` | **tiene que salir** | si no sale, el programa murio antes: mira cual fue la ultima linea | **salio** ✅ el programa llego al final |

### ⚠ Las TRES unicas que se pueden SUSPENDER

Las demas dicen lo que el CPU diga y no hay contra que compararlas. Estas tres
tienen respuesta conocida, **y ya dan cero en el emulador**:

```text
   bits       bit 0   cuenta_unos(255) no dio 8
              bit 1   cuenta_unos(0) no dio 0
              bit 2   ceros_detras(8) no dio 3
              bit 3   ceros_detras(1) no dio 0
              bit 4   primer_uno(8) no dio 3
              bit 5   ultimo_uno(8) no dio 3
              bit 6   ceros_delante(1) no dio 31
              bit 7   cuenta_unos(4095) no dio 12

   atomicas   bit 0   suma_y_devuelve no dio el valor ANTERIOR
              bit 1   ...y no dejo la suma
              bit 2   intercambia no dio lo que habia
              bit 3   ...y no dejo lo nuevo
              bit 4   compara_y_cambia no dio lo que habia
              bit 5   ...y no cambio cuando debia
              bit 6   cambio cuando NO debia
              bit 7   ...y encima toco la memoria
              bit 8   suma_atomica no sumo

   reglas     bit 0   desborde: 4e9 * 4e9 no atrapo (no devolvio 1001)
              bit 1   entre cero: dividir por cero no atrapo (no devolvio 1003)
              bit 2   conversion: entero32(1e30) no atrapo (no devolvio 1012)
```

★★ **La Regla 2 no esta en esa lista, y no es un olvido**: un `bufer` no lleva su
longitud, asi que no hay contra que comprobar el indice. Nace con `lista de T`.

★ Un cero en el Ryzen **confirma**; un numero distinto **senala al silicio y no a
la sonda**, porque la sonda ya dio cero en el emulador.

★★★ **Y las tres dieron cero en el Ryzen el 22-08.** Que las reglas atrapen en
los DOS sitios es el punto entero: si el emulador dijera cero y el metal otra
cosa, el sospechoso seria el emulador.

---

## Lo que esta sonda NO hace, y por que

De los 36 nombres de la tabla de maquina que el emulador no sabe ejecutar, **la
mayoria son de Ring 0** y un `.bex` corre en Ring 3. Llamarlos no daria un valor
raro: daria un `#GP` y se llevaria el programa por delante en la primera linea.

```text
   cli sti hlt wbinvd invd invlpg      paran o vacian la maquina entera
   lgdt lidt ltr lldt swapgs           tablas del sistema
   cr0 cr2 cr3 cr4                     LEER un registro de control tambien es
                                       Ring 0, no solo escribirlo
   rdmsr wrmsr xsetbv                  registros del modelo
   in* out*                            puertos de E/S
   monitor mwait                       esperar sin quemar el nucleo
```

**Que un driver los use es otra sonda y otro anillo.**

---

## `save`: como guarda, y por que no usa la biblioteca

El informe se escribe **por la puerta**, no con `usa archivo`.

⚠ Y no es una eleccion de estilo: `usa archivo` trae los nombres de REX y **hoy
sus llamadas no tienen destino** -- hace falta enlazado y no lo hay. Si la sonda
los usara, compilaria, correria, y **no guardaria nada**. Desde el 21-08 el
compilador al menos lo dice:

```text
   aviso: 1 cosa(s) no llegaron a un byte:
     - cierra: la llamada no tiene destino
```

Guardar no necesita biblioteca. Necesita cuatro operaciones de la puerta:

```text
   op_ruta            la ruta, de 8 en 8 bytes y POR VALOR
   op_archivo_crear   la consume y devuelve un HANDLE
   op_arch_escribir   sobre el handle: 7 bytes por llamada
   op_arch_cerrar     y AQUI es donde llega al disco
```

★★ Fijate en la tercera: va sobre **el handle del fichero**, no sobre la tarea.
Es la diferencia entre pedirle algo al sistema y pedirle algo a **una cosa que el
sistema te dio** -- el modelo de capabilities entero, visto desde un programa de
usuario de veinte lineas.

★ Y si el fichero no se puede crear, la sonda **sigue** y saca el informe solo por
pantalla. Parar ahi perderia las medidas por no poder guardarlas.

---

## Que hacer con lo que salga

1. Lee `bits` y `atomicas`. Si los dos son cero, **INTI corre en tu maquina**.
2. Trae `/inti/cpu.txt` de vuelta y pegalo aqui.
3. Si el programa murio antes de `-- fin -`, la ultima linea que salio dice
   donde: el sospechoso es la instruccion de la linea SIGUIENTE.
