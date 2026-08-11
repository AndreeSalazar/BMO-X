# RED MAESTRO -- perfilar la LATENCIA como se perfila el CPU

> Escrito el **2026-08-11**, antes del driver. Pregunta del dueno: *"mi mente
> gamer es que lo tome el internet con fuerza hasta sus limites total"*.
>
> El instinto es correcto y este documento lo toma en serio. Lo primero que hace
> es separar **el limite que el dueno cree que quiere** del **limite que de
> verdad le importa a un jugador**, porque no son el mismo y se optimizan en
> direcciones distintas.
>
> Mismo metodo que [`SMP_MAESTRO.md`](SMP_MAESTRO.md): antes de escribir codigo,
> mirar el silicio y decir los numeros incomodos.

---

# 1. EL LIMITE QUE IMPORTA NO ES EL QUE PARECE

"Hasta sus limites" suena a **ancho de banda**. Para un jugador no lo es.

| | ancho de banda | latencia |
|---|---|---|
| Descargar un juego | manda | da igual |
| **Jugar** | 50-200 KB/s bastan | **manda TODO** |

Un shooter competitivo manda del orden de 60 paquetes por segundo de unas
decenas de bytes. Eso son **kilobytes** por segundo. Lo que decide si el disparo
cuenta no es cuantos megabytes caben: es **cuantos microsegundos pasan entre que
la trama llega al cable y el juego la ve**.

> **Un sistema que satura el gigabit y anade 2 ms de latencia es peor para
> jugar que uno que hace la mitad de megabytes y anade 50 microsegundos.**

Y esto no es una curiosidad: **decide la arquitectura entera**, porque las
decisiones que dan ancho de banda (lotes grandes, buffers profundos, agrupar
interrupciones) son exactamente las que anaden latencia.

---

# 2. LO QUE DICE EL SILICIO QUE HAY DEBAJO

No es opinion. Es lo que contesta la maquina:

| | |
|---|---|
| NIC | `PCI\VEN_10EC&DEV_8168` -- Realtek RTL8111/8168 |
| MAC | `2C:F0:5D:D9:3C:E3` |
| Enlace negociado | **100 Mbps**, full duplex |

## El primer numero incomodo: el enlace es de 100, no de 1000

La tarjeta es Gigabit. **El enlace negocio a 100 Mbps.** O sea que el techo real
de esta maquina hoy son **12,5 MB/s**, no 125.

Eso casi siempre es el cable (un Cat5 viejo, o uno con un par roto: Fast
Ethernet usa dos pares y Gigabit necesita los cuatro) o el puerto del router. Es
un dato para el dueno mas que para el kernel -- pero conviene decirlo antes de
que alguien pase una semana optimizando para un gigabit que este cable no puede
dar.

## El segundo: a gigabit, el modelo "una interrupcion por paquete" se muere

| enlace | tramas de 1500 B por segundo | microsegundos entre trama y trama |
|---|---|---|
| 100 Mbps | ~8.100 | ~123 |
| 1 Gbps | ~81.000 | **~12** |

A 12 microsegundos por trama, una interrupcion por paquete deja al CPU
atendiendo interrupciones y nada mas. **A 100 Mbps hay 123 microsegundos de
margen**, que es holgado.

Lo que esto significa para el orden de trabajo: se puede empezar con el modelo
simple y correcto, **y este cable no lo va a delatar**. El dia que el enlace sea
de verdad gigabit hara falta agrupar avisos, y eso es una decision que se toma
con el numero delante, no ahora.

---

# 3. EL REPARTO: DONDE VIVE CADA COSA

```text
   Ring 0                          Ring 3
   ------                          ------
   KIND_RED                        ARP, IP, TCP, DNS, TLS
   tramas Ethernet crudas          todo lo que tiene versiones
   la MAC, el enlace, el DMA       y por tanto se equivoca
```

**El kernel no sabe lo que es una IP.** Los motivos, por orden de peso:

1. **Una pila TCP es la superficie de ataque mas grande de un sistema
   conectado.** Aqui puede morirse sin llevarse la maquina. Windows y Linux la
   tienen dentro del nucleo porque en 1990 no habia otra forma.
2. **Es la primera vez que BMO-X va a parsear bytes de un desconocido.** Un
   `.bex` malo lo trae quien ya tiene la maquina; una trama la manda cualquiera
   que comparta el cable. Ver [`../BITACORA.md`] y la sonda: hasta hoy, el
   atacante y el defensor eran la misma persona.
3. **Los protocolos cambian y el silicio no.** QUIC no existia hace diez anos.
   Un kernel que sabe de TCP tiene que recompilarse para hablar algo nuevo.

---

# 4. Y AQUI ESTA LA PIEZA QUE HACE QUE ESO NO CUESTE LATENCIA

El reparto de arriba tiene un peligro obvio, y hay que decirlo: **si cada trama
cruza un syscall, el diseno bonito pierde contra el feo.** Esa es la critica
clasica a los microkernels y en redes es donde mas duele.

No aplica aqui, y el motivo ya esta construido:

```rust
   MEM_OP_OFRECER   // ofrezco un trozo de MI bloque, a un TID concreto
   TASK_OP_TOMAR    // el otro lo toma -> handle KIND_PRESTADO
   PRESTADO_OP_BASE // donde quedo, EN MI ESPACIO
```

Los anillos de recepcion de la NIC **se mapean en el espacio de la pila de Ring
3**. La tarjeta escribe por DMA en esa memoria, y el proceso la lee con `MOV`
normales. En el camino del dato **no hay kernel**:

| | syscall por trama | anillo compartido |
|---|---|---|
| Kernel en el camino del dato | si, dos veces | **no** |
| Copias por trama | 1-2 | **0** |
| Coste por trama | un syscall | una lectura de memoria |

Es exactamente el mismo mecanismo que ya sostiene `KIND_FRAMEBUFFER` (el
compositor dibuja en VRAM sin pedir permiso por pixel) y DIRECTOR (una app
dibuja en su memoria y se compone sin copiarla).

> **El kernel reparte el aparato una vez; despues se aparta.**

El syscall queda para lo que de verdad es un evento: *"hay tramas nuevas"* -- y
ni siquiera eso hace falta si el proceso mira el anillo por su cuenta.

---

# 5. EL ESTADO REAL HOY, medido el 2026-08-11

| pieza | estado |
|---|---|
| `pci::cfg_read32` / `cfg_write32` | HECHO |
| `pci::msi_activar` | **HECHO y probado con AHCI** |
| `phys::alloc_frames_contig` | HECHO (lo usa la tabla de relocs) |
| `vmm::fisica_exacta` | HECHO (escalon 3) |
| `pci::find_net` | **HECHO** -- clase 0x02, primer BAR de memoria |
| `bmo_net::identificar` | **HECHO** -- MAC + `PHYstatus`, cero escrituras |
| Comando `net` del shell | HECHO |
| Contrato `KIND_RED` | falta |
| Anillos RX/TX | falta |
| Pila en Ring 3 | falta |

**Lo caro ya estaba pagado dos veces.** Enumerar PCIe, mapear un BAR, programar
MSI y llevar anillos DMA es lo que hacen `bmo-ahci` y `bmo-xhci` desde hace
meses. Lo unico genuinamente nuevo de la capa de abajo es el RTL8168.

## Lo que se borro para llegar aqui, y por que cuenta

`platform/drivers/net` eran 287 lineas de **Intel e1000** que no llamaba nadie:
la NIC por defecto de **QEMU**, escrita antes de mirar el aparato. No habria
encendido un LED en el Ryzen. Es el mismo hallazgo que el del sonido, donde el
audifono resulto ser USB y no HDA.

> **Mirar el aparato antes de escribir el driver.** Un crate huerfano no es
> trabajo adelantado: es una respuesta guardada a la pregunta equivocada.

---

# 6. EL ORDEN QUE PROPONE ESTE DOCUMENTO

### Paso 0 -- RECONOCER (hecho, falta la foto)

`find_net` + `identificar`, cero escrituras al aparato. Contesta tres preguntas
que ninguna teoria contesta, y **las tres estaban predichas**: la MAC tiene que
salir `2C:F0:5D:D9:3C:E3` y el enlace arriba a 100.

La prueba se puede tirar al suelo con la mano: **desenchufar el cable y escribir
`net`**. Si el enlace no se cae, lo que se lee no es el silicio.

### Paso 1 -- RECIBIR UNA TRAMA. Sin transmitir nada.

Este es el milestone que casi nadie aprovecha y aqui sale gratis: **un cable
enchufado ya lleva trafico**. ARP, mDNS, broadcast del router. Montando **solo el
anillo RX**, BMO-X imprime bytes que mando otro ordenador.

Sin IP, sin ARP, sin TCP, **y sin transmitir**. Seis bytes de destino, seis de
origen, un ethertype. La foto: `red: trama de 2C:F0:...:AA long=60 tipo=0806`.

Y como no se transmite, un error no puede molestar a nadie mas de la red.

### Paso 2 -- El contrato `KIND_RED`

Con una trama ya en la mano, el contrato se escribe sabiendo lo que tiene que
llevar en vez de imaginandolo. Reclamo exclusivo (como la pantalla), el anillo
prestado por `MEM_OP_OFRECER`, y la MAC como dato de solo lectura.

### Paso 3 -- TRANSMITIR, y ARP en Ring 3

La primera trama que sale. Y aqui se cruza la frontera: **el que construye el
paquete ARP es un programa de usuario**.

### Paso 4 -- Lo que el dueno queria

IP + UDP en Ring 3, y un `ping` que conteste. Ahi es donde la "mente gamer"
empieza a tener sentido de medir: latencia de ida y vuelta, en microsegundos,
contra la que da Windows en la misma maquina y el mismo cable.

**Esa comparacion es la unica prueba honesta de que el diseno vale.**

---

# 7. LA CABINA DE RED -- que tiene que confesar

Un camino rapido que nadie mide es un camino rapido que un dia deja de tomarse
en silencio (leccion del escalon 3). Asi que desde el primer anillo:

| numero | que delata si no es lo que debe |
|---|---|
| tramas recibidas / descartadas | un anillo que se llena sin que nadie lo vacie |
| **descriptores que el driver no llego a devolver** | la bomba parada, como el USB |
| avisos por MSI vs vueltas de sondeo | que la placa acepte MSI **y no lo enrute** |
| bytes que fueron DIRECTOS al anillo prestado | que el camino sin copia se este tomando |
| **microsegundos entre aviso y lectura** | la latencia, que es el titular de todo esto |

El ultimo es el que este documento pone en el centro. Sin el, "es rapido" es una
opinion.

---

# 8. LO QUE ESTE DOCUMENTO SE NIEGA A PROMETER

- **No va a haber Wi-Fi.** Un chip Wi-Fi necesita firmware propietario cargado
  desde el sistema, mas WPA2 (que es criptografia de verdad). Es otro proyecto.
- **No va a haber TLS pronto.** Sin curva eliptica ni AES no hay HTTPS, y eso
  esta detras de la misma deuda que aplazo la firma Ed25519.
- **La primera version va a ser lenta**, y esta bien. Correcto primero, medido
  segundo, rapido tercero -- y con el numero delante, no con la sensacion.

---

# El resumen en una frase

> **El kernel entrega el cable y se aparta; los protocolos son de usuario,
> porque el atacante ya no eres tu.**
