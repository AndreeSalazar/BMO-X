//! **DE UN APARATO ENCHUFADO A UN APARATO CON DIRECCION.**
//!
//! Puertos, `Enable Slot`, el anillo EP0 de cada ranura y `Address Device`.
//!
//! ## Por que soy un fichero (L6b)
//!
//! Porque contesto UNA pregunta y se puede decir en una linea: **como pasa un
//! aparato de estar enchufado a poder recibir ordenes.** Son cuatro pasos que
//! solo tienen sentido en ese orden y que nadie ejecuta por separado:
//!
//! ```text
//!    1. el puerto dice que hay algo, y a que velocidad
//!    2. el controlador da una RANURA para ese algo
//!    3. la ranura recibe su anillo EP0 -- por donde entran las ordenes
//!    4. `Address Device`, y a partir de ahi el aparato tiene nombre
//! ```
//!
//! *** Y el paso 3 es la trampa que este fichero deja junta a proposito: el
//! anillo EP0 se guarda POR RANURA, y un aparato que se desenchufa y vuelve
//! recibe otra ranura. Tener el almacenamiento al lado de quien lo llena es lo
//! que impide que uno de los dos cambie sin el otro.
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

use super::*;

// ===================================================================
//  Port ops
// ===================================================================

pub unsafe fn port_speed(port: u8) -> u8 {
    let c = match CTRL.as_ref() { Some(c) => c, None => return 0 };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    ((r32(c.mmio + pb + PORTSC as u64) >> 10) & 0x0F) as u8
}

pub unsafe fn port_peek(port: u8) -> u32 {
    let c = match CTRL.as_ref() { Some(c) => c, None => return 0 };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    r32(c.mmio + pb + PORTSC as u64)
}

/// Enciende la corriente del puerto **y espera** la estabilizacion de VBUS.
/// Para encender UNO. Quien encienda varios debe usar [`port_power_solo`] y
/// esperar una sola vez al final -- ver ahi por que.
pub unsafe fn port_power_on(port: u8) {
    port_power_solo(port);
    // Spec: >=20 ms de estabilizacion de VBUS antes de confiar en CCS.
    hal().delay_ms(20);
}

/// Enciende la corriente y **no espera**.
///
/// * La espera de VBUS es un tiempo FISICO del puerto, y los puertos se
/// estabilizan **en paralelo**: encender ocho y esperar 20 ms una vez es tan
/// correcto como esperar 20 ms ocho veces, y tarda 160 ms menos. Con dos
/// controladores en esta placa, eso es un tercio de segundo de arranque que no
/// compraba nada.
pub unsafe fn port_power_solo(port: u8) {
    let c = match CTRL.as_mut() { Some(c) => c, None => return };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    w32(c.mmio + pb + PORTSC as u64, r32(c.mmio + pb + PORTSC as u64) | PORTSC_PP);
}

/// Reset del puerto con TIEMPOS REALES. Un reset USB2 tarda ~10-50 ms; el
/// firmware/PHY latchea PED solo cuando termina. Poll a 1 ms, hasta 120 ms.
pub unsafe fn port_reset(port: u8) -> bool {
    let c = match CTRL.as_mut() { Some(c) => c, None => return false };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    let sc = r32(c.mmio + pb + PORTSC as u64);
    if sc & PORTSC_CCS == 0 { return false; }
    // Escribir PR preservando bits RW1C (no re-limpiar cambios por error):
    // solo PP + PR, el resto a 0 (los bits de estado son RO/RW1C).
    w32(c.mmio + pb + PORTSC as u64, (sc & PORTSC_PP) | PORTSC_PR);
    for _ in 0..120 {
        hal().delay_ms(1);
        let s = r32(c.mmio + pb + PORTSC as u64);
        // Reset completo cuando PR se auto-limpia. Reconocer PRC.
        if s & PORTSC_PR == 0 {
            if s & PORTSC_PRC != 0 {
                w32(c.mmio + pb + PORTSC as u64, (s & PORTSC_PP) | PORTSC_PRC);
            }
            // Recovery post-reset (spec: 10 ms) y comprobar habilitacion.
            hal().delay_ms(10);
            let e = r32(c.mmio + pb + PORTSC as u64);
            return e & PORTSC_PED != 0;
        }
    }
    false
}

// ===================================================================
//  Enable Slot
// ===================================================================

pub unsafe fn enable_slot() -> Option<u8> {
    hal().log("[xhci] enable_slot\n");
    let ev = send_cmd(Trb { dw0: 0, dw1: 0, dw2: 0, dw3: TRB_ENABLE << 10 })?;
    let cc = (ev.2 >> 24) & 0xFF;
    let slot = ((ev.3 >> 24) & 0xFF) as u8;
    hal().log_u64(" cc=", cc as u64);
    hal().log_u64(" slot=", slot as u64);
    if cc != CC_SUCCESS || slot == 0 { None } else { Some(slot) }
}

/// **Devolver un slot al controlador.** La pareja de `enable_slot`, y sin ella
/// el controlador se queda sin slots.
///
/// * Esto faltaba, y se vio en la primera foto: los slots subian `0x30`,
/// `0x31`, ... `0x40` en el registro de arranque. Cada intento de adopcion que
/// no acababa en un aparato instalado se llevaba un slot **para siempre**;
/// al llegar a los 64 que declara este xHC, el `Address Device` empezo a
/// contestar `cc=0x9` -- *No Slots Available* -- y a partir de ahi no se pudo
/// enumerar nada mas en toda la sesion.
///
/// Un recurso que se pide en un camino que puede fallar necesita su
/// devolucion **en el mismo sitio**, no en el camino feliz.
///
/// Lo que NO devuelve: las paginas DMA del anillo EP0 y del contexto de
/// dispositivo. El HAL no tiene `free_dma_pages` todavia, asi que eso sigue
/// siendo una fuga -- acotada, porque ahora los intentos estan contados.
pub unsafe fn disable_slot(slot: u8) -> bool {
    if slot == 0 { return false; }
    let ok = send_cmd(Trb {
        dw0: 0, dw1: 0, dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_DISABLE << 10),
    })
    .is_some();
    hal().log_u64("[xhci] disable_slot ", slot as u64);
    hal().log(if ok { " ok\n" } else { " FALLO\n" });
    // El puntero del contexto de dispositivo se retira SIEMPRE, salga bien el
    // comando o no: dejarlo puesto apuntando a un slot que el xHC ya no cree
    // suyo es peor que retirarlo de mas.
    if let Some(c) = CTRL.as_ref() {
        let dcbaa = hal().phys_to_virt(c.dcbaa_phys) as *mut u64;
        dcbaa.add(slot as usize).write_volatile(0);
    }
    if (slot as usize) < MAX_SLOTS {
        EP0_RINGS[slot as usize].valid = false;
    }
    ok
}

// ===================================================================
//  Per-slot EP0 ring storage
// ===================================================================

// [!] `pub(crate)` y no privados: son las DOS unicas cosas que cruzan a
// `transferencia.rs`, y ese cruce es real -- una transferencia de control
// necesita el anillo EP0 de la ranura, que es lo que este fichero llena.
//
// ** Se dicen en vez de cambiarlas callando: L6d exige que un reparto sea texto
// movido, y lo que no lo es tiene que verse. Que sean DOS y no veinte es ademas
// la medida de que el corte estaba en el sitio correcto.
pub(crate) const MAX_SLOTS: usize = 255;
#[derive(Clone, Copy)]
#[allow(dead_code)]
// ** Los campos van abiertos al crate por lo mismo que `ep0_mut`: quien hace
// una transferencia de control ESCRIBE en este anillo, y ese es todo el punto
// de que la ranura lo guarde. Antes del reparto eran privados porque "privado"
// significaba "de este fichero" y el fichero era uno solo.
pub(crate) struct Ep0Info {
    pub(crate) valid: bool,
    pub(crate) ring_phys: u64,
    pub(crate) ring_virt: *mut u32,
    pub(crate) pcs: bool,
    pub(crate) enqueue: usize,
}
// ** `pcs: false` Y NO `true`, y no cambia el comportamiento.
//
// El PCS de un anillo nuevo vale 1 por el xHCI spec, y por eso estaba escrito
// asi. Pero esta ranura NO es un anillo: es una ranura vacia (`valid: false`),
// y nadie la lee sin pasar antes por esa bandera. Al registrarla de verdad,
// `ep0_reg` escribe la estructura ENTERA con `pcs: true`.
//
// Lo que si hacia era caro: un unico campo distinto de cero manda el array
// entero a `.data` en vez de a `.bss`, o sea que **8 KiB de ceros viajaban
// dentro de la imagen del kernel** para llevar un bit puesto que nadie mira.
// Ver `EP_RINGS`, que es este mismo caso multiplicado por treinta y dos.
static mut EP0_RINGS: [Ep0Info; MAX_SLOTS] = [Ep0Info {
    valid: false, ring_phys: 0, ring_virt: core::ptr::null_mut(), pcs: false, enqueue: 0
}; MAX_SLOTS];
unsafe fn ep0_reg(slot: u8, phys: u64, virt: *mut u32) {
    EP0_RINGS[slot as usize] = Ep0Info { valid: true, ring_phys: phys, ring_virt: virt, pcs: true, enqueue: 0 };
}
pub(crate) fn ep0_mut(slot: u8) -> Option<&'static mut Ep0Info> {
    if (slot as usize) < MAX_SLOTS { unsafe { let p = &mut EP0_RINGS[slot as usize]; if p.valid { Some(p) } else { None } } }
    else { None }
}

// ===================================================================
//  Address Device
// ===================================================================

/// Pide un slot y direcciona el aparato del puerto.
///
/// * **Si algo falla despues de tener el slot, el slot se DEVUELVE.** Antes se
/// salia por cinco sitios distintos con un `?` o un `return None` y el slot se
/// quedaba pedido para siempre; el bucle de adopcion del arranque los fue
/// gastando de uno en uno hasta agotar los 64 del controlador. La pareja
/// pedir/devolver tiene que estar en la misma funcion o no esta.
pub unsafe fn address_device(port: u8, speed: u8) -> Option<u8> {
    let slot = enable_slot()?;
    match direccionar_en_slot(port, speed, slot) {
        Some(s) => Some(s),
        None => {
            disable_slot(slot);
            None
        }
    }
}

unsafe fn direccionar_en_slot(port: u8, speed: u8, slot: u8) -> Option<u8> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    let h = hal();
    let cs = ctx_sz(ctrl);

    let ep0_phys = h.alloc_dma_pages(1)?;
    let ep0_virt = h.phys_to_virt(ep0_phys) as *mut u32;
    core::ptr::write_bytes(ep0_virt as *mut u8, 0, 4096);
    let mut ring = TransferRing::new(ep0_virt, ep0_phys);
    // El productor (control_transfer) alterna su cycle state al dar la
    // vuelta -- el Link TRB necesita Toggle Cycle para que el xHC haga lo
    // mismo, o el anillo se desincroniza tras el primer wrap.
    ring.enable_toggle_cycle();
    ep0_reg(slot, ep0_phys & !0xF, ep0_virt);

    let in_phys = h.alloc_dma_pages(1)?;
    let in_virt = h.phys_to_virt(in_phys) as *mut u8;
    core::ptr::write_bytes(in_virt, 0, 4096);
    let dev_phys = h.alloc_dma_pages(1)?;
    let dev_virt = h.phys_to_virt(dev_phys) as *mut u8;
    core::ptr::write_bytes(dev_virt, 0, 4096);

    // Input Control Context
    let in32 = in_virt as *mut u32;
    in32.add(0).write_volatile(0); // Drop
    in32.add(1).write_volatile(3); // Add Slot+EP0

    // Slot Context
    let sc = in_virt.add(cs) as *mut u32;
    sc.add(0).write_volatile(((speed as u32) & 0xF) << 20 | (1 << 27));
    sc.add(1).write_volatile((port as u32 + 1) << 16);

    let mps: u32 = match speed { 1|2 => 8, 3 => 64, 4|5 => 512, _ => 8 };
    let dq = (ep0_phys & !0xF) | 1;

    // EP0 Context
    let ep0 = in_virt.add(2 * cs) as *mut u32;
    ep0.add(0).write_volatile(0);
    ep0.add(1).write_volatile((mps << 16) | (4 << 3) | (3 << 1));
    ep0.add(2).write_volatile((dq & 0xFFFF_FFFF) as u32);
    ep0.add(3).write_volatile(((dq >> 32) & 0xFFFF_FFFF) as u32);
    ep0.add(4).write_volatile(8);

    // DCBAA[slot]
    let dcbaa = h.phys_to_virt(ctrl.dcbaa_phys) as *mut u64;
    dcbaa.add(slot as usize).write_volatile(dev_phys & !0x3F);

    // Address Device TRB
    let trb = Trb {
        dw0: (in_phys & 0xFFFF_FFFF) as u32,
        dw1: ((in_phys >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_ADDRESS_DEV << 10),
    };
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    // * Esto tomaba el primer evento SIN MIRAR EL TIPO y le leia el `cc`. Un
    // Transfer Event correcto tambien trae `cc=1`, asi que un informe del
    // raton se leia como "el Address Device salio bien" -- y de paso ese
    // informe desaparecia.
    let ev = evt_poll_block(ctrl, Espera::Comando)?;
    let cc = (ev.2 >> 24) & 0xFF;
    h.log_u64(" addr_dev cc=", cc as u64);
    if cc != CC_SUCCESS { return None; }

    // Write EP0 dequeue into Device Context EP0 for future doorbell reloads
    let d_ep0 = dev_virt.add(cs) as *mut u32;
    d_ep0.add(2).write_volatile((ep0_phys & !0xF) as u32 | 1);
    d_ep0.add(3).write_volatile(((ep0_phys >> 32) & 0xFFFF_FFFF) as u32);

    Some(slot)
}
