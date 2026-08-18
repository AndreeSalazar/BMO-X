      * El MOTOR de la calculadora con botones. Sin adornos y sin preguntas.
      *
      * Lee tres numeros y escribe DOS lineas. Nada mas. Ni un "introduzca el
      * primer operando", ni una cabecera: quien lo llama es un programa, no una
      * persona, y todo lo que salga por aqui es la respuesta.
      *
      *     entra:  operando1 \n  codigo \n  operando2 \n
      *     sale:   estado    \n  valor   \n
      *
      *     estado: 0  lo que sigue ES el resultado
      *             1  no se contestar, y lo que sigue dice por que
      *
      *     codigo: 1 sumar   2 restar   3 multiplicar   4 dividir
      *             5 tanto por ciento -- DOS por ciento de UNO
      *             6 presentar UNO como DINERO
      *
      * * POR QUE EL ESTADO VA EN SU PROPIA LINEA.
      *
      * Antes salia una sola linea y quien la leia daba por hecho que era un
      * numero. Con la tecla `$` eso deja de valer: `$12,345.67` ES una
      * respuesta buena y NO parece un numero, asi que "adivinar mirando" ya no
      * puede funcionar ni siquiera mal. Y `DISPLAY` de este COBOL admite UN
      * operando, asi que el estado no se puede pegar delante: va en su linea.
      *
      * Lo que se gana no es solo eso. Antes, un codigo que este programa no
      * reconociera salia como texto por el mismo sitio que un importe, y el
      * escritorio lo pintaba en la pantallita como si fuera una cifra.
      *
      * * La cara la compila MAQUETA desde `calc.maqueta`. El CALCULO es esto,
      * en COBOL, con decimal exacto en centavos. Es la separacion que Windows
      * no hace --su calculadora lleva el motor dentro-- y es la que permite
      * cambiar la una sin tocar la otra.
      *
      * Compilar:
      *     cargo run -p bmo-cobol-front -- toolchain/lang/cobol/examples/2-decimal/calcgui.cob
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALCGUI.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 UNO    PIC S9(9)V99.
       01 DOS    PIC S9(9)V99.
       01 COD    PIC 9.
       01 RES    PIC S9(9)V99.
       01 VALE   PIC 9.
      *    El campo de PRESENTACION de la tecla `$`. Guarda lo mismo que RES;
      *    se ensena distinto. La mascara se gasta AL COMPILAR: en el .bex no
      *    queda ni la plantilla ni un interprete que la lea.
      *
      *    * Es ANCHA a proposito -- nueve digitos enteros, los mismos que
      *    RES. Con `$$$,$$9.99` cabrian cinco, y lo que no cabe en una PICTURE
      *    se pierde POR ARRIBA sin decirlo: 123456.78 saldria como $3,456.78.
      *    Un importe mal por un factor de cien es peor que no contestar.
       01 L-RES  PIC $$$,$$$,$$9.99.
       PROCEDURE DIVISION.
           ACCEPT UNO.
           ACCEPT COD.
           ACCEPT DOS.

           MOVE 0 TO RES.
      *    Nadie ha reconocido todavia el codigo. Si nadie lo hace, se dice.
           MOVE 0 TO VALE.

           IF COD = 1
               COMPUTE RES = UNO + DOS
               MOVE 1 TO VALE
           END-IF.
           IF COD = 2
               COMPUTE RES = UNO - DOS
               MOVE 1 TO VALE
           END-IF.
           IF COD = 3
               COMPUTE RES = UNO * DOS
               MOVE 1 TO VALE
           END-IF.

      *    * DIVIDIR ENTRE CERO SE CONTESTA ANTES DE INTENTARLO.
      *
      *    No es prudencia: es que la division no tiene resultado y cualquier
      *    numero que saliera de aqui seria inventado. Se pregunta con `AND`
      *    para no tocar las otras tres, donde un cero es un operando normal.
           IF COD = 4 AND DOS = 0
               MOVE 2 TO VALE
           END-IF.
           IF COD = 4 AND DOS NOT = 0
               COMPUTE RES = UNO / DOS
               MOVE 1 TO VALE
           END-IF.

      *    El tanto por ciento: DOS por ciento de UNO. `200 % 10` son 20.
      *    En centavos exactos, que es de lo que va este lenguaje -- el 21% de
      *    una factura no puede depender de como redondee una coma flotante.
           IF COD = 5
               COMPUTE RES = UNO * DOS / 100
               MOVE 1 TO VALE
           END-IF.

      *    La tecla `$`: no calcula, PRESENTA. Por eso no mira a DOS.
           IF COD = 6
               MOVE UNO TO RES
               MOVE 3 TO VALE
           END-IF.

           IF VALE = 1
               DISPLAY "0"
               DISPLAY RES
           END-IF.
           IF VALE = 3
               MOVE RES TO L-RES
               DISPLAY "0"
               DISPLAY L-RES
           END-IF.
           IF VALE = 2
               DISPLAY "1"
               DISPLAY "no se divide entre cero"
           END-IF.
           IF VALE = 0
               DISPLAY "1"
               DISPLAY "ese codigo no lo conozco"
           END-IF.
           STOP RUN.
