//! v1.7.1 — Paleta compartida entre welcome y desktop.
//!
//! Lenguaje visual unificado: dark, elegante, profesional. Acentos en
//! mint/teal (B.M.O. signature) con detalles en violeta y gold.

#![allow(dead_code)]

// ── Backdrop ────────────────────────────────────────────────────────
pub const BG_TOP:        u32 = 0xFF050B18;
pub const BG_BOT:        u32 = 0xFF0A101F;
pub const BG_VIGNETTE:   u32 = 0x80020810;

// ── Acentos (mint, gold, violet) ────────────────────────────────────
pub const MINT:          u32 = 0xFF4ECCA3;     // BMO signature
pub const MINT_DEEP:     u32 = 0xFF2E8C77;
pub const MINT_SOFT:     u32 = 0xFF6FFFD0;
pub const MINT_PILL_BG:  u32 = 0xFF0E2820;
pub const GOLD:          u32 = 0xFFE2C044;
pub const ORANGE:        u32 = 0xFFE07832;
pub const ORANGE_HI:     u32 = 0xFFFFA056;
pub const VIOLET:        u32 = 0xFF6C5CE7;
pub const CYAN_INFO:     u32 = 0xFF56D4DD;

// ── Texto ───────────────────────────────────────────────────────────
pub const TITLE:         u32 = 0xFFE6F1F5;
pub const BODY:          u32 = 0xFFCBD7E0;
pub const SUBTITLE:      u32 = 0xFF7B8FA1;
pub const DIM:           u32 = 0xFF455364;

// ── Superficies (glass) ─────────────────────────────────────────────
pub const SURFACE_0:     u32 = 0xFF0F1827;     // card body
pub const SURFACE_1:     u32 = 0xFF152033;     // raised
pub const SURFACE_2:     u32 = 0xFF1A2A3F;     // window body
pub const SURFACE_BORDER:u32 = 0xFF1F4D5C;     // teal border
pub const SURFACE_LINE:  u32 = 0xFF1A2D3A;     // inner divider
pub const GLASS_TINT:    u32 = 0x66000000;     // 40% black overlay
pub const GLASS_HIGHLIGHT:u32 = 0x1AFFFFFF;    // 10% white sheen
pub const CARD_SHADOW:   u32 = 0xFF020610;

// ── Estado / acciones ───────────────────────────────────────────────
pub const OK_FG:         u32 = 0xFF4ECCA3;
pub const OK_BG:         u32 = 0xFF0E2820;
pub const PENDING_FG:    u32 = 0xFF455364;
pub const PENDING_BG:    u32 = 0xFF101820;
pub const CURRENT_FG:    u32 = 0xFFE2C044;
pub const CURRENT_BG:    u32 = 0xFF28220E;
pub const HINT:          u32 = 0xFFFFAA3D;

// ── Prompt ──────────────────────────────────────────────────────────
pub const PROMPT_BG:     u32 = 0xFF070D17;
pub const PROMPT_BD:     u32 = 0xFF4ECCA3;
pub const PROMPT_FG:     u32 = 0xFFE6F1F5;
pub const PLACEHOLDER:   u32 = 0xFF3D4F5F;

// ── Neón (tonos suaves, no brillantes) ──────────────────────────────
pub const NEON_OUTER:    u32 = 0xFF0F2030;
pub const NEON_MID:      u32 = 0xFF143D54;
pub const NEON_INNER:    u32 = 0xFF144D4D;

// ── Menu bar ────────────────────────────────────────────────────────
pub const MENU_BG:       u32 = 0xDD0D1117;
pub const MENU_TEXT:      u32 = 0xFFCBD7E0;
pub const MENU_HOVER:     u32 = 0x2D4ECCA3;
pub const MENU_ACTIVE:    u32 = 0xFF4ECCA3;
pub const MENU_HEIGHT:    u32 = 28;

// ── Per-window properties (configurable) ────────────────────────────
pub const WINDOW_OPACITY:      f32 = 0.96;
pub const WINDOW_CORNER_RADIUS: u32 = 14;
pub const WINDOW_SHADOW_OFFSET:  i32 = 6;
pub const WINDOW_SHADOW_BLUR:   u32 = 4;

// ── Animations ──────────────────────────────────────────────────────
pub const ANIM_OPEN_MS:     u32 = 300;
pub const ANIM_CLOSE_MS:    u32 = 200;
pub const ANIM_MINIMIZE_MS: u32 = 300;
pub const ANIM_DOCK_MS:     u32 = 150;
