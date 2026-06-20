//! `CodegenBackend` trait — arquitectura-neutral codegen interface.
//!
//! Cada backend (x86_64, aarch64, riscv64) implementa este trait.
//! El Traductor usa SOLO este trait — nunca pattern-matches sobre TargetEmitter.
//! Para agregar una nueva arquitectura, basta implementar `CodegenBackend`.

/// Operación de comparación para `cmp_acc_*` y `set_acc_cmp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Igual,   // ==
    Mayor,   // >
    Menor,   // <
    MayIg,   // >=
    MenIg,   // <=
    Difer,   // !=
}

/// Operación lógica/bitwise para `bin_acc_op`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Suma,
    Resta,
    Mult,
    Div,
    Mod,
    Y,      // AND
    O,      // OR
    Xor,
    Shl,
    Shr,
}

/// Trait central que cada backend de codegen debe implementar.
///
/// Convenciones:
///   - "acc" = registro acumulador (RAX en x86-64, X0 en AArch64, A0 en RISC-V)
///   - frame_offset es siempre negativo respecto a RBP/FP (variables locales)
///   - Los métodos que devuelven `usize` retornan offset para back-patching
pub trait CodegenBackend {
    // ── Byte emission ────────────────────────────────────────────────

    /// Append raw bytes al code stream.
    fn emit_bytes(&mut self, bytes: &[u8]);
    /// Current offset in the code stream.
    fn here(&self) -> usize;
    /// Mutable access to the raw byte buffer (for back-patching).
    fn bytes_mut(&mut self) -> &mut alloc::vec::Vec<u8>;

    // ── Register move ────────────────────────────────────────────────

    /// `acc = imm64`
    fn mov_acc_imm(&mut self, imm: u64);
    /// `acc = src_reg`
    fn mov_acc_reg(&mut self, src: u32);
    /// `dst_reg = acc`
    fn mov_reg_acc(&mut self, dst: u32);
    /// `dst_reg = src_reg`
    fn mov_reg_reg(&mut self, dst: u32, src: u32);

    // ── Stack frame access ───────────────────────────────────────────

    /// `acc = [FP + offset]`  (load variable from stack frame)
    fn load_var(&mut self, frame_offset: i32);
    /// `[FP + offset] = acc`  (store variable to stack frame)
    fn store_var(&mut self, frame_offset: i32);

    // ── ALU (always acc ↔ reg) ──────────────────────────────────────

    /// `push acc` (save accumulator)
    fn push_acc(&mut self);
    /// `pop acc` (restore accumulator)
    fn pop_acc(&mut self);
    fn add_acc(&mut self, reg: u32);
    fn sub_acc(&mut self, reg: u32);
    fn mul_acc(&mut self, reg: u32);
    fn div_acc(&mut self, reg: u32);
    fn mod_acc(&mut self, reg: u32);
    fn and_acc(&mut self, reg: u32);
    fn or_acc(&mut self, reg: u32);
    fn xor_acc(&mut self, reg: u32);
    fn shl_acc(&mut self, reg: u32);
    fn shr_acc(&mut self, reg: u32);
    /// `acc = 0`
    fn zero_acc(&mut self);
    /// `acc = (acc == reg) ? 1 : 0`
    fn cmp_eq_acc(&mut self, reg: u32);
    /// `acc = (rcx > rax) ? 1 : 0` (op reg > acc → acc)
    fn cmp_gt_acc(&mut self, reg: u32);
    /// `acc = (rcx < rax) ? 1 : 0` (op reg < acc → acc)
    fn cmp_lt_acc(&mut self, reg: u32);
    /// `al = (condition) ? 1 : 0; movzx rax, al`
    fn sete_acc(&mut self);
    /// `setg al; movzx rax, al`
    fn setg_acc(&mut self);
    /// `setl al; movzx rax, al`
    fn setl_acc(&mut self);
    /// `test acc, acc`
    fn test_acc(&mut self);

    // ── Control flow ─────────────────────────────────────────────────

    /// `je rel32` — jump if zero. Returns patch offset.
    fn je_rel32(&mut self) -> usize;
    /// `jne rel32` — jump if not zero.
    fn jne_rel32(&mut self) -> usize;
    /// `jmp rel32` — unconditional jump.
    fn jmp_rel32(&mut self) -> usize;
    /// `nop`
    fn nop(&mut self);
    /// `syscall`
    fn syscall_inst(&mut self);
    /// `ret`
    fn ret(&mut self);

    // ── Function calls ───────────────────────────────────────────────

    /// `call rel32` — returns offset for back-patching.
    fn call_rel32(&mut self) -> usize;

    // ── String/label patching ────────────────────────────────────────

    fn patch_string_ref(&mut self, disp_offset: usize, rodata_offset: usize, final_code_len: usize);
    fn patch_rel32(&mut self, offset: usize, from: usize, to: usize);

    // ── Prologue/Epilogue ────────────────────────────────────────────

    /// Emit function prologue (save FP, set up frame). Returns offset to back-patch `sub SP, N`.
    fn emit_prologue(&mut self) -> usize;
    /// Back-patch the prologue's stack allocation with the real frame size.
    fn patch_frame_size(&mut self, prologue_offset: usize, frame_size: u32);
    /// Emit function epilogue (restore FP, return).
    fn emit_epilogue(&mut self);

    // ── ABI info ─────────────────────────────────────────────────────

    /// Number of argument registers (7 for BMO ABI on all archs).
    fn arg_reg_count(&self) -> usize;
    /// Register id for the Nth argument (0-based). Returns `None` if i >= arg_reg_count.
    fn arg_reg(&self, i: usize) -> Option<u32>;
    /// Register id for the return value (RAX on x86-64, X0 on AArch64, A0 on RISC-V).
    fn ret_reg(&self) -> u32;
    /// Register id for the accumulator (same as ret_reg).
    fn acc_reg(&self) -> u32;
    /// Register id for the scratch/second operand (RCX on x86-64, X1 on AArch64, A1 on RISC-V).
    fn scratch_reg(&self) -> u32;
    /// Parse a register name from source ("rax", "x0", "a0" etc.) to id.
    fn parse_reg(&self, name: &str) -> Option<u32>;
    /// Parse a BMO intrinsic name ("syscall", "nop", "int3" etc.) to raw bytes.
    fn intrinsic_bytes(&self, name: &str) -> Option<&'static [u8]>;
}
