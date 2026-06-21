//! BMO std::net — Operaciones de red.

#![allow(dead_code)]

pub fn tcp_connect(_addr: &str, _port: u16) -> Option<u64> { None }
pub fn tcp_send(_fd: u64, _data: &[u8]) -> Option<usize> { None }
pub fn tcp_recv(_fd: u64, _buf: &mut [u8]) -> Option<usize> { None }
pub fn udp_send(_fd: u64, _data: &[u8], _addr: &str, _port: u16) -> Option<usize> { None }
pub fn udp_recv(_fd: u64, _buf: &mut [u8]) -> Option<usize> { None }
pub fn close(_fd: u64) {}
