      * CIERRE — el batch escrito COMO SE ESCRIBE DE VERDAD.
      *
      * Los siete ejemplos anteriores son programas: una lista de sentencias de
      * arriba abajo. Ningun COBOL de banca esta escrito asi. Un batch de
      * verdad tiene un CUERPO PRINCIPAL que se lee en voz alta -cuatro
      * PERFORM- y el trabajo repartido en PARRAFOS con nombre y numero.
      *
      *     PERFORM 1000-INICIO.
      *     PERFORM 2000-PROCESO UNTIL SE-ACABO.
      *     PERFORM 3000-CIERRE.
      *     STOP RUN.
      *
      * Eso de arriba es el programa entero. Se entiende sin leer una linea
      * mas, y quien audite el cierre puede mirar SOLO el paso que le interesa.
      * Esa es la razon por la que COBOL se lee: no es el ingles, son los
      * parrafos.
      *
      * Lo que hace falta que el compilador sepa, y que no sabia hasta hoy:
      *
      *   1. NOMBRES DE PARRAFO. Una palabra sola con punto abre uno nuevo.
      *
      *   2. PERFORM <parrafo>, que llama Y VUELVE. La vuelta no es un `ret`
      *      fijo: cada parrafo pregunta EN EJECUCION si es aqui donde habia
      *      que volver, porque el mismo parrafo puede ser el final de un rango
      *      en una linea y estar en medio de otro en la de abajo.
      *
      *   3. PERFORM ... THRU ..., que recorre TODOS los parrafos del rango.
      *      Aqui: 4000-VALIDA THRU 4000-SALIR, tres parrafos de una llamada.
      *
      *   4. PERFORM <parrafo> UNTIL <cond>, que es EL bucle de un batch: el
      *      parrafo lee un registro y el UNTIL mira si se acabo.
      *
      *   5. VALUE, que inicializa. Antes se parseaba y no se emitia nunca, asi
      *      que TOTAL arrancaba con lo que hubiera en la pila.
      *
      *   6. OR en las condiciones, y con el los 88 con THRU y con varios
      *      valores: `88 GRANDE VALUE 500.00 THRU 99999.99`.
      *
      *   7. STOP RUN, que ahora TERMINA. Antes no emitia nada y colaba porque
      *      siempre era la ultima linea; con parrafos detras, no emitir nada
      *      significaba caerse dentro del primero y ejecutarlo otra vez.
      *
      * Compilar:
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/8-parrafos/cierre.cob -o apps/cierre.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CIERRE.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT MOVIM ASSIGN TO "datos/movim.txt".
       DATA DIVISION.
       FILE SECTION.
       FD MOVIM.
       01 IMPORTE   PIC S9(7)V99 COMP-3.
       WORKING-STORAGE SECTION.
       01 TOTAL     PIC S9(9)V99 COMP-3 VALUE ZERO.
       01 CUANTOS   PIC 9(5)             VALUE ZERO.
       01 GRANDES   PIC 9(5)             VALUE ZERO.
       01 ABONOS    PIC 9(5)             VALUE ZERO.
       01 FIN       PIC 9                VALUE ZERO.
           88 SE-ACABO                   VALUE 1.
       01 VALE      PIC 9                VALUE ZERO.
           88 ES-BUENO                   VALUE 1.
       01 LINEA     PIC $$$,$$9.99.
       PROCEDURE DIVISION.

      *    ── EL PROGRAMA ENTERO, y se lee en voz alta ──
           PERFORM 1000-INICIO.
           PERFORM 2000-PROCESO UNTIL SE-ACABO.
           PERFORM 3000-CIERRE.
           STOP RUN.

       1000-INICIO.
           DISPLAY "BANCO BMO - CIERRE DEL DIA".
           DISPLAY "--------------------------".
           OPEN INPUT MOVIM.

       2000-PROCESO.
           READ MOVIM
               AT END MOVE 1 TO FIN
               NOT AT END PERFORM 4000-VALIDA THRU 4000-SALIR
           END-READ.

      *    ── El RANGO: un PERFORM entra por 4000-VALIDA y no vuelve hasta
      *    pasar por 4000-SALIR, recorriendo los tres parrafos seguidos.
      *
      *    ★ El descarte se hace con un INTERRUPTOR y no con un GO TO. En el
      *    COBOL de los sesenta esto seria `GO TO 4000-SALIR`; aqui no hay GO
      *    TO todavia, y fingirlo con un PERFORM seria MENTIR: un PERFORM del
      *    parrafo de salida lo ejecuta y VUELVE, o sea que el trabajo de
      *    debajo se haria igual. Se escribe lo que de verdad pasa.
       4000-VALIDA.
           MOVE 0 TO VALE.
           IF IMPORTE NOT = 0
               MOVE 1 TO VALE
           END-IF.

       4100-CUENTA.
           IF ES-BUENO
               ADD IMPORTE TO TOTAL
               ADD 1 TO CUANTOS
      *        El OR: un movimiento llama la atencion por ser GRANDE o por ser
      *        un ABONO. Sin OR, esto son dos IF y la condicion deja de leerse.
               IF IMPORTE > 500.00 OR IMPORTE < 0
                   IF IMPORTE < 0
                       ADD 1 TO ABONOS
                   ELSE
                       ADD 1 TO GRANDES
                   END-IF
               END-IF
           END-IF.

       4000-SALIR.
           EXIT.

       3000-CIERRE.
           CLOSE MOVIM.
           DISPLAY "movimientos contados:".
           DISPLAY CUANTOS.
           DISPLAY "de mas de 500:".
           DISPLAY GRANDES.
           DISPLAY "abonos:".
           DISPLAY ABONOS.
           DISPLAY "total del dia:".
           MOVE TOTAL TO LINEA.
           DISPLAY LINEA.
