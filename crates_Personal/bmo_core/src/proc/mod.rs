pub mod process;
pub mod task;
pub mod user_init;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Realtime,
    HighGame,
    Game,
    Interactive,
    Idle,
}

pub fn schedule() {
    if let Some(h) = unsafe { crate::hal::HAL.as_ref() } { (h.schedule)(); }
}

pub fn yield_now() {
    if let Some(h) = unsafe { crate::hal::HAL.as_ref() } { (h.yield_now)(); }
}
