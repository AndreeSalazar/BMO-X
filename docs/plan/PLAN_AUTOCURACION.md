# El plan de la AUTO-CURACION: de informar a actuar

> Escrito el **2026-08-08**, el dia que el kernel empezo a redactar la autopsia
> de cada tarea que mata.
>
> Pregunta del dueno: *"curarse ES en sentido que mi BMO-X atrapa el bug o
> fallos que se escapan de memoria y entonces BMO-X expone para ser reparado
> pero ser reparado AUTOMATICAMENTE (...) es mas alla de metakernel, no?"*
>
> No hace falta una palabra nueva. **"Metakernel" ya significa eso** -- el
> sistema hace trabajo sobre sus propios fallos-- y hoy ese trabajo llega hasta
> INFORMAR. Este documento es como llega hasta ACTUAR.

---

# Lo que ya se cura solo, hoy, y no es aspiracion

Se lista primero porque cambia lo que hay que construir: la mitad esta hecha.

| Lo que pasa | Como se cura | Donde |
|---|---|---|
| Una tarea revienta | se aisla, sus capabilities se revocan, **BMO sigue vivo** | `plat/faults.rs` |
| Un programa se queda la pantalla y la entrada | `Ctrl+Alt+ESC`, en el punto UNICO por el que pasan las teclas | `dev/usb.rs` |
| Una transaccion se corta a medias | copy-on-write: lo viejo sigue entero | ESTRATOS |
| Un binario esta corrupto | la firma se verifica **antes** de ejecutar | `task/lanzar.rs` |
| Un raton dice un formato que no se entiende | **se degrada al BOOT** y lo dice | `bmo-uhid` |
| El dueno de la pantalla muere | el kernel la recupera y pinta sus ultimas cuatro lineas | `fb::proceso_muerto` |

★ La fila del raton es la unica que ya es auto-curacion COMPLETA: detecta que no
entiende el aparato y **se cae a un modo que si funciona**, en vez de morir. Las
demas contienen el dano; esa lo repara.

---

# El limite, dicho sin adornos

**Ningun sistema repara el CODIGO automaticamente.** Reparar exige saber que se
queria hacer, y eso no esta escrito en ninguna parte de la maquina.

Lo que si se puede hacer --y es mucho-- es que **el fallo deje de importar**:
contenerlo, restaurar el servicio, y entregar el informe exacto a quien si puede
arreglarlo.

Lo que describe el dueno tiene nombre y es de lo poco que funciono de verdad:
los **arboles de supervision** de Erlang/OTP. *Let it crash* -- deja que muera, y
un supervisor lo relanza con una politica.

★ **Y aqui BMO-X tiene una ventaja sobre Erlang que no es marketing**: cuando una
tarea muere, **sus capabilities mueren con ella**. En Erlang el proceso
reiniciado puede heredar estado del anterior; aqui no hay nada que heredar. El
reinicio empieza limpio *por construccion*, no porque alguien se acordo de
limpiar.

---

# ESCALON 1 -- Que el kernel COMPRUEBE que lo recupero todo

**El primero porque usa lo que se acaba de construir, y porque destapa fugas que
ya se sabe que existen.**

La tarea murio y el kernel revoco sus capabilities. Pero eso es lo que el codigo
*dice* que hace. Nadie comprueba que:

- las capabilities del muerto son **cero**,
- la pantalla ya no es suya,
- la entrada ya no es suya,
- sus ranuras de directorio volvieron,
- su memoria volvio a la cuenta.

- [ ] **1.0** (S) ★ Contar lo que queda del muerto DESPUES de revocar y
  anadirlo a la autopsia -- en `plat/faults.rs`, donde ya se redacta
- [ ] **1.1** (S) Si algo no volvio, la linea sale en ROJO y dice **que** no
  volvio -- misma autopsia, con `cabina::fault`
- [ ] **1.2** (S) La misma comprobacion en `EXIT`: una salida limpia tambien
  puede dejar cosas -- `syscall/ops.rs`, el brazo de `TASK_OP_EXIT`
- [ ] **1.3** (S) Un contador `fugas` en `info`, al lado de `cerrojos`, y
  **tiene que ser CERO** -- un `INFO_*` nuevo en `core/report.rs`

⚠ **Y no es hipotetico.** `AVANCES.md` ya lleva abierta esa auditoria desde el
02-08: *"memory accounting indexed by a pid that only counts up, and directory
slots freed only when the process dies, with a client -- the desktop -- that
never dies"*. Son fugas que hoy **no las ve nadie**. Este escalon las convierte
en un renglon.

**Como se sabe que esta hecho**: se lanza un programa que muere con un
`KIND_DIRECTORIO` abierto, y la autopsia dice `recursos  1 directorio SIN
devolver`.

---

# ESCALON 2 -- CUARENTENA: lo que muere siempre, deja de lanzarse

**Va antes que el reinicio automatico, y el orden importa: un supervisor sin
cuarentena es una maquina que se cae en bucle mas rapido.**

- [ ] **2.0** (S) Recordar `(programa, rip)` de cada muerte -- una tabla
  estatica al lado de la autopsia, en `plat/faults.rs`
- [ ] **2.1** (S) ★ Tres muertes en el **mismo `rip`** = el binario esta
  roto, no la maquina -- la comparacion vive con esa tabla
- [ ] **2.2** (S) `run` lo rechaza **con el motivo y la autopsia delante** --
  en `task/lanzar.rs`, antes de verificar la firma
- [ ] **2.3** (S) `run --igual` para forzar: la cuarentena informa, no manda
  -- la bandera se lee en el shell de Ring 0 y en `commands/mod.rs`

★ El `rip` es lo que hace esto util y no una cuenta tonta: tres muertes en
sitios distintos son tres bugs; tres en el mismo sitio son **uno solo, y
determinista**, que es la clase que se puede arreglar.

⚠ La decision: la cuarentena **no sobrevive al reinicio**. Es a proposito --
persistirla obligaria a un fichero de estado que hay que invalidar cuando el
binario cambia, y un `.bex` recompilado es otro programa. Un arranque limpio
empieza sin prejuicios.

---

# ESCALON 3 -- DEGRADAR en vez de morir

Generalizar lo que el raton ya hace: **cada driver declara su modo de respaldo,
y el kernel lo usa cuando el bueno falla.**

- [ ] **3.0** (M) Un contrato: `intenta_bueno()` / `respaldo()` /
  `por_que_cai()` -- ★ una TABLA y no un rasgo con herencia, en
  `platform/shared/` como sus hermanos
- [ ] **3.1** (S) El raton, portado al contrato: ya lo cumple a mano en
  `bmo-uhid`, y es **la prueba de que el contrato sirve**
- [ ] **3.2** (S) El teclado: si el endpoint se para, resetear y recolocar el
  puntero -- ya existe desde el 06-08 en `dev/usb/rescate.rs`, sin contrato
- [ ] **3.3** (M) El disco: si NCQ falla, a una orden por vez --
  `dev/disk/transfer.rs`, que hoy solo usa la ranura 0
- [ ] **3.4** (S) Que CABINA diga **siempre** que se degrado y por que --
  `cabina::warn`. Un respaldo silencioso es peor que el fallo

★ **3.4 es la que decide si esto vale.** Un sistema que se degrada sin decirlo
parece sano y va mal, que es exactamente el fallo que este proyecto persigue.

---

# ESCALON 4 -- SUPERVISION: relanzar con politica

El ultimo, y con el freno ya puesto por el escalon 2.

- [ ] **4.0** (M) Quien supervisa a quien: una tabla, no un arbol de objetos
  -- al lado del planificador, en `task/`
- [ ] **4.1** (M) Politicas: `nunca` / `una vez` / `siempre con cuarentena`
  -- una columna de esa misma tabla
- [ ] **4.2** (M) ★ Reiniciar **con las capabilities de origen**, no con
  las del muerto -- por `task/lanzar.rs`, como un lanzamiento cualquiera
- [ ] **4.3** (S) Que el reinicio quede en la autopsia: `relanzado 1 de 3`
  -- `plat/faults.rs`

⚠ **4.2 es la que puede introducir un agujero**: un proceso reiniciado no puede
heredar handles del anterior. En BMO-X eso sale gratis --las capabilities mueren
con la tarea-- **siempre que el supervisor las vuelva a conceder desde cero y no
las guarde**. Si el supervisor guarda handles para "ahorrarse" la concesion,
acaba de inventar la herencia ambiental que todo el sistema evita.

---

# [!] POR QUE ESTE PLAN NO TENIA CASILLAS HASTA EL 2026-09-10

Las diecisiete estaban aqui desde el 08-08 **en tablas**, con su numero y su
tamano. Lo que les faltaba era la SINTAXIS: `docs/plan/ABIERTO.md` cuenta
`- [ ]`, y una fila de tabla no lo es. O sea que este plan salia con **cero
casillas abiertas** en el indice, y su trabajo no se veia por ninguna parte.

** Al convertirlas se les anadio lo que `casillas.py` exige y a la mitad le
faltaba: **donde mirar**. Una casilla que no nombra un fichero, un crate o una
orden no la puede comprobar nadie -- ni una maquina ni una persona.

  > Un plan que no se puede contar no esta pendiente. Esta olvidado.

---

# El orden, y por que este

```
  1  comprobar la recuperacion   usa lo del 08-08, destapa fugas ya conocidas
  2  cuarentena                  el freno. Va ANTES del acelerador
  3  degradar                    donde mas se gana, y donde mas se puede mentir
  4  supervision                 el acelerador, con el freno ya puesto
```

**El dia que la maquina se relance sola, se ponga en cuarentena sola y deje el
informe escrito, no hara falta una palabra nueva: sera la misma, terminada.**
