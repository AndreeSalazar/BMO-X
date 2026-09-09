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

- [ ] **E4 -- TICKLESS / TSC-DEADLINE.** Hoy el LAPIC va en modo PERIODICO a
      1 kHz (`s2_mem/main.rs`, `0x320 = 48 | (1<<17)`), asi que **cada tarea se
      come mil interrupciones por segundo**, cada una con `xsave`/`xrstor` del
      estado AVX. Con un disparo programado al siguiente instante que importa, si
      DOOM esta solo **el temporizador no dispara**. Eso es lo que hace cierta de
      verdad la frase de la casa: *"un juego de un solo hilo tiene el nucleo
      entero por construccion"*.

- [ ] **E5 -- ORDENAR POR PLAZO (EDF).** Lo ultimo, y solo cuando existan plazos
      de verdad: `PLAN_EL_PLAZO`, bloque P3.

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
