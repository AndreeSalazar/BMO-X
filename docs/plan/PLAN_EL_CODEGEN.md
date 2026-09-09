# PLAN EL CODEGEN -- 35 instrucciones para escribir 8 bytes

> Escrito el **2026-09-09**, persiguiendo por que la expansion de DOOM costaba
> 6.738 us por fotograma. La cuenta no cuadraba por un factor de cien, y la
> respuesta no estaba en DOOM.
>
> ⚠ **Este documento no propone tocar nada todavia.** El backend de C lo usan
> todos los `.bex` del sistema, y este mes ya se pagaron cinco fallos de codegen.
> Aqui esta la MEDIDA y el orden en que se arreglaria. La decision es del dueno.

---

# 1. LA MEDIDA QUE LO ABRIO

DOOM en el Ryzen, escala x5 a 1600x1000, en pleno juego:

```text
   fotograma         14.948 us     (66 fps)
     blit             7.833 us     -> 52 % del fotograma
       expansion      6.738 us     -> 86 % del blit
       volcado        1.063 us     -> 14 % del blit
```

★ **El volcado es excelente**: 6,4 MB en 1.063 us son **6,0 GB/s** hacia memoria
write-combining. Ese camino es `memcpy`, o sea `rep movsb`, y no tiene nada que
mejorar.

La expansion escribe una fila de 1600 px en RAM normal, 200 veces por fotograma:

```text
   por pixel de origen   2 escrituras de 8 B + 1 de 4 B  =  3
   por fotograma         320 x 200 x 3                   =  192.000 escrituras
   medido                6.738 us x 4.488 MHz            =  30,2 M ciclos
   ------------------------------------------------------------------------
   ciclos por escritura util                                157
```

**157 ciclos para escribir 8 bytes en memoria cacheada.** Eso no lo hace ni un
CPU con la cache apagada.

---

# 2. ★★★ EL DESENSAMBLADO, Y NO ES DOOM

Se compilo `expandir_fila` con el propio BMO C y se desensamblo. El bucle
interior del fuente es este:

```c
    while (j > 0) { *d8 = par; d8 = d8 + 1; j = j - 1; }
```

Y sale esto -- **35 instrucciones, 122 bytes, de las cuales UNA hace el trabajo**:

```text
   movslq -0x48(%rbp),%rax     ; j, de la pila
   pushq  %rax
   movabsq $0x0,%rax           ; DIEZ BYTES para cargar el cero
   popq   %rdx
   cmpq   %rax,%rdx
   setg   %al
   movzbq %al,%rax
   testl  %eax,%eax
   je     salida
   movq   -0x10(%rbp),%rax     ; par
   pushq  %rax
   movq   -0x8(%rbp),%rax      ; d8
   popq   %rdx
   movq   %rdx,(%rax)          ; *** EL TRABAJO. Una instruccion.
   movq   %rdx,%rax
   movq   -0x8(%rbp),%rax
   pushq  %rax
   movabsq $0x1,%rax           ; el 1
   pushq  %rax
   movabsq $0x8,%rax           ; el sizeof
   popq   %rdx
   imulq  %rdx,%rax            ; *** UN IMUL PARA MULTIPLICAR 1 x 8
   movslq %eax,%rax
   popq   %rdx
   addq   %rdx,%rax
   movq   %rax,-0x8(%rbp)
   ... y otras nueve para `j = j - 1`
   jmp    arriba
```

```text
   utiles                  1
   push/pop a memoria      8
   movabsq de 10 bytes     3     (para los literales 0, 1 y 8)
   imulq                   1     (en la cadena de dependencias)
   ------------------------------------
   razon ruido / trabajo   35 a 1
```

★★ Con 8 accesos a memoria y un `imul` dentro de la cadena, **157 ciclos por
escritura deja de ser una anomalia y pasa a ser la cuenta exacta.**

## Y lo que duele de verdad

El autor del puerto de DOOM **ya habia optimizado esto a mano**, y lo dejo
escrito en el fichero:

> *"Aqui no hay indices: hay dos punteros que caminan. [...] cada una paga un
> `imul` para el indice, una lectura de la global y tres accesos a la pila."*

*** Quito el `imul`, y **el compilador lo puso otra vez** -- ahora para calcular
`1 * 8`, el tamano del tipo, en tiempo de ejecucion y en cada vuelta.

> Una optimizacion escrita en C que el generador de codigo deshace no es una
> optimizacion: es un comentario.

---

# 3. QUE ES ESTO, POR SU NOMBRE

BMO C emite como una **maquina de pila**: cada expresion pasa por `rax`, los
operandos van y vienen por `push`/`pop`, y cada variable local vive en su hueco
de `%rbp`. No hay asignacion de registros y no hay plegado de constantes.

[!] **Y eso no fue un error**: es lo que hace que un compilador entero quepa y se
pueda razonar. La deuda no es haberlo escrito asi -- es que **nunca se midio lo
que cuesta**, y por eso la cifra aparece hoy, por sorpresa, dentro de DOOM.

★ Alcance: **todo `.bex` de C y de C++ del sistema**. COBOL y Ada tienen sus
propios frontends; INTI tiene su propio emisor (`inti/emisor-x86_64`) y **no pasa
por aqui**, que es exactamente lo que se decia de INTI cuando se escribio.

---

# 4. LA ESCALERA, DE MAS BARATO A MAS CARO

Ninguno de los tres primeros es un asignador de registros. Son **mirillas**:
miran dos o tres instrucciones seguidas y las sustituyen.

- [ ] **C1 -- PLEGAR CONSTANTES.** `1 * 8` es `8`. Un operador binario con los
      dos operandos constantes se resuelve al compilar. Quita el `imul` de la
      cadena y dos `movabsq`.
      ★ Es el mas barato de los cuatro y el que mas quita del camino caliente.

- [ ] **C2 -- LITERALES PEQUENOS SIN `movabsq`.** `movabsq $0x1,%rax` son diez
      bytes para un uno. `movl $1,%eax` son cinco, y como operando inmediato de
      la instruccion que lo usa, cero.
      ⚠ **Sacrificio**: hay que distinguir el ancho, y ensanchar mal un literal
      es **el fallo que este mes se pago cinco veces** (`bmo-c-compilador-culpable`).
      No se toca sin una casilla del censo que lo ejecute en el emulador.

- [ ] **C3 -- NO PASAR POR LA PILA CUANDO EL OTRO OPERANDO ES CONSTANTE.**
      `push; movabs; pop; add` es `addq $imm, %rax`. Quita 4 de los 8 accesos a
      memoria del bucle de arriba.

- [ ] **C4 -- MANTENER EN REGISTRO LA VARIABLE DE UN BUCLE.** Aqui ya no es una
      mirilla: es analisis de vivos, y es otro proyecto.
      ⚠ Y su sacrificio es el que importa: **es donde un compilador deja de
      poder leerse de una sentada**, que es la propiedad por la que existe este.

## ★★ Como se mide cada escalon, y no de oidas

```text
   1. `expandir_fila` compilada y desensamblada -> se CUENTAN las instrucciones
   2. el emulador (`bmo_lower::emu`) la ejecuta -> se CUENTAN las ejecutadas
   3. DOOM en el Ryzen                          -> `expansion N us` en su [perf]
```

Los tres numeros existen ya. El tercero es el juez y los otros dos son la
sonda -- el mismo metodo que exonero al asignador en la semana del 01 al 04-09.

---

# 5. LO QUE ESTE PLAN NO PROMETE

```text
   [ ] no promete que DOOM vaya al doble. La expansion es el 45% del
       fotograma: quitarle tres cuartas partes son ~5 ms de 15, o sea de 66
       a ~95 fps. Bueno, y no es "el doble"
   [ ] no toca el asignador de registros. C1..C3 son mirillas; C4 es otro
       proyecto y esta escrito aparte por eso
   [ ] no arregla nada de INTI ni de COBOL ni de Ada: no comparten emisor
   [ ] y no empieza sin que el dueno lo diga. Este backend lo usan todos los
       `.bex`, y el precio de equivocarse aqui ya esta escrito en la memoria
       de la casa con nombre y fecha
```

> El puerto de DOOM quito un `imul` a mano y el compilador lo devolvio. **Antes
> de optimizar en C hay que saber que el generador respeta lo que se le escribe**
> -- y ahora, por primera vez, hay un numero que dice cuanto no lo respeta.
