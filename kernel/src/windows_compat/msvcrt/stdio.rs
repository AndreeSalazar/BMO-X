//! msvcrt.dll — C stdio (printf, fopen, fclose, etc.).

#![allow(dead_code)]

/// printf — formatted output to stdout.
#[no_mangle]
pub extern "C" fn printf(_format: u64, _args: ...) -> i32 {
    // TODO: implement printf
    crate::diag::info("wcompat::msvcrt", "printf stub");
    0
}

/// fprintf — formatted output to file.
#[no_mangle]
pub extern "C" fn fprintf(_file: u64, _format: u64, _args: ...) -> i32 { 0 }

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

/// sprintf — formatted string output.
#[no_mangle]
pub extern "C" fn sprintf(_buf: u64, _format: u64, _args: ...) -> i32 { 0 }
