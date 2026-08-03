//! Formatted output — vsnprintf with %d, %u, %x, %X, %s, %c, %p, %%.
//!
//! No heap, no allocation, pure stack. Designed for Ring 3 BEF apps.

/// Write a character to the output buffer.
fn put(buf: &mut [u8], pos: &mut usize, c: u8) {
    if *pos + 1 < buf.len() {
        buf[*pos] = c;
        *pos += 1;
    }
}

/// Write a string to the output buffer.
fn puts(buf: &mut [u8], pos: &mut usize, s: &str) {
    for &c in s.as_bytes() {
        put(buf, pos, c);
    }
}

/// Write an unsigned integer in the given radix (2-36).
fn putu(buf: &mut [u8], pos: &mut usize, mut v: u64, radix: u32, uppercase: bool) {
    if v == 0 {
        put(buf, pos, b'0');
        return;
    }
    let mut tmp: [u8; 21] = [0; 21];
    let mut i = 21;
    while v > 0 && i > 0 {
        i -= 1;
        let d = (v % radix as u64) as u8;
        tmp[i] = if d < 10 {
            b'0' + d
        } else if uppercase {
            b'A' + (d - 10)
        } else {
            b'a' + (d - 10)
        };
        v /= radix as u64;
    }
    while i < 21 {
        put(buf, pos, tmp[i]);
        i += 1;
    }
}

/// Write a signed integer.
fn puti(buf: &mut [u8], pos: &mut usize, v: i64) {
    if v < 0 {
        put(buf, pos, b'-');
        putu(buf, pos, (-(v as i128)) as u64, 10, false);
    } else {
        putu(buf, pos, v as u64, 10, false);
    }
}

/// Write a pointer address.
fn putp(buf: &mut [u8], pos: &mut usize, p: usize) {
    if p == 0 {
        puts(buf, pos, "(nil)");
    } else {
        puts(buf, pos, "0x");
        putu(buf, pos, p as u64, 16, false);
    }
}

/// Build a va_list from variadic args (x86-64 SysV ABI).
///
/// On x86-64, the first 6 integer args go in rdi, rsi, rdx, rcx, r8, r9.
/// The rest go on the stack. Floats go in xmm0-xmm7. Since we're in a
/// #![no_std] context without the full C ABI support, we provide a
/// simplified va_list that reads from a raw pointer to the stack args.
///
/// Callers pass the address of their first variadic argument.
pub struct VaListSimple {
    ptr: *const u64,
    count: u32,
}

impl VaListSimple {
    #[inline]
    pub unsafe fn from_ptr(args: *const u64) -> Self {
        Self { ptr: args, count: 0 }
    }

    /// Read the next u64 from the va_list.
    pub unsafe fn next_u64(&mut self) -> u64 {
        let v = *self.ptr;
        self.ptr = self.ptr.add(1);
        self.count += 1;
        v
    }

    /// Read the next pointer.
    pub unsafe fn next_ptr<T>(&mut self) -> *const T {
        self.next_u64() as *const T
    }
}

/// Core formatted print engine. Supports:
/// - %d, %i: signed decimal
/// - %u: unsigned decimal
/// - %x: lowercase hex
/// - %X: uppercase hex
/// - %s: null-terminated string
/// - %c: character
/// - %p: pointer
/// - %%: literal percent
/// - %ld, %lld: long variants (treated as 64-bit on x86-64)
pub fn vsnprintf(buf: &mut [u8], fmt: &str, args: *const u64) -> usize {
    let mut pos = 0;
    let mut va = unsafe { VaListSimple::from_ptr(args) };
    let bytes = fmt.as_bytes();
    let mut i = 0;

    while i < bytes.len() && pos < buf.len() - 1 {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            i += 1;
            if i >= bytes.len() { break; }

            // Skip length modifiers (l, ll)
            if bytes[i] == b'l' {
                i += 1;
                if i < bytes.len() && bytes[i] == b'l' { i += 1; }
            }
            if i >= bytes.len() { break; }

            match bytes[i] {
                b'd' | b'i' => {
                    let v = unsafe { va.next_u64() } as i64;
                    puti(buf, &mut pos, v);
                }
                b'u' => {
                    let v = unsafe { va.next_u64() };
                    putu(buf, &mut pos, v, 10, false);
                }
                b'x' => {
                    let v = unsafe { va.next_u64() };
                    putu(buf, &mut pos, v, 16, false);
                }
                b'X' => {
                    let v = unsafe { va.next_u64() };
                    putu(buf, &mut pos, v, 16, true);
                }
                b's' => {
                    let ptr = unsafe { va.next_ptr::<u8>() };
                    if !ptr.is_null() {
                        let len = unsafe { crate::string::strlen(ptr) };
                        for j in 0..len.min(256) {
                            put(buf, &mut pos, unsafe { *ptr.add(j) });
                        }
                    } else {
                        puts(buf, &mut pos, "(null)");
                    }
                }
                b'c' => {
                    let v = unsafe { va.next_u64() } as u8;
                    put(buf, &mut pos, v);
                }
                b'p' => {
                    let v = unsafe { va.next_u64() } as usize;
                    putp(buf, &mut pos, v);
                }
                b'%' => {
                    put(buf, &mut pos, b'%');
                }
                _ => {
                    put(buf, &mut pos, bytes[i]);
                }
            }
        } else {
            put(buf, &mut pos, bytes[i]);
        }
        i += 1;
    }

    buf[pos] = 0;
    pos
}

/// Print formatted output to a buffer.
pub fn snprintf(buf: &mut [u8], fmt: &str, args: *const u64) -> usize {
    vsnprintf(buf, fmt, args)
}

/// C-compatible printf: reads format string from RDI and variadic args
/// from the stack (x86-64 SysV ABI). Designed to be called by the C frontend.
///
/// The C codegen emits:
///   - RDI = format string pointer
///   - pushes variadic args right-to-left
///   - calls this function
#[no_mangle]
pub unsafe extern "C" fn bmo_printf(fmt: *const u8, num_args: u64, args_ptr: *const u64) -> i32 {
    if fmt.is_null() { return 0; }
    let mut buf: [u8; 1024] = [0; 1024];
    let mut pos = 0;
    let mut arg_idx = 0;
    let mut i = 0;

    while i < 8096 {
        let c = *fmt.add(i);
        if c == 0 { break; }
        if c == b'%' && *fmt.add(i + 1) != 0 {
            i += 1;
            let ch = *fmt.add(i);
            if ch == b'%' {
                put(&mut buf, &mut pos, b'%');
            } else {
                let v = if arg_idx < num_args { *args_ptr.add(arg_idx as usize) } else { 0u64 };
                arg_idx += 1;
                match ch {
                    b'd' | b'i' => puti(&mut buf, &mut pos, v as i64),
                    b'u' => putu(&mut buf, &mut pos, v, 10, false),
                    b'x' => putu(&mut buf, &mut pos, v, 16, false),
                    b's' => {
                        let sp = v as *const u8;
                        if !sp.is_null() {
                            let len = crate::string::strlen(sp);
                            for j in 0..len.min(256) { put(&mut buf, &mut pos, *sp.add(j)); }
                        }
                    }
                    b'c' => put(&mut buf, &mut pos, v as u8),
                    b'p' => putp(&mut buf, &mut pos, v as usize),
                    _ => { put(&mut buf, &mut pos, b'%'); put(&mut buf, &mut pos, ch); }
                }
            }
        } else {
            put(&mut buf, &mut pos, c);
        }
        i += 1;
    }
    buf[pos] = 0;
    crate::syscall::debug_print(buf.as_ptr(), pos as u64);
    pos as i32
}

/// Format into a heap-allocated string (malloc).
/// Caller must free() the result.
pub fn sprintf(fmt: &str, args: *const u64) -> *mut u8 {
    let mut buf: [u8; 4096] = [0; 4096];
    let len = vsnprintf(&mut buf, fmt, args);
    let p = crate::heap::malloc(len + 1) as *mut u8;
    if p.is_null() { return p; }
    unsafe {
        crate::string::memcpy(p, buf.as_ptr(), len + 1);
    }
    p
}
