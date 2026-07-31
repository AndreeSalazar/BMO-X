      * CARTERA — el mismo batch, escrito como se LEE en un banco.
      *
      * Lee movimientos y los reparte en tres cubos segun su importe. Todo lo
      * que decide esta escrito con NOMBRES, no con numeros:
      *
      *     PERFORM UNTIL SE-ACABO          en vez de   UNTIL FIN = 1
      *     IF ES-DEVOLUCION                en vez de   IF SIGNO = 2
      *
      * Eso es el nivel 88: no reserva ni un byte, le pone nombre a una
      * comparacion. La prueba esta en el compilador — declarar veinte no
      * cambia una sola instruccion del .bex.
      *
      * Y es la razon por la que COBOL se lee en voz alta en una auditoria:
      * quien revisa esto no tiene que acordarse de que el 2 significaba
      * devolucion. Lo dice el programa.
      *
      * El fichero trae un importe por linea. Negativo = devolucion.
      *
      * Compilar:
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/6-condiciones/cartera.cob -o apps/carter.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CARTERA.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT MOVIM ASSIGN TO "datos/movim.txt".
       DATA DIVISION.
       FILE SECTION.
       FD MOVIM.
       01 IMPORTE  PIC S9(7)V99.
       WORKING-STORAGE SECTION.
       01 FIN      PIC 9.
           88 SE-ACABO       VALUE 1.
       01 CUANTOS  PIC 9(5).
           88 NO-HUBO-NADA   VALUE 0.
       01 COBROS   PIC $$$,$$9.99.
      * Las devoluciones llevan CR: son un saldo en contra, y una mascara sin
      * signo las imprimiria como si fueran cobros. `$$$,$$9.99` se come el
      * menos — correcto segun el estandar, y mentira en un informe.
       01 DEVOLS   PIC $$$,$$9.99CR.
       PROCEDURE DIVISION.
           DISPLAY "CARTERA DEL DIA - BANCO BMO".

           MOVE 0 TO FIN.
           MOVE 0 TO CUANTOS.
           MOVE 0 TO COBROS.
           MOVE 0 TO DEVOLS.

           OPEN INPUT MOVIM.
      *    Se lee "hasta que se acabo", que es lo que hace.
           PERFORM UNTIL SE-ACABO
               READ MOVIM
                   AT END MOVE 1 TO FIN
                   NOT AT END ADD 1 TO CUANTOS
                       IF IMPORTE < 0
                           ADD IMPORTE TO DEVOLS
                       ELSE
                           ADD IMPORTE TO COBROS
                       END-IF
               END-READ
           END-PERFORM.
           CLOSE MOVIM.

      *    Y el informe. El caso de "no hubo movimientos" tiene su nombre: en
      *    un cierre nocturno, un fichero vacio y un fichero que no se pudo
      *    leer se parecen demasiado si los dos imprimen ceros callando.
           IF NO-HUBO-NADA
               DISPLAY "sin movimientos hoy"
           ELSE
               DISPLAY "cobros:"
               DISPLAY COBROS
               DISPLAY "devoluciones:"
               DISPLAY DEVOLS
           END-IF.
           STOP RUN.
