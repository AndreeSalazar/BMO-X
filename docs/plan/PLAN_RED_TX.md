# PLAN RED TX -- transmitir, con el DMA contado y el cable detras de un grifo

> Escrito el **2026-09-13**, el mismo dia que la RTL8168 recibio en el Ryzen
> (16 tramas, 0 perdidas, ARP 4 e IPv4 12). Lo pidio el dueno asi:
>
> > *"en internet, puede tener el mapping y el unmapping? pero hot, como se hizo
> > con mi DMA -- y fuerte con firmas"* ... *"dale, eso es buen plan pero mas
> > maduro"*.
>
> Maduro quiere decir una cosa concreta: **la primera trama que salga por el
> cable no puede ser la primera vez que corre el codigo que la arma.** Cada
> escalon tiene su prueba, y ninguno empieza hasta que el anterior la paso.

---

# 0. LAS TRES COSAS QUE SE PIDEN, Y QUE PROTEGE CADA UNA

```text
   mapear/desmapear en caliente   que la tarjeta solo tenga lo que esta EN VUELO.
                                   Sin IOMMU es CONTABILIDAD (detecta y acusa);
                                   con IOMMU es un MURO (la tarjeta no ve el resto)
   el grifo                        que solo salga lo que el dueno abrio, con NUESTRA
                                   MAC de origen, acotado en tiempo, cupo y ritmo
   las firmas                      que un paquete falso o cambiado se RECHACE. No
                                   paran una escritura DMA: el hardware no las mira
```

**Ninguna de las tres sustituye a otra**, y por eso el plan tiene las tres.

---

# 1. LO QUE YA ESTA

- [x] **E0 -- recibir, en metal.** `red` en Ejecutar el 2026-09-13: cogidas 16,
      perdidas 0, reparto ARP 4 IPv4 12. El corral de entrada es
      `platform/drivers/net/src/anillo.rs` y el armado vive en
      `Ultra_kernel_x86-64/kernel/src/ring0/red/mod.rs` (`rx_start`).
- [x] **E1 -- el plano de salida, los vuelos y el grifo, en el anfitrion.**
      `platform/drivers/net/src/tx.rs` (2026-09-13): un solo `EOR`, largo
      acotado a 60..1514, vuelos en orden con caducidad al tick exacto, y el
      grifo cerrado al nacer. Se comprueba con `cargo test -p bmo-net`.
- [x] **E2 -- el corral de RX se PRESTA en el titular, sin transmitir.** Foto
      del Ryzen el 2026-09-13: `save` dice en vuelo 9, pisados 0, choques 0.
      Ese mismo dia `red rx` volvio a CERO tramas: el sondeo paraba en el primer
      descriptor con error y el anillo se quedaba atascado para siempre. Ahora
      `Llegada` (`platform/drivers/net/src/lib.rs`) solo para en `DeLaTarjeta`;
      una trama mala se cuenta (`INFO_NET_RX_MALAS`), se devuelve y se sigue.
      Y la red sale de `dev`: es su propia familia, `ring0/red`.
      Antes de E2 `rx_start` ponia el corral como `Titular::Neutro` pero el titular no sabia que la NIC escribia ahi. Ahora
      `prestar_tramo` de `Ultra_kernel_x86-64/kernel/src/ring0/mm/titular/roja.rs`
      marca sus 9 paginas con `APARATO_NIC` ANTES de tocar la tarjeta, y
      `devolver_tramo` las devuelve si armar falla.
      ** Es un PRESTAMO y no un vuelo: un anillo esperando trafico esta ocioso,
      no callado, y contarlo como silencio envenenaria el plazo de R-DMA-8
      (`NEUTRO/DMA/REGLAS.txt`, punto 3).

---

# 2. LOS ESCALONES QUE FALTAN

- [x] **E2b -- la foto de `red rx` tras el atasco.** Ryzen, 2026-09-13: el total
      sube 5 -> 9 -> 10 -> 14 entre fotos, tiradas 0 y malas 0. La red recibe y
      esta callada; lo que la paraba era que nadie vaciaba el anillo entre dos
      ordenes, y ahora lo vacia el escritorio cuatro veces por segundo.

- [x] **E3 -- EL GATE RED: transmitir, pagando UNA vez.** HECHO en el Ryzen el
      2026-09-14: `red prueba` dijo `PASA` -- el router de la LAN contesto a la
      sonda ARP. Eddi: *"burocratico
      en sentido de firmas pero si es valido ... una sola vez paga y luego sin
      burocracia, pero viene con radar de 4 ms"*. Los cuatro tiempos de
      `docs/identidad/LA_RUTA.md`, en codigo el 2026-09-13:
      `platform/shared/bmo-puerta-red` (el pase, el buzon y el radar, con 21
      pruebas en el anfitrion), `Ultra_kernel_x86-64/kernel/src/ring0/red/puerta.rs`
      (la puerta, el latido de 4 ms y la revocacion a una LAPIDA) y
      `Ultra_kernel_x86-64/kernel/src/ring0/red/salida.rs` (`CR.TE`, `TNPDS`,
      `TCR` y la campana). `RED_OP_ABRIR` juzga en orden --autoridad de Ring 0,
      tarjeta, enlace, receptor, un pase a la vez-- y cada no tiene nombre; la
      trama del proceso se COPIA a memoria del kernel antes del grifo, y el
      radar revoca por indice o largo imposible, MAC ajena, inundacion, plazo o
      cupo. Ordenes: `red abrir 60`, `red arp <ip del router>`, `red pase`.
      **Como se sabe que salio:** `red pase` dice `ARP ... CONTESTO desde` una
      MAC que no es la nuestra. El router solo contesta si la trama llego al
      cable: es la prueba que no se puede fingir.

- [ ] **E4 -- el muro: IOMMU, si la placa lo da.** Foto de `placa` (la operacion
      `PLACA_OP_IOMMU` de `Ultra_userspace/userland/src/red.rs`). Si hay AMD-Vi:
      un dominio solo para la NIC con el corral de `tx.rs` y `anillo.rs` mapeado
      una vez y desmapeado al soltar. Si NO hay: se dice en `red` y en `save`
      que la proteccion es contabilidad, no muro.

- [ ] **E5 -- la pila encima: un `ping` que contesta.** `platform/shared/bmo-pila`
      (`nodo.rs`) sobre la operacion de E3: ARP propio, eco ICMP, y el tiempo de
      ida y vuelta medido contra Windows en el mismo cable.

- [ ] **E6 -- firmas: cifrado autenticado sobre UDP.** ChaCha20-Poly1305 y
      BLAKE2s en `platform/shared/bmo-cripto`, con sus vectores oficiales
      (regla 1 de ese crate), y el handshake de WireGuard en Ring 3. Cada paquete
      que no autentica se tira con su motivo.

---

# 3. LO QUE ESTE PLAN SE NIEGA A PROMETER

```text
   [!] sin IOMMU, "desmapear" no impide que una tarjeta rota escriba fuera:
       lo DETECTA (pisados, choques, caducados) y lo DICE, nada mas
   [!] el grifo no sabe de IP -- el kernel no lo sabe a proposito. Filtra lo que
       es Ethernet: origen, largo, tipo y ritmo
   [!] el enlace sale a 10 Mbit (el paso 0 leyo 100). No es de este plan: es
       cable, puerto o autonegociacion, y se prueba antes de medir latencias
```

---

# 4. EL CAMINO A GEMINI (anotado el 2026-09-13)

> Eddi, con la primera foto del GATE RED delante: *"eso significa que PODEMOS
> IR A GEMINI para navegar?"*. Todavia no. Esta es la escalera, en orden, y
> cada peldano dice donde se mira.

La primera foto (2026-09-13): pase ABIERTO, `salieron 1`, 56 tramas en el
buzon, y el ARP a 192.168.1.1 sin respuesta. Salir del grifo no probaba salir al
cable, y la IP se eligio a ciegas; por eso existe `red prueba`.

Gemini es el destino bueno porque se ahorra lo mas caro de HTTPS: el certificado
se recuerda la primera vez (TOFU) y no hay cadena X.509 que validar. Y gemtext
son cinco clases de linea.

- [x] **G1 -- el router contesta.** Es E3: `red prueba` en el Ryzen dijo `PASA`
      el 2026-09-14.
      Si dice `FALLA en la TARJETA`, se mira
      `Ultra_kernel_x86-64/kernel/src/ring0/red/salida.rs`.
- [x] **G2 -- tener IP.** HECHO en el Ryzen el 2026-09-14: `red ip` dijo
      `CONCEDIDA` (dos horas de concesion, con router, mascara y DNS) y
      `red perfil` ensena la IP propia. DHCP en Ring 3 sobre el buzon (UDP 67/68):
      `platform/shared/bmo-pila/src/dhcp.rs` (el protocolo y el cliente, con
      banco contra servidores de mentira) y la orden `red ip` en
      `Ultra_userspace/services/director/src/commands/red_ip.rs`, en codigo el
      2026-09-14. La IP vive en memoria: nunca en disco ni en el repositorio
      (seccion 5). **Como se sabe:** `red ip` dice `CONCEDIDA` y `red perfil`
      ensena un numero en `IP propia`.
- [x] **G3 -- la pila sobre el buzon.** HECHO en el Ryzen el 2026-09-14: el
      router y un servidor de Internet contestaron a los cuatro ecos. [!] Los
      tiempos salen de 16 en 16 ms: el latido real se mide en `red ping`. `platform/shared/bmo-pila` (`nodo.rs`,
      con `Nodo::eco`) leyendo y escribiendo con `bmo::red::recibir` y
      `bmo::red::enviar`, en codigo el 2026-09-14: `red ping <ip>` en
      `Ultra_userspace/services/director/src/commands/red_nodo.rs` (ARP con
      nuestra IP, cuatro ecos con su tiempo). Es E5. **Como se sabe:** el router
      contesta a los cuatro ecos.
- [ ] **G4 -- DNS.** `platform/shared/bmo-pila/src/dns.rs`, en codigo el
      2026-09-14: una pregunta A por UDP, punteros de compresion solo hacia atras
      y CNAME seguido solo dentro de la respuesta. `red dns <nombre>`. **Como se
      sabe:** un nombre da la misma IP que da Windows en el mismo cable.
- [ ] **G5 -- TCP de verdad.** `platform/shared/bmo-pila/src/tcp` contra un
      servidor real: el saludo de tres pasos y un cierre limpio.
- [ ] **G6 -- TLS 1.3: EL MURO.** En `platform/shared/bmo-cripto`, con los
      vectores oficiales: X25519 (RFC 7748), ChaCha20-Poly1305 (RFC 8439), HKDF
      (RFC 5869) y la maquina de estados del handshake (RFC 8446). SHA-256 y
      Ed25519 ya estan. [!] Escrita mal no falla: funciona y no protege.
- [ ] **G7 -- el cliente Gemini.** Puerto 1965, TOFU guardado en el disco de
      BMO y gemtext pintado por el DIRECTOR
      (`Ultra_userspace/services/director`). **Como se sabe:** una capsula real
      se lee en pantalla.

---

# 5. LO QUE NO SE EXPONE (regla, 2026-09-13)

Eddi: *"no quiero exponer donde vivo, no quiero ser expuesto"*. El repositorio
es publico, asi que:

```text
   la MAC entera    NO entra en el repositorio. Las pruebas usan una de ejemplo
                    (02:1A:2B:3C:4D:5E) y la pantalla y CABINA ensenan solo el
                    fabricante. `red mac completa` la da entera a quien la pide
   la IP de la LAN  NO entra en el repositorio. En pantalla si: 192.168.x.x no
                    dice donde vive nadie
   la IP PUBLICA    BMO-X no la conoce ni la pregunta. Es la unica que localiza
   una IP fija      si hace falta, en `director.cfg` del disco de BMO, no aqui
```

[!] Lo publicado ANTES de esta regla sigue en la historia de git: se limpio el
arbol, no la historia. Reescribirla es una decision del dueno, no de un commit.
