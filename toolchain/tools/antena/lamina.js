// lamina.js -- el recorrido del DOM que convierte una pagina en una LAMINA.
//
// L0 de docs/plan/PLAN_CLOUD_LOCAL.md (seccion 11). Corre DENTRO del navegador
// de la antena (el WebView de la app Android, o Chromium sin cabeza en el Arch):
// la pagina ya esta cargada, con su JavaScript y su CSS aplicados, y esto solo
// LEE lo que el navegador ya calculo -- getBoundingClientRect, estilos
// calculados, nodos de texto -- y lo escribe en el formato que BMO-X sabe leer
// (platform/shared/bmo-antena/src/lamina.rs). Aqui no se maqueta nada: eso
// ya lo hizo el motor. Es el mismo movimiento que MAQUETA: emitir coordenadas
// ya calculadas.
//
//    LAMINA <ancho> <alto> <n>
//    CAJA   x y ancho alto rrggbb
//    TEXTO  x y escala rrggbb <bytes Latin-1>
//    IMAGEN x y ancho alto <id>      ENLACE ... <id>      CAMPO ... <id>
//
// Lo que este recorrido decide, dicho para que nadie lo descubra por sorpresa:
//
//  - el ANCHO de la lamina es el del navegador: la antena pone el WebView al
//    ancho que BMO-X pide (640, 800, 1280) y esto no reescala nada.
//  - la lamina es la pagina ENTERA (scrollHeight), no la ventana: el scroll se
//    hace en BMO-X, en local. Tope 32768 px de alto.
//  - la fuente de BMO-X es UNA, 8x16: el tamano de letra se vuelve una ESCALA
//    1..4 (16 px -> 1, 32 px -> 2). Una tira que no cabe en su ancho se PARTE
//    por letras, nunca se deja salir: el lector la rechazaria entera.
//  - el texto va en Latin-1. Lo que no tiene byte se vuelve `?` (o su letra
//    base si la tiene: "e" con acento que la fuente no conozca cae a "e").
//  - cajas: solo los fondos con color y area; las de fuera de la pagina se
//    recortan al borde. Nada de sombras, bordes ni degradados.
//  - orden de pintado = orden del DOM. Un z-index que reordene se pinta mal:
//    es un limite conocido, no un fallo a arreglar en BMO-X.
//  - tope 4096 lineas. Si la pagina tiene mas, se corta y `truncada` lo dice:
//    BMO-X ensena una lamina entera o ninguna, nunca una a medias sin avisar.
//
// Devuelve { lamina, imagenes, enlaces, campos, truncada }: el texto de la
// lamina y las tablas id -> src / href / nombre que la antena guarda para
// contestar a IMAGEN <id>, CLIC <id> y TECLA <id>.
//
// Se prueba en cualquier navegador de escritorio antes que en el movil:
// pegar en la consola, llamar a `metricaBMO()` y luego a `lamina()`.
// `cliente.py --lamina f` juzga la salida con las mismas reglas que el lector
// de BMO-X.
//
// ** MEDIDO el 2026-09-16 en un articulo de Wikipedia a 640 px: sin
// `metricaBMO()` la pagina media 22.117 px de alto y el texto se salia por la
// derecha (la fuente del navegador es proporcional y mas estrecha que 8 px);
// con ella, 16.407 px, 2.158 lineas, 91 KB, 60 ms de recorrido, y NINGUNA tira
// pegada al borde. La lamina no se arregla en BMO-X: se maqueta con su metrica.

// La antena maqueta con la METRICA de BMO-X: monoespaciada, 8 px por letra y
// 16 de alto (Courier New a 13,33 px mide exactamente eso). Asi el navegador
// parte las lineas donde BMO-X las va a pintar, y `escala` sale entera. Se
// llama UNA vez por pagina, antes de `lamina()`. Devuelve lo que midio, para
// que la antena lo compruebe en vez de suponerlo.
function metricaBMO() {
  var st = document.getElementById("bmo-metrica");
  if (!st) {
    st = document.createElement("style");
    st.id = "bmo-metrica";
    document.head.appendChild(st);
  }
  st.textContent =
    '* { font-family: "Courier New", "Liberation Mono", "DejaVu Sans Mono", monospace !important;' +
    '    letter-spacing: 0 !important; word-spacing: 0 !important; }' +
    'html, body, p, li, td, th, a, span, div, dd, dt, small, sup, sub, code, pre, b, i, em, strong,' +
    'input, button, label, caption, figcaption, h4, h5, h6 { font-size: 13.33px !important; line-height: 16px !important; }' +
    'h1, h2, h3 { font-size: 26.66px !important; line-height: 32px !important; }';
  // Se comprueba, no se supone: diez M en un span.
  var m = document.createElement("span");
  m.textContent = "MMMMMMMMMM";
  m.style.cssText = "position:absolute;visibility:hidden;white-space:pre";
  document.body.appendChild(m);
  var r = m.getBoundingClientRect();
  m.remove();
  return { letraAncho: r.width / 10, letraAlto: r.height };
}

function lamina(opciones) {
  var o = opciones || {};
  var ANCHO_MAX = 1280, ALTO_MAX = 32768, LINEAS_MAX = 4096, ESCALA_MAX = 4;
  var LETRA_ANCHO = 8, LETRA_ALTO = 16;
  var doc = document, win = window;
  var W = Math.min(ANCHO_MAX, Math.floor(o.ancho || doc.documentElement.clientWidth || win.innerWidth));
  var H = Math.min(ALTO_MAX, Math.max(1, Math.floor(doc.documentElement.scrollHeight)));
  var sy = win.scrollY || 0, sx = win.scrollX || 0;

  var lineas = [], imagenes = {}, enlaces = {}, campos = {};
  var nImg = 0, nEnl = 0, nCam = 0, truncada = false;

  function poner(l) {
    if (lineas.length >= LINEAS_MAX) { truncada = true; return false; }
    lineas.push(l);
    return true;
  }

  // -- color ---------------------------------------------------------------
  function hex(c) {
    // "rgb(1, 2, 3)" o "rgba(1, 2, 3, 0.5)". Alfa 0 = no hay color.
    var m = /rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)/.exec(c || "");
    if (!m) return null;
    if (m[4] !== undefined && parseFloat(m[4]) === 0) return null;
    function d(n) { var s = (+n).toString(16); return s.length < 2 ? "0" + s : s; }
    return d(m[1]) + d(m[2]) + d(m[3]);
  }

  // -- zonas: dentro de la pagina o recortadas -----------------------------
  function zona(r) {
    var x = Math.floor(r.left + sx), y = Math.floor(r.top + sy);
    var x2 = Math.ceil(r.right + sx), y2 = Math.ceil(r.bottom + sy);
    if (x < 0) x = 0;
    if (y < 0) y = 0;
    if (x2 > W) x2 = W;
    if (y2 > H) y2 = H;
    if (x2 - x <= 0 || y2 - y <= 0) return null;
    return { x: x, y: y, w: x2 - x, h: y2 - y };
  }
  function zonaLinea(z) { return z.x + " " + z.y + " " + z.w + " " + z.h; }

  // -- texto: Latin-1 y tiras que caben -----------------------------------
  function latin1(s) {
    var out = "";
    for (var i = 0; i < s.length; i++) {
      var c = s.charCodeAt(i);
      if (c >= 0x20 && c <= 0x7E) { out += s[i]; continue; }
      if (c >= 0xA0 && c <= 0xFF) { out += s[i]; continue; }
      // Una letra con marca que la fuente no tenga cae a su letra base.
      var base = s[i].normalize ? s[i].normalize("NFD").charAt(0) : "?";
      var b = base.charCodeAt(0);
      out += (b >= 0x20 && b <= 0x7E) ? base : "?";
    }
    return out;
  }
  function escalaDe(px) {
    var e = Math.round(px / LETRA_ALTO);
    return e < 1 ? 1 : (e > ESCALA_MAX ? ESCALA_MAX : e);
  }
  // Una tira ya en UNA linea de pantalla: se parte por letras si no cabe.
  function tira(x, y, escala, color, texto) {
    var paso = LETRA_ANCHO * escala;
    if (y + LETRA_ALTO * escala > H || x >= W) return;
    var caben = Math.floor((W - x) / paso);
    for (var i = 0; i < texto.length && caben > 0; i += caben) {
      var trozo = texto.substr(i, caben);
      if (!/\S/.test(trozo)) continue;
      if (!poner("TEXTO " + x + " " + y + " " + escala + " " + color + " " + trozo)) return;
    }
  }
  // Un nodo de texto puede ocupar varias lineas de pantalla: se recorre por
  // palabras y se corta donde el navegador ya corto (un rect mas).
  function textoNodo(nodo, estilo) {
    var s = nodo.nodeValue.replace(/\s+/g, " ");
    if (!/\S/.test(s)) return;
    var color = hex(estilo.color) || "000000";
    var escala = escalaDe(parseFloat(estilo.fontSize) || 16);
    var rango = doc.createRange();
    var palabras = s.split(/(?<=\s)/); // cada trozo conserva su espacio
    var ini = 0, pos = 0, fila = null, textoFila = "";
    for (var i = 0; i < palabras.length; i++) {
      var fin = pos + palabras[i].length;
      // La posicion en `s` (blancos ya juntos) no es la del nodo: se mide con
      // el rango sobre el nodo real, palabra a palabra.
      var fin0 = indiceOriginal(nodo.nodeValue, fin);
      rango.setStart(nodo, indiceOriginal(nodo.nodeValue, ini));
      rango.setEnd(nodo, fin0);
      var rects = rango.getClientRects();
      if (rects.length > 1) {
        // Esta palabra ya no cabe en la fila: se emite la fila y se empieza otra.
        if (fila) tira(fila.x, fila.y, escala, color, textoFila.replace(/\s+$/, ""));
        ini = pos; textoFila = "";
        rango.setStart(nodo, indiceOriginal(nodo.nodeValue, ini));
        rango.setEnd(nodo, fin0);
        rects = rango.getClientRects();
      }
      if (rects.length) {
        var z = zona(rects[0]);
        fila = z ? { x: z.x, y: z.y } : fila;
      }
      textoFila += palabras[i];
      pos = fin;
    }
    if (fila && /\S/.test(textoFila)) tira(fila.x, fila.y, escala, color, textoFila.replace(/\s+$/, ""));
  }
  // `s` es el nodo con los blancos juntados; esto vuelve al indice del nodo.
  function indiceOriginal(orig, iJunto) {
    var j = 0, enBlanco = false;
    for (var i = 0; i < orig.length; i++) {
      var b = /\s/.test(orig[i]);
      if (b && enBlanco) continue;
      if (j === iJunto) return i;
      j++;
      enBlanco = b;
    }
    return orig.length;
  }

  // -- el recorrido ----------------------------------------------------------
  var SALTAR = { SCRIPT: 1, STYLE: 1, NOSCRIPT: 1, TEMPLATE: 1, HEAD: 1, SVG: 1, CANVAS: 1, VIDEO: 1, AUDIO: 1, IFRAME: 1 };
  function recorrer(el) {
    if (lineas.length >= LINEAS_MAX) { truncada = true; return; }
    if (el.nodeType === 3) return; // los textos los pinta su padre
    if (el.nodeType !== 1 || SALTAR[el.tagName]) return;
    var estilo = win.getComputedStyle(el);
    if (estilo.display === "none" || estilo.visibility === "hidden" || parseFloat(estilo.opacity) === 0) return;
    var r = el.getBoundingClientRect();
    // Recortado a nada: el rotulo "solo para lectores de pantalla" es un span
    // de 1x1 px con overflow hidden y clip. En una caja mas pequena que una
    // letra no se ve texto; sin esto salia "Men / u d / e u / sua / rio".
    if (r.width < 1 || r.height < 1) return;
    if (estilo.overflow === "hidden" && (r.width < LETRA_ANCHO || r.height < LETRA_ALTO)) return;
    var z = zona(r);
    var tag = el.tagName;

    if (z) {
      var fondo = hex(estilo.backgroundColor);
      // El fondo del body ya salio como primera caja, del alto de la pagina.
      if (fondo && el !== doc.body) poner("CAJA " + zonaLinea(z) + " " + fondo);
      if (tag === "IMG") {
        var id = "img" + (++nImg);
        imagenes[id] = el.currentSrc || el.src || "";
        poner("IMAGEN " + zonaLinea(z) + " " + id);
        return;
      }
      if ((tag === "INPUT" && /^(text|search|email|url|password|number|tel|)$/i.test(el.type || "")) || tag === "TEXTAREA") {
        var idc = "c" + (++nCam);
        campos[idc] = el.name || el.id || idc;
        poner("CAMPO " + zonaLinea(z) + " " + idc);
        if (el.value) tira(z.x + 4, z.y + Math.max(0, Math.floor((z.h - LETRA_ALTO) / 2)), 1, hex(estilo.color) || "000000", latin1(el.value));
        return;
      }
    }
    // Un enlace ocupa una zona por linea de pantalla en la que cae.
    if (tag === "A" && el.href) {
      var ide = "e" + (++nEnl);
      enlaces[ide] = el.href;
      var rs = el.getClientRects();
      for (var k = 0; k < rs.length; k++) {
        var ze = zona(rs[k]);
        if (ze) poner("ENLACE " + zonaLinea(ze) + " " + ide);
      }
    }
    for (var h = el.firstChild; h; h = h.nextSibling) {
      if (h.nodeType === 3) textoNodo(h, estilo);
      else recorrer(h);
    }
  }
  // El fondo del documento, si lo tiene, es la primera caja.
  var fondoDoc = hex(win.getComputedStyle(doc.body || doc.documentElement).backgroundColor);
  if (fondoDoc) poner("CAJA 0 0 " + W + " " + H + " " + fondoDoc);
  if (doc.body) recorrer(doc.body);

  // Latin-1 al final, sobre todas las tiras: asi el conteo de letras que
  // decidio el ancho es el de los bytes que viajan.
  for (var i = 0; i < lineas.length; i++) {
    if (lineas[i].indexOf("TEXTO ") === 0) {
      var p = lineas[i].split(" ");
      var cab = p.slice(0, 5).join(" ");
      lineas[i] = cab + " " + latin1(p.slice(5).join(" "));
    }
  }
  return {
    lamina: "LAMINA " + W + " " + H + " " + lineas.length + "\n" + lineas.join("\n") + "\n",
    imagenes: imagenes,
    enlaces: enlaces,
    campos: campos,
    truncada: truncada
  };
}
