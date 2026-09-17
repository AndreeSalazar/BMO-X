#!/data/data/com.termux/files/usr/bin/bash
# arrancar.sh -- la ANTENA entera con UNA orden, en Termux (2026-09-16).
#
#     bash ~/storage/shared/bmo-antena/arrancar.sh <IP de BMO-X o del PC>
#
# Eddi: *"la idea en Termux necesitaria automatizar eso, para que guie"*.
# Esto hace, en orden, lo que antes eran tres ventanas:
#
#   1. pide a Android que no duerma Termux (termux-wake-lock)
#   2. enciende un Chromium SIN CABEZA con el puerto de depuracion 9222:
#        - el de Termux si esta instalado (`pkg install tur-repo chromium`)
#        - si no, el de un Debian de proot-distro (`proot-distro login debian`)
#        - si no hay ninguno, la antena arranca SIN navegador y PAGINA
#          contesta NO: no se finge
#   3. espera hasta 40 s a que el 9222 conteste
#   4. arranca antena.py con la carpeta de al lado y --navegador 9222
#
# Al salir de la antena (Ctrl+C) apaga el Chromium que encendio. Un Chromium
# que ya estuviera encendido antes se respeta y no se toca.
#
# Se lanza con `bash` y no como ejecutable a proposito: el almacenamiento
# compartido de Android no permite el bit de ejecucion.

set -u
PERMITIR="${1:-}"
if [ -z "$PERMITIR" ]; then
    echo "uso: bash arrancar.sh <IP permitida (BMO-X o el PC)>"
    exit 1
fi
AQUI="$(cd "$(dirname "$0")" && pwd)"
PUERTO=9222
# Sin imagenes: la antena mastica, no muestra, y esperar las imagenes de
# Wikipedia eran 2 de los 7 s del HONOR (medido el 16-09). La lamina sale
# byte a byte igual con o sin ellas.
FLAGS="--headless=new --no-sandbox --disable-gpu --disable-dev-shm-usage --blink-settings=imagesEnabled=false --remote-debugging-port=$PUERTO about:blank"

command -v termux-wake-lock >/dev/null 2>&1 && termux-wake-lock

escucha() {
    python - <<'PY'
import socket, sys
s = socket.socket(); s.settimeout(0.5)
try:
    sys.exit(0 if s.connect_ex(("127.0.0.1", 9222)) == 0 else 1)
finally:
    s.close()
PY
}

# proot-distro escribe "Alias: debian" e "Installed: yes" en DOS lineas, asi
# que no se busca en su listado ni se supone su carpeta: se le pide ENTRAR.
# Si entra, hay Debian.
hay_debian() {
    command -v proot-distro >/dev/null 2>&1         && proot-distro login debian -- true >/dev/null 2>&1
}

# Solo puede haber UNA antena en el 7117; si ya hay otra (otra sesion de
# Termux), se dice quien y como quitarla en vez de un traceback.
if python - <<'PY'
import socket, sys
s = socket.socket(); s.settimeout(0.5)
try:
    sys.exit(0 if s.connect_ex(("127.0.0.1", 7117)) == 0 else 1)
finally:
    s.close()
PY
then
    echo "antena: ya hay una antena en el 7117 (otra sesion de Termux). Quitala con:"
    echo "        pkill -f antena.py"
    exit 1
fi

CHROMIUM_PID=""
if escucha; then
    echo "antena: ya hay un navegador en el $PUERTO, lo uso"
elif command -v chromium >/dev/null 2>&1; then
    echo "antena: enciendo el chromium de Termux"
    chromium $FLAGS > "$HOME/chromium.log" 2>&1 &
    CHROMIUM_PID=$!
elif command -v chromium-browser >/dev/null 2>&1; then
    echo "antena: enciendo chromium-browser"
    chromium-browser $FLAGS > "$HOME/chromium.log" 2>&1 &
    CHROMIUM_PID=$!
elif hay_debian && proot-distro login debian -- sh -c "command -v chromium" >/dev/null 2>&1; then
    # Dentro de proot no hay zygote que valga (no puede crear espacios de
    # nombres), asi que se le dice que no lo intente. El puerto es el mismo:
    # proot comparte la red de Termux.
    echo "antena: enciendo el chromium del Debian de proot"
    proot-distro login debian -- chromium --no-zygote $FLAGS < /dev/null > "$HOME/chromium.log" 2>&1 &
    CHROMIUM_PID=$!
elif hay_debian; then
    echo "antena: hay Debian en proot pero sin chromium (proot-distro login debian -- apt install -y chromium): PAGINA contestara NO"
else
    echo "antena: no hay chromium (ni en Termux ni en proot): PAGINA contestara NO"
    echo "        proot-distro: $(command -v proot-distro || echo 'no esta en el PATH')"
    echo "        login debian: $(proot-distro login debian -- true >/dev/null 2>&1 && echo entra || echo 'no entra (proot-distro list)')"
fi

NAVEGADOR=""
if [ -n "$CHROMIUM_PID" ] || escucha; then
    for i in $(seq 1 40); do
        if escucha; then
            NAVEGADOR="--navegador $PUERTO"
            echo "antena: navegador listo en el $PUERTO ($i s)"
            break
        fi
        sleep 1
    done
    if [ -z "$NAVEGADOR" ]; then
        echo "antena: el navegador no contesto en 40 s (mira ~/chromium.log); sigo sin el"
    fi
fi

apagar() {
    if [ -n "$CHROMIUM_PID" ]; then
        echo; echo "antena: apago el navegador que encendi"
        kill "$CHROMIUM_PID" 2>/dev/null
        pkill -f "remote-debugging-port=$PUERTO" 2>/dev/null
    fi
}
trap apagar EXIT

# shellcheck disable=SC2086
python "$AQUI/antena.py" --carpeta "$AQUI" --permitir "$PERMITIR" $NAVEGADOR
