# PLAN EL BUS APARTE -- el USB en su propio nucleo, y lo que hay que pagar antes

> Escrito el **2026-09-18**, la noche en que los "tirones" del escritorio
> resultaron ser el hilo del bus GIRANDO con prioridad 2 mientras el
> compositor (prioridad 0) no recibia turno. El dueno lo pidio asi:
>
> > *"verifica el hilo por completo, para educar a BMO-X: el hilo que no se
> > vaya a otro lado, pero si quiere otro, el CPU tiene otros hilos -- son 12
> > basado en el perfil del CPU. El core, como siempre, lo suyo. Podemos
> > redefinir y mejorar?"*
>
> Se puede. Y no es una tarde: todo lo que el bus toca esta escrito para UN
> nucleo. Este plan dice que hay que pagar, en que orden, y que se gana con
> cada paso -- de modo que cada paso valga solo, aunque el ultimo no llegue.

```text
   [ ]  pendiente        [~]  a medias, y se dice cuanto        [x]  hecho, con fecha
```

---

# 0. LO QUE PASA HOY, medido

```text
   hilo del bus     prioridad 2, late cada 4 ms (`dev/usb/bus.rs`)
   escritorio       prioridad 0
   choose_next      prioridad ESTRICTA sin envejecimiento (`scheduler/verde.rs`)
   nucleos en pie   1 de 12 (`save`: "en pie 1 hilos")
```

Mientras el hilo del bus esta LISTO, el escritorio no corre. Y enumerar un
puerto son esperas de verdad --debounce 100 ms, reset, corte de corriente
200 ms, tres lecturas de descriptores--: cada intento sobre el puerto mudo
era un cuarto de segundo sin fotogramas. Eso eran los tirones.

**Lo que ya se hizo el 18-09, sin mover el bus de nucleo** (`e483a205`,
`2ddae871`):

- [x] 18-09 -- en el hilo del bus, esperar es DORMIR: `arranque.rs::delay_ms`
      llama a `park_until` cuando `bus::soy_el_hilo_del_bus()`, y el reloj lo
      despierta. Fuera del hilo (arranque, syscall) se gira como antes.
- [x] 18-09 -- esperar al controlador tambien duerme: `XhciHal::respirar()`
      en `bmo-xhci/src/lib.rs`, medio ms girando y despues 1 ms dormido, 100
      veces como mucho.
- [x] 18-09 -- enumerar (avisos de enchufe y barrido) es SOLO del hilo:
      `enchufe.rs::barrer_si_toca` y `mod.rs::bombear_interno` lo filtran. Un
      barrido dentro del syscall del escritorio era el compositor parado en su
      propia puerta.

**Lo que eso NO arregla**: mientras el hilo duerme en un debounce, tampoco
bombea el teclado ni el raton. Dormir salva la pantalla, no el bombeo. Para
que el teclado siga a 250 Hz DURANTE una enumeracion hace falta que enumerar
y bombear no compartan hilo -- y en esta maquina sobran once nucleos.

---

# 1. POR QUE NO ES "MOVER EL HILO", con la lista

Un obrero hoy (`plat/smp/obrero.rs`) ejecuta PARTES del catalogo de
`bmo-orquesta` y vuelve a una barrera. No tiene planificador, ni reloj, ni
GS/TSS propios. Y el bus, desde el hilo, toca esto:

```text
   QUE                                   COMO ESTA HOY                 CON DOS NUCLEOS
   cola cruda de teclas (`teclas.rs`)    static mut + 2 indices        carrera; y el productor
                                                                       MOVIA el indice del consumidor
   cola de caracteres (`keyboard.rs`)    static mut + 2 indices        carrera
   puntero X/Y/botones/rueda (`mod.rs`)  static mut i32/u8             un i32 a medio escribir
   PUMPING (`bus.rs`)                    static mut bool               no cierra nada
   CABINA (`cabina/ring.rs`)             BUSY: bool + cli              cli no para al otro nucleo
   estado de modificadores, HELD_*       static mut, solo del bus      bien si SOLO el bus los toca
   `set_leds` (Bloq Mayus)               control transfer desde el     el escritorio tocando el bus
                                         lado del escritorio           desde otro nucleo
   una excepcion en un AP (`tramp.rs`)   #UD -> doble -> TRIPLE FALLO  un #PF del driver = el PC
                                                                       se REINICIA
```

Ese ultimo renglon es el que manda el orden: **un bus en un nucleo que no
puede tomar un fallo es un bus que apaga la maquina cuando falla.** Y lo que
hace falta para que un AP tome un fallo --GS, TSS, pila de excepcion, IDT
cargada-- es exactamente lo que el dueno llamo el SUB-DIRECTOR el 12-09
(`docs/maestro/AXION_MAESTRO.md`). Son el mismo trabajo con dos nombres.

---

# 2. EL ORDEN, y que vale cada paso por si solo

## A0 -- la frontera entre el bus y el escritorio es ATOMICA

Vale aunque el bus nunca se mueva: quita cuatro carreras que hoy no se ven
porque hay un nucleo, y el dia que haya dos no habra que buscarlas.

- [x] 18-09 -- `platform/shared/bmo-cola`: la cola de UN productor y UN
      consumidor, indices atomicos, cada lado toca solo el suyo; llena = se
      tira lo nuevo y se cuenta. Cinco pruebas, una con dos hilos de verdad.
- [x] 18-09 -- la cola cruda (`dev/usb/teclas.rs`) y la de caracteres
      (`dev/keyboard.rs`) son `bmo_cola::Cola`. Vaciar la cruda al enchufar
      se PIDE (`VACIAR_CRUDA`) y lo hace el consumidor: el productor ya no
      mueve el indice ajeno.
- [x] 18-09 -- puntero, rueda y contadores (`dev/usb/mod.rs`) son atomicos;
      `rueda()` es un `swap(0)`, no leer-y-borrar.
- [x] 18-09 -- `PUMPING` es `AtomicBool` con `swap`; `BUSY` de CABINA se toma
      con `compare_exchange` y `EV_SEQ`/`EV_LOST` son atomicos
      (`cabina/ring.rs`). CABINA sigue sin girar: tomado = perdido y contado.
- [x] 18-09 -- A0.1: el radar de CABINA (`cabina/radar.rs`) YA ERA atomico
      (`CUENTA`, `ULTIMO`, `RITMO`, `VENTANAS`: `AtomicU32`/`AtomicU64`). La
      casilla se escribio sin mirar; se miro, y no habia nada que hacer.
- [x] 18-09 -- A0.2: el escritorio ya NO bombea el bus mientras el hilo late.
      `bus.rs::pump_bus` cede (`PUMP_CEDIDOS`) si hay hilo, no soy el, y
      su latido tiene menos de un segundo; con eso los LEDs (`sync_leds`, un
      control transfer) y todo el reparto son del lado del bus. Si el hilo
      lleva un segundo sin latir, el syscall bombea el solo: el rescate.
      De paso desaparecen los dos cambios de CR3 por fotograma del syscall.
- [x] 18-09 -- A0.3: los 57 `static mut` de `dev/usb/` dicen en su linea
      quien los escribe (`// [escribe] bus|bombeo|escritorio|arranque|ambos`)
      y `toolchain/tools/escritores/escritores.py` lo exige en el build:
      uno nuevo sin etiqueta para, y `ambos` (hoy 7: audio TUBO/CEROS/
      PRESTADO, bus PUMP_OVERLAPS, rescate PRIMER_INTENTO/SWALLOW_ESC_RELEASE/
      SOLTADA) solo puede bajar. Esos siete son la lista exacta de A2.

## A1 -- un nucleo que puede FALLAR sin apagar la maquina (el sub-director)

- [x] 18-09 -- A1.1: `plat/smp/tss.rs` -- cada obrero carga al aterrizar una
      GDT con la forma del BSP (0x08 = codigo de 64 bits) y un TSS con IST1
      propio (4 KiB), y enciende OSXSAVE con el XCR0 medido. Salieron TRES
      motivos del triple fallo y el peor no estaba escrito: la IDT manda los
      fallos a 0x08, que en la GDT del trampolin era CODIGO DE 16 BITS. Y
      `fault_dispatch` (`faults/roja.rs`) tiene rama de obrero: apunta
      vector+rip en su ficha (`ficha::fallo`, estado FALLADO) y devuelve 0
      = `cli; hlt` de ese nucleo solo. El BSP lo dice en CABINA en el
      siguiente reparto o censo (`ficha::reportar_fallos`), y `crew::repartir`
      hace las partes de los fallados y no les espera.
      Sin GS por-CPU ni `rsp0` a proposito: un obrero no corre Ring 3.
- [~] A1.2 -- la prueba ESCRITA, sin correr en el Ryzen: `Parte::Tropezar`
      (`bmo-orquesta`, 5 partes) hace `ud2` en el atril 1; `smp tropezar` en
      el escritorio la toca. Lo que tiene que pasar: "SIGUES AQUI", CABINA
      con "un OBRERO tomo una excepcion", F11 con ese obrero en FALLO. Lo que
      pasaba: reinicio. Hasta que Eddi la corra, A2 no empieza.
- [ ] A1.3 -- el AP con reloj: un LAPIC timer propio, o el MWAITX con plazo
      que ya usa `obrero.rs` (27 ms) bajado a 4 ms para el residente.

## A2 -- el bus como RESIDENTE de un nucleo, elegido por perfil

- [ ] A2.1 -- `bmo-orquesta`: una clase de parte nueva, RESIDENTE, que no
      vuelve a la barrera; el nucleo sale del reparto mientras la tenga.
      El perfil del CPU dice cual (el ultimo hilo logico del ultimo nucleo
      fisico, para no robarle la cache al BSP: seccion 3 de AXION).
- [ ] A2.2 -- `pump_bus` corre en el residente; el BSP solo DRENA las colas
      (`poll_ascii`, `evento_tecla` dejan de llamar a `pump_bus`). El hilo
      de kernel `bus_thread` se retira.
- [ ] A2.3 -- `delay_ms` y `respirar` en el residente vuelven a girar: ahi
      no hay a quien cederle el nucleo, y girar es lo correcto.
- [ ] A2.4 -- `[consumo]` del residente medido y dicho en `save`: un nucleo
      latiendo a 250 Hz con MWAITX entre latidos, en vatios (R21).
- [ ] A2.5 -- la prueba de lo que se gana: `save` durante una enumeracion
      del puerto mudo dice `latido TARDE` = 0 y el teclado no pierde ni una
      tecla escribiendo mientras se enchufa un movil.

---

# 3. LO QUE ESTE PLAN NO PROMETE

- No promete que el teclado vaya mas rapido: ya va a 250 Hz y el aparato
  pide 8 ms. Promete que NO SE PARE mientras el bus hace otra cosa.
- No promete audio en su propio nucleo. `audio::latido` come en `pump_bus`
  y se mudaria con el; si un dia hace falta un nucleo para el, es otro plan.
- No toca el planificador. El fantasma de la prioridad estricta
  (`docs/plan/PLAN_SUELO_RING3.md`) sigue ahi para cualquier otro hilo de
  kernel que gire; este plan solo saca al bus de su alcance.
