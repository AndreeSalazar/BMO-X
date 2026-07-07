pub mod amd {
    pub mod cpu {
        pub mod zen3 {
            pub mod errata_workarounds {
                use crate::hal;
                pub fn issue_ibpb() {
                    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.issue_ibpb)(); }
                }
            }
        }
    }
}
