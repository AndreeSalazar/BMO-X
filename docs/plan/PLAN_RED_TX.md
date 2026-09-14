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

- [ ] **E2b -- la foto de `red rx` tras el atasco.** Con el escritorio abierto
      30 segundos, `red rx` en Ejecutar dice tramas > 0 y cuantas malas.
      **Como se sabe que salio:** el total sube entre dos fotos seguidas; si
      malas sube a la par, el atasco era ese y el enlace de 10 Mbit es su causa.
      Sin esta foto E3 no se puede probar: su prueba es LEER la respuesta.

- [ ] **E3 -- transmitir detras del grifo.** `CR.TE`, `TNPDS`, `TCR` y la campana
      `TPPOLL.NPQ` de `platform/drivers/net/src/tx.rs`, en
      `Ultra_kernel_x86-64/kernel/src/ring0/red/mod.rs`. Una operacion nueva de
      `TASK_OP_RED` que COPIA la trama de Ring 3 al bufer, la pasa por `Grifo` y
      solo despues toca la campana. El grifo lo abre el dueno con una orden
      (`red abrir <segundos>`), y sin abrir contesta `Cerrado` por su nombre.
      **Como se sabe que salio:** una pregunta ARP al router y, en el mismo
      minuto, `red rx` ve su RESPUESTA dirigida a nuestra MAC. El router solo
      contesta si la trama llego al cable: es la prueba que no se puede fingir.

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
