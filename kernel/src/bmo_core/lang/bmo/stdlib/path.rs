//! BMO std::path — Manipulación de rutas.

#![allow(dead_code)]

use alloc::string::String;

pub fn join(a: &str, b: &str) -> String {
    if a.ends_with('/') {
        let mut r = String::with_capacity(a.len() + b.len());
        r.push_str(a);
        r.push_str(b);
        r
    } else {
        let mut r = String::with_capacity(a.len() + 1 + b.len());
        r.push_str(a);
        r.push('/');
        r.push_str(b);
        r
    }
}

pub fn parent(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) => "/",
        Some(i) => &path[..i],
        None => ".",
    }
}

pub fn filename(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

pub fn extension(path: &str) -> &str {
    let name = filename(path);
    match name.rfind('.') {
        Some(0) => "",
        Some(i) => &name[i + 1..],
        None => "",
    }
}

pub fn stem(path: &str) -> &str {
    let name = filename(path);
    match name.rfind('.') {
        Some(0) => name,
        Some(i) => &name[..i],
        None => name,
    }
}

pub fn is_absolute(path: &str) -> bool { path.starts_with('/') }

pub fn normalize(path: &str) -> String {
    let mut parts: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    let mut result = String::new();
    let absolute = path.starts_with('/');

    for component in path.split('/') {
        match component {
            "" | "." => continue,
            ".." => { parts.pop(); }
            seg => parts.push(seg),
        }
    }

    if absolute { result.push('/'); }
    for (i, part) in parts.iter().enumerate() {
        if i > 0 { result.push('/'); }
        result.push_str(part);
    }

    if result.is_empty() { result.push('.'); }
    result
}
