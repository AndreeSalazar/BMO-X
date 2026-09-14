# PLAN SATELITE -- el movil es la ANTENA, BMO-X es la pantalla

> Escrito el **2026-09-14**, el dia que `red ping` llego a Internet desde el
> Ryzen. Lo pidio el dueno despues de chocar con el muro de la web:
>
> > *"mi celular Android se convierte en antena y eso puedo ver YouTube, por
> > completo; BMO ya no navega pero si la ANTENA. Es como Steam, si quiero jugar
> > en mi PC"*.
>
> Y la respuesta es que si: es el mismo reparto que Steam Link / Remote Play. Un
> aparato hace el trabajo pesado y otro solo ensena lo que le llega. Aqui el
> pesado es el movil y la pantalla es BMO-X.

---

# 0. POR QUE ESTO Y NO UN NAVEGADOR DENTRO DE BMO-X

Ver un video de la web desde BMO-X solo pide, a la vez:

```text
   HTTPS          TLS 1.3 -- G6 de PLAN_RED_TX, el muro
   la web         JavaScript, que las plataformas de video exigen
   el codec       VP9 / AV1 / H.264 en software: miles de lineas de lo mas duro
   el audio       Opus / AAC, sincronizado
```

Cada una son meses. **La antena se come las cuatro**: habla con la web como
cualquier movil y le entrega a BMO-X algo que BMO-X ya sabe tratar.

```text
   ANTENA (Android)                                     BMO-X (el Ryzen)
   ----------------------------------                   ----------------------
   la web, HTTPS, JavaScript, el codec   --  LAN, TCP  --> recibe MPEG-1 y MP2
   moderno; lo convierte a MPEG-1 +          sin cifrar     y lo pinta con
   MP2 640x360, que pesa ~1,5 Mbit/s         solo en casa   pl_mpeg en su ventana
```

** Con el enlace a 10 Mbit de hoy entra de sobra. Sin comprimir (640x360 RGB a
30 fps, unos 160 Mbit/s) NO entraria: por eso la antena convierte y no manda la
pantalla tal cual.

---

# 1. LO QUE ESTE PLAN SE NIEGA A PROMETER

```text
   [!] sin antena encendida no hay video: BMO-X NO navega, y no finge que si
   [!] el canal va SIN CIFRAR. Vale dentro de casa y con UNA sola IP permitida;
       fuera de la LAN, no
   [!] las plataformas de video tienen condiciones de uso: descargar o convertir
       su contenido puede incumplirlas. Las pruebas se hacen con videos propios
       o de licencia libre, y lo que haga la antena es decision de su dueno
   [!] el modo ESPEJO (seccion 3, S6) cuesta mucho mas que el modo PEDIDO: no se
       empieza por el
   [!] el kernel sigue sin saber lo que es una IP: la lista de "solo la antena"
       vive en Ring 3, en la app, no en el grifo
```

---

# 2. LOS DOS MODOS

```text
   PEDIDO (primero)   BMO-X dice "dame ESTE video"; la antena lo trae, lo
                      convierte y lo sirve. Basta con Termux en el movil.
   ESPEJO (despues)   la antena manda lo que tiene en pantalla, en vivo, y BMO-X
                      le devuelve teclado y raton. Es Steam Link al reves, y pide
                      una app Android propia (captura de pantalla + codificador)
```

---

# 3. LOS ESCALONES

- [ ] **S0 -- lo que tiene que estar antes.** G5 de `docs/plan/PLAN_RED_TX.md`:
      TCP de verdad desde `platform/shared/bmo-pila/src/tcp` contra un servidor
      de la LAN. Y el latido del GATE RED medido (hoy los tiempos salen de 16 en
      16 ms: `red ping` dice el real desde el 2026-09-14). **Como se sabe:** una
      conexion TCP abre, pasa bytes y cierra limpia contra un `nc -l` en Windows.

- [ ] **S1 -- el reproductor LOCAL.** Una app `.bex` en BMO C con pl_mpeg
      (licencia MIT, un solo fichero) que abre un `.mpg` del disco y lo pinta en
      su ventana, con el audio por el tubo. Sin red: primero se prueba que BMO-X
      sabe ENSENAR video. **Como se sabe:** un `.mpg` hecho en Windows con
      `ffmpeg -i video.mp4 -c:v mpeg1video -c:a mp2 -s 640x360 video.mpg` se ve y
      se oye en el Ryzen, sin desfase notable en 60 segundos.

- [ ] **S2 -- el protocolo ANTENA/1, escrito y con banco.** Un crate puro,
      `platform/shared/bmo-antena`: lineas de texto para pedir (`PIDE`, `PARA`,
      `LISTA`) y una cabecera con el largo antes de los bytes del video. Lista
      blanca como `bmo-pila`: lo que no esta escrito, se rechaza por su nombre.
      **Como se sabe:** `cargo test -p bmo-antena`, con respuestas mutadas.

- [ ] **S3 -- la antena, modo PEDIDO, en el movil.** Un programa pequeno para
      Termux en `toolchain/tools/antena/` (Python): recibe `PIDE`, convierte con
      `ffmpeg` a MPEG-1 + MP2 640x360 y sirve el flujo por TCP a UNA IP. De donde
      saca el video lo decide el dueno de la antena (seccion 1). **Como se sabe:**
      desde Windows, un cliente de prueba recibe un `.mpg` que se reproduce.

- [ ] **S4 -- BMO-X pide y ve.** La app de S1 lee de la conexion en vez del
      disco, con la IP de la antena en `director.cfg` del disco de BMO (nunca en
      el repositorio: seccion 5 de `docs/plan/PLAN_RED_TX.md`) y rechazando
      cualquier otra IP. **Como se sabe:** un video pedido desde BMO-X se ve en
      el Ryzen mientras llega.

- [ ] **S5 -- el mando.** Pausa, seguir y parar desde el teclado de BMO-X, por la
      misma conexion (`PARA`). **Como se sabe:** la antena deja de enviar en
      menos de un segundo.

- [ ] **S6 -- modo ESPEJO (lejos).** App Android propia: captura de pantalla,
      codificador y canal de vuelta para teclado y raton. Android no trae
      codificador MPEG-1, asi que o se lleva uno en software (pesa y calienta) o
      BMO-X aprende H.264 (otro decodificador). Se decide cuando S4 ande.
      **Como se sabe:** la pantalla del movil se ve y se maneja desde el Ryzen.

---

# 4. EL PARECIDO CON STEAM, Y DONDE SE ACABA

```text
   Steam Link / Remote Play    el PC potente juega; el Deck, la tele o el movil
                               solo ensenan y mandan los botones
   este plan                   el MOVIL es el potente para la web; BMO-X ensena
```

** El Steam Deck en si es otra cosa: es un PC entero con SteamOS (Linux con
Proton). Traer eso aqui seria traer POSIX por la puerta de atras, que es justo
lo que la antena evita: lo sucio queda FUERA de BMO-X, en un aparato que ya lo
sabe hacer.
