      * COMISION — la TABLA DE DECISION y el REDONDEO LEGAL.
      *
      * Los ocho niveles de antes ensenan lo que un compilador tiene que saber.
      * Este ensena lo que un BANCO tiene que decidir, que no es lo mismo.
      *
      * ── 1. EVALUATE TRUE: el escalado ──
      *
      * Un tramo de comisiones se escribe asi y se lee en voz alta. Cada rama es
      * una condicion entera y LA PRIMERA QUE ACIERTA GANA — por eso van de
      * mayor a menor, y por eso un saldo de 1500 cae en la primera aunque
      * tambien cumpla las dos de abajo.
      *
      * Con IF anidados dice exactamente lo mismo y no se lee. Y quien audita un
      * escalado necesita leerlo, no descifrarlo.
      *
      * ── 2. ROUNDED: el redondeo es una DECISION LEGAL ──
      *
      * El 0,75 % de 133,33 son 0,99999... Sin ROUNDED se guarda 0,99; con
      * ROUNDED, 1,00. Ese centimo no es un detalle de formato.
      *
      * Y hay DOS redondeos aqui a proposito, porque no son el mismo:
      *
      *   ROUNDED                       el clasico: el empate SIEMPRE sube
      *   ROUNDED MODE IS NEAREST-EVEN  el del banquero: el empate va al PAR
      *
      * El clasico tiene SESGO: en una muestra grande los empates siempre suben,
      * o sea que acumulan a favor de quien redondea. Por eso hay
      * jurisdicciones que obligan al del banquero. No es una preferencia de
      * estilo: es de quien responde del cuadre.
      *
      * ── 3. El 88 que le pone nombre al tramo ──
      *
      * `IF ES-PREFERENTE` en vez de `IF SALDO > 1000.00`. El programa dice QUE
      * significa el numero, no cual es.
      *
      * Compilar:
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/9-decision/comision.cob -o apps/comisio.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. COMISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SALDO      PIC S9(7)V99 COMP-3 VALUE ZERO.
           88 ES-PREFERENTE            VALUE 1000.00 THRU 9999999.99.
       01 TASA       PIC S9V9(4)        VALUE ZERO.
       01 COMIS      PIC S9(5)V99 COMP-3 VALUE ZERO.
       01 COMIS-PAR  PIC S9(5)V99 COMP-3 VALUE ZERO.
       01 TRAMO      PIC 9              VALUE ZERO.
       01 LINEA      PIC $$$,$$9.99.
       PROCEDURE DIVISION.

           PERFORM 1000-CABECERA.

           MOVE 1500.00 TO SALDO.
           PERFORM 2000-UN-CLIENTE.

           MOVE 500.00 TO SALDO.
           PERFORM 2000-UN-CLIENTE.

           MOVE 50.00 TO SALDO.
           PERFORM 2000-UN-CLIENTE.

           PERFORM 3000-EL-SESGO.
           STOP RUN.

       1000-CABECERA.
           DISPLAY "BANCO BMO - COMISIONES".
           DISPLAY "----------------------".

       2000-UN-CLIENTE.
      *    ★ LA TABLA DE DECISION. De mayor a menor, y la primera gana.
           EVALUATE TRUE
               WHEN SALDO > 1000.00
                   MOVE 0.0025 TO TASA
                   MOVE 1 TO TRAMO
               WHEN SALDO > 100.00
                   MOVE 0.0050 TO TASA
                   MOVE 2 TO TRAMO
               WHEN OTHER
                   MOVE 0.0075 TO TASA
                   MOVE 3 TO TRAMO
           END-EVALUATE.

      *    El calculo, redondeado. Sin ROUNDED se truncaria, y truncar una
      *    comision siempre en contra del banco tambien es una decision - solo
      *    que tomada sin darse cuenta.
           COMPUTE COMIS ROUNDED = SALDO * TASA.

           DISPLAY "saldo:".
           MOVE SALDO TO LINEA.
           DISPLAY LINEA.
           DISPLAY "tramo:".
           DISPLAY TRAMO.
           DISPLAY "comision:".
           MOVE COMIS TO LINEA.
           DISPLAY LINEA.

           IF ES-PREFERENTE
               DISPLAY "cliente preferente"
           ELSE
               DISPLAY "cliente normal"
           END-IF.

       3000-EL-SESGO.
      *    ★ Los dos redondeos sobre CUATRO EMPATES seguidos.
      *
      *    La suma exacta de 0,005 + 0,015 + 0,025 + 0,035 es 0,08.
      *    El clasico sube los cuatro y da 0,10 - dos centimos de la nada.
      *    El del banquero reparte y da 0,08, que cuadra.
      *
      *    Cuatro empates no son nada. Cuatro millones son dinero.
           DISPLAY "----------------------".
           DISPLAY "el sesgo, con cuatro empates:".
           MOVE 0 TO COMIS.
           MOVE 0 TO COMIS-PAR.

           MOVE 0.005 TO TASA.
           ADD TASA TO COMIS ROUNDED.
           ADD TASA TO COMIS-PAR ROUNDED MODE IS NEAREST-EVEN.
           MOVE 0.015 TO TASA.
           ADD TASA TO COMIS ROUNDED.
           ADD TASA TO COMIS-PAR ROUNDED MODE IS NEAREST-EVEN.
           MOVE 0.025 TO TASA.
           ADD TASA TO COMIS ROUNDED.
           ADD TASA TO COMIS-PAR ROUNDED MODE IS NEAREST-EVEN.
           MOVE 0.035 TO TASA.
           ADD TASA TO COMIS ROUNDED.
           ADD TASA TO COMIS-PAR ROUNDED MODE IS NEAREST-EVEN.

           DISPLAY "clasico (el empate sube):".
           DISPLAY COMIS.
           DISPLAY "banquero (el empate va al par):".
           DISPLAY COMIS-PAR.
           DISPLAY "la suma exacta es 0.08".
