//! LA CONVENCION: la tabla de la maquina dice lo mismo que el contrato
//!
//! `arch/x86_64/inti.toml` repite, para quien lee, que registro es cada
//! argumento (`orden = k`) y cuales se preservan. Hasta el 2026-09-19 decia
//! CINCO argumentos --le faltaba el cuarto, `rcx`-- mientras el emisor usaba
//! seis y `bmo-abi` declaraba siete. Aqui la tabla se compara con
//! `bmo_abi::types::convention`, que es lo que el emisor de verdad importa.

use bmo_abi::types::convention::{ARGUMENTOS, PRESERVADOS};

/// Las filas de `[registros]`: (numero, rol, orden).
fn filas() -> Vec<(u8, String, Option<usize>)> {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../forge/sem-asm/tables/arch/x86_64/inti.toml");
    let texto = std::fs::read_to_string(&ruta).expect("la tabla de la maquina");
    let tabla: toml::Table = texto.parse().expect("la tabla es TOML");
    let registros = tabla["registros"].as_table().expect("[registros]");
    registros
        .values()
        .map(|v| {
            let n = v["n"].as_integer().unwrap() as u8;
            let rol = v["rol"].as_str().unwrap().to_string();
            let orden = v.get("orden").and_then(|o| o.as_integer()).map(|o| o as usize);
            (n, rol, orden)
        })
        .collect()
}

#[test]
fn la_tabla_dice_los_seis_argumentos_del_contrato() {
    let mut por_orden = [None; 6];
    for (n, _, orden) in filas() {
        if let Some(k) = orden {
            assert!((1..=6).contains(&k), "orden {k} fuera de 1..=6");
            assert!(por_orden[k - 1].is_none(), "dos registros con orden {k}");
            por_orden[k - 1] = Some(n);
        }
    }
    let tabla: Vec<u8> = por_orden.iter().map(|o| o.expect("falta un argumento en la tabla")).collect();
    assert_eq!(tabla, ARGUMENTOS, "inti.toml y el contrato no dicen los mismos argumentos");
}

#[test]
fn la_tabla_dice_los_preservados_del_contrato() {
    let mut tabla: Vec<u8> = filas()
        .into_iter()
        .filter(|(_, rol, _)| rol == "preservado" || rol == "marco")
        .map(|(n, _, _)| n)
        .collect();
    tabla.sort();
    let mut contrato = PRESERVADOS.to_vec();
    contrato.sort();
    assert_eq!(tabla, contrato);
}
