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
//!
//! ** DOS FUENTES desde el 2026-09-16, y la prueba es EXHAUSTIVA. Las
//! operaciones se leen del kernel, como siempre. La FORMA de la superficie
//! (`sup_*`, `evento_*`, `vista_*`) no esta en el kernel --presta bytes y se
//! aparta--: esta en `bmo_abi::syscalls::surface::superficie`, y se lee de
//! ahi. Y toda constante de `[constantes]` TIENE que tener fila: la version
//! anterior comparaba 40 y la 41 (`mi_tarea`) no la miraba nadie, y la que
//! entrara manana tampoco. Una constante sin fila hace fallar la prueba con su
//! nombre.

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

fn abi_superficie() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("platform")
        .join("abi")
        .join("bmo-abi")
        .join("src")
        .join("syscalls")
        .join("surface")
}

/// De donde se lee cada fila: del fuente del kernel, o del ABI.
#[derive(Clone, Copy)]
enum Fuente {
    Kernel(&'static str),
    Abi(&'static str),
}

/// El valor de `const NOMBRE: <tipo> = <numero>;` en ese fichero del kernel.
///
/// ** Busca `const NOMBRE:` entero, con los dos puntos: sin ellos,
/// `INFO_CPU_HILOS` encontraria `INFO_CPU_HILOS_POR_NUCLEO` y la prueba
/// aprobaria un numero que no es.
///
/// Entiende tres formas, las que el kernel usa: decimal, `0x..` y `1 << n` (las
/// banderas). Cualquier otra cosa hace fallar la prueba en vez de adivinar.
fn del_kernel(fichero: &str, nombre: &str) -> u64 {
    constante_en(ring0().join(fichero), fichero, nombre)
}

fn del_abi(fichero: &str, nombre: &str) -> u64 {
    constante_en(abi_superficie().join(fichero), fichero, nombre)
}

fn constante_en(ruta: PathBuf, fichero: &str, nombre: &str) -> u64 {
    let texto = std::fs::read_to_string(&ruta)
        .unwrap_or_else(|e| panic!("no puedo leer {}: {}", ruta.display(), e));
    let aguja = format!("const {}:", nombre);
    let desde = texto
        .find(&aguja)
        .unwrap_or_else(|| panic!("{} no esta en {}", nombre, fichero))
        + aguja.len();
    let resto = &texto[desde..];
    let igual = resto.find('=').expect("la constante no tiene `=`") + 1;
    let fin = resto.find(';').expect("la constante no termina en `;`");
    let valor = resto[igual..fin].trim().replace('_', "");
    let numero = |s: &str| -> u64 {
        let s = s.trim();
        match s.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16),
            None => s.parse(),
        }
        .unwrap_or_else(|_| panic!("{} = {} no es un numero", nombre, valor))
    };
    match valor.split_once("<<") {
        Some((a, b)) => numero(a) << numero(b),
        None => numero(&valor),
    }
}

/// nombre en INTI, de donde se lee, nombre alli.
const ESPEJO: &[(&str, Fuente, &str)] = &[
    ("mi_tarea", Fuente::Abi("puertas.rs"), "CURRENT_TASK"),
    ("op_info", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_INFO"),
    ("op_consola_escribir", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_CONSOLE_WRITE"),
    ("op_ruta", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_RUTA"),
    ("op_pedir_memoria", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_MEMORIA_PEDIR"),
    ("op_base_del_bloque", Fuente::Kernel("obj/memory.rs"), "MEM_OP_BASE"),
    ("op_archivo_abrir", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_ARCHIVO_ABRIR"),
    ("op_archivo_crear", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_ARCHIVO_CREAR"),
    ("op_arch_tamano", Fuente::Kernel("obj/file.rs"), "ARCH_OP_TAMANO"),
    ("op_arch_leer_en", Fuente::Kernel("obj/file.rs"), "ARCH_OP_LEER_EN"),
    ("op_arch_escribir", Fuente::Kernel("obj/file.rs"), "ARCH_OP_ESCRIBIR"),
    ("op_arch_escribir_de", Fuente::Kernel("obj/file.rs"), "ARCH_OP_ESCRIBIR_DE"),
    ("op_arch_cerrar", Fuente::Kernel("obj/file.rs"), "ARCH_OP_CERRAR"),
    ("op_ofrecer", Fuente::Kernel("syscall/ops.rs"), "MEM_OP_OFRECER"),
    ("op_tomar", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_TOMAR"),
    ("op_mi_padre", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_MI_PADRE"),
    ("info_tsc_hz", Fuente::Kernel("core/report.rs"), "INFO_TSC_HZ"),
    ("info_cpu_hilos", Fuente::Kernel("core/report.rs"), "INFO_CPU_HILOS"),
    ("info_ticks", Fuente::Kernel("core/report.rs"), "INFO_TICKS"),
    ("info_smp_vivos", Fuente::Kernel("core/report.rs"), "INFO_SMP_VIVOS"),
    ("info_cpu_uj_paquete", Fuente::Kernel("core/report.rs"), "INFO_CPU_UJ_PAQUETE"),
    ("info_cpu_uj_nucleo", Fuente::Kernel("core/report.rs"), "INFO_CPU_UJ_NUCLEO"),
    ("info_cpu_mperf", Fuente::Kernel("core/report.rs"), "INFO_CPU_MPERF"),
    ("info_cpu_aperf", Fuente::Kernel("core/report.rs"), "INFO_CPU_APERF"),
    ("info_cpu_sensores", Fuente::Kernel("core/report.rs"), "INFO_CPU_SENSORES"),
    ("info_puertas", Fuente::Kernel("core/report.rs"), "INFO_SYSCALL_CUENTA"),
    ("info_mem_quien_pid", Fuente::Kernel("core/report.rs"), "INFO_MEM_QUIEN_PID"),
    ("error_no_existe", Fuente::Kernel("syscall/ops.rs"), "ERROR_UNSUPPORTED"),
    ("error_sin_permiso", Fuente::Kernel("obj/cap.rs"), "ERROR_PERMISSION_DENIED"),
    ("bandera_falta_capability", Fuente::Kernel("obj/cap.rs"), "FLAG_NEEDS_CAP"),
    ("prestado_base", Fuente::Kernel("obj/loan.rs"), "OP_BASE"),
    ("prestado_bytes", Fuente::Kernel("obj/loan.rs"), "OP_BYTES"),
    ("prestado_dueno", Fuente::Kernel("obj/loan.rs"), "OP_DUENO"),
    ("prestado_soltar", Fuente::Kernel("obj/loan.rs"), "OP_SOLTAR"),
    ("op_argumentos", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_ARGUMENTOS"),
    ("op_sonido_reclamar", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_AUDIO_CLAIM"),
    ("op_sonido_soltar", Fuente::Kernel("syscall/ops.rs"), "TASK_OP_AUDIO_RELEASE"),
    ("sonido_pitar", Fuente::Kernel("obj/audio.rs"), "AUDIO_OP_BEEP"),
    ("sonido_callar", Fuente::Kernel("obj/audio.rs"), "AUDIO_OP_SILENCE"),
    ("sonido_tubo", Fuente::Kernel("obj/audio.rs"), "AUDIO_OP_TUBO"),
    ("sup_magic", Fuente::Abi("superficie.rs"), "SUP_MAGIC"),
    ("sup_cabecera", Fuente::Abi("superficie.rs"), "SUP_CABECERA"),
    ("sup_bgra32", Fuente::Abi("superficie.rs"), "SUP_BGRA32"),
    ("sup_campo_secuencia", Fuente::Abi("superficie.rs"), "SUP_CAMPO_SECUENCIA"),
    ("sup_buzon_cabecera", Fuente::Abi("superficie.rs"), "SUP_BUZON_CABECERA"),
    ("sup_buzon_ranura", Fuente::Abi("superficie.rs"), "SUP_BUZON_RANURA"),
    ("evento_raton", Fuente::Abi("superficie.rs"), "SUP_EV_RATON"),
    ("evento_letra", Fuente::Abi("superficie.rs"), "SUP_EV_CARACTER"),
    ("evento_configurar", Fuente::Abi("superficie.rs"), "SUP_EV_CONFIGURE"),
    ("estado_ventana", Fuente::Abi("superficie.rs"), "SUP_ESTADO_VENTANA"),
    ("estado_maximizada", Fuente::Abi("superficie.rs"), "SUP_ESTADO_MAXIMIZADA"),
    ("estado_completa", Fuente::Abi("superficie.rs"), "SUP_ESTADO_COMPLETA"),
    ("sup_tomada", Fuente::Abi("superficie.rs"), "SUP_TOMADA"),
    ("vista_se_ve", Fuente::Abi("superficie.rs"), "SUP_VISTA_SE_VE"),
    ("vista_minimizada", Fuente::Abi("superficie.rs"), "SUP_VISTA_MINIMIZADA"),
    ("vista_fuera", Fuente::Abi("superficie.rs"), "SUP_VISTA_FUERA"),
    ("vista_prestada", Fuente::Abi("superficie.rs"), "SUP_VISTA_PRESTADA"),
    ("vista_tapada", Fuente::Abi("superficie.rs"), "SUP_VISTA_TAPADA"),
];

#[test]
fn los_numeros_del_perfil_y_del_prestamo_son_los_del_kernel() {
    let m = Modulos::cargar(&Roots::find());
    let mut mal = Vec::new();
    for (inti, fuente, nombre) in ESPEJO {
        let tabla = m
            .constante(inti)
            .unwrap_or_else(|| panic!("`{}` no esta en modulos.toml", inti));
        let real = match fuente {
            Fuente::Kernel(f) => del_kernel(f, nombre),
            Fuente::Abi(f) => del_abi(f, nombre),
        };
        if tabla != real {
            mal.push(format!("{} = {:#x}, y {} dice {:#x}", inti, tabla, nombre, real));
        }
    }
    assert!(mal.is_empty(), "modulos.toml discrepa del kernel o del ABI:\n{}", mal.join("\n"));
}

/// ** EXHAUSTIVA: una constante de `[constantes]` sin fila aqui es un numero
/// copiado a mano que nadie juzga -- que es exactamente lo que esta prueba
/// existe para que no haya. Falla con el nombre de la que falta.
#[test]
fn toda_constante_de_la_tabla_tiene_espejo() {
    let m = Modulos::cargar(&Roots::find());
    let con_fila: std::collections::HashSet<&str> = ESPEJO.iter().map(|(n, _, _)| *n).collect();
    let mut sin: Vec<String> = m
        .constantes()
        .into_iter()
        .filter(|n| !con_fila.contains(n.as_str()))
        .collect();
    sin.sort();
    assert!(
        sin.is_empty(),
        "constante(s) de modulos.toml sin fila en ESPEJO -- nadie las compara con nada: {}",
        sin.join(", ")
    );
    assert_eq!(ESPEJO.len(), m.constantes().len(), "y ninguna fila sobra");
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
