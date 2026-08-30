# `critic/` -- los CARRILES, y hoy estan vacios a proposito

La ley es **L6g** en `META-KERNEL_HARD.md`, y la cobra `contrato.py`, regla R8.

[!] Este fichero vive DENTRO del arbol del kernel, asi que es fuente: va en
ASCII como todo lo demas de aqui. El espanol con acentos es para la pantalla.

## Para que existe esta carpeta

Para contestar **una sola pregunta**, y es la que se hace de verdad:

> **Voy a tocar esto. Que arrastro?**

Palabras del dueno: *"son para facilitar en orden si un dia quiero cambiar,
**como podre identificar?** Imaginate que sin eso seria dificil."*

*** **El eje NO es de que TIPO es la pieza.** Eso ya lo dice el nombre del
fichero donde vive. El eje es **que cuesta cambiarla** -- un letrero de
autopista, no una etiqueta de inventario.

## Los carriles

```text
   roja.rs       un fallo aqui PARA LA MAQUINA y no deja autopsia
                 -> se toca con las cuatro reglas de abajo y el hash de L6d

   amarilla.rs   VA A CAMBIAR, y al cambiar ARRASTRA A OTRO
                 -> la cabecera nombra a quien arrastra, y se tocan JUNTOS
```

**El verde no esta, y es a proposito.** Es todo lo demas. Un carril que
contiene casi todo no informa de nada: se entra en la autopista, no se vive en
ella.

*** **`amarilla` es el carril que faltaba.** El 30-08, `phys::free_frame` y
`vmm::caminable` juzgaban el mismo numero con dos techos distintos --16 GiB
contra 64 TiB-- y **cambiar uno sin el otro fue el bug**. Eso no es una pieza
critica: son **dos piezas atadas**, y lo que faltaba era el letrero que dijera
que van juntas. Es `ESPEJO` de L6f, ascendido de etiqueta a carril.

## Las cuatro reglas de dentro

```text
   1. DECLARAR NO ES OPCIONAL   `[cuesta]` y `[riesgo]` son obligatorios
   2. TIENE QUE SABER DECIR NO  banco de pruebas propio, o no entra (L4)
   3. NINGUN NUMERO SUELTO      todo tope sale de la constante que lo define
   4. TOPE DURO DE 300 LINEAS   la tercera parte de lo que L6a deja a un
                                modulo cualquiera
```

[!] **La 3 no la comprueba la maquina y hay que decirlo.** Un juez que
intentara distinguir `1 << 46` de una constante legitima acabaria adivinando, y
**un guardian que adivina da permiso con autoridad**. Esa la cobra la revision
humana; esta escrita para que se pueda citar. R8 cobra las otras tres y que el
nombre del fichero sea un carril.

## Por que estan vacios

Porque **la mudanza paga L6d**: un reparto se demuestra con el compilador
emitiendo los mismos bytes, no con los tests pasando. Una pieza entra **de una
en una y con su hash**.

> Mover medio Ring 0 de golpe para hacerlo mas seguro es exactamente la clase
> de cambio que mete el fallo que venia a evitar.

## Las candidatas, y en que carril caen

No se eligieron por parecer importantes: **fallaron el 2026-08-30**.

| pieza | carril | por que ese |
|---|---|---|
| `mm::vmm::caminable` | **amarilla** | no esta sola: `free_frame` juzga el mismo numero. Cambiar una sin la otra ES el bug que paso |
| `mm::phys::zero_frame` | **amarilla** | lo mismo, por el otro lado del par |
| `PHYSMAP_SIZE` / `MAX_PHYS` | **roja** | el techo del que cuelgan los dos. Cambiarlo mueve el suelo de todo el kernel |

[!] `uhid::poll` --la tercera del 30-08-- vive en `platform/`, no en Ring 0. La
ley dice **solo Ring 0**, asi que esa no muda: se queda con su etiqueta
`SILENCIO`. Si la jurisdiccion se extiende a los drivers de `platform/` es una
decision del dueno, y esta anotada como tal en vez de tomada.
