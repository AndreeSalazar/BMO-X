# EL NEUTRO ES AGNOSTICO -- la misma categoria en cualquier CPU

> Es la regla **N6** y la razon de que esta carpeta este en la raiz y no dentro
> de `Ultra_kernel_x86-64/`. Lo dijo el dueno con la palabra puesta:
>
> > *"luego ese mismo NEUTRO va a vivir otras arquitecturas de CPU si es que
> > llega, por algo se llama **agnostico**"*

---

## 1. ★★ LA TABLA QUE LO DEMUESTRA

Cuatro filas. **Las tres primeras son la misma frase en los tres sitios.**

| | x86-64 | ARM64 | RISC-V |
|---|---|---|---|
| **quien escribe sin permiso** | un aparato PCIe | un aparato del bus | un aparato del bus |
| **como llega a la RAM** | DMA | DMA | DMA |
| **a que obedece** | a nada | a nada | a nada |
| quien lo vigilaria | **VT-d** (Intel) / **AMD-Vi** | **SMMU** | **IOMMU** |
| esta en BMO-X? | no | -- | -- |

★ **Solo la cuarta fila cambia de nombre.** Y es justo la que no esta puesta en
ninguna de las tres, asi que hoy las tres columnas son **identicas**.

> La categoria no depende del CPU. Solo el nombre del guardia que no tenemos.

---

## 2. Que se mueve y que no, si BMO-X arranca en otra arquitectura

```text
   NO SE MUEVE
   [x] la definicion del neutro (las tres condiciones)
   [x] las seis reglas N1..N6
   [x] la frontera: que es neutro y que no
   [x] el argumento entero de por que el celo no llega ahi

   SE MUEVE
   [ ] el CENSO: otra placa, otros aparatos. Mismo formato
   [ ] el nombre de la MMU de los aparatos, y como se enciende
   [ ] como se descubre el bus (PCIe / device tree / ACPI)
```

** O sea: **se mueve el perfil, no la ley.** Es la LEY 24 --*el hardware se
PERFILA*-- aplicada a una ley en vez de a un driver, y por eso N6 prohibe
nombrar un CPU dentro de las reglas.

---

## 3. ⚠ Lo unico que de verdad cambia de forma: como se descubre el bus

Aqui si hay una diferencia real y conviene tenerla escrita antes de tropezarla:

```text
   x86-64    PCIe se ENUMERA: se recorre y los aparatos contestan
             -> es lo que hace `dev/pci.rs` y el portero del bus
   ARM64     muchos aparatos NO se enumeran: vienen escritos en un
             `device tree` que trae el firmware
   RISC-V    igual, `device tree`
```

★ **Y eso afecta a N1, no a las otras cinco.** En x86-64 el censo se puede
comprobar recorriendo el bus; en ARM habria que compararlo contra el arbol que
trae el firmware. **La regla sigue siendo la misma --hay que declararlos--;
cambia de donde sale la lista con la que se compara.**

---

## 4. Por que esto no es especular

No se esta diseniando para un ARM que no existe. Se esta **impidiendo escribir
la ley de forma que solo valga aqui**, que cuesta lo mismo hoy y ahorra
reescribirla entera despues.

```text
   escribir "activar VT-d"                 mas corto, mas concreto, caduca
   escribir "la MMU de los aparatos"       un nivel mas, y no caduca
```

** El precio esta escrito en N6 como sacrificio: **se pierde el atajo**. Nadie
puede comprobar *"la MMU de los aparatos"* con un `grep`; hace falta que alguien
rellene el perfil. Se paga claridad inmediata a cambio de que la ley no muera
con la placa.

---

## 5. Lo que este fichero NO afirma

```text
   [ ] no dice que BMO-X vaya a arrancar en ARM ni en RISC-V
   [ ] no dice que portar sea facil: dice que ESTA CARPETA no habria que
       reescribirla, que es una afirmacion mucho mas pequena
   [ ] y no propone escribir soporte para ninguna de las tres MMU
```

> Agnostico no significa portable. Significa **que la pregunta es la misma**
> aunque la respuesta cambie de nombre.
