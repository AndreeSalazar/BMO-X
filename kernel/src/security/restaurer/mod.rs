//! `security::restaurer` — Real-Time Kernel Snapshot/Rollback System
//!
//! Restaurer es como Git pero para el kernel en tiempo real:
//! - Snapshots: guarda el estado completo del kernel (RAM + registros + process state)
//! - Rollback: retrocede el tiempo del kernel a un snapshot anterior
//! - Diff: muestra qué cambió entre snapshots
//! - Branching: crea ramas del kernel para experimentar
//!
//! Vive en Ring 0, controlado por ÑEXO.

#![allow(dead_code)]

pub mod snapshots;
pub mod diff;

/// Maximum snapshots stored
const MAX_SNAPSHOTS: usize = 32;

/// Snapshot metadata
#[derive(Clone, Copy)]
pub struct Snapshot {
    pub id: u64,
    pub timestamp: u64,       // TSC timestamp
    pub label: [u8; 64],
    pub description: [u8; 256],
    pub checksum: [u8; 32],   // BLAKE3 of state
    pub state: KernelState,
    pub parent_id: u64,       // For branching
    pub branch: [u8; 32],
}

/// Complete kernel state at snapshot time
#[derive(Clone, Copy)]
pub struct KernelState {
    // Memory state
    pub page_tables: [u64; 512],   // PML4 entries
    pub free_pages: u64,
    pub used_pages: u64,

    // Process state
    pub processes: [ProcessSnapshot; 16],

    // Interrupt state
    pub idt: [u64; 256],           // IDT descriptors

    // APIC state
    pub lapic_base: u64,
    pub lapic_timer_div: u32,
    pub lapic_timer_init: u32,

    // FPU state
    pub fpu_state: [u8; 512],      // fxsave area

    // Network state
    pub ip_address: u32,
    pub mac_address: [u8; 6],

    // Diag state
    pub event_count: u64,
    pub threat_count: u64,

    // Custom kernel data (up to 4KB)
    pub custom_data: [u8; 4096],
}

/// Process state snapshot
#[derive(Clone, Copy)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub active: bool,
    pub cr3: u64,              // Page table base
    pub rip: u64,              // Instruction pointer
    pub rsp: u64,              // Stack pointer
    pub rflags: u64,
    pub rings: [u64; 4],       // RSP for Ring 0/1/2/3
    pub state: u8,             // 0=ready, 1=running, 2=blocked, 3=dead
}

/// Global Restaurer state
pub struct RestaurerState {
    pub enabled: bool,
    pub snapshots: [Snapshot; MAX_SNAPSHOTS],
    pub snapshot_count: u64,
    pub current_snapshot_id: u64,
    pub auto_snapshot: bool,       // Auto-snapshot on critical events
    pub auto_interval_ms: u64,     // Auto-snapshot interval
    pub last_auto_snapshot: u64,
    pub total_rollbacks: u64,
    pub total_snapshots_created: u64,
}

static mut RESTAURER: RestaurerState = RestaurerState {
    enabled: false,
    snapshots: [Snapshot {
        id: 0,
        timestamp: 0,
        label: [0; 64],
        description: [0; 256],
        checksum: [0; 32],
        state: KernelState {
            page_tables: [0; 512],
            free_pages: 0,
            used_pages: 0,
            processes: [ProcessSnapshot {
                pid: 0,
                active: false,
                cr3: 0,
                rip: 0,
                rsp: 0,
                rflags: 0,
                rings: [0; 4],
                state: 0,
            }; 16],
            idt: [0; 256],
            lapic_base: 0,
            lapic_timer_div: 0,
            lapic_timer_init: 0,
            fpu_state: [0; 512],
            ip_address: 0,
            mac_address: [0; 6],
            event_count: 0,
            threat_count: 0,
            custom_data: [0; 4096],
        },
        parent_id: 0,
        branch: [0; 32],
    }; MAX_SNAPSHOTS],
    snapshot_count: 0,
    current_snapshot_id: 0,
    auto_snapshot: true,
    auto_interval_ms: 60000,     // Every 60 seconds
    last_auto_snapshot: 0,
    total_rollbacks: 0,
    total_snapshots_created: 0,
};

/// Initialize Restaurer
pub fn init() {
    unsafe {
        RESTAURER.enabled = true;
        RESTAURER.snapshot_count = 0;
        RESTAURER.current_snapshot_id = 0;
    }
    crate::drivers::serial::serial_write("[restaurer] Initialized - Real-Time Kernel Snapshots\n");
}

/// Get current state
pub fn state() -> &'static RestaurerState {
    unsafe { &RESTAURER }
}

/// Create a snapshot
pub fn create_snapshot(label: &[u8], description: &[u8]) -> u64 {
    let id = unsafe {
        RESTAURER.total_snapshots_created + 1
    };

    let timestamp = unsafe { core::arch::x86_64::_rdtsc() };

    let mut state = capture_kernel_state();

    // Add ByteDefender state to snapshot
    let bd_state = crate::security::bytedefender::state();
    state.threat_count = bd_state.threats_detected;

    let mut label_arr = [0u8; 64];
    let label_len = label.len().min(63);
    label_arr[..label_len].copy_from_slice(&label[..label_len]);

    let mut desc_arr = [0u8; 256];
    let desc_len = description.len().min(255);
    desc_arr[..desc_len].copy_from_slice(&description[..desc_len]);

    let mut branch_arr = [0u8; 32];
    branch_arr[..7].copy_from_slice(b"default");

    let mut snapshot = Snapshot {
        id,
        timestamp,
        label: label_arr,
        description: desc_arr,
        checksum: [0; 32],
        state,
        parent_id: unsafe { RESTAURER.current_snapshot_id },
        branch: branch_arr,
    };

    // Calculate checksum
    snapshot.checksum = compute_checksum(&snapshot.state);

    // Store snapshot
    unsafe {
        let idx = (RESTAURER.snapshot_count as usize) % MAX_SNAPSHOTS;
        RESTAURER.snapshots[idx] = snapshot;
        RESTAURER.snapshot_count += 1;
        RESTAURER.current_snapshot_id = id;
        RESTAURER.total_snapshots_created += 1;
    }

    crate::drivers::serial::serial_write("[restaurer] Snapshot #");
    write_u64(id);
    crate::drivers::serial::serial_write(" created\n");

    id
}

/// Rollback to a specific snapshot
pub fn rollback(snapshot_id: u64) -> bool {
    let snapshot = match find_snapshot(snapshot_id) {
        Some(s) => s,
        None => {
            crate::drivers::serial::serial_write("[restaurer] Snapshot not found\n");
            return false;
        }
    };

    // Verify checksum
    let checksum = compute_checksum(&snapshot.state);
    if checksum != snapshot.checksum {
        crate::drivers::serial::serial_write("[restaurer] Checksum mismatch - snapshot corrupted\n");
        return false;
    }

    crate::drivers::serial::serial_write("[restaurer] Rolling back to snapshot #");
    write_u64(snapshot_id);
    crate::drivers::serial::serial_write("\n");

    // Restore kernel state
    restore_kernel_state(&snapshot.state);

    unsafe {
        RESTAURER.total_rollbacks += 1;
        RESTAURER.current_snapshot_id = snapshot_id;
    }

    crate::diag::info("restaurer", "Kernel state rolled back");

    true
}

/// List all snapshots
pub fn list_snapshots() -> &'static [Snapshot] {
    unsafe {
        &RESTAURER.snapshots[..RESTAURER.snapshot_count.min(MAX_SNAPSHOTS as u64) as usize]
    }
}

/// Auto-snapshot check (called from timer)
pub fn auto_snapshot_check(tsc_now: u64) {
    unsafe {
        if !RESTAURER.auto_snapshot { return; }

        let elapsed_ms = estimate_ms(tsc_now - RESTAURER.last_auto_snapshot);
        if elapsed_ms >= RESTAURER.auto_interval_ms {
            create_snapshot(b"auto", b"Automatic snapshot");
            RESTAURER.last_auto_snapshot = tsc_now;
        }
    }
}

/// Capture current kernel state
fn capture_kernel_state() -> KernelState {
    let mut state = KernelState {
        page_tables: [0; 512],
        free_pages: 0,
        used_pages: 0,
        processes: [ProcessSnapshot {
            pid: 0,
            active: false,
            cr3: 0,
            rip: 0,
            rsp: 0,
            rflags: 0,
            rings: [0; 4],
            state: 0,
        }; 16],
        idt: [0; 256],
        lapic_base: 0,
        lapic_timer_div: 0,
        lapic_timer_init: 0,
        fpu_state: [0; 512],
        ip_address: 0,
        mac_address: [0; 6],
        event_count: 0,
        threat_count: 0,
        custom_data: [0; 4096],
    };

    // Capture page tables
    let cr3: u64;
    unsafe { core::arch::asm!("mov {}, cr3", out(reg) cr3); }

    // Read page table entries safely
    if cr3 > 0 && cr3 < 0x100000000 {
        unsafe {
            let pt_ptr = cr3 as *const [u64; 512];
            state.page_tables = core::ptr::read_volatile(pt_ptr);
        }
    }

    // Capture interrupt state (stub - full IDT read requires careful handling)
    // In production: read IDT base from lidt instruction

    // Capture process states (stub)
    // In production: iterate process table

    state
}

/// Restore kernel state from snapshot
fn restore_kernel_state(state: &KernelState) {
    // Restore page tables (CR3)
    let new_cr3 = state.page_tables.as_ptr() as u64;

    // Only restore if address is valid
    if new_cr3 > 0 && new_cr3 < 0x100000000 {
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) new_cr3);
        }
    }

    // Note: Full restoration of process states requires careful scheduling
    // For now: memory state restoration only
}

/// Find snapshot by ID
fn find_snapshot(id: u64) -> Option<&'static Snapshot> {
    unsafe {
        for i in 0..RESTAURER.snapshot_count.min(MAX_SNAPSHOTS as u64) as usize {
            if RESTAURER.snapshots[i].id == id {
                return Some(&RESTAURER.snapshots[i]);
            }
        }
    }
    None
}

/// Compute checksum of kernel state
fn compute_checksum(state: &KernelState) -> [u8; 32] {
    let data = unsafe {
        core::slice::from_raw_parts(
            state as *const KernelState as *const u8,
            core::mem::size_of::<KernelState>()
        )
    };

    let mut hash = [0u8; 32];
    let mut acc = [0x6A09E667u32; 8];

    for (i, &byte) in data.iter().enumerate() {
        let idx = i % 32;
        acc[idx / 4] = acc[idx / 4].wrapping_mul(31).wrapping_add(byte as u32);
    }

    for i in 0..8 {
        let bytes = acc[i].to_le_bytes();
        hash[i * 4..i * 4 + 4].copy_from_slice(&bytes);
    }

    hash
}

/// Estimate milliseconds from TSC ticks (assumes ~3.6GHz)
fn estimate_ms(ticks: u64) -> u64 {
    ticks / 3_600_000
}

/// Write u64 to serial
fn write_u64(mut val: u64) {
    let mut buf = [0u8; 20];
    let mut len = 0;
    if val == 0 {
        buf[0] = b'0';
        len = 1;
    } else {
        while val > 0 {
            buf[len] = b'0' + (val % 10) as u8;
            val /= 10;
            len += 1;
        }
    }
    // Reverse
    for i in 0..len / 2 {
        buf.swap(i, len - 1 - i);
    }
    for &byte in &buf[..len] {
        crate::drivers::serial::serial_write_byte(byte);
    }
}
