# PLAN: LA PILA HUERFANA

**El kernel corriendo sobre una pila que alguien ya devolvio.**

Abierto el 2026-08-31. El dueno sabe provocarlo con los dedos: matar el
servidor de Ring 3 y volver a entrar.

---

## 1. Lo que la maquina dice

Dos arranques distintos, el mismo vecindario:

```text
#PF  vec=0x0E  err=0x02      no-presente, ESCRIBIENDO, desde el KERNEL
rip=0x0000000000000000       <- CERO NO ES UNA DIRECCION: no se pudo leer
cr2=0x000000008FFFFFFF
rsp=0xFFFF800000B88C50       pila de HILO DEL KERNEL -- de NADIE VIVO
corria tid=05 (Ring 3)
sw0B c=800000B8DB00 b=800000B8DF60 n=800000B8DF60
gs b=0000009F6040 k=000000000000 pc=0000009F6040
iq: en rsp no hay marco de iretq (cs=0x0000)
```

El 30-08 el mismo fallo salio con `rsp=0xFFFF800000B87C50`: **una pagina de
diferencia**. No es azar, es un orden de reservas que se repite.

---

## 2. Lo que ya esta COMPROBADO

Cada linea se verifico leyendo el arbol, no razonando sobre el.

| hecho | como se comprobo |
|---|---|
| `fault_rsp` es la pila que el kernel pisaba de verdad | sale del marco de `iretq`, no del manejador. `plat/faults/roja.rs`, `fault_dispatch(.., fault_rsp)` |
| `de NADIE VIVO` es HONESTO, no un falso negativo | `spawn_user` guarda la pila de kernel de una tarea de Ring 3 en `stack_phys`, asi que `duenno_de_pila` la habria visto. `task/scheduler/roja.rs` |
| `cr2 = 0x8FFFFFFF` es BASURA, no un calculo | cae entre `USER_STACK_TOP` (0x8000_0000) y `CHANNEL_VA_BASE` (0xC000_0000): un hueco donde no se mapea nada jamas. `mm/vmm/verde.rs` |
| ...y es un valor de 32 bits | los 32 bits altos en cero. Una direccion calculada del kernel no tiene esa forma |
| `gs k=0` NO es un sintoma | Ring 0 corre con `KERNEL_GS_BASE = 0` por diseno. `task/percpu.rs`, cabecera |
| la pila de SYSCALL estatica no cuelga nunca | `SYSCALL_STACKS` es un `static` de 32 KiB x 16 en `.bss`. `task/percpu.rs` |
| la pila del `#PF` no es la de IST1 | IST1 lo monta el TSS del arranque, no el asignador; `rsp` es del physmap |

---

## 3. Lo que se DESCARTO, y por que

- **`exit_and_park` dejando un hilo sobre su pila.** Era mi hipotesis del
  30-08. Por inspeccion NO se sostiene: `reap` corre al final de
  `schedule_locked`, que sigue en la pila SALIENTE, y la guarda del `rsp` la
  protege. `task/scheduler/roja.rs`
- **El `if` de las rampas de aterrizaje.** `schedule_locked` solo refresca
  `TSS.RSP0` `if next_task.kernel_stack_top != 0`, y eso es falso unicamente
  para la tarea 0 (el shell de Ring 0). Deja `RSP0` colgando mientras el shell
  corre -- pero durante ese rato no hay codigo de Ring 3 ejecutandose, asi que
  nadie entra por ahi. **Es una mina armada, no la explosion de hoy.**

> Las tres rutas de muerte quedan bien por inspeccion. O sea que la lectura
> dice que esto no puede pasar y la maquina dice que pasa.
>
> **Cuando la lectura y el metal se contradicen, el que se equivoca es la
> lectura.** Lo que hace falta no es otra teoria: es un NOMBRE.

---

## 4. EL HUECO, dicho exacto

`reap` decide liberar una pila mirando **una sola cosa**: el estado de la
tarea, mas una guarda que protege la pila que estamos pisando.

Y hay **cuatro punteros publicados** que pueden apuntar dentro de ese rango:

```text
   TSS.RSP0                  donde aterriza un trap de Ring 3
   percpu.syscall_stack_top  donde aterriza un SYSCALL
   percpu.trap_rsp           el contexto vigente en este CPU
   otra tarea .context_rsp   un contexto guardado ajeno
```

**Ninguno se comprueba.** Que hoy sea seguro depende de un invariante que
nadie escribio y nadie vigila:

> *"Antes de que Ring 3 vuelva a entrar siempre hay un cambio de contexto que
> refresca RSP0."*

Un invariante que no esta escrito no es un invariante: **es una suerte que dura
hasta que deja de durar.** Y esta es la clase de cosa que el dueno pidio no
volver a parchear despues.

---

## 5. EL PLAN

Los pasos van en este orden a proposito: **primero se mide, y solo despues se
toca el planificador.** Tocar la ruta que corre 250 veces por segundo con una
hipotesis sin confirmar es como se mete el fallo que este trabajo viene a
evitar.

- [x] **0. MEDIR, sin cambiar nada.** `reap` comprueba los cuatro punteros
      contra el rango que va a liberar y grita en CABINA si alguno cae dentro.
      No arregla: nombra. Hecho el 2026-08-31 en
      `task/scheduler/roja.rs`, con `proc::tss_rsp0()` y
      `percpu::syscall_stack_top()` nuevos para poder leer lo que hoy solo se
      escribe.
- [x] **0b. LA MORGUE.** Las ocho ultimas pilas liberadas, con tid y tick, y la
      pantalla azul las consulta: `de NADIE VIVO` pasa a decir **de quien fue**.
      Hecho el 2026-08-31 en `task/scheduler/roja.rs` y
      `plat/faults/amarilla.rs`.
- [x] **0c. QUE LA AZUL PUEDA CONTESTAR.** Hecho el 2026-09-02, y no estaba
      en este plan porque nadie lo habia mirado. El paso 1 se intento y **volvio
      con media frase**: la foto del dueno acaba en `marco OCUPADO,` y ahi se
      corta. Dos cosas, las dos silenciosas:

      * **El renglon media 80 bytes** y descartaba el resto sin decirlo. La
        linea del veredicto mide 102, o sea que `(morgue: NN liberadas)` --el
        numero del que cuelga la seccion 6 de este documento-- se calculaba en
        cada azul y **no llegaba nunca al cristal**. No era el borde de la
        pantalla: `CHAR_W` son 10 pixeles y el informe empieza en `w/12`, asi
        que a 1920 caben 176 caracteres. Ahora el renglon mide 112 y, si aun
        asi se pasa, lo dice con `>>>`. Ver `plat/faults/verde.rs`.
      * **Y "mirar CABINA" no se puede hacer.** Los cuatro instrumentos del
        paso 0 gritan a un anillo en RAM, y un fallo de Ring 0 pinta la azul
        encima y reinicia a los veinte segundos: *el grito no sobrevive al
        suceso que lo provoca*. Asi que el motivo viaja ahora **en la ficha de
        la morgue**, que la azul si consulta, y sale por su nombre:
        `-- Y APUNTABA DENTRO: TSS.RSP0`.

- [ ] **1. ARRANCAR Y LEER.** Reproducir --matar Ring 3, volver a entrar-- y
      mirar **la azul**, que ahora lo dice todo ella sola. Tres resultados, y
      los tres cierran algo:

      1. La morgue reconoce el `rsp` **y trae motivo**: caso cerrado con
         nombre, y el nombre es cual de los cuatro punteros. Se va al paso 2.
      2. La morgue lo reconoce **y el motivo es 0**: `reap` la libero y ninguno
         de los cuatro apuntaba dentro. Se mira la ruta del tid que sale.
      3. La morgue **no lo reconoce** y `(morgue: NN)` es menor o igual que 32:
         entonces las fichas estan todas y la respuesta es firme --esa pila no
         la libero `reap`-- y este plan pierde. Ver la seccion 6.

      [!] Si `NN` pasa de 32 la tercera lectura vuelve a ser ambigua, y por eso
      el numero se imprime: es la diferencia entre "no fue `reap`" y "puede que
      la ficha se fuera por el anillo".
- [ ] **2. EL JUEZ, en su crate.** `platform/shared/bmo-pila-juicio`: *"se
      puede liberar este rango?"*, dados los punteros publicados. Puro, sin
      dependencias, sin `unsafe`, y con la direccion EXACTA de esta pantalla
      azul como caso de prueba -- el mismo patron que `bmo-fisica-juicio`.
- [ ] **3. `reap` PREGUNTA AL JUEZ** en vez de mirar solo su `rsp`. El cambio
      es de dos lineas porque el paso 2 se llevo la decision fuera.
- [ ] **4. CERRAR LA MINA DEL `if`.** `schedule_locked` publica SIEMPRE una
      rampa: para una tarea sin pila propia, la estatica de `percpu`, nunca la
      del anterior. Hoy es inofensivo y manana no lo sera. Ver
      `task/scheduler/roja.rs`, el bloque de las rampas de aterrizaje.
- [ ] **5. L6g SOBRE EL FICHERO CRITICO.** Este analisis destapo un tercer
      concepto sin casa: **las rampas de aterrizaje** viven repartidas entre
      `task/scheduler/roja.rs`, `task/proc.rs` y `task/percpu.rs`, y ninguno de
      los tres se llama asi. Un concepto critico sin sitio es la aguja del
      pajar que L6g existe para quitar.

---

## 6. Que TUMBARIA este plan

Si en el paso 1 no grita ningun puntero **y** la morgue no reconoce el `rsp`,
entonces esa pila no la libero `reap` -- y todo lo de arriba es la casa
equivocada. En ese caso el siguiente sitio es quien mas llama a `free_frame`
sobre memoria que fue pila: `mm::vmm::destroy_address_space` y
`obj::memory::process_died`.

Se escribe aqui para que el plan pueda perder, que es la unica forma de que
ganar signifique algo.
