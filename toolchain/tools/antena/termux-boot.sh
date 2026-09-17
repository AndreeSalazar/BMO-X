#!/data/data/com.termux/files/usr/bin/bash
# termux-boot.sh -- la antena SIEMPRE ACTIVA: arranca sola al encender el movil.
#
# Eddi (2026-09-17): *"un comando simple que active en python siempre activo
# como para que mi BMO-X reconozca"*. Esto es ese comando, y no hace falta
# escribirlo nunca: lo lanza Android al arrancar.
#
# == Como se instala (una vez) ==
#
#   1. Instala la app Termux:Boot (de F-Droid, la misma tienda que Termux) y
#      abrela UNA vez para que Android le de permiso de arrancar.
#   2. En Termux:
#
#        mkdir -p ~/.termux/boot
#        cp ~/storage/shared/bmo-antena/termux-boot.sh ~/.termux/boot/antena.sh
#        chmod +x ~/.termux/boot/antena.sh
#
#      (aqui SI hace falta el bit de ejecucion, y ~/.termux esta en la memoria
#      privada de Termux, donde se puede)
#   3. Escribe abajo, en PERMITIR, la IP de BMO-X o del PC que puede pedir.
#   4. Reinicia el movil. A los pocos segundos la antena esta en el 7117, con
#      el Chromium encendido, y BMO-X la encuentra sin que toques nada.
#
# == Lo que NO hace ==
#
#   - No enciende el anclaje por USB ni cambia el modo de USB: eso es de
#     Android y sin root no se puede desde aqui. Se hace UNA vez en Ajustes >
#     Opciones de desarrollador > Configuracion USB predeterminada > Anclaje
#     USB, y desde entonces el movil se presenta asi al enchufarlo.
#   - No abre nada a Internet: la antena solo contesta a PERMITIR.
#
# El registro queda en ~/antena-boot.log por si un dia no arranca.

# La IP que puede pedir (BMO-X o el PC). Sin ella la antena no arranca.
PERMITIR=""

if [ -z "$PERMITIR" ]; then
    echo "termux-boot: falta PERMITIR en ~/.termux/boot/antena.sh" >> "$HOME/antena-boot.log"
    exit 1
fi

termux-wake-lock
# El almacenamiento compartido tarda un poco en montarse tras el arranque.
for i in $(seq 1 30); do
    [ -f "$HOME/storage/shared/bmo-antena/arrancar.sh" ] && break
    sleep 1
done
echo "termux-boot: $(date) arranco la antena para $PERMITIR" >> "$HOME/antena-boot.log"
exec bash "$HOME/storage/shared/bmo-antena/arrancar.sh" "$PERMITIR" >> "$HOME/antena-boot.log" 2>&1
