# BMO C++ -- APARCADO, no borrado

> Decision del dueno, **2026-08-12**. Igual que Vulkan: no se toca, y existe
> para no tener que reconstruirlo.

---

# 1. LOS NUMEROS QUE LLEVARON A ESTO

Medidos el 2026-08-12, no recordados:

```text
   toolchain/lang/cpp          3.874 lineas
   dependientes del crate      CERO
   lo construye build.ps1      no aparece ni una vez
   .bex de C++ desplegados     CERO
   tests                       23, y el UNICO rojo de toda la suite
```

Encaja fila por fila con los seis crates que se borraron el 2026-08-02 por la
regla de la casa --**cablear o borrar**-- y sin embargo aqui no se borra. El
motivo esta en la parte 3.

---

# 2. POR QUE NO SE SIGUE HOY

## 2.1 -- "Librerias para C" no puede existir todavia

La idea que lo mantenia vivo era esa: que C++ sirviera para escribir librerias
que los programas de C usaran. **Hoy eso no es posible, y no por el compilador.**

BMO C tiene **una sola unidad de traduccion** y no hay enlazado. Una "libreria"
que hay que pegar entera dentro del mismo fichero fuente no es una libreria: es
un copiar-pegar con otro nombre.

O sea que el bloqueante de "C++ para librerias" **no es C++**: es la compilacion
separada, que esta en `docs/QUE_DESBLOQUEA.md` como la palanca 2.

## 2.2 -- Heredaria una ruta que nadie ha visto funcionar

`docs/AVANCES.md` ya lo dice, y lo dice antes que esto:

> *"SSE en el emulador va delante de C++ a proposito, porque es barato y tapa un
> agujero que YA existe: hoy la ruta de coma flotante de BMO C tiene 9 tests y
> **ninguno la ejecuta**. Ademas C++ hereda esa ruta entera."*

Construir un lenguaje encima de un camino que nadie ha ejecutado es apilar.

## 2.3 -- Y no es lo que desbloquea aplicaciones

De `docs/QUE_DESBLOQUEA.md`, y es del dueno:

> *"C++ no desbloquea aplicaciones. Lo que desbloquea aplicaciones es la
> **superficie del sistema**."*

Las cuatro palancas por delante --SDL, compilacion separada, el asignador, la
red-- **ninguna pide C++**.

---

# 3. POR QUE NO SE BORRA

Porque a diferencia de `bmo-nvme` --un driver para un disco que esta prohibido
tocar-- **este si tiene destinatario declarado y escrito**:

- `AVANCES.md`: *"BMO C++ (esencial, ACOTADO) -- SIGUIENTE lenguaje; barato
  encima de C porque hereda todo"*, con su lista de lo que entra (clases, RAII,
  referencias, vtables, namespaces, templates basicos) y lo que **no** (concepts,
  coroutines, modules, ranges, la STL gigante).
- `QUE_DESBLOQUEA.md`: el mejor retorno del lenguaje es **Dear ImGui** (~40k
  lineas), una GUI de herramientas sobre el framebuffer crudo.

Y 3.874 lineas de parser de C++ son meses. El historial de git no olvida, pero
**recuperar de un commit y retomar un diseno no son la misma operacion**.

---

# 4. LAS DOS CONDICIONES QUE LO REVIVEN

No "algun dia". Dos, concretas, y **las dos comprobables**:

| # | condicion | como se sabe que esta |
|---|---|---|
| 1 | **SSE ejecutado en el emulador** | los 9 tests de coma flotante de BMO C dejan de ser verdes-sin-ejecutar |
| 2 | **Compilacion separada** | dos `.c` producen un `.bex` sin pegarlos a mano |

Con las dos puestas, C++ pasa de "un parser sin destino" a "el lenguaje con el
que se escribe ImGui". Sin ellas, cada linea que se le anada es deuda.

★ Y hay una tercera que no es condicion sino aviso: **el dueno lo dudaba desde
julio.** *"C++ Eddi lo duda el mismo"*, escrito el 2026-07-28. Aparcarlo no
contradice nada: lo pone por escrito.

---

# 5. EL ESTADO EXACTO EN QUE SE DEJA

Lo que **funciona** (22 de 23 tests): el parser, clases y structs, ctor/dtor,
referencias, sobrecarga, herencia con vtables, namespaces.

Lo que **no** -- `matriz_cpp_ejecuta_correctamente`, 108 de 110 filas:

```text
   un global lee 0 donde deberia leer 42
   un literal de cadena indexado sale vacio
```

[!] **Los dos sintomas son exactamente los que ya se arreglaron en BMO C**: la
seccion de datos y las relocations (`2bc13367`, `46506e51`). O sea que **no es un
bug del C++: es que el frontend de C++ no recibio el arreglo del de C**, y lleva
asi desde el 08-08.

Ese test queda **`#[ignore]` con el motivo dentro**, y no se borra. Un rojo
permanente entrena a no mirar los rojos --ya escondio 400 tests una vez-- y un
test borrado hace desaparecer la unica descripcion que existe del fallo.

Quitar el `#[ignore]` es el **primer paso** del dia que se retome: dice
exactamente que falta.

---

# 6. LO QUE ESTE DOCUMENTO NO PROMETE

- **Que se retome.** Puede que las dos condiciones se cumplan y C++ siga sin
  hacer falta, porque para entonces SDL haya traido lo que se queria.
- **Que el diseno siga siendo el correcto.** Esta escrito para el BMO de agosto
  de 2026; si el ABI cambia, este parser habla con un sistema que ya no existe.
- **Que 22 tests verdes signifiquen que funciona.** Significan que *ese* camino
  funciona en el ANFITRION. Ningun `.bex` de C++ ha tocado un CPU jamas.

---

Ver `AVANCES.md` (el alcance acotado), `docs/QUE_DESBLOQUEA.md` (por que no es
la palanca) y `platform/drivers/gpu/rdna4/PLAN_VULKAN.md`, que es el precedente
de aparcar bien.
