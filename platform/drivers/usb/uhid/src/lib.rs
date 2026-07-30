//! USB HID driver — full lifecycle: descriptor reading, Configure Endpoint,
//! interrupt transfers, HID boot protocol parsing for keyboard + mouse.

#![no_std]

use bmo_input::hal::{InputHal, PointerMode};
use bmo_input::event::InputEvent;

/// Máximo de interfaces que consideramos por dispositivo (fijo, sin alloc:
/// el driver corre dentro de Ring 0 de BMO, que no tiene allocator).
const MAX_IFACES: usize = 8;
/// Tamaño máximo aceptado del config descriptor completo (fijo, sin alloc).
const MAX_CFG: usize = 512;

// ── HID boot protocol report structures ──────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct KbdReport { modifiers: u8, _reserved: u8, keys: [u8; 6] }

#[repr(C)]
#[derive(Clone, Copy)]
struct MouseReport { buttons: u8, dx: i8, dy: i8, wheel: i8 }

// ── USB HID usage → PS/2 Set 1 scancode ─────────────────────

static HID_TO_PS2: [u8; 104] = [
    0,0,0,0, 0x1E,0x30,0x2E,0x20,0x12,0x21,0x22,0x23,
    0x17,0x24,0x25,0x26,0x32,0x31,0x18,0x19,0x10,0x13,
    0x1F,0x14,0x16,0x2F,0x11,0x2D,0x15,0x2C,
    0x02,0x03,0x04,0x05,0x06,0x07,0x08,0x09,0x0A,0x0B,
    // El indice 50 (usage 0x32, "Non-US # and ~") es la tecla junto al Enter
    // de los teclados ISO: en español es la de } ] `. Mapea al mismo Set 1
    // 0x2B que la barra invertida; estaba en 0 = tecla muerta de verdad.
    0x1C,0x01,0x0E,0x0F,0x39,0x0C,0x0D,0x1A,0x1B,0x2B,0x2B,
    0x27,0x28,0x29,0x33,0x34,0x35,
    0x3A,0x3B,0x3C,0x3D,0x3E,0x3F,0x40,0x41,0x42,0x43,0x44,0x57,0x58,
    // Navegacion (Insert..flechas). Llevaban los MISMOS codigos Set 1 que
    // el teclado numerico (flecha arriba = 0x48 = KP8), asi que pulsar una
    // flecha escribia un numero. Set 1 real las distingue con el prefijo
    // 0xE0, que no cabe en un byte: se les da codigo propio 0x66..0x6F.
    0x37,0x46,0x45,SC_INSERT,SC_HOME,SC_PGUP,SC_DELETE,SC_END,SC_PGDN,
    SC_RIGHT,SC_LEFT,SC_DOWN,SC_UP,0x45,
    // El '/' del teclado NUMERICO (usage 0x54) llevaba 0x35, el mismo Set 1
    // que la tecla '/' de la fila principal. En US da igual porque ambas son
    // '/', pero en español esa tecla es '-': el numpad escribia guiones.
    // Set 1 real es 0xE0 0x35 (dos bytes); 0x62 esta libre y el consumidor lo
    // resuelve como '/' en cualquier distribucion.
    0x62,0x37,0x4A,0x4E,0x1C,0x4F,0x50,0x51,0x4B,0x4C,0x4D,0x47,0x48,0x49,0x52,0x53,
    // 0x64 = la tecla EXTRA de los teclados ISO (la de < > junto al Shift
    // izquierdo, que los US no tienen): Set 1 la llama 0x56. Estaba en 0 =
    // ignorada, así que en un teclado español faltaba una tecla entera.
    0x56,0,0,0,
];

fn hid_to_ps2(usage: u8) -> Option<u8> {
    let idx = usage as usize;
    if idx < HID_TO_PS2.len() { let v = HID_TO_PS2[idx]; if v != 0 { Some(v) } else { None } }
    else { None }
}

// ── Modifier bits ───────────────────────────────────────────

/// Scancode propio para AltGr (Alt derecho). Set 1 lo expresa como la
/// secuencia `0xE0 0x38`, imposible de meter en un solo byte de InputEvent;
/// 0x63 está libre en Set 1 y el consumidor lo trata como AltGr. Sin esto
/// AltGr llegaba como 0x38 (Alt izquierdo) y el tercer nivel del teclado
/// español — @ # \ | { } [ ] — era inalcanzable.
pub const SC_ALTGR: u8 = 0x63;

// Teclas de navegacion con codigo propio (ver la nota en HID_TO_PS2).
pub const SC_UP: u8 = 0x66;
pub const SC_DOWN: u8 = 0x67;
pub const SC_LEFT: u8 = 0x68;
pub const SC_RIGHT: u8 = 0x69;
pub const SC_INSERT: u8 = 0x6A;
pub const SC_HOME: u8 = 0x6B;
pub const SC_PGUP: u8 = 0x6C;
pub const SC_DELETE: u8 = 0x6D;
pub const SC_END: u8 = 0x6E;
pub const SC_PGDN: u8 = 0x6F;

const MOD_LCTRL: u8 = 1<<0; const MOD_LSHIFT: u8 = 1<<1;
const MOD_LALT: u8 = 1<<2; const MOD_LGUI: u8 = 1<<3;
const MOD_RCTRL: u8 = 1<<4; const MOD_RSHIFT: u8 = 1<<5;
const MOD_RALT: u8 = 1<<6; const MOD_RGUI: u8 = 1<<7;

// ── Peripheral tracking ─────────────────────────────────────

struct KbdDev {
    slot: u8,
    dci: u8,
    /// Numero de interface HID: lo pide SET_REPORT para encender los
    /// LEDs (Bloq Mayus / Num) del teclado.
    iface: u8,
    buf_phys: u64,
    buf_virt: *mut u8,
    prev_mod: u8,
    prev_keys: [u8; 6],
    queued: bool,
}

struct MouseDev {
    slot: u8,
    dci: u8,
    buf_phys: u64,
    buf_virt: *mut u8,
    prev_buttons: u8,
    queued: bool,
}

// ── UsbHidHal ───────────────────────────────────────────────

pub struct UsbHidHal {
    kbd: Option<KbdDev>,
    mouse: Option<MouseDev>,
    /// El ratón que tenemos vino de la interfaz de ratón de un TECLADO, no de
    /// un ratón dedicado. Se guarda porque un teclado se enumera antes y no se
    /// puede saber, en ese momento, si más adelante habrá uno de verdad.
    mouse_provisional: bool,
    initialized: bool,
}

impl UsbHidHal {
    pub const fn new() -> Self {
        Self { kbd: None, mouse: None, mouse_provisional: false, initialized: false }
    }

    /// ¿Enumeró un teclado? (interface HID subclass 1 / protocol 1)
    pub fn has_kbd(&self) -> bool { self.kbd.is_some() }
    /// ¿Enumeró un mouse? (interface HID subclass 1 / protocol 2)
    pub fn has_mouse(&self) -> bool { self.mouse.is_some() }
    /// Slot xHCI del teclado / mouse (0 si ausente) — para diagnóstico.
    pub fn kbd_slot(&self) -> u8 { self.kbd.as_ref().map_or(0, |k| k.slot) }
    pub fn mouse_slot(&self) -> u8 { self.mouse.as_ref().map_or(0, |m| m.slot) }
    /// DCI (endpoint id) del teclado — para comparar con el endpoint del
    /// Transfer Event y detectar el desajuste que impide re-encolar.
    pub fn kbd_dci(&self) -> u8 { self.kbd.as_ref().map_or(0, |k| k.dci) }

    /// Enciende/apaga los LEDs del teclado (bit0 Num, bit1 Mayus, bit2 Scroll).
    ///
    /// Las lucecitas NO las maneja el teclado por su cuenta: es el HOST quien
    /// le dice como dejarlas, con un SET_REPORT de tipo Output. Por eso Bloq
    /// Mayus funcionaba por dentro mientras la luz seguia apagada — nadie se
    /// lo estaba contando al teclado.
    pub fn set_leds(&self, leds: u8) -> bool {
        let k = match self.kbd.as_ref() { Some(k) => k, None => return false };
        let mut data = [leds];
        // bmRequestType 0x21 = Host->Device, Class, Interface.
        // bRequest 0x09 = SET_REPORT; wValue 0x0200 = Output report id 0.
        let n = unsafe {
            bmo_xhci::control_transfer(k.slot, 0x21, 0x09, 0x0200, k.iface as u16, &mut data, false)
        };
        n > 0
    }

    // ── Parsing helpers ─────────────────────────────────────

    /// Read 2 bytes from a slice as little-endian u16.
    fn le_u16(buf: &[u8], off: usize) -> u16 {
        (buf[off] as u16) | ((buf[off + 1] as u16) << 8)
    }

    /// Parse interface descriptors from a full config descriptor set into a
    /// fixed buffer. Returns how many (interface_number, class, subclass,
    /// protocol) entries were written.
    fn parse_interfaces(cfg: &[u8], out: &mut [(u8, u8, u8, u8); MAX_IFACES]) -> usize {
        let mut n = 0;
        let total = if cfg.len() >= 2 { Self::le_u16(cfg, 2) as usize } else { 0 };
        let limit = if total > 0 && total <= cfg.len() { total } else { cfg.len() };
        let mut off = if cfg.len() >= 1 { cfg[0] as usize } else { 9 };
        while off + 3 <= limit && n < MAX_IFACES {
            let len = cfg[off] as usize;
            let dtype = cfg[off + 1];
            if len < 2 || off + len > limit { break; }
            if dtype == 4 && len >= 9 {
                out[n] = (cfg[off + 2], cfg[off + 5], cfg[off + 6], cfg[off + 7]);
                n += 1;
            }
            off += len;
        }
        n
    }

    /// Find interrupt IN endpoint for a given interface.
    /// Returns (endpoint_address, max_packet_size, interval, dci).
    fn find_intr_in(cfg: &[u8], iface_num: u8) -> Option<(u8, u16, u8, u8)> {
        let total = if cfg.len() >= 2 { Self::le_u16(cfg, 2) as usize } else { 0 };
        let limit = if total > 0 && total <= cfg.len() { total } else { cfg.len() };
        let mut off = if cfg.len() >= 1 { cfg[0] as usize } else { 9 };
        let mut current_iface = 0u8;
        while off + 3 <= limit {
            let len = cfg[off] as usize;
            let dtype = cfg[off + 1];
            if len < 2 || off + len > limit { break; }
            if dtype == 4 && len >= 9 { current_iface = cfg[off + 2]; }
            if dtype == 5 && len >= 7 && current_iface == iface_num {
                let ep_addr = cfg[off + 2];
                let attr = cfg[off + 3];
                let mps = Self::le_u16(cfg, off + 4);
                let interval = cfg[off + 6];
                // IN direction + Interrupt transfer type (bits 1:0 = 3)
                if (ep_addr & 0x80) != 0 && (attr & 3) == 3 {
                    let ep_num = ep_addr & 0x0F;
                    let dci = if ep_num == 0 { 1 } else { ep_num * 2 + 1 };
                    return Some((ep_addr, mps, interval, dci));
                }
            }
            off += len;
        }
        None
    }
}

// ═══════════════════════════════════════════════════════════════
//  InputHal impl
// ═══════════════════════════════════════════════════════════════

impl InputHal for UsbHidHal {
    fn init(&mut self) -> bool {
        if self.initialized { return true; }

        // Initialize xHCI controller if needed
        if !bmo_xhci::is_controller_initialized() {
            let mmio = match bmo_xhci::get_mmio() { Some(m) => m, None => return false };
            if !unsafe { bmo_xhci::init(mmio) } { return false; }
        }

        let ctrl = match bmo_xhci::controller() { Some(c) => c, None => return false };
        let h = bmo_xhci::hal();

        // ── Enumerate ports ──
        for port in 0..ctrl.max_ports {
            unsafe {
                bmo_xhci::port_power_on(port);
                // Additional settling time after power-on (AMD chipset)
                for _ in 0..50000 { core::hint::spin_loop(); }
                // ★ Los tres `continue` de aqui abajo eran MUDOS, y por eso
                // hicieron falta tres rondas de fotos para entender por que el
                // raton no aparecia: un puerto que falla al resetear, uno vacio
                // y uno que no acepta direccion se veian todos igual, o sea
                // nada. Ahora cada puerto dice que le pasa. Es una linea por
                // puerto en el arranque, y a cambio la proxima foto contesta.
                if !bmo_xhci::port_reset(port) {
                    h.log_u64("[uhid] puerto sin reset: ", port as u64);
                    continue;
                }
                let speed = bmo_xhci::port_speed(port);
                if speed == 0 {
                    // Vacio no es un fallo: es que ahi no hay nada enchufado.
                    continue;
                }
                h.log_u64("[uhid] puerto con algo: ", port as u64);

                let slot = match bmo_xhci::address_device(port, speed) {
                    Some(s) => s,
                    None => {
                        h.log_u64("[uhid] NO acepta direccion, puerto ", port as u64);
                        continue;
                    }
                };
                h.log_u64("[uhid] slot=", slot as u64);

                // Read device descriptor (18 bytes)
                let mut dev_desc = [0u8; 18];
                // CON REINTENTOS: la enumeracion demostro ser inestable entre
                // arranques — un mismo binario da "no dev desc" en un encendido
                // y enumera bien en el siguiente. Un dispositivo recien
                // reseteado puede no estar listo para el primer control
                // transfer; darle tres oportunidades cuesta milisegundos y
                // evita perder el teclado hasta el proximo reinicio.
                let mut n = 0usize;
                for _ in 0..3 {
                    n = bmo_xhci::get_device_descriptor(slot, &mut dev_desc);
                    if n >= 8 { break; }
                    bmo_xhci::hal().delay_ms(50);
                }
                if n < 8 { h.log("[uhid] no dev desc\n"); continue; }
                let dev_class = dev_desc[4];
                h.log_u64(" class=", dev_class as u64);

                // Read config descriptor header (9 bytes first for total length)
                let mut cfg_hdr = [0u8; 9];
                let mut n2 = 0usize;
                for _ in 0..3 {
                    n2 = bmo_xhci::get_config_descriptor(slot, 0, &mut cfg_hdr);
                    if n2 >= 9 { break; }
                    bmo_xhci::hal().delay_ms(50);
                }
                if n2 < 9 { h.log("[uhid] no cfg hdr\n"); continue; }
                let total_len = Self::le_u16(&cfg_hdr, 2) as usize;
                let cfg_val = cfg_hdr[5];
                h.log_u64(" cfg_val=", cfg_val as u64);
                h.log_u64(" total_len=", total_len as u64);

                // Read full config descriptor (fixed buffer, no alloc)
                if total_len > MAX_CFG { h.log("[uhid] cfg too big\n"); continue; }
                let mut cfg_buf = [0u8; MAX_CFG];
                let n3 = bmo_xhci::get_config_descriptor(slot, 0, &mut cfg_buf[..total_len]);
                if n3 < total_len { h.log("[uhid] cfg short\n"); continue; }
                let cfg_full: &[u8] = &cfg_buf[..total_len];

                // Parse interfaces
                let mut if_buf = [(0u8, 0u8, 0u8, 0u8); MAX_IFACES];
                let n_ifs = Self::parse_interfaces(cfg_full, &mut if_buf);
                let mut found_kbd = false;
                let mut found_mouse = false;

                // ── ¿Este aparato es un TECLADO que además dice tener ratón? ──
                //
                // ★ Aquí estaba el bug del ratón muerto. Muchos teclados —el
                // SEISA de esta máquina, entre ellos— exponen una SEGUNDA
                // interfaz HID con protocolo de ratón (protocol=2) para sus
                // teclas de medios. Como el teclado se enumera primero, esa
                // interfaz se llevaba el puesto de "el ratón", y el ratón de
                // verdad —que se enumera después— se saltaba entero por el
                // `self.mouse.is_none()`. Sintomas exactos: `m=OK` en el mismo
                // slot que el teclado, cero eventos para siempre, y el ratón
                // fisico sin configurar (por eso ni se le encienden las luces).
                //
                // La regla: **un ratón dedicado gana a la interfaz de ratón de
                // un teclado.** Un aparato que trae las dos es un teclado
                // compuesto; uno que sólo trae la de ratón es un ratón.
                let compuesto = if_buf[..n_ifs]
                    .iter()
                    .any(|(_, c, s, p)| *c == 3 && *s == 1 && *p == 1)
                    && if_buf[..n_ifs]
                        .iter()
                        .any(|(_, c, s, p)| *c == 3 && *s == 1 && *p == 2);

                for (iface_num, class, subclass, protocol) in &if_buf[..n_ifs] {
                    // HID class = 3
                    if *class != 3 { continue; }

                    // Keyboard: subclass=1, protocol=1
                    // Mouse: subclass=1, protocol=2
                    let is_kbd = *subclass == 1 && *protocol == 1 && self.kbd.is_none();
                    // El ratón de un teclado compuesto sólo se coge si NO hay
                    // otro, y aun así se marca como provisional: si más tarde
                    // aparece un ratón dedicado, lo reemplaza.
                    let mouse_libre = self.mouse.is_none() || (self.mouse_provisional && !compuesto);
                    let is_mouse = *subclass == 1 && *protocol == 2 && mouse_libre;

                    if !is_kbd && !is_mouse { continue; }

                    // Find interrupt IN endpoint
                    if let Some((ep_addr, mps, interval, dci)) =
                        Self::find_intr_in(cfg_full, *iface_num)
                    {
                        h.log_u64(" found ep addr=", ep_addr as u64);
                        h.log_u64(" mps=", mps as u64);
                        h.log_u64(" dci=", dci as u64);

                        // SET_CONFIGURATION
                        bmo_xhci::control_transfer(slot, 0x00, 0x09, cfg_val as u16, 0, &mut [], false);

                        // Configure Endpoint in xHCI. `interval` es el
                        // bInterval CRUDO del descriptor; la conversión al
                        // exponente que espera el Endpoint Context la hace
                        // bmo_xhci::encode_interval — el frontend no debe
                        // adivinar codificaciones del controlador.
                        h.log_u64(" bInterval=", interval as u64);
                        if !bmo_xhci::configure_endpoint(slot, dci, 7, mps, interval) {
                            h.log("[uhid] cfg_ep FAIL\n"); continue;
                        }
                        // Lo que dice el xHC, no lo que creemos: 1 = Running.
                        h.log_u64(" ep_state=", bmo_xhci::ep_state(slot, dci) as u64);

                        // HID SET_PROTOCOL(boot)
                        bmo_xhci::control_transfer(slot, 0x21, 0x0B, 0, *iface_num as u16, &mut [], false);

                        // HID SET_IDLE(0)
                        bmo_xhci::control_transfer(slot, 0x21, 0x0A, 0, *iface_num as u16, &mut [], false);

                        // Allocate DMA buffer for interrupt reports
                        let buf_size = if is_kbd { 8usize } else { 4usize };
                        let buf_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => continue };
                        let buf_virt = h.phys_to_virt(buf_phys);
                        core::ptr::write_bytes(buf_virt, 0, 4096);

                        if is_kbd {
                            bmo_xhci::queue_interrupt_in(slot, dci, buf_phys, buf_size as u16);
                            bmo_xhci::ring_doorbell(slot, dci);
                            self.kbd = Some(KbdDev {
                                slot, dci, iface: *iface_num, buf_phys, buf_virt,
                                prev_mod: 0, prev_keys: [0; 6], queued: true,
                            });
                            found_kbd = true;
                            h.log("[uhid] kbd ready\n");
                        } else {
                            bmo_xhci::queue_interrupt_in(slot, dci, buf_phys, buf_size as u16);
                            bmo_xhci::ring_doorbell(slot, dci);
                            // Provisional si viene de un teclado compuesto: un
                            // ratón dedicado que aparezca después lo sustituye.
                            self.mouse_provisional = compuesto;
                            if compuesto {
                                h.log("[uhid] iface de raton en un TECLADO: provisional\n");
                            }
                            self.mouse = Some(MouseDev {
                                slot, dci, buf_phys, buf_virt,
                                prev_buttons: 0, queued: true,
                            });
                            found_mouse = true;
                            h.log("[uhid] mouse ready\n");
                        }
                    }
                }

                // ★★ AQUI ESTABA EL RATON MUERTO, y no en el parseo del informe.
                //
                // Esto era `if found_kbd && found_mouse { break; }`. El teclado
                // trae DOS interfaces HID —la suya y una de protocolo de raton
                // para las teclas de medios—, asi que al enumerarlo se marcaban
                // las dos banderas y el bucle **CORTABA AQUI**. El puerto donde
                // esta el raton de verdad no se visitaba nunca.
                //
                // De ahi los tres sintomas a la vez: `m=OK` en el mismo slot que
                // el teclado, cero eventos para siempre, y —el que lo confirma—
                // **el raton fisico sin luces**: un dispositivo USB al que nadie
                // le manda SET_CONFIGURATION no arranca su firmware.
                //
                // Ahora solo se corta con un raton DEDICADO. Si el que hay vino
                // de un teclado compuesto, se sigue buscando: puede haber uno de
                // verdad en un puerto de mas adelante, y si no lo hay, nos
                // quedamos con el provisional sin perder nada.
                if found_kbd && found_mouse && !self.mouse_provisional {
                    break;
                }
            }
        }

        self.initialized = true;
        self.kbd.is_some()
    }

    fn name(&self) -> &'static str { "USB-HID" }

    fn poll(&mut self, buf: &mut [InputEvent]) -> usize {
        if !self.initialized { return 0; }
        let mut count = 0usize;

        unsafe {
            // ── Poll transfer events (non-blocking) ──
            while let Some((ev_slot, ev_ep, cc)) = bmo_xhci::poll_transfer_event() {
                // Keyboard
                if let Some(ref mut k) = self.kbd {
                    if ev_slot == k.slot && ev_ep == k.dci {
                        k.queued = false;
                        if cc == 1 || cc == 13 {
                            let report = core::ptr::read_volatile(k.buf_virt as *const KbdReport);
                            // Diff modifiers
                            let mod_chg = report.modifiers ^ k.prev_mod;
                            if mod_chg & MOD_LCTRL != 0 {
                                let on = report.modifiers & MOD_LCTRL != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x1D, on); count += 1; }
                            }
                            if mod_chg & MOD_LSHIFT != 0 {
                                let on = report.modifiers & MOD_LSHIFT != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x2A, on); count += 1; }
                            }
                            if mod_chg & MOD_RSHIFT != 0 {
                                let on = report.modifiers & MOD_RSHIFT != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x36, on); count += 1; }
                            }
                            if mod_chg & MOD_LALT != 0 {
                                let on = report.modifiers & MOD_LALT != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x38, on); count += 1; }
                            }
                            if mod_chg & MOD_RCTRL != 0 {
                                let on = report.modifiers & MOD_RCTRL != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x1D, on); count += 1; }
                            }
                            if mod_chg & MOD_LGUI != 0 {
                                let on = report.modifiers & MOD_LGUI != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x5B, on); count += 1; }
                            }
                            if mod_chg & MOD_RALT != 0 {
                                let on = report.modifiers & MOD_RALT != 0;
                                // AltGr, NO el Alt izquierdo: en las
                                // distribuciones latinas abre el tercer nivel
                                // (@ # \ | { } [ ]). Set 1 lo codifica como
                                // 0xE0 0x38, dos bytes que no caben en un
                                // InputEvent — de ahí el código propio.
                                if count < buf.len() { buf[count] = InputEvent::key(SC_ALTGR, on); count += 1; }
                            }
                            if mod_chg & MOD_RGUI != 0 {
                                let on = report.modifiers & MOD_RGUI != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x5C, on); count += 1; }
                            }

                            // Diff keys
                            for &k_prev in &k.prev_keys {
                                if k_prev == 0 { continue; }
                                if !report.keys.contains(&k_prev) {
                                    if let Some(ps2) = hid_to_ps2(k_prev) {
                                        if count < buf.len() { buf[count] = InputEvent::key(ps2, false); count += 1; }
                                    }
                                }
                            }
                            for &k_new in &report.keys {
                                if k_new == 0 { continue; }
                                if !k.prev_keys.contains(&k_new) {
                                    if let Some(ps2) = hid_to_ps2(k_new) {
                                        if count < buf.len() { buf[count] = InputEvent::key(ps2, true); count += 1; }
                                    }
                                }
                            }

                            k.prev_mod = report.modifiers;
                            k.prev_keys = report.keys;
                        }
                        // Re-queue
                        bmo_xhci::queue_interrupt_in(k.slot, k.dci, k.buf_phys, 8);
                        bmo_xhci::ring_doorbell(k.slot, k.dci);
                        k.queued = true;
                    }
                }

                // Mouse
                if let Some(ref mut m) = self.mouse {
                    if ev_slot == m.slot && ev_ep == m.dci {
                        m.queued = false;
                        if cc == 1 || cc == 13 {
                            let report = core::ptr::read_volatile(m.buf_virt as *const MouseReport);
                            if report.dx != 0 || report.dy != 0 {
                                if count < buf.len() {
                                    buf[count] = InputEvent::mouse_move(report.dx as i16, report.dy as i16);
                                    count += 1;
                                }
                            }
                            if report.buttons != m.prev_buttons {
                                if count < buf.len() {
                                    buf[count] = InputEvent::mouse_button(report.buttons);
                                    count += 1;
                                }
                                m.prev_buttons = report.buttons;
                            }
                            if report.wheel != 0 {
                                if count < buf.len() {
                                    buf[count] = InputEvent::mouse_wheel(report.wheel);
                                    count += 1;
                                }
                            }
                        }
                        // Re-queue
                        bmo_xhci::queue_interrupt_in(m.slot, m.dci, m.buf_phys, 4);
                        bmo_xhci::ring_doorbell(m.slot, m.dci);
                        m.queued = true;
                    }
                }
            }
        }

        count
    }

    fn pointer_mode(&self) -> PointerMode { PointerMode::Relative }
    fn is_ready(&self) -> bool { self.initialized }
}
