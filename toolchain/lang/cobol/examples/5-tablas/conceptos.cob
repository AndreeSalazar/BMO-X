      * CONCEPTOS -- totalizar por concepto, que es para lo que existe OCCURS.
      *
      * Lee dos ficheros en paralelo: uno con el CONCEPTO de cada movimiento
      * (1..4) y otro con su IMPORTE. Va sumando cada importe en la casilla de
      * su concepto y al final imprime las cuatro con su mascara.
      *
      * Esto es un cierre por concepto: lo que un banco saca cada noche para
      * saber cuanto se fue en comisiones, cuanto en nomina y cuanto en
      * transferencias. Sin OCCURS habria que declarar TOTAL-1, TOTAL-2,
      * TOTAL-3, TOTAL-4 y escribir el mismo IF cuatro veces.
      *
      * Por que DOS ficheros y no uno con dos columnas: un registro de este
      * COBOL es UN campo con UNA PIC. Partir la linea en columnas es otra
      * cosa (y se dira cuando llegue), asi que aqui cada dato tiene su
      * fichero y su PIC -- que ademas es como se leen las cintas de verdad.
      *
      * El subindice va de 1 a 4 y el compilador lo comprueba: con un literal
      * fuera de rango no compila, y con una variable fuera de rango el
      * programa PARA diciendo la tabla. Seguir con una direccion inventada
      * sumaria en la casilla del vecino, y ese descuadre aparece semanas
      * despues en otro informe.
      *
      * Compilar (el nombre de salida cabe en 8.3 a proposito: el volumen es
      * FAT32 y `apps/conceptos.bex` --nueve letras-- NO se puede cargar):
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/conceptos.cob -o apps/concep.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CONCEPTOS.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CONCS ASSIGN TO "datos/concs.txt".
           SELECT IMPS  ASSIGN TO "datos/imps.txt".
       DATA DIVISION.
       FILE SECTION.
       FD CONCS.
       01 CONCEPTO PIC 9.
       FD IMPS.
       01 IMPORTE  PIC S9(7)V99.
       WORKING-STORAGE SECTION.
       01 TABLA-TOTALES.
           05 TOTAL-CONCEPTO PIC $$$,$$9.99 OCCURS 4 TIMES.
       01 K        PIC 9.
       01 FIN      PIC 9.
       PROCEDURE DIVISION.
           DISPLAY "CIERRE POR CONCEPTO - BANCO BMO".

      *    Las cuatro casillas a cero. La tabla se recorre con el subindice,
      *    que es justo lo que OCCURS viene a permitir.
           MOVE 1 TO K.
           PERFORM UNTIL K > 4
               MOVE 0 TO TOTAL-CONCEPTO(K)
               ADD 1 TO K
           END-PERFORM.

           MOVE 0 TO FIN.
           OPEN INPUT CONCS.
           OPEN INPUT IMPS.
           PERFORM UNTIL FIN = 1
               READ CONCS
                   AT END MOVE 1 TO FIN
                   NOT AT END READ IMPS
                       AT END MOVE 1 TO FIN
                       NOT AT END ADD IMPORTE TO TOTAL-CONCEPTO(CONCEPTO)
                   END-READ
               END-READ
           END-PERFORM.
           CLOSE CONCS.
           CLOSE IMPS.

      *    Y el informe: una linea por concepto, cada una con su mascara.
           DISPLAY "totales por concepto:".
           MOVE 1 TO K.
           PERFORM UNTIL K > 4
               DISPLAY TOTAL-CONCEPTO(K)
               ADD 1 TO K
           END-PERFORM.
           STOP RUN.
