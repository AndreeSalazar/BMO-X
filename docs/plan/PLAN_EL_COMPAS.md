# PLAN EL COMPAS -- el quantum se retira, y el turno se CONCEDE

> Escrito el **2026-09-08**, la misma noche en que un quantum regalado a una
> tarea dormida dejo el escritorio en un fotograma cada diez segundos.
>
> El dueno lo dijo asi: *"vamos a reemplazar el quantum con otro mejor,
> inspirado en OS, RTOS y otros mas, para el Orquestador que es BMO-X"*.
>
> Y el nombre lo puso el: `CUPO` no, *"me recuerda cosas turbias en Peru"*. Al
> buscarle otro salio que el mecanismo tiene **dos mitades**, y la metafora de la
> casa ya tenia las dos palabras esperando.

---

# 0. POR QUE EL QUANTUM CONTRADICE LA DOCTRINA

Un quantum contesta *"cuanto tardo en interrumpirte"*. Esa es la respuesta del
**multiplexor**: reparte, y reparte a ciegas. La ley de esta casa dice lo
contrario:

> *multiplexar es ser generoso, orquestar es ser **CELOSO*** --
> [`EL ORQUESTAL`](../../META-KERNEL_HARD.md)

★★ **Y un quantum no sabe decir que NO.** Solo divide. Por **L4** --una regla se
prueba diciendo que no-- un quantum no es una politica: es una aritmetica.

---

# 1. LAS DOS MITADES, Y SUS NOMBRES

```text
   EL COMPAS   lo que una tarea DECLARA      cuanto y cada cuanto  (C, T)
   EL AFORO    lo que el kernel COMPRUEBA    si cabe entra; si no, NO
```

**El compas** es literalmente eso en musica: cuantos tiempos hay y cuanto dura
cada uno. Para EL ORQUESTAL es la palabra nativa -- una orquesta no se reparte el
tiempo, **lleva un compas**.

**El aforo** es la mitad que sabe negar. Un local con aforo no negocia cuando
esta lleno: **rechaza**.

## Y la metafora aguanta hasta el final, que es la prueba de que es la buena

```text
   declarar el compas     "necesito 200 us cada 4 ms"
   el aforo lo admite     suma U = Sum(C/T). Si pasa de la cota -> NO ENTRA
   salirse del compas     gastar mas de tu C -> quedas DESACOMPASADO y
                          esperas al compas siguiente          (eso es CBS)
   marcar el compas       el foco alarga el TUYO, no te sube por encima de
                          nadie -- que es lo que ya dice `QUANTUM_DELANTE`
   best-effort            los que no llevan compas tocan en los silencios
```

---

# 2. ★★★ EL FANTASMA DEL 08-09, DICHO EN ESTE IDIOMA

El hilo del bus USB pedia **el compas entero**: cuatro milisegundos cada cuatro
milisegundos, o sea `U = 100 %`. Nadie se lo pregunto, asi que entro.

```text
   lo que paso            entro en silencio, y el compositor --prioridad 0--
                          se quedo sin sitio. Meses despues, un fotograma
                          cada diez segundos y tres horas de caza
   lo que habria pasado   el aforo suma y contesta: "pides toda la sala"
                          -> RECHAZADO EN EL ARRANQUE
```

> El quantum reparte la sala. **El aforo dice cuanta gente cabe.**

Ver `BITACORA.md`, Ep. 51, y `bmo-planificador-suelo` en la memoria.

---

# 3. LA SOPA: DE DONDE SALE CADA PIEZA

| de donde | que se le toma | que NO |
|---|---|---|
| round-robin + quantum | la rueda que no deja a nadie fuera | la ceguera |
| prioridad fija (RTOS) | el orden cuando de verdad hay urgencia | que EXCLUYA |
| Rate Monotonic | la idea de que el periodo manda | tener que ordenarlo a mano |
| **EDF** | ordenar por PLAZO, optimo en un nucleo | exige plazos: ver [`PLAN_EL_PLAZO`](PLAN_EL_PLAZO.md) |
| **CBS / sporadic server** | ★ **estrangular al que se pasa** | -- |
| **SCHED_DEADLINE** | ★★ **la ADMISION: el kernel acepta o rechaza** | su complejidad entera |
| CFS (Linux) | -- | ★ es la obra maestra del MULTIPLEXOR, y por eso no |

** De todos, **el unico que sabe decir que no es la admision**. Por eso es el
corazon de este plan y no una de sus mejoras.

---

# 4. LA ESCALERA, Y EL ORDEN IMPORTA MAS QUE LAS PIEZAS

- [ ] **E0 -- LA TAREA IDLE.** Prioridad minima, siempre lista, cuerpo
      `loop { hlt }`. Hoy no existe: `choose_next` devuelve `self.current` cuando
      nadie mas esta listo, y `schedule_locked` vuelve sin cambiar, **asi que una
      tarea que se bloqueo a si misma sigue corriendo**. Es lo que hace que
      `WAIT` "vuelva sin dormir" con la maquina ociosa -- medido el 08-09:
      `latido 79026/s`. Diez lineas en `scheduler/roja.rs`.
      ⚠ **Sacrificio**: hoy ese fallo es lo que mantiene vivo al compositor.
      Ponerla hace que bloquearse bloquee de verdad, y eso cambia mucho: exige
      el instrumento delante y un arranque para el solo.

- [ ] **E1 -- EL TIEMPO DE CPU POR TAREA.** Un contador en el cambio de contexto
      y un campo de `OP_INFO`. **No se puede presupuestar lo que no se mide**:
      hoy `Tick::cuerpo_ms` mide RELOJ DE PARED y no distingue *"trabaje"* de
      *"espere de pie"* --su propia cabecera en `desktop/tick.rs` lo avisa, y el
      08-09 me mando al sitio equivocado con `cuerpo 1066` de los que solo 2,36
      us eran trabajo. Es el mismo escalon que `P2.3` de `PLAN_EL_PLAZO`.

- [ ] **E2 -- (C,T) DECLARADOS Y EL AFORO.** Cada tarea trae su compas; el kernel
      suma `U` y **rechaza** al que no quepa. Aqui el planificador aprende a
      decir que NO, que es lo que lo convierte en politica.
      ⚠ **Sacrificio**: el formato `.bex` gana un campo (ver
      [`META-APP_HARD.md`](../../META-APP_HARD.md)), y el que no lo declare
      necesita un valor por defecto **que no mienta**. Un defecto generoso
      convierte el aforo en un adorno.

- [ ] **E3 -- ESTRANGULAR AL DESACOMPASADO (CBS).** Quien gasta mas de su `C` en
      una ronda espera a la siguiente. La garantia deja de ser confianza y pasa a
      ser mecanismo -- que es la doctrina de `NEUTRO/` aplicada al tiempo: **no
      se confia, se ACOTA**.

- [ ] **E4 -- TICKLESS / TSC-DEADLINE.** ★ Y es quien contesta *"se puede bajar
      de 1 ms, a 0,1?"*: **hoy no, y no por lentitud.** Con el LAPIC en modo
      periodico a 1 kHz, **un milisegundo es la unidad mas pequena que el
      sistema sabe NOMBRAR** -- no hay forma de pedir un plazo de 100 us porque
      no hay reloj que lo exprese. Con un disparo programado al instante exacto
      (TSC-deadline) la unidad pasa a ser el ciclo, y 0,1 ms deja de ser un
      numero raro.

      ### ★★ Y la pregunta del dueno: *"no se puede dividir? 0,5 + 0,5"*

      **Se puede, y son DOS registros que ya estan escritos** (`s2_mem/main.rs`):

      ```text
         0x3E0 = 3            el DIVISOR: hoy divide el reloj del bus entre 16
         0x380 = hz / 1000    la CUENTA: cuantos de esos hasta disparar
         0x320 = 48|(1<<17)   el modo: PERIODICO, o sea "y vuelta a empezar"
      ```

      Poner `hz / 2000` da 2 kHz. Es **una escritura**, y funciona.

      ⚠ Pero eso no exprime: **multiplica el coste**. Cada disparo cuesta lo
      mismo --entrada, `xsave`, manejador, `xrstor`, salida-- asi que el doble de
      disparos es el doble de gasto para el mismo trabajo. Partir un tick en dos
      mitades da dos ticks, no medio.

      ★★★ **Y el truco que buscabas existe: es esa idea DADA LA VUELTA.** En vez
      de partir mas fino, se quita el bit 17 --modo de UN SOLO DISPARO-- y se le
      programa el instante exacto del proximo evento que importa:

      ```text
         hoy        1.000 disparos por segundo, y nadie puede decir por que el 743
         un disparo  N disparos, y CADA UNO TIENE DUENO Y MOTIVO
      ```

      *** Eso es literalmente *"que cada uno diga que aporta"*. La diferencia es
      que no se consigue dividiendo mas: se consigue **no disparando cuando no
      hace falta**. Y de regalo, cuando de verdad hagan falta 100 us se piden --
      sin pagar diez mil por segundo el resto del tiempo.

      [!] Pero eso es **precision de despertar**, no latencia de punta a punta:
      el bus USB pone hasta 4 ms y el escaner de video hasta 16,7. Ver
      [`PLAN_EL_PIXEL`](PLAN_EL_PIXEL.md), seccion 1.

      Hoy el LAPIC va en modo PERIODICO a
      1 kHz (`s2_mem/main.rs`, `0x320 = 48 | (1<<17)`), asi que **cada tarea se
      come mil interrupciones por segundo**, cada una con `xsave`/`xrstor` del
      estado AVX. Con un disparo programado al siguiente instante que importa, si
      DOOM esta solo **el temporizador no dispara**. Eso es lo que hace cierta de
      verdad la frase de la casa: *"un juego de un solo hilo tiene el nucleo
      entero por construccion"*.

- [ ] **E5 -- ORDENAR POR PLAZO (EDF).** Lo ultimo, y solo cuando existan plazos
      de verdad: `PLAN_EL_PLAZO`, bloque P3.

- [ ] **E6 -- ★★★ LAS ANTEOJERAS.** Que una tarea pueda declarar *"mientras
      corro, que no me toque nadie"*. Lo pidio el dueno el 09-09 y lo explico
      mejor de lo que lo dice la literatura:

      > *"no es mas velocidad: es que WAIT ponga trabas a otros puntos que no le
      > interrumpan. Es concentrar al caballo con todo para ganar la carrera --
      > cuando un caballo esta concentrado tiene ventaja."*

      ** Y tiene nombre en la industria: **interrupt shielding** y **core
      isolation**. Es lo que hace un sistema de trading o de audio antes que
      cualquier optimizacion: **no acelerar el trabajo, sino quitarle de encima
      todo lo que lo interrumpe.**

      ### Por que hoy el caballo va distraido, con la cuenta

      ```text
         el LAPIC va en modo PERIODICO a 1 kHz  (s2_mem/main.rs, 0x320)
         -> MIL interrupciones por segundo, POR NUCLEO, pase lo que pase
         -> cada una con su `xsave`/`xrstor` del estado AVX
         -> y DOOM solo en un nucleo las paga TODAS sin que nadie las use
      ```

      ★ El coste en ciclos es pequeno --del orden del 0,05 %-- y **ese no es el
      dano**. El dano son las otras dos cosas: cada interrupcion **ensucia la
      cache y el TLB** del que estaba trabajando, y **mete un punto de
      expropiacion** cada milisegundo. Para el tiempo real eso significa que el
      peor caso de cualquier cosa incluye siempre una interrupcion.

      ### Y por que va por WAIT, y no por un syscall nuevo

      Los dos syscalls estan CONGELADOS, y eso no se toca. Pero `WAIT` ya lleva
      dentro la frase entera: *"despiertame cuando X, y no antes de Y"*. Lo que
      falta es la otra mitad del contrato -- **lo que la tarea promete a cambio**:

      ```text
         hoy    WAIT(que, testigo, plazo)          "despiertame cuando"
         E6     + el compas declarado (C, T)       "y necesito C sin que me
                                                    toquen, cada T"
      ```

      *** Con eso `WAIT` deja de ser solo una puerta de salida y pasa a ser
      **donde se declara el trato**: el que espera bien es el que puede pedir
      que le dejen en paz cuando le toque. Es la pieza que une E2 (el aforo) con
      lo que el dueno describio.

      ⚠ **Sacrificio, y es grande**: una ventana en la que no se interrumpe a
      alguien es una ventana en la que **nadie mas entra**. Si esa tarea se
      cuelga dentro de su ventana, la maquina se queda -- que es exactamente la
      azul que E0 vino a arreglar. Por eso las anteojeras **necesitan E3**
      (estrangular al que se pasa) puesto ANTES: la ventana tiene que acabar por
      reloj, no por confianza.

- [ ] **E7 -- ★ LAS ANTEOJERAS SOLAS, sin que nadie las pida.** La otra mitad
      de la idea del dueno, y la trajo asi:

      > *"eso lo veia MAS para cuando BMO-X, si pasa 15 minutos, se automatiza
      > para concentrar TODO en un objetivo. Es como un plus."*

      E6 es **declarada**: la tarea pide su ventana. E7 es **ganada**: una tarea
      que lleva mucho rato con el foco y sin nadie compitiendo **se gana las
      anteojeras sin declarar nada**. Nadie tiene que cambiar su programa.

      ```text
         el foco lo tiene UNO
         nadie mas esta listo desde hace N minutos
         -> el temporizador deja de disparar por costumbre
         -> los avisos que no son suyos esperan al final de su tramo
      ```

      ★★ Y encaja con la ley de la casa sin anadir nada: `EL ORQUESTAL` ya dice
      que **el foco decide CUANTO y no QUIEN**. Esto es esa frase llevada hasta
      el final -- un foco sostenido no sube de prioridad, **le quitan las
      distracciones**.

      ⚠ **Sacrificio, y aqui es de gusto y no de ingenieria**: un sistema que
      cambia de comportamiento a los quince minutos **se comporta distinto de
      como lo probaste**. Es la clase de cosa que hace que un fallo salga solo en
      sesiones largas -- justo el modo de fallo mas caro que tiene esta casa. Si
      se hace, la barra tiene que DECIRLO: un modo invisible es el `[riesgo]
      SILENCIO` con otro nombre, y ya costo el Bloq Num.

## ★★ Por que E1 va antes que E2, y no es negociable

Un presupuesto sobre una medida de reloj de pared es un presupuesto sobre humo.
El 08-09 esa confusion costo una caza entera: `cuerpo 1066 ms` parecia *"el
compositor trabaja mucho"* y era *"al compositor lo echaron del CPU"*.

> **No se puede presupuestar lo que no se mide.**

---

# 5. LO QUE ESTE PLAN NO PROMETE

```text
   [ ] no hace tiempo real DURO: sin plazos medidos hay presupuesto, no
       garantia. La palabra honesta sigue siendo SOFT REAL TIME
   [ ] no quita la prioridad: E2 la deja donde esta y le pone una cota
       encima. Quitarla es otro plan, y probablemente innecesario
   [ ] no acelera nada por si mismo. Lo que da es que **el reparto se pueda
       AUDITAR**: hoy la unica forma de saber quien se comia el CPU fue una
       caza de tres horas
   [ ] y E2..E5 es lo mas complejo que tendria el kernel. E0 y E1 son diez
       lineas y un contador; el resto es un proyecto, y merece admitirlo
```

> Un quantum reparte el tiempo entre los que ya estan dentro. **Un aforo decide
> quien entra.** La diferencia entre las dos frases es la diferencia entre
> multiplexar y orquestar.
