

use core::fmt::{self, Write};

/// Simula el acceso al contador de ciclo de reloj (TSC) de x86 para timestamps de ultra-alta precisión.
#[inline(always)]
fn read_tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let mut aux = 0;
        core::arch::x86_64::__rdtscp(&mut aux)
    }
    #[cfg(not(target_arch = "x86_64"))]
    0 // Fallback genérico
}

// Emulador del puerto serial (0x3F8 COM1) para 
pub struct SerialPort;
impl Write for SerialPort {
    fn write_str(&mut self, _s: &str) -> fmt::Result {
        // En el verdadero FastOS aquí se harían instrucciones outb(0x3F8, byte)
        // Por la compilación de ABI dejamos la firma compatible.
        Ok(())
    }
}

pub static mut SERIAL_PORT: SerialPort = SerialPort;

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        let _ = core::fmt::Write::write_fmt(unsafe { &mut $crate::telemetry::SERIAL_PORT }, format_args!($($arg)*));
    };
}

#[derive(Copy, Clone)]
pub struct IoctlRecord {
    pub timestamp: u64,
    pub ioctl_code: u32,
    pub handle: u32,
    pub dma_buffer_head: [u8; 64],
}

impl IoctlRecord {
    pub const fn empty() -> Self {
        Self {
            timestamp: 0,
            ioctl_code: 0,
            handle: 0,
            dma_buffer_head: [0; 64],
        }
    }
}

pub const RING_BUFFER_SIZE: usize = 256;

pub struct TelemetryRingBuffer {
    pub records: [IoctlRecord; RING_BUFFER_SIZE],
    pub head: usize,
    pub tail: usize,
}

impl TelemetryRingBuffer {
    pub const fn new() -> Self {
        Self {
            records: [IoctlRecord::empty(); RING_BUFFER_SIZE],
            head: 0,
            tail: 0,
        }
    }

    pub fn push(&mut self, code: u32, handle: u32, dma_ptr: *const u8) {
        let mut record = IoctlRecord {
            timestamp: read_tsc(),
            ioctl_code: code,
            handle,
            dma_buffer_head: [0; 64],
        };

        // Copiar los primeros 64 bytes del DMA Buffer si no es nulo
        if !dma_ptr.is_null() {
            unsafe {
                core::ptr::copy_nonoverlapping(dma_ptr, record.dma_buffer_head.as_mut_ptr(), 64);
            }
        }

        self.records[self.head] = record;
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        if self.head == self.tail {
            self.tail = (self.tail + 1) % RING_BUFFER_SIZE; // Overwrite oldest
        }
    }

    /// Imprime el historial en caso de un SUBMITCOMMAND
    pub fn dump_history(&self) {
        serial_print!("--- TELEMETRY RING BUFFER DUMP ---\n");
        let mut curr = self.tail;
        while curr != self.head {
            let r = &self.records[curr];
            serial_print!("[{}] IOCTL: 0x{:02X} | Handle: 0x{:08X}\n", r.timestamp, r.ioctl_code, r.handle);
            serial_print!("DMA Head: ");
            for b in &r.dma_buffer_head[0..16] {
                serial_print!("{:02X} ", b);
            }
            serial_print!("...\n");
            curr = (curr + 1) % RING_BUFFER_SIZE;
        }
        serial_print!("--- END DUMP ---\n");
    }
}

pub static mut TELEMETRY: TelemetryRingBuffer = TelemetryRingBuffer::new();

/// Función de conveniencia para loggear IOCTLs globalmente
pub unsafe fn log_ioctl(code: u32, handle: u32, dma_ptr: *const u8) {
    TELEMETRY.push(code, handle, dma_ptr);
    
    // El usuario pidió que al llegar a SUBMITCOMMAND, volquemos todo el buffer.
    // 0x04 es D3DKMT_CODE_SUBMITCOMMAND según ioctl/mod.rs
    if code == 0x04 {
        TELEMETRY.dump_history();
        
        // También imprimir todo el DMA buffer crudo de este submit_command específico si está presente
        if !dma_ptr.is_null() {
            serial_print!("--- SUBMITCOMMAND FULL DMA RAW TRACE (64B SAMPLE) ---\n");
            for i in 0..64 {
                if i > 0 && i % 16 == 0 { serial_print!("\n"); }
                serial_print!("{:02X} ", *dma_ptr.add(i));
            }
            serial_print!("\n-----------------------------------------------------\n");
        }
    }
}
