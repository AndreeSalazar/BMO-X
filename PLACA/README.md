# PLACA -- lo que BMO-X supone de la placa base, en un sitio

> Pedido por el dueno el **2026-09-07**:
>
> > *"se que uso la A320M, pero puedes buscar esa carpeta y aislar para cuando
> > cambie de placa base?"*
>
> Es la **LEY 24** --*el hardware se PERFILA*-- aplicada a la placa. Ya estaba
> hecho para el CPU (`cpu_vendor/profile.rs`, con su frase: *"swapping the CPU
> is a profile swap, never a kernel edit"*) y **no estaba hecho para la placa**.

---

## 1. ★★ POR QUE UNA CARPETA EN LA RAIZ, y no un modulo

Porque lo que se sabe de esta placa **no cabe en ninguna capa**. Al buscarlo
aparecio repartido en tres, y ninguna sabia de las otras:

```text
   build.ps1                     por que uefi_chain es UNIFICADO
   boot_context/src/lib.rs       por que existe `PreloadInfo`
   faggin/s1_cpu/src/uefi.rs     dos rodeos: el GOP y la copia del kernel
   kernel/.../core/klog.rs       por que hay una linea de tiempo de arranque
```

** Meterlo en el kernel no vale: `s1_cpu` corre **antes** de que el kernel
exista y no puede leer un modulo suyo. Meterlo en `boot_context` tampoco: ese
crate es el sobre del traspaso, no un sitio donde vivan opiniones.

★ Es el mismo motivo que `NEUTRO/`, por el otro lado:

```text
   NEUTRO   esta en la raiz porque es AGNOSTICO: no depende de ninguna capa
   PLACA    esta en la raiz porque las ATRAVIESA TODAS
```

---

## 2. ⚠ EL HALLAZGO, y no es el que se esperaba

**Ninguno de los cuatro rodeos se rompe al cambiar de placa.** Los cuatro son
DEFENSIVOS: rodean a un firmware que hace algo mal, y en un firmware que lo hace
bien el rodeo sobra pero funciona igual.

> Asi que el peligro no era que se rompieran. Era que **se siguen pagando y
> nadie sabe que se pueden quitar.**

```text
   lo que cuesta hoy el uefi_chain unificado    ~1,2 MB de imagen, siempre
   lo que costaria si el firmware fuera bueno   cargar del ESP, y ya
```

*** Por eso el perfil no dice *"esto se rompe"*: dice **"esto se paga por ESTA
placa"**. Con otra, la pregunta deja de ser *"que arreglo"* y pasa a ser *"que
puedo dejar de pagar"*, que es la pregunta util y la que hoy nadie podia hacerse.

---

## 3. Que hay aqui

| fichero | contesta |
|---|---|
| [`PERFIL.txt`](PERFIL.txt) | **las cuatro manas** de este firmware, con el fichero de cada rodeo. Lo lee un guardian |
| este | por que la carpeta, y el hallazgo |

Y en el kernel, su otra mitad:

```text
   plat/perfil_placa.rs   [carril] VERDE -- lo dice en CABINA al arrancar
```

** Las dos mitades las compara `toolchain/tools/perfil-placa` en cada build: si
un rodeo desaparece del codigo y sigue en el perfil --o al reves-- el build
falla. Es el `[riesgo] ESPEJO`, y ya se pago con las constantes del ABI y con el
censo del neutro.

---

## 4. ★ CUANDO CAMBIES DE PLACA, esto es lo que se hace

```text
   1. cambiar `fabricante` y `modelo` en PERFIL.txt y en plat/perfil_placa.rs
   2. arrancar. CABINA dice el perfil que cree tener
   3. ir mana por mana y PROBAR si todavia hace falta:
      quitar el rodeo, arrancar, y ver si sigue arrancando
   4. la que ya no haga falta: se borra del perfil Y del codigo, juntas
      -- el guardian no deja borrar solo una
```

⚠ **Y el paso 3 se hace de UNA EN UNA.** Quitar los cuatro rodeos a la vez y
que no arranque no dice cual era: dice que hay que empezar de cero. Es la misma
regla del metal -- una variable por prueba.
