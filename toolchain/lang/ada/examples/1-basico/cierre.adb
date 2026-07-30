--  CIERRE — el primer programa de Ada que BMO-X compila y ejecuta.
--
--  Tres cuotas de 19.99 y una devolucion, en decimal EXACTO. El mismo calculo
--  que hace `hola_COBOL.cob`, y da el mismo numero por la misma razon: el
--  dinero vive en centimos, como enteros, sin coma flotante.
--
--  `type Saldo is delta 0.01 digits 12` no es una convencion ni una libreria:
--  es un TIPO, y el compilador sabe que sus valores son multiplos de 0.01. Eso
--  es el Annex F de Ada (Information Systems), que esta definido sobre las
--  reglas de COBOL — por eso este frontend nacio con el decimal ya resuelto.
--
--  Y no hay runtime detras. `Put_Line` baja a la puerta de la consola y de ahi
--  al unico syscall que existe. Un .bex de Ada no enlaza nada.
--
--  Compilar:
--    cargo run -p bmo-ada-front -- \
--      toolchain/lang/ada/examples/1-basico/cierre.adb -o apps/cierre.bex

with Ada.Text_IO; use Ada.Text_IO;

procedure Cierre is
   type Saldo is delta 0.01 digits 12;

   Total  : Saldo   := 0.00;
   Cuota  : Saldo   := 19.99;
   Vueltas : Integer := 0;
begin
   Put_Line("CIERRE EN ADA - BANCO BMO");

   --  Tres cuotas. 19.99 x 3 = 59.97 EXACTO: ni 59.969999 ni 59.97000001.
   while Vueltas < 3 loop
      Total := Total + Cuota;
      Vueltas := Vueltas + 1;
   end loop;

   Put_Line("total de tres cuotas:");
   Put_Line(Total);

   --  Y una devolucion, para que el signo tambien cruce el camino entero.
   Total := Total - 19.99;
   Put_Line("tras la devolucion:");
   Put_Line(Total);
end Cierre;
