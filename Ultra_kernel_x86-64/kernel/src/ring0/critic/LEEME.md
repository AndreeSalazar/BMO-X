# `critic/` -- la jurisdiccion, y hoy esta vacia a proposito

Aqui viven las piezas de Ring 0 **cuyo fallo no se puede deshacer**. La ley es
**L6g** en `META-KERNEL_HARD.md` y la cobra `contrato.py`, regla R8.

## Por que una carpeta y no otra etiqueta

Porque `[cuesta]` (L6e) y `[riesgo]` (L6f) son **trinquetes**: obligan al que los
pone, no al que no. Un sitio obliga al que entra.

> Lo que compra el nivel 3 no es orden: es **jurisdiccion**.

## Las cuatro reglas de dentro

```text
   1. DECLARAR NO ES OPCIONAL   `[cuesta]` y `[riesgo]` son obligatorios
   2. TIENE QUE SABER DECIR NO  banco de pruebas propio, o no entra (L4)
   3. NINGUN NUMERO SUELTO      todo tope sale de la constante que lo define
   4. TOPE DURO DE 300 LINEAS   la tercera parte de lo que L6a deja a un
                                modulo cualquiera
```

**La 3 no la comprueba la maquina y hay que decirlo.** Un juez que intentara
distinguir `1 << 46` de una constante legitima acabaria adivinando, y un
guardian que adivina da permiso con autoridad. Esa regla la cobra la revision
humana; esta escrita para que se pueda citar.

## Por que esta vacia

Porque **la mudanza paga L6d**: un reparto se demuestra con el compilador
emitiendo los mismos bytes, no con los tests pasando. Una pieza entra **de una
en una y con su hash**.

> Mover medio Ring 0 de golpe para hacerlo mas seguro es exactamente la clase
> de cambio que mete el fallo que venia a evitar.

## Las tres candidatas, y por que son ESTAS

Las tres del 2026-08-30. No se eligieron por parecer importantes: fallaron.

| pieza | que es | como fallo |
|---|---|---|
| `mm::vmm::caminable` | el juez de si una fisica se puede tocar | techo `1 << 46` donde ya existia `PHYSMAP_SIZE` |
| `mm::phys::zero_frame` | el que ESCRIBE 4 KiB | sin cota, al lado de `free_frame` que si la tiene |
| `uhid::poll` (drenaje) | el que vacia el anillo de eventos | `while let` sin tope, con un productor nuevo encima |

Las tres son **jueces y cotas**: funciones cortas que deciden si algo peligroso
puede pasar, viviendo dentro de ficheros grandes y leyendose como codigo normal.

[!] `uhid` es de `platform/`, no de Ring 0. La ley dice **solo Ring 0**, asi que
esa no muda: se queda donde esta con su etiqueta. Decidir si la jurisdiccion se
extiende a los drivers de `platform/` es una decision del dueno, no de esta
carpeta.
