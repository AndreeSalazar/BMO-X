# `inti/` -- lo que se lleva al Ryzen

```text
   run inti/cpu.bex
```

Copia esta carpeta al volumen de datos de BMO-X. Dentro va **`cpu.bex`**, y el
programa deja su informe en **`/inti/cpu.txt`** ademas de sacarlo por pantalla.

## Volver a generarlo

```bash
cargo build -p bmo-inti-x86-64 --bin inti
./target/debug/inti toolchain/lang/inti/sondas/cpu.inti -o inti/cpu.bex --informe
```

El `.bex` **no se guarda en git**: es un artefacto. Lo que se guarda es el
fuente, en `toolchain/lang/inti/sondas/cpu.inti`.

---

## LA PREDICCION, escrita ANTES de correrlo

★★★ **Esto es lo que espero que salga.** Va escrito por delante a proposito: una
prediccion que se escribe despues de ver el resultado no vale nada, y este
proyecto ya tiene el metodo -- *medir en vez de opinar*, y **decir antes lo que
se espera medir**.

| linea | prediccion | si sale otra cosa |
|---|---|---|
| `-- cpu -` (1) | **entre 0x0D y 0x10**. Un Ryzen moderno entiende hasta la hoja 13-16 | **0x00000000** = `cpuid` no llego a ejecutarse |
| `-- cpu -` (2) | **0x00A20F1x** o parecido: familia 0x19 (Zen 3/4) codificada | 0 = lo mismo de arriba |
| `tsc` | **entre 0x400 y 0x2000** (1.000-8.000 ciclos para mil vueltas de bucle) | **0** = el contador no avanza, y toda medida futura vale cero |
| `xcr0` | **0x00000007** (x87 + SSE + AVX) o **0x00000207** con AVX-512 | **0** = el estado extendido no esta encendido, y eso explicaria el `#GP` de `xrstor` |
| `azar` | **0x00000001** -- dos tiradas distintas | **0** = `rdrand` devuelve siempre lo mismo, o no ejecuto |
| `bits` | ★ **0x00000000** | **cualquier otro numero**: cada bit dice que cuenta fallo. Ver la tabla de abajo |
| `atomicas` | ★ **0x00000000** | idem, y ahi el sospechoso es la memoria, no el CPU |
| `-- fin -` | **tiene que salir** | si no sale, el programa murio antes: mira cual fue la ultima linea |

### ⚠ Las dos unicas que se pueden SUSPENDER

Las demas dicen lo que el CPU diga y no hay contra que compararlas. Estas dos
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
```

★ Un cero en el Ryzen **confirma**; un numero distinto **senala al silicio y no a
la sonda**, porque la sonda ya dio cero en el emulador.

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
