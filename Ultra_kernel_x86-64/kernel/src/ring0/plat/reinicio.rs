//! Reiniciar la máquina. Tres intentos, del más limpio al más bruto.
//!
//! ## Por qué tres y no uno
//!
//! El `reboot` del shell hacía **sólo** el pulso del 8042 (`0xFE` al puerto
//! `0x64`), que es el método de 1984. En esta placa el i8042 ni siquiera
//! entrega datos coherentes —ya está anotado en el driver de teclado: "sólo
//! entrega ruido"— así que ese `out` no reiniciaba nada y la máquina se
//! quedaba en el `hlt` de después. Escribir `reboot` y que no pase nada es un
//! comando que miente.
//!
//! Así que se intenta en orden, y **el último no puede fallar**:
//!
//! 1. **`0xCF9`, el registro de reset del PCI.** Es lo que usan las placas
//!    modernas y lo que usa Linux por defecto. `0x02` arma el reset del
//!    sistema y `0x06` lo dispara con el CPU incluido.
//! 2. **El pulso del 8042.** Por si acaso hay un controlador de verdad
//!    escuchando. Cuesta dos instrucciones.
//! 3. **Triple fault.** Se carga una IDT vacía y se provoca una excepción: el
//!    CPU no encuentra manejador, no encuentra manejador del fallo del
//!    manejador, y a la tercera se rinde y se reinicia solo. Es feo, es
//!    legítimo, y **funciona siempre** — no depende de ningún chipset.
//!
//! El orden importa: los dos primeros dan al firmware la oportunidad de hacer
//! un reinicio ordenado. El tercero es la garantía de que la máquina arranca
//! de nuevo pase lo que pase, que es justo lo que hace falta al final de una
//! pantalla de fallo.

use core::arch::asm;

unsafe fn outb(puerto: u16, valor: u8) {
    asm!("out dx, al", in("dx") puerto, in("al") valor, options(nomem, nostack));
}

unsafe fn inb(puerto: u16) -> u8 {
    let v: u8;
    asm!("in al, dx", out("al") v, in("dx") puerto, options(nomem, nostack));
    v
}

/// Espera corta por TSC. En este camino las interrupciones están apagadas y no
/// hay temporizador que valga: contar vueltas de bucle mediría la velocidad
/// del CPU, no el tiempo.
fn esperar_us(us: u64) {
    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        // Sin TSC calibrado, unas cuantas vueltas y a otra cosa. Aquí el
        // objetivo es dar aire al chipset, no medir nada.
        for _ in 0..200_000 {
            core::hint::spin_loop();
        }
        return;
    }
    let hasta = crate::ring0::task::scheduler::rdtsc() + (hz / 1_000_000) * us;
    while crate::ring0::task::scheduler::rdtsc() < hasta {
        core::hint::spin_loop();
    }
}

/// Reinicia. No vuelve.
pub fn ahora() -> ! {
    unsafe {
        asm!("cli", options(nomem, nostack));

        // 1. Reset por PCI (0xCF9). Se conserva lo que hubiera puesto el
        //    firmware en los bits altos y sólo se tocan los de reset.
        let previo = inb(0xCF9) & !0x06;
        outb(0xCF9, previo | 0x02); // SYS_RST armado
        esperar_us(100);
        outb(0xCF9, previo | 0x06); // + RST_CPU: dispara
        esperar_us(50_000);

        // 2. El pulso del 8042, por si hay alguien escuchando.
        let mut vueltas = 0u32;
        while inb(0x64) & 0x02 != 0 && vueltas < 100_000 {
            vueltas += 1;
            core::hint::spin_loop();
        }
        outb(0x64, 0xFE);
        esperar_us(50_000);

        // 3. Triple fault. Una IDT de límite 0: cualquier excepción no
        //    encuentra manejador y el CPU se rinde a la tercera.
        #[repr(C, packed)]
        struct Idtr {
            limite: u16,
            base: u64,
        }
        let vacia = Idtr { limite: 0, base: 0 };
        asm!("lidt [{}]", in(reg) &vacia, options(readonly, nostack));
        asm!("int3", options(nomem, nostack));

        // Inalcanzable de verdad, pero el tipo de retorno lo pide.
        loop {
            asm!("hlt", options(nomem, nostack));
        }
    }
}
