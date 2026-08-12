# PRUEBA EN METAL -- el arranque del 2026-08-12

Guia para el Ryzen. **No es una lista de deseos: es lo que hay que traer de
vuelta** para que el siguiente paso se decida con datos y no con teorias.

Han entrado **nueve commits y ninguno ha visto un CPU**. Dos de ellos tocan lo
que se nota en el primer segundo: **el camino de la entrada** y **el del
pintado**. Por eso el orden de abajo es *si esto falla, para*.

> La guia de la tanda anterior queda en `PRUEBA_EN_METAL_0810.md`.

---

# PARTE 0 -- El comando, y lo que se puede saber sin arrancar

```powershell
Ultra_kernel_x86-64\build.ps1 -Flash -Drive A -Data A
```

Si el build **para**, mirar en este orden: el guardian de ASCII (hay comentarios
nuevos en cinco ficheros), el guardian de contrato de syscalls, y el enlazado.
Ninguno de los tres deberia saltar -- `cargo check` esta limpio en kernel,
userspace y toolchain.

Y **antes de ir al Ryzen**, esto se corre desde Windows:

```powershell
cargo test -p bmo-verify --test ram_del_disco -- --nocapture
```

Imprime la tabla de transporte de todos los `.bex` recien desplegados. Si
`doom.bex` no sale, el despliegue no lo copio y no hace falta reiniciar nada.

---

# PARTE 1 -- LO QUE SE MIRA PRIMERO, porque para todo lo demas

## 1.1 -- Arranca?

Lo de siempre: escritorio pintado. Si **no** arranca, el sospechoso numero uno es
el corte de `dev/usb.rs` en cuatro modulos, o el hilo del bus:

```
   git revert 61a8fa2f     el corte en modulos
   git revert 58888f46     el hilo del bus
```

## 1.2 -- El hilo del bus late

En **F11**, fila `usb`, campo NUEVO:

```text
   bus=turns:overlaps
```

**`turns` tiene que SUBIR, y sobre todo mientras un programa de Ring 3 tiene la
entrada.** Es el numero que dice que el teclado ya no depende de que alguien
pregunte.

Y en el arranque, una linea nueva:

```text
   usb: el bus tiene hilo propio, tid =3
```

Si sale `NO hubo ranura para el hilo del bus` o `sin aparatos`, **el hilo no
esta** y el sistema se comporta como antes -- o sea, con el fallo de
congelacion.

[!] Si mas tarde aparece `FALLO usb: el hilo del bus DEJO DE LATIR`, eso es el
vigilante nuevo y significa exactamente lo que dice.

## 1.3 -- El parpadeo

**Mover el raton por el escritorio.** No tiene que parpadear.

Y el numero, con la orden `perf`:

```text
   fotogramas  ...
   media       ...
   peor        ...
   cajas       <- FILA NUEVA
```

- `cajas 2` o `3` con un `peor` pequeno -> el troceado trabaja.
- `cajas 1` con un `peor` de ~8 MB -> degenero, y el sospechoso es
  `COSTE_DE_UNA_CAJA` en `sin_gpu/sucio.rs`, no el volcado.

Vuelta atras: `git revert 758ab20f`.

---

# PARTE 2 -- EL TECLADO, que es el fallo que se sufrio

## 2.1 -- Desenchufar

Desenchufa el teclado. En F11 tienen que salir **DOS** lineas, no una:

```text
   AVISO usb: puerto: algo se DESENCHUFO =N
   AVISO usb:   ...y ERA UN APARATO MIO: lo suelto =N
```

★ **Sin la segunda, el olvido no ocurrio** y lo de abajo va a fallar.

## 2.2 -- Volver a enchufar

**Tiene que escribir.** Y en F11:

```text
   INFO usb: puerto: ENCHUFADO y adoptado =N
```

Si en vez de eso sale `puerto: ENCHUFADO, nada que adoptar` seguido de
`...y creo tener teclado:raton =0b1_0000_0001`, el olvido fallo: el adoptador
cree que todavia tiene el teclado.

Vuelta atras: `git revert 11d97e99`.

## 2.3 -- El rescate desde la puerta cruda

**Lanzar DOOM y volver con `Ctrl+Alt+Esc`.** Antes, desde un programa que lee
teclas crudas, no volvia.

```text
   AVISO input: entrada RESCATADA por el teclado =PID
```

★ Esto es lo que de verdad cierra el commit del hilo del bus: **funciona aunque
el dueno de la entrada este colgado**.

---

# PARTE 3 -- LO QUE SE COBRA DE UNA VEZ

## 3.1 -- Las unidades de CABINA

Ya no hace falta convertir a mano. En F11:

```text
   red:  MAC                             =2C:F0:5D:D9:3C:E3
   red:  PHYstatus crudo                 =0b1011
   red:  enlace ARRIBA, megabits         =100        <- antes salia 64
   arch: archivo REFLEJADO para leer     =4.0 MiB (4196020)
   usb:  el bus tiene hilo propio, tid   =3
```

Si alguna sale en hexadecimal pelado, esa llamada no se migro -- no es un fallo,
es una que falta.

## 3.2 -- `smp`

```text
   smp stop      -> "obreros parados" + "[!] seguiran contando como en pie"
   smp           -> "12 de 12" + "[!] pero estan PARADOS"
   smp all       -> despierta
   smp test      -> TIENE QUE VOLVER A ACELERAR
```

★ Lo ultimo es lo que importa: antes de `11d97e99`, un `smp all` tras un `stop`
habria dado 12 en pie y **cero obreros**, sin decir por que.

## 3.3 -- La red RECIBE

```text
   net rx        -> "receptor ARMADO, anillo en la fisica =0x..."
   (esperar unos segundos)
   net rx        -> "red: trama de 2CF0..." con tipo 0806 (ARP) u 0800 (IPv4)
```

[!] **Cero en la primera vuelta es lo esperado**, no un fallo: el anillo se acaba
de armar y el broadcast llega cada pocos segundos.

Si NUNCA sube: `la NIC no termina su reset` (el BAR o el aparato) o `sin marco
para el anillo` (memoria). Si sale `trama demasiado corta`, llegan bytes y el
sospechoso es el descuento del FCS.

Vuelta atras: `git revert abd9cf1c`.

## 3.4 -- El audio dice como quiere las muestras

Con el audifono **enchufado antes de arrancar**:

```text
   audio
```

Y en F11, los numeros del paso 0:

```text
   audio: interfaz AudioStreaming, alt        =1
   audio: canales                             =2
   audio: bits por muestra                    =16
   audio: bytes por trama (wMaxPacketSize)    =192 B
   audio: frecuencia que acepta               =48000
   audio: frecuencia elegida                  =48000
   audio: y una trama suya ocupa              =192 B
   audio: el endpoint isocrono es el DCI      =2
```

★★ **Las dos ultimas deciden si el plan de audio es posible**: la trama tiene que
CABER en el paquete. Si sale `ninguna frecuencia suya cabe en su propio paquete`,
no hay codigo correcto que lo arregle.

Si no aparece nada: `puertos libres mirados, y ninguno reproduce =N`. Con el
audifono enchufado y `N > 0`, el aparato esta y **no es UAC1 como se creia** --
lo cual tambien es una respuesta, y cambia el plan.

---

# PARTE 4 -- DOOM

`run apps/doom.bex`. Lo ultimo que se supo es que **pasa de `M_LoadDefaults` y
muere despues**. Lo que hace falta es **donde**:

| Sintoma | Sospechoso |
|---|---|
| no sale nada | el reflejo de ficheros -- `git revert cf878698` |
| se para y no sale `W_Init` | el WAD otra vez; mirar `arch` en CABINA |
| arranca y muere sin pintar | el monton: 12 MiB CONTIGUOS. CABINA dice si el kernel los nego |
| pinta y no responde | mirar si `bus=turns` sigue subiendo |
| anda solo y no para | la cola cruda: se perdio un `soltar` |

---

# QUE TRAER DE VUELTA, en orden de utilidad

1. **`A:\datos\salida.txt`** -- se llena solo con lo que se lanza desde
   `Ejecutar`, y `guarda` vuelca el historial entero. **Vale mas que cualquier
   foto**: se puede leer, buscar y comparar.
2. **Foto de F11 (CABINA)**, con el filtro `A` para la ultima accion o sin
   filtro para el historial.
3. **Foto de la fila `usb`** completa: ahi van `bus=`, `apk=` y `kev=`.
4. **La salida de `perf`**, por la fila `cajas`.
5. **La salida de `audio`**, que es la unica que no tiene precedente.

Y si algo se cuelga antes de poder escribir: **la foto de lo ultimo que quedo en
pantalla sirve igual**. CABINA se pinta desde el bucle del shell, asi que lo que
se ve es lo ultimo que el sistema alcanzo a contar.

---

# LOS NUEVE COMMITS, y su vuelta atras

| commit | que toca | si algo falla |
|---|---|---|
| `58888f46` | **el camino de entrada de todo** | teclado mudo |
| `abd9cf1c` | la NIC (solo con `net rx`) | nada, es opt-in |
| `adfbcd20` | como se pintan los numeros de CABINA | lineas raras |
| `11d97e99` | teclado replug + smp | el teclado no vuelve |
| `61a8fa2f` | **corte de `usb.rs`** (cero logica) | no arranca |
| `758ab20f` | **el pintado del compositor** | parpadeo o basura |
| `34ddeb4a` | solo mover un fichero | nada |
| `8c1f5ab4` | la orden `audio` | nada, es opt-in |
| `af285731` | solo toolchain | nada en metal |

★ Los dos en negrita son los unicos que pueden dejar la maquina inservible. Los
demas o son opt-in o solo cambian texto.
