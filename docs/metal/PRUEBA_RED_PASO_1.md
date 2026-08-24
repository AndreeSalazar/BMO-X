# LA FOTO DEL PASO 1 -- que BMO-X imprima bytes que mando otro ordenador

> Hoja de arranque, escrita el 2026-08-24. **Se escribe ANTES de arrancar**, con
> las respuestas predichas, porque un resultado que no se predijo no distingue
> "funciono" de "salio algo".
>
> Es el metodo de las cinco sondas del `#GP` de julio: **predecir, leer,
> comparar.** Y aqui sale barato porque **no se escribe un solo byte que salga
> por el cable**: `CR.TE` se queda apagado a proposito.

---

# 0. QUE SE ESTA PROBANDO, Y QUE NO

```text
   SI     que el anillo RX esta bien armado y la tarjeta escribe en NUESTRA memoria
   SI     que los descriptores se devuelven y el anillo da la vuelta sin perderse
   SI     que la cabecera Ethernet se interpreta con el orden de bytes correcto
   NO     transmitir. Nada sale de esta maquina en esta prueba
   NO     IP, ARP, TCP. Eso es Ring 3 y es otro paso
```

** Y como no se transmite, **un error aqui no puede molestar a nadie mas de la
red.** Es el hito que casi nadie aprovecha, y es gratis.

---

# 1. ANTES DE ARRANCAR: EL CABLE

El paso 0 ya dijo que el enlace esta arriba a 100 Mbps. Si no lo estuviera, el
comando se niega **antes de armar nada** y lo dice:

```text
   [red] el enlace esta ABAJO: enchufa el cable antes de armar nada
```

*** Y la prueba se puede tirar al suelo con la mano: **desenchufa el cable y
escribe `net`**. Si el enlace no se cae, lo que se esta leyendo no es el silicio.

---

# 2. LA SECUENCIA

```text
   net           <- censo, no toca nada. Confirma MAC y enlace. Tambien F9
   net rx        <- ARMA el receptor
   (esperar de 5 a 30 segundos)
   net rx        <- vuelve a mirar
```

** `net` a secas y `net rx` estan separados a proposito, con la misma forma que
`smp`: **la palabra sola hace censo y el argumento ACTUA**. Este es el primer
codigo que deja a un aparato escribir en la memoria de esta maquina por su
cuenta, y eso no se consigue tecleando el comando de diagnostico.

---

# 3. LO QUE TIENE QUE SALIR -- predicho

## 3.1 -- El censo (`net`)

```text
   red:  MAC                             =2C:F0:5D:D9:3C:E3
   red:  PHYstatus crudo                 =0b1011
   red:  enlace ARRIBA, megabits         =100
```

## 3.2 -- Al armar (`net rx`, primera vez)

```text
   [red] receptor armado. tramas ahora 0, total 0
   [red] cero de momento es normal: vuelve a escribir `net rx` en unos segundos
```

[!] **CERO EN LA PRIMERA VUELTA ES LA RESPUESTA ESPERADA, no un fallo.** El
anillo se acaba de armar y el trafico de broadcast llega cada pocos segundos.
Decirlo es lo que impide que el minuto siguiente se gaste buscando un fallo en
un driver que funciona.

## 3.3 -- *** LA FOTO (`net rx`, segunda vez)

En CABINA, y son **cuatro lineas por trama**:

```text
   red:  trama DE                        =XX:XX:XX:XX:XX:XX
   red:       PARA                       =FF:FF:FF:FF:FF:FF
   red:       tipo ARP                   =0x0806
   red:       largo                      =60 B
```

**Eso es el paso 1 hecho.** Bytes que mando otro ordenador, leidos por codigo
escrito aqui, sobre un anillo armado aqui.

### Que dice cada linea, y por que son cuatro y no dos

| linea | que contesta |
|---|---|
| `trama DE` | hay otro ordenador en el cable, y la tarjeta nos lo trae |
| **`PARA`** | *** **si el FILTRO de recepcion funciona.** Ver 3.4 |
| `tipo` | que la cabecera se leyo con el orden de bytes del CABLE, no el nativo |
| `largo` | que el descuento del FCS es correcto |

## 3.4 -- *** POR QUE EL DESTINO ES LA LINEA QUE MAS VALE

Hasta el 2026-08-24 esta foto imprimia **solo el origen**, y con eso no se puede
contestar una de las tres preguntas:

> Una tarjeta en modo promiscuo y una filtrando bien dan **los mismos origenes**.
> Lo que cambia es **a quien iban dirigidas las tramas**.

Asi que se mira `PARA`:

| lo que sale en `PARA` | que significa |
|---|---|
| `FF:FF:FF:FF:FF:FF` | broadcast. **Es lo normal y es lo esperado** |
| `2C:F0:5D:D9:3C:E3` | dirigida a NOSOTROS. Correcto, y mas raro sin IP |
| **cualquier otra MAC** | [!] estamos viendo trafico de terceros: **el filtro esta abierto** |

** Lo ultimo no rompe nada hoy y es exactamente lo que hay que saber antes de
escribir el paso 2, porque cambia lo que `KIND_RED` puede prometer.

---

# 4. LO QUE PUEDE SALIR MAL, Y QUE DICE CADA COSA

| lo que sale | de quien es la culpa |
|---|---|
| `el receptor no se pudo armar` | sin marco para el anillo (memoria), o la NIC no termina su reset. **CABINA dice cual** |
| tramas siempre en 0, con cable | la NIC no sale del reset, o el BAR elegido no lleva a los registros |
| `trama demasiado corta para tener cabecera` | llegan bytes y el sospechoso es **el descuento del FCS** |
| `tipo` = `0x0608` en vez de `0x0806` | orden de bytes: se leyo nativo en vez de del cable |
| `[!] es un LARGO, no un tipo (802.3)` | el campo vale menos de `0x0600`. En una LAN moderna, **eso es el hallazgo** |
| ★ `[!] dice venir de NOSOTROS` | ver abajo |

## 4.1 -- *** LA TRAMPA QUE SE CAZA AQUI Y NO TRES ARRANQUES DESPUES

**Este paso NO TRANSMITE.** Asi que una trama que diga venir de nuestra propia
MAC no puede ser nuestra. Significa una de dos, y las dos son hallazgos:

```text
   la tarjeta esta en LOOPBACK interno
   el anillo esta leyendo memoria que NO ES LA SUYA
```

Sin ese aviso, las dos se verian como *"la red RECIBE"* y **la casilla se
pondria verde por el motivo equivocado** -- que es el fallo que este proyecto
persigue desde el primer dia.

---

# 5. LA VUELTA ATRAS

```text
   git revert abd9cf1c
```

Y el riesgo real de esta prueba, dicho entero: **es opt-in.** `net` a secas no
toca nada, y sin escribir `net rx` el anillo no se arma nunca. Un arranque que
no teclee esas dos palabras se comporta exactamente como el de ayer.

---

# 5b. Y EN EL MISMO ARRANQUE: `placa`

Cuesta una palabra mas y es el paso 0 del **firmware**, con la misma forma: cero
escrituras, respuesta predecible.

```text
   placa     <- o `firmware`, o la tecla F10
```

## Lo que tiene que salir

```text
   [placa] firmware de <OEM>, tabla <MODELO>
        APIC   nnn B  el censo de nucleos (MADT)
        FACP   nnn B  energia, reset y el RTC (FADT)
        MCFG   nnn B  donde vive la config de PCIe en memoria
    AML DSDT  nnnn B  [!] AML -- un PROGRAMA, y aqui no se ejecuta
    AML SSDT  nnnn B  [!] AML -- un PROGRAMA, y aqui no se ejecuta
        ...
   [placa] NN tablas, M son AML (no se ejecutan), 0 sin suma valida
```

## *** LA CIFRA QUE HAY QUE MIRAR

**No es cuantas tablas hay: es cuantas NO PASAN SU SUMA.** En una placa sana ese
numero es **cero**.

** Un puntero del XSDT que apunte a memoria que no es una tabla produce una
cabecera con campos **plausibles** -- cuatro bytes cualesquiera parecen una
firma, y un `u32` cualquiera parece un largo. Sin la suma, el censo se creeria
cualquier cosa.

[!] Y si sale mayor que cero, **lo que falla no es la placa**: es el mapeo de
esas direcciones fisicas. Un fallo del kernel disfrazado de firmware raro.

## Y las dos filas que se leen de verdad, no solo se cuentan

```text
   [placa] PCIe config en 0xE0000000  buses 0..255  (4096 B por funcion)
   [placa] IOMMU tipo 0x10  registros en 0xFEB80000
   [placa]     la hay y se sabe donde. ENCENDERLA es otro trabajo
```

| lo que sale | que significa |
|---|---|
| una direccion de ECAM | **se puede leer la config extendida de PCIe** -- AER, ATS, SR-IOV |
| `sin MCFG` | PCI se queda en 256 B por funcion. No es un fallo, es una respuesta |
| una direccion de IOMMU | la hay. Cerrar el agujero del DMA **es posible** |
| `sin IVRS` | [!] nada limita adonde escribe un aparato con DMA |

** La direccion de ECAM en un AM4 suele ser `0xE0000000` o `0xF0000000`, y
Windows la lista igual. Si BMO-X dice otra cosa, uno de los dos lee mal.

## Y la prediccion que se puede hacer sin arrancar

El otro sistema de esta misma maquina lista las tablas ACPI. **Las firmas tienen
que coincidir**, y el numero de SSDT tambien. Si BMO-X ve menos tablas que
Windows, el XSDT se esta recorriendo corto.

---

# 6. Y SI SALE BIEN, QUE SE DESBLOQUEA

```text
   paso 2   KIND_RED, escrito CON UNA TRAMA EN LA MANO en vez de imaginandola
   paso 3   el anillo TX: la primera trama que SALE
   paso 4   IP + UDP en Ring 3, y un `ping` que conteste
```

** El paso 2 es la razon de que el 1 vaya antes. Un contrato escrito mirando una
trama de verdad no se parece a uno escrito mirando un documento -- y `KIND_RED`
va a durar mas que este mes.
