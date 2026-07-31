/* semantic/cpu.h — el estado del procesador.
 *
 * Registros de control, MSR, el contador de ciclos y las interrupciones. Casi
 * todo esto es **Ring 0**: desde Ring 3 da #GP, y eso es lo correcto — son las
 * palancas con las que un kernel decide qué puede hacer todo lo demás.
 */
#ifndef SEMANTIC_CPU_H
#define SEMANTIC_CPU_H

#include <semantic/tipos.h>

/* ── Bits de CR0 que importan ── */
#define CR0_PE 0x00000001 /* modo protegido */
#define CR0_MP 0x00000002 /* monitorizar el coprocesador */
#define CR0_EM 0x00000004 /* emular FPU: si esta a 1, SSE da #UD */
#define CR0_TS 0x00000008 /* tarea cambiada: la FPU perezosa. BMO NO lo usa */
#define CR0_WP 0x00010000 /* el kernel respeta las paginas de solo lectura */
#define CR0_PG 0x80000000 /* paginacion */

/* ── Bits de CR4 ── */
#define CR4_PSE 0x00000010    /* paginas de 4 MiB */
#define CR4_PAE 0x00000020    /* extension de direcciones fisicas */
#define CR4_PGE 0x00000080    /* paginas globales: no se vacian con CR3 */
#define CR4_OSFXSR 0x00000200 /* el SO sabe guardar el estado SSE */
#define CR4_OSXSAVE 0x00040000 /* el SO sabe usar XSAVE */
#define CR4_SMEP 0x00100000   /* Ring 0 no ejecuta paginas de usuario */
#define CR4_SMAP 0x00200000   /* Ring 0 no LEE paginas de usuario sin permiso */

/* ── El bit de RFLAGS que se mira ── */
#define FLAG_IF 0x00000200 /* interrupciones activas */

u64 cpu_cr0() { return __cr0(); }
void cpu_poner_cr0(u64 v) { __set_cr0(v); }

/* CR2 = la direccion que causo el ultimo fallo de pagina. Se lee DENTRO del
 * manejador y antes de nada: cualquier otro #PF la pisa. */
u64 cpu_cr2() { return __cr2(); }

u64 cpu_cr3() { return __cr3(); }

/* ★ Escribir CR3 cambia de espacio de direcciones **y vacia el TLB entero**
 * (salvo las paginas globales). No es gratis: son dos vaciados por cada ida y
 * vuelta, y por eso el camino de teclado de BMO lo evita cuando puede. */
void cpu_poner_cr3(u64 raiz) { __set_cr3(raiz); }

u64 cpu_cr4() { return __cr4(); }
void cpu_poner_cr4(u64 v) { __set_cr4(v); }

/* ── MSR ── */
u64 msr_leer(u32 nr) { return __rdmsr(nr); }
void msr_escribir(u32 nr, u64 v) { __wrmsr(nr, v); }

/* ── Tiempo ──
 *
 * `ciclos()` es rapido y NO serializa: el CPU puede adelantarlo o retrasarlo
 * respecto a lo que hay alrededor, asi que medir un trozo corto con el da
 * numeros que no son. `ciclos_exactos()` (rdtscp) espera a que lo de antes
 * termine, y es el que sirve para medir. */
u64 ciclos() { return __rdtsc(); }
u64 ciclos_exactos() { return __rdtscp(); }

/* ── Interrupciones ──
 *
 * ★ `sin_interrupciones()` NO se anida. Dos secciones criticas una dentro de
 * otra y la de dentro las vuelve a encender al salir, dejando a la de fuera
 * desprotegida sin decir nada. La forma correcta es guardar RFLAGS antes y
 * restaurarlo:
 *
 *     u64 antes = cpu_flags();
 *     sin_interrupciones();
 *     ... lo delicado ...
 *     cpu_poner_flags(antes);
 */
void sin_interrupciones() { __cli(); }
void con_interrupciones() { __sti(); }
u64 cpu_flags() { return __flags(); }
void cpu_poner_flags(u64 v) { __set_flags(v); }

/* ── Estado extendido (XSAVE) ── */
u64 xcr_leer(u32 indice) { return __xgetbv(indice); }
void xcr_escribir(u32 indice, u64 v) { __xsetbv(indice, v); }

/* ── Parar ──
 *
 * `esperar()` para el nucleo hasta la siguiente interrupcion. Con las
 * interrupciones apagadas **no vuelve nunca**: eso es como se apaga una maquina
 * a proposito, y como se cuelga sin querer. */
void esperar() { __hlt(); }

/* Instruccion invalida a proposito: dispara #UD. Es como se marca un camino al
 * que no se debe llegar, y se distingue de una direccion basura porque el
 * manejador sabe que fue deliberado. */
void imposible() { __ud2(); }

/* Una hoja de CPUID. Hoy solo devuelve eax; ebx/ecx/edx piden que el mecanismo
 * de la tabla sepa devolver varios registros, y eso todavia no esta. */
u32 cpuid_eax(u32 hoja) { return __cpuid(hoja); }

#endif /* SEMANTIC_CPU_H */
