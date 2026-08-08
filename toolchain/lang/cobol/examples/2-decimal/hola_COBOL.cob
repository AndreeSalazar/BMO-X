      * hola_COBOL.bex -- el programa que BMO-X ejecuta en el Ryzen real.
      *
      * Cada seccion prueba algo que estaba FINGIENDO hasta esta semana:
      *
      *   IF/ELSE   emitia un salto con desplazamiento cero: ejecutaba las
      *             DOS ramas, siempre
      *   PERFORM   emitia `xor rax,rax` repetido, o sea nada
      *   operandos todo se tomaba como literal: ADD CUOTA TO SALDO sumaba 0
      *   COMPUTE   parseaba la expresion entera como numero y guardaba 0
      *
      * El dinero vive en centavos (PIC 9(5)V99), sin punto flotante: es la
      * razon por la que COBOL sigue vivo en un banco y la razon por la que
      * esta en BMO.
      *
      * Compilar:
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/hola_COBOL.cob \
      *     -o Ultra_kernel_x86-64/kernel/src/ring0/hola_COBOL.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HOLA-COBOL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SALDO   PIC 9(5)V99.
       01 CUOTA   PIC 9(5)V99.
       01 CARGO   PIC 9(3).
       01 VUELTA  PIC 9(3).
       PROCEDURE DIVISION.
           DISPLAY "hola desde COBOL en el Ryzen".

           MOVE 0 TO SALDO.
           MOVE 19.99 TO CUOTA.
           PERFORM 3 TIMES
               ADD CUOTA TO SALDO
           END-PERFORM.

           IF SALDO = 59.97
               DISPLAY "3 x 19.99 = 59.97 exacto"
           ELSE
               DISPLAY "el decimal se perdio"
           END-IF.

      *    Escalas distintas: un entero sumado a centavos debe reescalar.
           MOVE 5 TO CARGO.
           ADD CARGO TO SALDO.
           IF SALDO = 64.97
               DISPLAY "cargo entero aplicado bien"
           ELSE
               DISPLAY "mezclo pesos con centavos"
           END-IF.

           MOVE 0 TO VUELTA.
           PERFORM UNTIL VUELTA IS NOT LESS THAN 2
               DISPLAY "recibo emitido"
               ADD 1 TO VUELTA
           END-PERFORM.

           COMPUTE SALDO = SALDO - (CUOTA * 2).
           IF SALDO IS EQUAL TO 24.99
               DISPLAY "dos devoluciones aplicadas"
           ELSE
               DISPLAY "COMPUTE se equivoco"
           END-IF.

           DISPLAY "COBOL termino ok".
           STOP RUN.
