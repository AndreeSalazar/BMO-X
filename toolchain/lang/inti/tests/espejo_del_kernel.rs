//! LOS NUMEROS DE `modulos.toml` SON LOS DEL KERNEL, leidos de su fuente.
//!
//! `modulos.toml` lleva numeros del ABI --operaciones de la puerta, campos de
//! `INFO`, operaciones sobre un prestamo-- y un numero copiado a mano es un
//! numero que un dia deja de ser el mismo sin que nada falle al compilar: el
//! programa pregunta por el campo de al lado y recibe un valor plausible.
//!
//! ** Por eso esta prueba no compara contra una lista escrita aqui: abre el
//! fuente del kernel y busca la constante. Es la misma idea que
//! `los_numeros_son_los_de_rex` en `bmo-orquesta`.
//!
//! Vive en `tests/` y no en `src/` por la regla de `agnostico.rs`: el
//! compilador no mira el kernel, y quien lo comprueba si.

use bmo_inti_front::tablas::Modulos;
use bmo_mods::Roots;
use std::path::{Path, PathBuf};

fn ring0() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("Ultra_kernel_x86-64")
        .join("kernel")
        .join("src")
        .join("ring0")
}

/// El valor de `const NOMBRE: u64 = 0x..;` en ese fichero del kernel.
///
/// ** Busca `const NOMBRE: u64` entero, con los dos puntos: sin ellos,
/// `INFO_CPU_HILOS` encontraria `INFO_CPU_HILOS_POR_NUCLEO` y la prueba
/// aprobaria un numero que no es.
fn del_kernel(fichero: &str, nombre: &str) -> u64 {
    let ruta = ring0().join(fichero);
    let texto = std::fs::read_to_string(&ruta)
        .unwrap_or_else(|e| panic!("no puedo leer {}: {}", ruta.display(), e));
    let aguja = format!("const {}: u64 = ", nombre);
    let desde = texto
        .find(&aguja)
        .unwrap_or_else(|| panic!("{} no esta en {}", nombre, fichero))
        + aguja.len();
    let resto = &texto[desde..];
    let fin = resto.find(';').expect("la constante no termina en `;`");
    let valor = resto[..fin].trim().replace('_', "");
    match valor.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => valor.parse(),
    }
    .unwrap_or_else(|_| panic!("{} = {} no es un numero", nombre, valor))
}

/// nombre en INTI, fichero del kernel, nombre en el kernel.
const ESPEJO: &[(&str, &str, &str)] = &[
    ("op_info", "syscall/ops.rs", "TASK_OP_INFO"),
    ("op_ceder", "syscall/ops.rs", "TASK_OP_YIELD"),
    ("op_ofrecer", "syscall/ops.rs", "MEM_OP_OFRECER"),
    ("op_tomar", "syscall/ops.rs", "TASK_OP_TOMAR"),
    ("op_mi_padre", "syscall/ops.rs", "TASK_OP_MI_PADRE"),
    ("info_tsc_hz", "core/report.rs", "INFO_TSC_HZ"),
    ("info_cpu_hilos", "core/report.rs", "INFO_CPU_HILOS"),
    ("info_ticks", "core/report.rs", "INFO_TICKS"),
    ("info_smp_vivos", "core/report.rs", "INFO_SMP_VIVOS"),
    ("info_cpu_hz_real", "core/report.rs", "INFO_CPU_HZ_REAL"),
    ("info_cpu_mw_paquete", "core/report.rs", "INFO_CPU_MW_PAQUETE"),
    ("info_cpu_mw_nucleo", "core/report.rs", "INFO_CPU_MW_NUCLEO_ACTUAL"),
    ("info_cpu_sensores", "core/report.rs", "INFO_CPU_SENSORES"),
    ("info_puertas", "core/report.rs", "INFO_SYSCALL_CUENTA"),
    ("prestado_base", "obj/loan.rs", "OP_BASE"),
    ("prestado_bytes", "obj/loan.rs", "OP_BYTES"),
    ("prestado_dueno", "obj/loan.rs", "OP_DUENO"),
    ("prestado_soltar", "obj/loan.rs", "OP_SOLTAR"),
];

#[test]
fn los_numeros_del_perfil_y_del_prestamo_son_los_del_kernel() {
    let m = Modulos::cargar(&Roots::find());
    let mut mal = Vec::new();
    for (inti, fichero, kernel) in ESPEJO {
        let tabla = m
            .constante(inti)
            .unwrap_or_else(|| panic!("`{}` no esta en modulos.toml", inti));
        let real = del_kernel(fichero, kernel);
        if tabla != real {
            mal.push(format!("{} = {:#x}, y {} dice {:#x}", inti, tabla, kernel, real));
        }
    }
    assert!(mal.is_empty(), "modulos.toml discrepa del kernel:\n{}", mal.join("\n"));
}

/// ** Y la prueba no se aprueba sola: un numero cambiado a proposito tiene que
/// dar distinto. Sin esto, un `del_kernel` que devolviera siempre lo mismo que la
/// tabla pasaria la prueba de arriba con cualquier kernel.
#[test]
fn el_lector_del_kernel_distingue_dos_constantes() {
    assert_ne!(
        del_kernel("core/report.rs", "INFO_CPU_HZ_REAL"),
        del_kernel("core/report.rs", "INFO_CPU_MW_PAQUETE")
    );
    assert_eq!(del_kernel("core/report.rs", "INFO_CPU_HZ_REAL"), 0x20);
}
