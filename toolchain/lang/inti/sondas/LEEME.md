# `sondas/` -- lo que solo el metal puede contestar

## `cpu.inti` -- que le cuenta este procesador a un programa de usuario

Es la sonda del **peldano 3** de [`PLAN_DE_PRUEBAS.md`](../PLAN_DE_PRUEBAS.md):
la primera vez que una linea de INTI corre en un procesador de verdad.

**El `.bex` lo produce `build.ps1`**, en `staging\BMO-DATA\inti\cpu.bex` -- el
espejo de lo que va al volumen de datos. Sale del mismo paso que los ejemplos de
C, COBOL y Ada, y por el mismo helper:

```powershell
Compilar-Ejemplos @(@{ src = '...\cpu.inti'; out = 'cpu.bex'; dir = 'inti' }) `
    'bmo-inti-x86-64' 'inti' 'ok:|error|aviso' $dataBase $repo
```

En la maquina: `run inti/cpu.bex`. A mano, para probar:

```bash
cargo run -p bmo-inti-x86-64 -- toolchain/lang/inti/sondas/cpu.inti -o cpu.bex --informe
```

OJO: la primera version se dejo en una carpeta `inti/` del repo y **nunca llego
al FAT32**. Es exactamente lo que el bloque de ejemplos del build ya tenia
escrito de cuando le paso a C -- *un lenguaje que compila y cuyo binario no se
despliega esta a medias*.

La prediccion de lo que tiene que salir esta en
[`PREDICCION.md`](PREDICCION.md), escrita antes de correrla.

### Como se lee el informe

Cada resultado son tres escrituras de consola de ocho bytes: el nombre, y los
dieciseis digitos hexadecimales del valor.

```text
-- cpu -     el numero de hoja mas alto que este CPU entiende
             la firma: familia, modelo y revision
tsc          cuanto avanzo el contador de ciclos en mil vueltas
xcr0         que partes del estado extendido estan encendidas
bits         **CERO es aprobado**: cada bit dice que cuenta no cuadro
atomicas     **CERO es aprobado**: nueve comprobaciones sobre memoria de verdad
-- fin -     llego hasta el final
```

★ **Las dos lineas que dicen CERO son las unicas que se pueden aprobar o
suspender sin mirar un manual.** Las demas dicen lo que el CPU diga, y no hay
contra que compararlas. Estas dicen si el CPU y el compilador estan de acuerdo.

Y las dos **ya dan cero en el emulador**, comprobado por
`las_cuentas_de_bits_de_la_sonda_dan_cero_en_el_emulador`. Asi que un cero en el
Ryzen confirma, y un numero distinto senala al silicio y no a la sonda.

### ⚠ Lo que esta sonda NO hace, y por que

De los 36 nombres que el emulador no sabe ejecutar, **la mayoria son de Ring 0**
y un `.bex` corre en Ring 3. Llamarlos aqui no daria un valor raro: daria un
`#GP` y se llevaria el programa por delante en la primera linea.

```text
   cli sti hlt wbinvd invd invlpg      paran o vacian la maquina entera
   lgdt lidt ltr lldt swapgs           tablas del sistema
   cr0 cr2 cr3 cr4                     LEER un registro de control tambien es
                                       Ring 0, no solo escribirlo
   rdmsr wrmsr xsetbv                  registros del modelo
   in* out*                            puertos de E/S
   monitor mwait                       esperar sin quemar el nucleo
```

**Que un driver los use es otra sonda y otro anillo.** Decirlo aqui vale mas que
una tabla con treinta y seis huecos que nadie sabe explicar.

### El instrumento se calibra antes de medir con el

`cpu.inti` va a sacar numeros por la consola. Si el formateador estuviera mal,
esos numeros serian basura y la culpa pareceria del CPU.

Por eso hay cuatro pruebas en el banco del emisor que leen **este mismo
fichero**, le quitan su `principal` y le ponen otro: comprueban que un numero
conocido sale con sus dieciseis digitos y en orden, que el cero no se encoge, que
las etiquetas son texto legible, y que la sonda entera compila y pasa el gate.

★ Se calibra con el fichero de verdad y no con una copia de sus funciones: una
copia se separaria del original en la primera correccion, y entonces la prueba
aprobaria un formateador que ya no es el que va al metal.

### Lo que la calibracion encontro

**`desplaza` no se emitia.** `x desplaza izquierda 8` devolvia `x` intacto:
compilaba, corria, y daba otro numero. Llevaba asi desde F2d, y no lo vio nadie
porque **ningun programa de INTI habia necesitado desplazar de verdad hasta que
hubo que imprimir un hexadecimal**.

Y no bastaba con emitir la instruccion: el silicio se queda con los seis bits
bajos del contador, asi que desplazar 64 posiciones desplaza cero y devuelve el
numero entero. La **Regla 7** dice que da cero, y eso hay que emitirlo aparte.
