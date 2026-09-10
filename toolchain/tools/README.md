# `tools/` -- los que dicen que NO, y los que fabrican

> Creado el **2026-09-10**. Aqui vivian **veintinueve herramientas y ninguna
> puerta**: cada linea `Validating ...` del build sale de una de ellas, y no
> habia un sitio que dijera cual ni que comprueba.

---

# 1. LA DIVISION, Y NO ES DE GUSTO

```text
   GUARDIANES   corren en cada build y pueden PARARLO. Su trabajo es decir
                que NO -- y una regla solo existe si sabe decirlo (L4)
   OBREROS      fabrican algo: un `.bex`, una fuente, un enlace, un formato.
                Si fallan, no hay salida; no hay nada que juzgar
```

*** La diferencia importa al leerlas: **un guardian que no sabe rechazar nada es
decoracion**, y un obrero que juzga es un obrero que un dia se niega a trabajar.

---

# 2. LOS GUARDIANES -- 16, y que dice que NO cada uno

| herramienta | dice que NO cuando |
|---|---|
| [`ascii-sweep`](ascii-sweep/) | un comentario lleva un byte no-ASCII, **o una ene caida cambia la palabra** -- los dos ejemplos del diccionario van citados aqui con su exencion <!-- ene-caida-adrede --> (`ano`, `sueno`) |
| [`enlaces`](enlaces/) | una cita apunta a un documento que no existe. Y desde el 10-09 tambien avisa de **documentos que no cita nadie** |
| [`censo-modular`](censo-modular/) | un modulo nuevo pasa de 1.000 lineas, o uno de la linea base crecio (L6a: trinquete, no muro) |
| [`casillas`](casillas/) | una casilla de un plan **no dice donde mirar**, o sea que nadie la puede comprobar |
| [`planes`](planes/) | el indice de lo que falta y los planes dejan de decir lo mismo. Ver [`docs/plan/ABIERTO.md`](../../docs/plan/ABIERTO.md) |
| [`avisos`](avisos/) | los avisos del compilador SUBEN |
| [`fases`](fases/) | un fichero de BMO C pierde su `[fase]`, o sea donde APARECE su fallo |
| [`ambitos`](ambitos/) | un commit usa un ambito que no esta en `AMBITOS.txt` |
| [`censo-neutro`](censo-neutro/) | el censo del neutro y el codigo no dicen lo mismo. Ver [`NEUTRO/CENSO.txt`](../../NEUTRO/CENSO.txt) |
| [`perfil-placa`](perfil-placa/) | el perfil de la placa y los rodeos que se le hacen no cuadran |
| [`perfil`](perfil/) | un perfil expone a un fichero que no existe |
| [`perfil-campos`](perfil-campos/) | un campo de un perfil dice un numero y el codigo dice otro |
| [`relevo`](relevo/) | una bandera del traspaso se pierde entre una etapa y la siguiente |
| [`contrato`](contrato/) | cualquiera de sus **20 reglas**, y las 20 estan probadas con 90 casos. Es el mayor de todos |
| [`tamano`](tamano/) | un ejecutable cambia de tamano sin que nadie lo acepte |
| [`procedencia`](procedencia/) | -- corre a mano; no esta en el build |

** El unico que AVISA sin parar el build es la mitad nueva de `enlaces`: un
documento recien escrito esta huerfano un rato por definicion.

---

# 3. LOS OBREROS -- 13

| herramienta | fabrica |
|---|---|
| [`bex-link`](bex-link/) | el `.bex` a partir del binario de Rust |
| [`bmo-linker`](bmo-linker/) | el enlazado propio |
| [`bmo-pack`](bmo-pack/) | mete los recursos DENTRO del `.bex` |
| [`bef-bootstrap`](bef-bootstrap/) | el arranque del formato BEF |
| [`c-gen`](c-gen/) | los ejemplos de C |
| [`cobol-gen`](cobol-gen/) | los de COBOL |
| [`fontgen`](fontgen/) | la fuente de la consola |
| [`maqueta`](maqueta/) | compila un `.maqueta` a Rust |
| [`estratos-fmt`](estratos-fmt/) | formatea el sistema de ficheros propio |
| [`hello-bex`](hello-bex/) | el `.bex` mas pequeno que existe, para probar la puerta |
| [`rpc-demo`](rpc-demo/) | la demostracion de IPC |
| [`vista-ciudad`](vista-ciudad/) | la vista de `bmo-ciudad` |
| [`simbolo`](simbolo/) | la tabla de simbolos |

---

# 4. COMO SE ANADE UN GUARDIAN

```text
   1. un .py con --check / --apply / --dry-run
   2. que imprima una linea que empiece por `clean:` cuando todo va bien
   3. una linea `Guardian` en Ultra_kernel_x86-64/build.ps1
   4. y una fila en la tabla de arriba
```

[!] Y **el paso 0 es demostrar que sabe decir que NO.** Un guardian que se
escribe, se enchufa y sale verde a la primera no ha demostrado nada: puede estar
mirando el sitio equivocado. La cabecera de `casillas.py` cuenta el caso en que
eso paso de verdad -- el primer intento no cazo ninguna de las ocho casillas
malas, y la leccion no fue la que se fue a buscar.

> Un guardian se estrena rompiendo algo a proposito. Si no rompe, no vigila.
