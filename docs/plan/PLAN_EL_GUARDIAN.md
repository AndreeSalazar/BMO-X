# PLAN EL GUARDIAN -- BMO-X como aparato, no como invitado

> Escrito el **2026-09-08**. La idea es del dueno, y la trajo asi:
>
> > *"si crean un PCI con RISC-V donde BMO-X vive gracias a DMA, se convierte en
> > guardian LITERAL: se conecta a internet en vez de la placa base, inspirado en
> > la GPU -- que el HDMI de la placa es adorno."*
>
> No es una idea loca. **Tiene nombre, existe, y es lo que hace funcionar la nube
> mas grande del mundo.**

---

# 0. QUE ES ESTO, POR SU NOMBRE

Se llama **DPU** (o SmartNIC, o IPU): una tarjeta con su propio CPU, su propia
memoria y su propio sistema, que se queda con la red, el almacenamiento y la
seguridad **en vez del host**. NVIDIA BlueField, AMD Pensando, Intel IPU.

★ Y el ejemplo que importa: **AWS Nitro**. Amazon saco el hipervisor, la red, el
disco y la seguridad del CPU del host y los metio en tarjetas propias. El host
quedo como *"solo computo"*. La analogia del HDMI del dueno es literalmente lo
que ellos hicieron con la placa base entera.

---

# 1. POR QUE ENCAJA CON BMO-X MEJOR QUE CON CASI CUALQUIER OS

| lo que en un OS normal es un problema | aqui deja de serlo |
|---|---|
| **no hay POSIX** | una DPU corre un juego FIJO de aparatos, no apps de terceros. **La mayor debilidad desaparece** |
| superficie minima | dos syscalls y capabilities es exactamente lo que quieres en algo que tiene que ser de fiar |
| **LEY 24: el hardware se PERFILA** | en una tarjeta TU eliges el hardware. Una placa, un perfil. Se acaba la matriz infinita |
| `NEUTRO/` | *"lo que BMO-X no controla"* -- en una DPU el NEUTRO **es el host**, y estas aislado de el por construccion |
| otra arquitectura de CPU | ya esta previsto: [`NEUTRO/ARQUITECTURAS.md`](../../NEUTRO/ARQUITECTURAS.md) se escribio para esto |

## ★★ Y la inversion que hace grande la idea

Hoy BMO-X es **invitado** en un hardware que no controla, y su propia ley lo
dice: *el celo es CIEGO con el DMA* (`NEUTRO/LEY.md`). Cualquier aparato puede
escribir en su RAM sin pedirle permiso -- eso explica la azul de la purga, el xHC
muerto y el asignador colgado a la vez.

> **En una tarjeta, el que hace DMA eres tu.**

El modelo de amenaza se da la vuelta entero: de victima a dueno. Eso no es una
mejora de rendimiento: es un cambio de bando.

---

# 2. ⚠ LO QUE ES DURO, DICHO ANTES DE EMPEZAR

```text
   ser un DISPOSITIVO PCIe no es usar el bus PCIe. Hace falta endpoint de
   verdad: decodificar BARs, motor de DMA, generar MSI-X. Eso es FPGA (caro)
   o silicio (impensable para una persona)
   el driver del lado del HOST es la parte que NO controlas, y por ahi se
   escapa exactamente lo que este plan vende: la confianza
   el port a RISC-V es un backend nuevo del toolchain -- x86-64 ya esta hecho,
   asi que el camino se conoce, pero no es gratis
```

---

# 3. LA ESCALERA, CON PRECIOS

## G1 -- BMO-X en RISC-V, a secas

- [ ] **G1.1 -- una placa.** VisionFive 2 / Milk-V, ~60-100 EUR. Sin PCIe
      endpoint, y no hace falta todavia.
- [ ] **G1.2 -- backend RISC-V en el toolchain.** El emisor de x86-64 vive en
      `toolchain/lang/inti/` y `sem-asm`; la tabla de intrinsecos ya es TOML.
      Aqui se comprueba si la apuesta de *"tablas y no cerebros"* era cierta.
- [ ] **G1.3 -- el arranque.** No hay UEFI GOP: en RISC-V es SBI + device tree.
      Es un `boot_context` distinto, y **eso ya es una frontera declarada**.
- [ ] **G1.4 -- `PERFIL/` para la placa nueva.** Una placa, un perfil, y el
      guardian `perfil-campos` comparandolo con el codigo desde el primer dia.

★ **Lo que demuestra G1**: que la palabra *agnostico* de `NEUTRO/` era verdad.
Hoy es una afirmacion; con esto es una foto.

## G2 -- BMO-X como CAJA EN LINEA (el guardian de verdad)

- [ ] **G2.1 -- dos puertas de red y reenvio entre ellas.** Entre el router y el
      PC. El driver Realtek ya existe (`ring0/dev/net/`) y `PERFIL/RED.txt`
      declara su `pci_vendor`.
- [ ] **G2.2 -- la politica.** Que pasa y que no. Es donde EL ORQUESTAL deja de
      ser una palabra: *"multiplexar es ser generoso, orquestar es ser CELOSO"*.
- [ ] **G2.3 -- la bitacora de lo que paro**, en ESTRATOS. Un guardian que no
      deja constancia no es un guardian: es un filtro.
- [ ] **G2.4 -- ⚠ y ANTES de nada, la deuda de la firma.** Un aparato de
      seguridad cuyo propio software sale con `sig_algo = 0` y ancla vacia no se
      puede defender. Ver `docs/maestro/` y `platform/abi/bmo-firma/`.

## G3 -- BMO-X como tarjeta PCIe (la vision)

- [ ] **G3.1 -- endpoint PCIe en FPGA.** Placa con PCIe (Xilinx/Lattice). Es
      donde el precio se dispara.
- [ ] **G3.2 -- el driver del host.** Windows y Linux. **La pieza que no
      controlas.**
- [ ] **G3.3 -- DMA hacia el host, con el celo del lado correcto.**

---

# 4. ★★★ LO CONTRAINTUITIVO, Y ES LA DECISION DEL PLAN

La tarjeta PCIe **depende de que el host coopere**: su driver tiene que mandar el
trafico por ti. La caja en linea **no depende de nada**: estas fisicamente en el
cable.

> Para un **guardian**, cuanto menos confies en el host, mejor.

★★ Asi que **G2 no es una version degradada de G3: es mas fuerte en seguridad y
mas debil en rendimiento.** Y este proyecto vende confianza, no ancho de banda.

**De ahi el orden: G1 -> G2 -> (G3 si alguna vez hay dinero).** G2 ya es un
producto, y la gente de infraestructura compra *appliances*, no tarjetas.

---

# 5. DONDE ENCAJA CON LO QUE YA SE DECIDIO

El companero del dueno llego por su cuenta a *"BMO-X como componente adicional,
si o si, seguridad dura, y el OS para uso comun diario"*. Y la hoja de ruta ya
decia **BANCA + Ada**.

★ Tres caminos independientes al mismo sitio: **BMO-X no es un OS de escritorio
que ademas es seguro. Es un aparato de confianza que ademas sabe pintar.**

Este plan es esa frase con forma fisica.

---

# 6. LO QUE ESTE PLAN NO PROMETE

```text
   [ ] no es para maquinas de nadie mas hasta que la FIRMA este pagada
   [ ] no da rendimiento: da AISLAMIENTO. Son cosas distintas y G2 sacrifica
       lo primero a proposito
   [ ] G3 no es alcanzable por una persona sin dinero, y decirlo ahora vale
       mas que descubrirlo en el escalon 2
   [ ] y G1 no demuestra el producto: demuestra que la palabra `agnostico`
       era verdad. Que no es poco, y no es lo mismo
```

> Una tarjeta que hace DMA en tu maquina o es tuya, o es de otro. Lo que este
> plan cambia no es lo que BMO-X puede hacer: es **de que lado de esa frase
> esta**.
