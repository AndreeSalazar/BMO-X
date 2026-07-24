      * Flujo de control REAL en COBOL sobre BMO-X.
      *
      * Hasta ahora este frontend fingia: el IF emitia un salto con
      * desplazamiento cero que nadie parcheaba (ejecutaba las DOS ramas) y
      * el PERFORM emitia `xor rax,rax` repetido, o sea nada. Compilaba y
      * validaba igual. Este programa solo da la salida correcta si el
      * descenso bifurca y repite de verdad.
      *
      * El dinero vive en centavos (PIC 9(5)V99 = escala 2), sin punto
      * flotante: 3 cuotas de 19.99 dan 59.97 EXACTO.
      *
      * Compilar:
      *     cargo run -p bmo-cobol-front -- toolchain/lang/cobol/examples/banco.cob
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BANCO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SALDO   PIC 9(5)V99.
       01 CUOTA   PIC 9(5)V99.
       01 CUENTA  PIC 9(3).
       PROCEDURE DIVISION.
           DISPLAY "BMO-X: caja COBOL".
           MOVE 0 TO SALDO.
           MOVE 19.99 TO CUOTA.

           PERFORM 3 TIMES
               ADD CUOTA TO SALDO
               DISPLAY "cobrada una cuota"
           END-PERFORM.

           IF SALDO = 59.97
               DISPLAY "total exacto: 59.97"
           ELSE
               DISPLAY "el decimal se perdio"
           END-IF.

           MOVE 0 TO CUENTA.
           PERFORM UNTIL CUENTA IS NOT LESS THAN 2
               DISPLAY "recibo emitido"
               ADD 1 TO CUENTA
           END-PERFORM.

           COMPUTE SALDO = SALDO - (CUOTA * 2).
           IF SALDO IS EQUAL TO 19.99
               DISPLAY "dos devoluciones aplicadas"
           ELSE
               DISPLAY "COMPUTE se equivoco"
           END-IF.

           STOP RUN.
