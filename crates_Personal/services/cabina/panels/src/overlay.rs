use crate::fb::FrameBuffer;
use crate::panels;
use cabina_core::SystemSnapshot;

pub struct Overlay {
    enabled: bool,
    dirty: bool,
    current_tab: u8,
    current_query: u8,
}

impl Overlay {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            dirty: false,
            current_tab: 0,
            current_query: 0,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
        self.dirty = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if self.enabled {
            self.dirty = true;
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn current_tab(&self) -> u8 {
        self.current_tab
    }

    pub fn current_query(&self) -> u8 {
        self.current_query
    }

    pub fn cycle_tab(&mut self) {
        let max = (panels::PANEL_COUNT as u8).saturating_sub(1);
        self.current_tab = if self.current_tab >= max {
            0
        } else {
            self.current_tab + 1
        };
        self.dirty = true;
    }

    pub fn cycle_query(&mut self) {
        self.current_query = if self.current_query >= 5 {
            0
        } else {
            self.current_query + 1
        };
        self.dirty = true;
    }

    pub fn set_tab(&mut self, tab: u8) {
        if tab < panels::PANEL_COUNT as u8 {
            self.current_tab = tab;
            self.dirty = true;
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.dirty = true;
        }
    }

    pub fn paint(&mut self, fb: &mut dyn FrameBuffer, snapshot: &SystemSnapshot) {
        if !self.enabled {
            return;
        }
        panels::render(fb, self.current_tab, snapshot);
        self.dirty = false;
    }

    pub fn tab_name(&self) -> &'static str {
        panels::name(self.current_tab)
    }
}
