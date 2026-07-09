#![no_std]

static mut TSC_FREQ_HZ: u64 = 0;
static mut VOLUME: u8 = 50;

/// Inicializa el crate con la frecuencia calibrada de TSC.
pub fn init(tsc_freq: u64) {
    unsafe { TSC_FREQ_HZ = tsc_freq; }
}

/// Ajusta el volumen global (0 a 100).
pub fn set_volume(vol: u8) {
    unsafe { VOLUME = vol; }
}

/// Obtiene el volumen global.
pub fn get_volume() -> u8 {
    unsafe { VOLUME }
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, preserves_flags));
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port, options(nomem, preserves_flags));
    v
}

/// Genera un retardo preciso en milisegundos basado en TSC.
pub fn delay_ms(ms: u64) {
    let freq = unsafe { TSC_FREQ_HZ };
    let start = rdtsc();
    let target_cycles = if freq > 0 {
        (freq / 1000) * ms
    } else {
        // Fallback genérico a 3.7 GHz
        3_700_000 * ms
    };
    
    while rdtsc().wrapping_sub(start) < target_cycles {
        core::hint::spin_loop();
    }
}

#[inline]
fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high); }
    ((high as u64) << 32) | low as u64
}

/// Emite un tono en el PC Speaker usando el volumen global.
pub fn beep(freq_hz: u32, duration_ms: u32) {
    let vol = get_volume();
    beep_ex(freq_hz, duration_ms, vol);
}

/// Emite un tono en el PC Speaker con volumen explícito.
/// - volume == 0: Silencio
/// - volume <= 50: Volumen bajo (PIT Modo 2)
/// - volume > 50: Volumen alto (PIT Modo 3)
pub fn beep_ex(freq_hz: u32, duration_ms: u32, volume: u8) {
    unsafe {
        if freq_hz == 0 || volume == 0 {
            let p = inb(0x61);
            outb(0x61, p & 0xFC);
            delay_ms(duration_ms as u64);
            return;
        }
        
        let div = (1_193_180u32 / freq_hz) as u16;
        
        // Volumen bajo usa PIT modo 2 (Rate Generator, pulsos muy estrechos)
        // Volumen alto usa PIT modo 3 (Square Wave Generator, 50% duty cycle)
        if volume <= 50 {
            outb(0x43, 0xB4); // Canal 2, LSB/MSB, Modo 2, Binario
        } else {
            outb(0x43, 0xB6); // Canal 2, LSB/MSB, Modo 3, Binario
        }
        
        outb(0x42, (div & 0xFF) as u8);
        outb(0x42, ((div >> 8) & 0xFF) as u8);
        
        let p = inb(0x61);
        outb(0x61, p | 0x03);

        delay_ms(duration_ms as u64);

        let p2 = inb(0x61);
        outb(0x61, p2 & 0xFC);
    }
}

/// Reproduce la melodía típica de logon de Windows 10/11 (pentatónica de G# mayor).
pub fn play_logon_chime() {
    // G#4 (415 Hz), D#5 (622 Hz), C#5 (554 Hz), G#5 (830 Hz)
    beep(415, 120);
    delay_ms(30);
    beep(622, 120);
    delay_ms(30);
    beep(554, 120);
    delay_ms(30);
    beep(830, 500);
}
