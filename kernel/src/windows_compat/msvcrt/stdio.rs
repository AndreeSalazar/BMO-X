//! msvcrt.dll — C stdio (printf, fopen, fclose, etc.).
//!
//! Note: C variadic functions are not stable in no_std, so we provide
//! fixed-argument versions. Real PE binaries that call printf will
//! need a proper variadic implementation (future work).

#![allow(dead_code)]

/// printf — formatted output to stdout (stub).
///
/// Note: Real printf is variadic. This stub accepts a format pointer
/// and returns 0. Full implementation requires variadic support.
#[no_mangle]
pub extern "C" fn printf(_format: u64) -> i32 {
    // TODO: implement printf with fixed args or variadic support
    0
}

/// fprintf — formatted output to file (stub).
#[no_mangle]
pub extern "C" fn fprintf(_file: u64, _format: u64) -> i32 { 0 }

/// sprintf — formatted string output (stub).
#[no_mangle]
pub extern "C" fn sprintf(_buf: u64, _format: u64) -> i32 { 0 }

/// snprintf — formatted string output with size limit (stub).
#[no_mangle]
pub extern "C" fn snprintf(_buf: u64, _size: u64, _format: u64) -> i32 { 0 }

/// fopen — open a file.
#[no_mangle]
pub extern "C" fn fopen(_name: u64, _mode: u64) -> u64 { 0 }

/// fclose — close a file.
#[no_mangle]
pub extern "C" fn fclose(_file: u64) -> i32 { 0 }

/// fread — read from file.
#[no_mangle]
pub extern "C" fn fread(_buf: u64, _size: u64, _count: u64, _file: u64) -> u64 { 0 }

/// fwrite — write to file.
#[no_mangle]
pub extern "C" fn fwrite(_buf: u64, _size: u64, _count: u64, _file: u64) -> u64 { 0 }

/// fgets — read a line from file.
#[no_mangle]
pub extern "C" fn fgets(_buf: u64, _max: i32, _file: u64) -> u64 { 0 }

/// fputs — write a string to file.
#[no_mangle]
pub extern "C" fn fputs(_str: u64, _file: u64) -> i32 { 0 }

/// fseek — seek in file.
#[no_mangle]
pub extern "C" fn fseek(_file: u64, _offset: i64, _origin: i32) -> i32 { 0 }

/// ftell — get file position.
#[no_mangle]
pub extern "C" fn ftell(_file: u64) -> i64 { 0 }

/// fflush — flush file buffer.
#[no_mangle]
pub extern "C" fn fflush(_file: u64) -> i32 { 0 }

/// feof — check end of file.
#[no_mangle]
pub extern "C" fn feof(_file: u64) -> i32 { 0 }

/// ferror — check file error.
#[no_mangle]
pub extern "C" fn ferror(_file: u64) -> i32 { 0 }
