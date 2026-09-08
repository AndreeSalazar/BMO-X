# PERFIL -- lo que BMO-X SUPONE de lo que tiene debajo

> Idea del dueno, **2026-09-07**:
>
> > *"el NEUTRO es para archivar GPU y perfilar, y la placa base, y lo mismo con
> > CPU y RAM... es decir, TODO el RING 0 espera que tipo de perfil es, para
> > facilitar. Crear carpeta para los 2, que es para PERFILAR"*
>
> La idea es correcta y esta carpeta es esa. Pero llega con **dos correcciones**
> y **una restriccion fisica**, y las tres cambian su forma.

---

## 0b. ★★ EL CRITERIO: que merece perfil y que NO

Lo pregunto el dueno el 07-09 -- *"falta el teclado y el mouse, aunque no se si
esos 2 importan"*-- y al contestarlo salio la regla que faltaba. **Sin ella,
acaba habiendo un perfil de cada cosa.**

```text
   se le PUEDE preguntar     ->  se le PREGUNTA. No hace falta perfil
   NO se le puede preguntar  ->  hay que SUPONERLO, y entonces se ESCRIBE
```

★ Y el teclado es el mejor ejemplo, porque cae **en los dos lados a la vez**:

```text
   quien es              te lo DICE. Clase, subclase, protocolo, y desde el
                         07-09 tambien su nombre (idVendor/idProduct)
                         -> NO se perfila. Lo apunta el portero, al llegar

   que letra es cada     NO te lo dice NADIE. Un teclado manda SCANCODES, y
   tecla                 que la de al lado del 1 sea `!` o `"` lo decide el
                         sistema  -> ESO si se perfila
```

*** Por eso [`ENTRADA.txt`](ENTRADA.txt) no tiene ni un campo de identidad y su
campo mas importante es `distribucion: espanol (ISO)`.

** Y la regla explica la carpeta entera hacia atras: la placa esta aqui porque su
firmware **no cuenta sus manas**; el CPU porque sus erratas **no se preguntan**;
la RAM porque su ancho de banda **solo se mide**; y el disco porque cual de las
letras es el Windows del dueno **no se puede averiguar desde dentro**.

> Un aparato que contesta no se perfila: se enumera.

---

## 1. ★★ LAS TRES FRONTERAS -- y por eso las tres estan en la raiz

Esto es lo que ordena todo lo demas, y no estaba escrito en ningun sitio:

```text
   VALKYRIE-ABI    hacia ARRIBA   lo que BMO-X PROMETE a quien se apoya en el
   PERFIL          hacia ABAJO    lo que BMO-X SUPONE de lo que tiene debajo
   NEUTRO          al LADO        lo que BMO-X NO CONTROLA
```

★ **Ninguna de las tres es parte del kernel. Las tres son FRONTERAS**, y por eso
las tres viven en la raiz y no dentro de `Ultra_kernel_x86-64/`.

El dueno pregunto si V-ABI aporta algo aqui. **Aporta esto**: es la cara opuesta.
Lo que hace que las dos sean utiles es que **no se mezclen** -- una promesa hacia
arriba y una suposicion hacia abajo se escriben distinto y caducan distinto.

---

## 2. ⚠ CORRECCION 1: NEUTRO no puede ser el archivo

El dueno propuso usar `NEUTRO/` como el sitio donde archivar los perfiles. **No,
y por una razon de esta casa:** NEUTRO ya tiene un trabajo -- decir *que hay
fuera de la ley*. Darle ademas el de guardar perfiles serian dos preguntas en un
sitio, que es lo que la ley llama **mal cortado** (A2, L6b).

```text
   NEUTRO   contesta   "quien escribe en mi RAM sin permiso"
   PERFIL   contesta   "que supongo de la maquina que tengo debajo"
```

Se parecen porque **las dos listan hardware**. Pero el censo del neutro seria
identico en otra maquina --su ley es agnostica-- y un perfil es lo contrario:
**es la cosa mas atada a ESTA maquina que hay en el repo.**

---

## 3. ⚠ CORRECCION 2: un perfil no es solo datos

Esta es la que mas cambia el plan. **Cada perfil tiene DOS MITADES**, y solo una
puede vivir aqui:

```text
   DECLARATIVA   nombres, numeros, manas, lo que se sabe y lo que NO
                 es DATO -> vive AQUI, y aqui es donde se mira

   OPERATIVA     el codigo que HABLA con el silicio
                 no puede salir -> vive junto al kernel
```

★ Y el numero que lo demuestra: `cpu_vendor/ryzen_5_5600x/` son **1.072 lineas
de codigo** -- `cpuid`, cache, errata, `power`, `presupuesto`, topologia, TSC.
Eso no es una tabla, es un driver.

** Sacarlo a un fichero de texto exigiria escribir un interprete que lo ejecute,
y entonces BMO-X tendria **un lenguaje mas** para mantener. Es exactamente lo que
`docs/identidad/` ya decide sobre el AML de ACPI: *tablas estaticas SI, un
interprete de bytecode de terceros en Ring 0 NUNCA*.

---

## 4. ⚠ LA RESTRICCION FISICA: Ring 0 no puede leer esto al arrancar

El dueno lo dijo como *"el RING 0 se cree archivo para llamar los exteriores"*.
**En ejecucion eso no se puede hacer**, y hay que decirlo antes de disenar nada
encima:

```text
   cuando el kernel arranca      no hay sistema de ficheros todavia
   el kernel es un binario PLANO no puede abrir un .txt
   y el perfil hace falta ANTES  de que exista con que leerlo
```

### ★ Pero SI se puede en el BUILD, y ese patron ya existe y esta probado

```text
   .maqueta  ->  Rust generado  ->  un guardian comprueba que dicen lo mismo
                 (sale en cada build: "caras: 1 .maqueta, y su Rust generado
                  dice lo mismo")
   sem-asm   ->  62 intrinsecos en una tabla TOML
```

** Asi que *"Ring 0 llama al exterior"* se convierte en algo mejor y comprobable:
**el dato manda, el codigo lo repite, y un guardian no deja que se separen.** Que
es justo lo que hacen ya `perfil-placa` y `censo-neutro`.

---

## 5. Que hay aqui, y que falta

| perfil | mitad declarativa | mitad operativa | guardian |
|---|---|---|---|
| [`PLACA/`](PLACA/README.md) | ★ aqui | `plat/perfil_placa.rs` | ★ `perfil-placa` |
| [`CPU.txt`](CPU.txt) | aqui | `cpu_vendor/ryzen_5_5600x/` (1.072 lineas) | -- |
| [`RAM.txt`](RAM.txt) | aqui | `core/shell/banda.rs` (la MIDE) | -- |
| [`GPU.txt`](GPU.txt) | aqui | `platform/drivers/gpu/rdna4/` | -- |
| [`ENTRADA.txt`](ENTRADA.txt) | aqui | `uhid/` + `dev/usb/` | -- |
| disco | falta | `dev/disk/` | -- |
| red | falta | `dev/net/` | -- |

⚠ **Solo la placa tiene guardian PROPIO.** Los otros cinco son declaraciones sin
nadie que las compare campo por campo, y eso hay que decirlo en vez de dejar que
la tabla parezca completa. El de la placa se escribio primero porque era el unico
con manas que se pueden quedar obsoletas en silencio.

★ Lo que SI cubre a los seis son **DOS guardianes**: el de la **exposicion**
(seccion 7) --que cada uno diga a quien rompe, y que ese alguien exista-- y el de
los **campos**, que compara lo que cada perfil afirma con lo que el codigo dice.

** Y el de campos hizo falta mirar, uno por uno, QUE SE PUEDE COMPARAR DE VERDAD.
La respuesta no fue "todo", y lo que NO se puede vale tanto como lo que si:

```text
   CPU     fabricante y microarquitectura SI. Los nucleos NO: el codigo los
           MIDE en el arranque, no los declara -- y eso esta BIEN
   GPU     `pci_vendor`, y que `pci_devices` siga VACIO
   RED     `pci_vendor` contra `VENDOR_REALTEK`. El device NO: no es constante
   DISCO   ** LOS TRES CIERRES de discos.ps1. Si alguien quita uno, el build para
   RAM     NADA. Su unico numero se MIDE, y no se ha medido
   ENTRADA `latido_ms` y `subclase_exigida` SI. **La distribucion NO**: es una
           TABLA de scancodes, no una constante -- y es justo el campo cuyo
           fallo no da error nunca
```

*** Y el orden de lectura no es el de la tabla: **[`DISCO.txt`](DISCO.txt) va
primero.** Los demas, si mienten, hacen que algo vaya lento o no arranque. Ese,
si miente, escribe en el disco equivocado -- y ahi vive el Windows del dueno.

---

## 6. ★ LA REGLA DEL FORMATO: cada perfil dice COMO SE NOTA que esta mal

Pedido por el dueno: *"quizas que pongan por que razones, para saber por que
falla"*. Es la ley **L6f** --el `[riesgo]` de un modulo-- aplicada a un dato:

> Un `[riesgo]` no dice **que cuesta** un fallo. Dice **por que esa pieza va a
> fallar**.

Asi que todo perfil de aqui lleva una seccion `SI ESTE PERFIL ESTA MAL, ASI SE
NOTA`, y cada fila es un sintoma real, no una advertencia:

```text
   el campo que puede estar mal
       que se ve desde fuera cuando lo esta
       y como se caza
```

*** El caso que lo justifica esta en [`RAM.txt`](RAM.txt): los modulos dicen
3200 MT/s y **una A320M puede no estar dandoselos**. Un tercio de diferencia en
el ancho de banda, invisible desde dentro de BMO-X, y el sintoma es *"el
asistente va lento y parece que el modelo esta mal elegido"*. Sin esa seccion,
ese dia se audita el modelo -- que es lo unico que no tiene la culpa.

---

## 7. Lo que esta carpeta NO promete

```text
   [ ] no hace que cambiar de hardware sea automatico
   [ ] no saca el codigo del kernel: la mitad operativa se queda donde esta
   [ ] no lo lee Ring 0 en ejecucion, y no podria
   [ ] y no detecta nada: un perfil se DECLARA. Preguntarle al hardware es
       otro trabajo, y cada pieza lo tiene por su lado
```

> Un perfil no hace que la maquina funcione. Hace que se sepa **que maquina se
> creia tener** el dia que deje de funcionar.
