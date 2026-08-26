# LOS TRES VERTICES -- de que esta hecho BMO-X, y por que esos tres

> El triangulo es del dueno, con sus palabras y su dibujo:
>
> ```text
>                [ LA ELEGANCIA DE MAC ]
>              (Interfaz estetica y fluida)
                          
                          / \
>                        /   \
>                       /     \
>                      / BMO-X \
>                     /_________\

>    [ LA LIBERTAD DE WINDOWS ] --- [ LA POTENCIA BARE METAL ]
>    (Compatibilidad/Familiaridad)   (Cero latencia/Maxima seguridad)
> ```
>
> Este documento no lo adorna: lo somete a la regla de la casa. **Cada vertice
> trae lo que BMO-X YA TIENE que se lo gane, lo que se niega a coger de ese
> mundo, y lo que le cuesta a los otros dos.**
>
> Un triangulo donde los tres lados solo suman es un folleto. Los tres se
> estorban, y **saber donde** es lo que lo convierte en un diseno.

---

# 0. LA TESIS, EN UNA FRASE

> **Los tres vertices estan en tension en cualquier sistema, y aqui no, por UNA
> decision: la superficie es pequena.**

Y esa no es una virtud abstracta. Es un numero:

```text
   2 puertas congeladas       INVOKE y WAIT
   93 operaciones aditivas    sobre capabilities
   -----------------------------------------------
   la promesa entera cabe en una pagina
```

De ahi salen los tres, y por eso el orden de este documento es ese: primero por
que normalmente pelean, y luego por que una promesa de una pagina los deja
convivir.

## 0.1 -- Por que normalmente pelean

| | lo que gana | lo que paga |
|---|---|---|
| **Mac** | coherencia: un solo objetivo, todo encaja | **libertad**: corre lo que ellos deciden |
| **Windows** | libertad: tu maquina, tu binario de hace 20 anos | **coherencia**: treinta anos de capas encima de capas |
| **bare metal** | potencia: nada entre tu y el silicio | **las dos**: te lo escribes todo |

★★ **Cada uno de los tres compra su vertice pagando con otro.** Un sistema que
prometa los tres sin decir con que los paga esta vendiendo, no disenando.

---

# 1. ▲ LA ELEGANCIA DE MAC -- pero lo que Apple hace BIEN no es lo bonito

## 1.1 -- Lo que de verdad se copia

No los degradados. **La coherencia que sale de controlar la pila entera y tener
UN objetivo.** Apple no es elegante porque dibuje bien: es elegante porque no
tiene que funcionar en diez mil maquinas distintas.

BMO-X tiene exactamente esa propiedad, y la tiene escrita como ley:

> **LEY 24: el hardware se PERFILA.** El software es agnostico; una estimacion
> generica es una estimacion de OTRO proyecto.

## 1.2 -- Lo que ya existe y se lo gana

| pieza | que es |
|---|---|
| **MAQUETA** | la maquetacion se compila **en el anfitrion** y emite coordenadas. Una app **no lleva dentro un motor de composicion** |
| **DIRECTOR** | una app dibuja en SU memoria y se compone en un marco. Recibe teclas y raton por un buzon en su propia memoria, **cero syscalls** |
| **el rasterizador propio** | recorte, linea, triangulo. Es el ORACULO contra el que se juzgara la GPU el dia que haya una |
| **la ciudad del arranque** | 3.123 lineas que dibuja la CPU y no se guardan |

★ La fila de MAQUETA es la que mas se parece a Apple y menos lo parece: **quitarle
el motor de composicion a la app** es lo que hace que dos apps no se vean
distintas. Es coherencia comprada por la via de quitar, no de anadir.

## 1.3 -- ⚠ Lo que este vertice le cuesta a los otros dos

**A la libertad**: si MAQUETA compila la cara, el que escribe la app **no puede
inventarse su propio motor**. Se le quita una libertad a cambio de que el
escritorio sea uno.

**A la potencia**: y aqui hay un numero, no una opinion.

```text
   volcar la pantalla entera    27,6 ms
   un fotograma a 60 Hz         16,7 ms
```

*** **A pantalla completa esta maquina no llega a 60 fotogramas por segundo
ANTES de dibujar nada.** Asi que la elegancia cede: el primer video va en ventana
pequena, y las animaciones que no caben en el presupuesto no se hacen. **Medido,
no supuesto.**

---

# 2. ◣ LA LIBERTAD DE WINDOWS -- y hay que empezar por lo que NO es

## 2.1 -- [!] BMO-X no corre programas de Windows. Nunca va a correrlos.

Y va primero para que nadie lo lea al reves. No hay Win32, no hay `.exe`, no hay
capa de compatibilidad y no esta en el plan. `EL_FUERO` lo dice en su seccion de
lo que no se concede, y `QUE_DESBLOQUEA` pone los numeros.

## 2.2 -- Entonces que es la libertad de Windows, de verdad

**Que un binario de hace veinte anos sigue arrancando.** Esa es la superpotencia
real de Microsoft y no la reconoce casi nadie: no es que tenga muchos programas
-- es que **nunca rompio el contrato**.

> **Lo que compras de un sistema no es su codigo: es la lista de cosas que
> promete no romper.**

Y esa lista, en Windows, se paga con treinta anos de capas. **BMO-X coge la
promesa sin la deuda**, y puede porque la promesa cabe en una pagina.

## 2.3 -- Lo que ya existe y se lo gana

| pieza | que es |
|---|---|
| **dos puertas congeladas** | `INVOKE` y `WAIT`. Un tercero existio y **se retiro**; su numero no se recicla |
| **el menor del ABI es aditivo** | y desde el 26-08 el cargador lo comprueba de verdad -- antes decia serlo y comprobaba lo contrario |
| **`toolchain/tools/contrato/`** | cinco reglas que **cobran el peaje** de todo lo que entra en la superficie. Con trinquete: la deuda solo puede encoger |
| **EL FUERO** | no es un SDK: es una carta de concesion. **Las leyes viajan dentro de lo que se entrega** |
| **una app es UN fichero** | `.bex` con sus recursos dentro. No existe el fallo de que falte uno a mitad de ejecucion |

★★ Y la libertad de verdad, la que se nota: **las reglas de la casa no viajan.**
Los guardianes leen `git ls-files` de este repositorio. Un `.bex` de 4.000 lineas
de fuente arranca igual que uno de cuarenta -- lo que el sistema le exige a una
app es que declare lo que necesita y que no mienta, no que sea bonita por dentro.

## 2.4 -- ⚠ Lo que este vertice le cuesta a los otros dos

**A la elegancia**: si un tercero puede traer su app, el escritorio deja de estar
bajo control. La respuesta no es prohibir -- es `R-APP3` y `R-APP4`: **dibuja en
SU memoria y la ofrece; no toma la pantalla.**

**A la potencia**: y esta es la incomoda. Libertad significa que corre codigo que
no escribiste tu, y en bare metal sin IOMMU **un aparato al que ese codigo diera
ordenes escribiria donde quisiera**. Hoy no puede --lo que se cede es de solo
lectura-- pero el dia que se abra, hara falta una IOMMU. Esta escrito en el plan
del suelo, parte 4, y no se disimula.

---

# 3. ◢ LA POTENCIA BARE METAL -- la unica que se mide en ciclos

## 3.1 -- Lo que ya existe y se lo gana

| pieza | el numero |
|---|---|
| **el precio de una puerta** | **969 ciclos**, medidos. No estimados |
| **el compositor no cruza la frontera** | escribe pixeles con `mov`. No se optimiza el cruce: **se borra** |
| **los siete muros del silicio** | CPL3, U/S, NX+W^X, SMEP, SMAP, UMIP y la puerta unica. Los cuatro ultimos casi nadie los enciende. Aqui estan los cuatro, confirmados en metal |
| **AML nunca** | tablas ACPI estaticas SI, un interprete de bytecode de terceros en Ring 0 **jamas** |
| **el reparto medido** | 11,59x con doce hilos. Y dicho que **no se puede extrapolar** |

## 3.2 -- La seguridad, contada como es y no como suena

★★ **Tres cosas que se parecen y no lo son**, y confundirlas es lo que hace que
"maxima seguridad" no signifique nada:

```text
   una APP falla         la tarea muere, BMO vive     -> AISLAMIENTO FUNCIONANDO
   el KERNEL falla       pantalla azul y reinicio     -> no hay aislamiento posible
   una VULNERABILIDAD    otro decide que hace tu maquina
```

Los dos primeros son **fiabilidad**. El tercero es **autoridad**. Una app puede
reventar cada minuto sin ser una vulnerabilidad, y un kernel que lleva un ano sin
caerse puede estar lleno de agujeros.

★ Y desde el 26-08 hay un cuarto escalon que casi ningun sistema tiene: **la
patada.** Cuando el kernel ve que su propia contabilidad esta danada, **recupera
la maquina el solo**, limpia la pantalla y explica por que. No espera a que se lo
pidan.

## 3.3 -- ⚠ Lo que este vertice le cuesta a los otros dos

**A la elegancia**: cero latencia y "bonito" compiten por el mismo presupuesto de
milisegundos, y aqui gana el numero.

**A la libertad**: **69.300 lineas corren con privilegio**, y de las 18.100 que
interpretan bytes de un desconocido, 13.400 no tienen ni un `unsafe`. Las que si
--`xhci` con 66-- son las que no pueden bajar a Ring 3 sin IOMMU. Bare metal
significa que no hay nadie debajo que te recoja.

---

# 4. ★★ EL CENTRO: por que los tres caben, y es UNA decision

Vuelve la tesis, ya con los tres vertices puestos:

> Los tres pelean **cuando la superficie es grande**. Una superficie grande hay
> que mantenerla (mata la elegancia), no se puede prometer entera durante diez
> anos (mata la libertad), y necesita capas para gobernarse (mata la potencia).

**Una promesa que cabe en una pagina se puede cumplir diez anos.** Y de ahi salen
los tres a la vez:

| porque la superficie es pequena... | ...se puede |
|---|---|
| cabe en la cabeza de una persona | **mantener la coherencia** (Mac) |
| se puede prometer entera | **no romperla nunca** (Windows) |
| no hace falta una capa que la gobierne | **hablarle al silicio directamente** (bare metal) |

★ Y la prueba de que no es un eslogan: **la superficie crecio de 69 a 93
operaciones en una semana y las dos puertas no se movieron.** Lo que impide que
93 se conviertan en 350 es una regla de una linea -- `R-REX3`: *comodidad es
cabecera, autoridad es operacion*.

## 4.1 -- Y el peaje, que es como se mantiene pequena

Nada entra gratis. Seis cosas paga todo lo que se anade a la superficie, y desde
el 26-08 **las cobra una herramienta**, no la memoria de nadie:

```text
   1  un numero que quepa en su campo
   2  libre en las DOS tablas
   3  un NO con nombre para cada forma de negarlo
   4  se suelta al morir el dueno
   5  una prueba que pueda VER el fallo -- no una que pase
   6  una linea en las tres tablas: kernel, ABI, userland
```

*** **Y el peaje mas caro no es una linea: es lo que se decide NO conceder.**
Ninguna operacion de `KIND_MMIO` acepta una direccion fisica, porque un proceso
que pudiera nombrarla estaria pidiendo ser el kernel. Esa renuncia es lo que hace
que las seis filas signifiquen algo.

---

# 5. LO QUE ESTE TRIANGULO NO PROMETE

- **No corre software de Windows, ni de Mac, ni de Linux.** El vertice de la
  libertad es la PROMESA de compatibilidad hacia adelante, no una capa hacia
  atras.
- **No hay navegador, ni suite ofimatica, ni nada que arrastre millones de lineas
  ajenas.** No es un problema de sistema operativo: es de plantilla. **Un sistema
  que promete Photoshop es un sistema que no termina la calculadora.**
- **El DMA no esta cerrado.** Sin IOMMU, un aparato mal mandado escribe donde
  quiere. Se dice en el vertice de la potencia y se repite aqui.
- **Y los tres vertices no estan terminados.** Estan **empezados y medidos**, que
  es otra cosa -- y la unica que se puede ensenar sin mentir.

---

# El resumen en una frase

> **Coherencia de Mac, promesa de Windows y silicio sin intermediarios: los tres
> caben porque la superficie cabe en una pagina, y esa pagina se cobra.**
