//! **The AudioStreaming side: where the samples travel.**
//!
//! Step 0 of `docs/maestro/AUDIO_MAESTRO.md`, and it is deliberately the one step that
//! **cannot break anything**: not a single byte is written to the device here.
//! This module reads a configuration descriptor and answers questions.
//!
//! # Why this is a module and not more lines in `lib.rs`
//!
//! Because they are two different jobs and the owner's rule is modular:
//!
//! | | |
//! |---|---|
//! | `lib.rs` | **AudioControl**: volume, mute. Commands *about* the device |
//! | this | **AudioStreaming**: the pipe the samples go through |
//!
//! They share the class constants and nothing else. `lib.rs` says so itself in
//! its line 51 -- *"the interface that carries samples is AUDIOSTREAMING (0x02)
//! and is not touched here"* -- and that sentence stops being a promise and
//! becomes a boundary the moment there are two files.
//!
//! # What it is for, in one line
//!
//! Answering, **before writing any driver**, the four numbers that decide
//! whether the rest of the plan is even possible:
//!
//! ```text
//!    which interface and which alternate setting carry the isochronous endpoint
//!    how many bytes fit in one frame            (wMaxPacketSize)
//!    what shape the samples have                (channels, bits)
//!    and at what rate the device wants them     (sampling frequencies)
//!
//! ```
//!
//! And they can be **predicted before booting** by looking at what the other
//! operating system on this same machine says -- which is the method that
//! already worked twice: the `#GP` of July and the NIC's MAC.
//!
//! # The trap that this module exists to not fall into
//!
//! An AudioStreaming interface declares its endpoint **only in alternate
//! setting 1 or above**. Alternate setting 0 exists on purpose and has ZERO
//! endpoints: it is the "I am not using any bandwidth" mode, and a device sits
//! there until somebody sends a `SET_INTERFACE`.
//!
//! So a walker that tracks `bInterfaceNumber` and ignores `bAlternateSetting`
//! --which is exactly what `bmo_uhid::enumera::intr_in` does, correctly, because
//! HID has no alternate settings-- would find the interface, find no endpoint,
//! and conclude the device cannot play. **The device would be perfect.**

use super::{CLASS_AUDIO, DESC_CS_INTERFACE, DESC_INTERFACE};

/// Subclass AUDIOSTREAMING: the interface that carries samples.
pub const SUBCLASS_AUDIOSTREAMING: u8 = 0x02;

/// Standard endpoint descriptor.
const DESC_ENDPOINT: u8 = 0x05;
/// Class-specific AS interface descriptor, subtype AS_GENERAL.
const AS_GENERAL: u8 = 0x01;
/// ...and subtype FORMAT_TYPE.
const AS_FORMAT_TYPE: u8 = 0x02;
/// Format Type I: the only one that is plain PCM, and the only one this plan
/// wants. Type II is compressed, type III is IEC61937 passthrough.
const FORMAT_TYPE_I: u8 = 0x01;
/// `wFormatTag` for uncompressed PCM.
pub const FORMAT_PCM: u16 = 0x0001;

/// Transfer type in `bmAttributes`, bits 1:0. `1` = isochronous.
const XFER_ISOCHRONOUS: u8 = 0x01;

/// How many discrete sampling frequencies are kept.
///
/// Six: a device that offers more than 8/11.025/16/22.05/44.1/48 kHz is offering
/// them for recording studios, and this plan does not resample -- so the extra
/// ones would be information nobody can act on.
pub const MAX_RATES: usize = 6;

/// How the device keeps time with the host. It is in `bmAttributes` bits 3:2 and
/// it decides **who corrects the drift**, which is the difference between audio
/// that plays for an hour and audio that slowly desynchronises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sync {
    /// No synchronisation declared.
    None,
    /// The device runs at its own pace and tells the host how much it consumed
    /// through a feedback endpoint. **The host has to adapt.**
    Async,
    /// The device adapts to the rate at which the host feeds it. The easy case:
    /// send one frame per interval and it follows.
    Adaptive,
    /// The device locks to the bus clock.
    Synchronous,
}

impl Sync {
    fn from_attrs(a: u8) -> Self {
        match (a >> 2) & 0x03 {
            1 => Sync::Async,
            2 => Sync::Adaptive,
            3 => Sync::Synchronous,
            _ => Sync::None,
        }
    }
}

/// **A playback pipe, already located.** Everything needed to configure the
/// endpoint and to know what to put in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Playback {
    /// `bInterfaceNumber` of the AudioStreaming interface.
    pub interface: u8,
    /// The alternate setting that carries the endpoint. **Never 0.**
    pub alt_setting: u8,
    /// `bEndpointAddress`, raw. Bit 7 clear = OUT, which is what playback is.
    pub endpoint: u8,
    /// The xHCI Device Context Index of that endpoint. `2*N` for OUT, `2*N+1`
    /// for IN -- the same convention `bmo_uhid` uses, and it has to be, because
    /// it is the xHC's and not ours.
    pub dci: u8,
    /// `wMaxPacketSize`: the most the device accepts **per interval**.
    pub max_packet: u16,
    /// `bInterval`. For a full-speed isochronous endpoint, 1 = every frame = 1 ms.
    pub interval: u8,
    /// Who corrects the drift. See [`Sync`].
    pub sync: Sync,
    /// Logical channels. 2 = stereo.
    pub channels: u8,
    /// Bytes each sample occupies **on the wire** (`bSubframeSize`).
    pub subframe: u8,
    /// Bits that are actually meaningful (`bBitResolution`).
    ///
    /// * It is NOT always `subframe * 8`: a device can carry 24 bits inside 4
    /// bytes. Confusing them shifts every sample and the result is noise that
    /// sounds like a broken cable.
    pub bits: u8,
    /// Sampling frequencies the device declares, in Hz.
    pub rates: [u32; MAX_RATES],
    /// How many of the above are valid.
    pub n_rates: usize,
    /// True when the device declared a continuous RANGE instead of a list. Then
    /// `rates[0]` is the minimum and `rates[1]` the maximum.
    pub continuous: bool,
}

impl Playback {
    /// **Bytes that one interval of `rate` occupies.**
    ///
    /// This is the number that has to fit in [`Self::max_packet`], and checking
    /// it is the whole point of reading the descriptor before writing a driver:
    /// if it does not fit, no amount of correct code will make a sound.
    ///
    /// `interval` is assumed to be 1 ms, which is what a full-speed isochronous
    /// endpoint uses. See [`Self::fits`].
    pub fn bytes_per_interval(&self, rate: u32) -> u32 {
        (rate / 1000) * self.channels as u32 * self.subframe as u32
    }

    /// Does one interval at that rate fit in what the device accepts?
    ///
    /// A `false` here is not a bug in BMO-X: it is the device saying it cannot
    /// do that combination, and the answer is to pick another rate -- not to
    /// send a truncated frame, which is a click.
    pub fn fits(&self, rate: u32) -> bool {
        self.bytes_per_interval(rate) <= self.max_packet as u32
    }

    /// The declared rates, as a slice.
    pub fn rates(&self) -> &[u32] {
        &self.rates[..self.n_rates]
    }

    /// Does the device accept exactly this rate?
    ///
    /// **Exactly**, and that word is the decision: this plan does not resample
    /// (`AUDIO_MAESTRO.md`, part 8). A near miss is not a match -- playing 44100
    /// samples through a 48000 pipe does not sound slightly off, it sounds like
    /// a different animal.
    pub fn accepts(&self, rate: u32) -> bool {
        if self.continuous {
            return self.n_rates >= 2 && rate >= self.rates[0] && rate <= self.rates[1];
        }
        self.rates().iter().any(|&r| r == rate)
    }

    /// The best usable rate for a wanted one: the exact one if the device has it
    /// and it fits, otherwise the highest one that does fit.
    ///
    /// Returns `None` when nothing fits, which is worth being able to say
    /// instead of returning a rate that will click.
    pub fn best_rate(&self, wanted: u32) -> Option<u32> {
        if self.accepts(wanted) && self.fits(wanted) {
            return Some(wanted);
        }
        if self.continuous {
            // Down from the wanted one in 1 kHz steps: a continuous device takes
            // anything in range, so the only limit is what fits in a packet.
            let mut r = wanted.min(self.rates[1]);
            while r >= self.rates[0] {
                if self.fits(r) {
                    return Some(r);
                }
                r = r.saturating_sub(1000);
            }
            return None;
        }
        let mut best = None;
        for &r in self.rates() {
            if self.fits(r) && best.map_or(true, |b| r > b) {
                best = Some(r);
            }
        }
        best
    }
}

/// Reads a 24-bit little-endian number: how USB Audio 1.0 stores a frequency.
///
/// Three bytes, because 48000 does not fit in two and four would have been
/// generous in 1998.
#[inline]
fn le_u24(b: &[u8], off: usize) -> u32 {
    if off + 3 > b.len() {
        return 0;
    }
    (b[off] as u32) | ((b[off + 1] as u32) << 8) | ((b[off + 2] as u32) << 16)
}

#[inline]
fn le_u16(b: &[u8], off: usize) -> u16 {
    if off + 2 > b.len() {
        return 0;
    }
    (b[off] as u16) | ((b[off + 1] as u16) << 8)
}

/// **Finds the playback pipe** in a configuration descriptor.
///
/// `None` means this device cannot play PCM over an isochronous OUT endpoint --
/// which is a real answer and not a failure: a microphone-only device gives
/// exactly that.
///
/// # How it walks, and why like this
///
/// Same rule as [`super::find_audio_control`]: advance by `bLength` and never by
/// the size of the struct one expects. A device can include descriptors this
/// code does not know, and skipping them by their own length is the only thing
/// that does not break when it does.
///
/// [!] A `bLength` of 0 would spin forever. It is checked: this is data from
/// outside and nothing can be assumed about it.
///
/// # What it collects, and in what order it becomes valid
///
/// The three pieces of one alternate setting arrive **separately and in this
/// order**: the interface header, then the format, then the endpoint. So the
/// candidate is built as it goes and is only returned when the endpoint shows
/// up -- and it is reset on every new interface header, because a format
/// belonging to alt 1 must never be attributed to alt 2.
pub fn find_playback(config: &[u8]) -> Option<Playback> {
    let total = if config.len() >= 4 { le_u16(config, 2) as usize } else { 0 };
    let limit = if total > 0 && total <= config.len() { total } else { config.len() };
    let mut off = if !config.is_empty() { config[0] as usize } else { 9 };

    // The alternate setting being walked right now. `None` = not an
    // AudioStreaming one, so everything until the next interface is ignored.
    let mut current: Option<Playback> = None;

    while off + 2 <= limit {
        let len = config[off] as usize;
        let dtype = config[off + 1];
        if len < 2 || off + len > limit {
            break;
        }

        if dtype == DESC_INTERFACE && len >= 9 {
            let class = config[off + 5];
            let subclass = config[off + 6];
            current = if class == CLASS_AUDIO && subclass == SUBCLASS_AUDIOSTREAMING {
                Some(Playback {
                    interface: config[off + 2],
                    alt_setting: config[off + 3],
                    endpoint: 0,
                    dci: 0,
                    max_packet: 0,
                    interval: 0,
                    sync: Sync::None,
                    channels: 0,
                    subframe: 0,
                    bits: 0,
                    rates: [0; MAX_RATES],
                    n_rates: 0,
                    continuous: false,
                })
            } else {
                None
            };
        } else if dtype == DESC_CS_INTERFACE && len >= 3 {
            let subtype = config[off + 2];
            if let Some(p) = current.as_mut() {
                if subtype == AS_GENERAL && len >= 7 {
                    // Anything that is not plain PCM is discarded HERE and not
                    // later: a compressed stream fed as if it were samples is
                    // the loudest possible noise, and this plan only wants PCM.
                    if le_u16(config, off + 5) != FORMAT_PCM {
                        current = None;
                    }
                } else if subtype == AS_FORMAT_TYPE && len >= 8 && config[off + 3] == FORMAT_TYPE_I {
                    p.channels = config[off + 4];
                    p.subframe = config[off + 5];
                    p.bits = config[off + 6];
                    let kind = config[off + 7];
                    if kind == 0 {
                        // Continuous: exactly two frequencies, min and max.
                        p.continuous = true;
                        p.rates[0] = le_u24(config, off + 8);
                        p.rates[1] = le_u24(config, off + 11);
                        p.n_rates = 2;
                    } else {
                        let n = (kind as usize).min(MAX_RATES);
                        for i in 0..n {
                            p.rates[i] = le_u24(config, off + 8 + i * 3);
                        }
                        p.n_rates = n;
                    }
                }
            }
        } else if dtype == DESC_ENDPOINT && len >= 7 {
            if let Some(mut p) = current {
                let addr = config[off + 2];
                let attrs = config[off + 3];
                let is_out = addr & 0x80 == 0;
                let is_isoch = attrs & 0x03 == XFER_ISOCHRONOUS;
                // A format with no channels is an alternate setting whose
                // FORMAT_TYPE was not understood. Returning it would hand the
                // caller a pipe whose sample shape is zero.
                if is_out && is_isoch && p.channels > 0 && p.subframe > 0 {
                    p.endpoint = addr;
                    p.dci = (addr & 0x0F) * 2;
                    p.max_packet = le_u16(config, off + 4);
                    p.interval = config[off + 6];
                    p.sync = Sync::from_attrs(attrs);
                    return Some(p);
                }
            }
        }

        off += len;
    }
    None
}

// -- The tests --------------------------------------------------------------
//
// ** THESE ONES DO RUN, and it is worth knowing why -- the first version of this
// comment said they did not, copying the warning from `sin_gpu/sucio.rs`, and it
// was false.
//
// `bmo-uaudio` is `#![no_std]` but has **no `#[panic_handler]` of its own**, so
// `cargo test` builds it for the host, the harness brings `std`, and they run
// with the rest of the workspace. What cannot be tested is a crate that
// *provides* the panic handler -- the kernel, and `Ultra_userspace` with its own
// linker script.
//
//     cargo test -p bmo-uaudio
//
// So the rule is not "no_std cannot be tested": it is **"whoever owns the panic
// handler cannot be tested"**. And it is worth writing down, because it decides
// where new logic should live: anything put HERE is checkable by the suite, and
// the same code put in the kernel is checkable only by reading it.
//
// A `#[cfg(test)]` nobody has run is not a test, it is an intention.
#[cfg(test)]
mod tests {
    use super::*;

    /// A USB Audio 1.0 headset, built descriptor by descriptor: interface 1 with
    /// its silent alt 0 and its alt 1 carrying stereo 16-bit PCM at 48 kHz.
    ///
    /// It is written by hand rather than captured because **every byte here is a
    /// decision that has to be visible**: the day one of these tests fails, the
    /// question is which field moved, and a captured blob does not answer that.
    fn headset() -> [u8; 61] {
        [
            // -- Configuration descriptor, 9 bytes. wTotalLength = 62.
            9, 2, 61, 0, 2, 1, 0, 0x80, 50,
            // -- Interface 1, alt 0: AudioStreaming with ZERO endpoints.
            //    This is the "I use no bandwidth" mode, and finding it is not
            //    finding a pipe.
            9, DESC_INTERFACE, 1, 0, 0, CLASS_AUDIO, SUBCLASS_AUDIOSTREAMING, 0, 0,
            // -- Interface 1, alt 1: the one that carries the endpoint.
            9, DESC_INTERFACE, 1, 1, 1, CLASS_AUDIO, SUBCLASS_AUDIOSTREAMING, 0, 0,
            // -- CS_INTERFACE / AS_GENERAL: terminal 1, no delay, PCM.
            7, DESC_CS_INTERFACE, AS_GENERAL, 1, 0, 0x01, 0x00,
            // -- CS_INTERFACE / FORMAT_TYPE_I: 2 channels, 2 bytes, 16 bits,
            //    one discrete frequency: 48000 = 0x00BB80.
            11, DESC_CS_INTERFACE, AS_FORMAT_TYPE, FORMAT_TYPE_I, 2, 2, 16, 1, 0x80, 0xBB, 0x00,
            // -- Endpoint 0x01 (OUT), isochronous + adaptive, 192 bytes, 1 ms.
            9, DESC_ENDPOINT, 0x01, 0x09, 192, 0, 1, 0, 0,
            // -- CS_ENDPOINT, which this module ignores and must skip cleanly.
            7, 0x25, 0x01, 0x01, 0, 0, 0,
        ]
    }

    /// ** THE FOUR NUMBERS THAT DECIDE IF EL PLAN ES POSIBLE.
    ///
    /// They are the ones the boot is going to print, and they are the ones that
    /// can be predicted against what Windows says about this same headset before
    /// turning the machine on.
    #[test]
    fn the_four_numbers_come_out_of_the_descriptor() {
        let p = find_playback(&headset()).expect("this headset does play");
        assert_eq!(p.interface, 1);
        assert_eq!(p.alt_setting, 1, "never alt 0: that one has no endpoint");
        assert_eq!(p.channels, 2);
        assert_eq!(p.bits, 16);
        assert_eq!(p.subframe, 2);
        assert_eq!(p.rates(), &[48000]);
        assert_eq!(p.max_packet, 192);
        assert_eq!(p.interval, 1);
        assert_eq!(p.sync, Sync::Adaptive);
    }

    /// ** ALTERNATE SETTING 0 IS A TRAP, AND IT IS THE ONE THAT WOULD HAVE BEEN
    /// ** FALLEN INTO.
    ///
    /// A walker that tracks the interface NUMBER and ignores the alternate
    /// setting --which is exactly what `bmo_uhid` does, correctly, because HID
    /// has no alternate settings-- would find interface 1, find no endpoint in
    /// it, and conclude the device cannot play. The device would be perfect.
    #[test]
    fn alt_zero_is_not_a_pipe() {
        let d = headset();
        // Cut the descriptor right where alt 1 begins: only alt 0 is left.
        let solo_alt0 = &d[..18];
        assert!(find_playback(solo_alt0).is_none(), "alt 0 has no endpoint and is not a pipe");
    }

    /// ** THE DCI IS THE xHC'S CONVENTION, NOT OURS.
    ///
    /// `2*N` for OUT and `2*N+1` for IN. Endpoint 0x01 OUT is DCI 2. Getting
    /// this wrong configures a DIFFERENT endpoint of the same device -- and the
    /// xHC would accept it, because DCI 3 is a perfectly valid index.
    #[test]
    fn the_dci_follows_the_out_convention() {
        let p = find_playback(&headset()).unwrap();
        assert_eq!(p.dci, 2, "endpoint 1 OUT is DCI 2");
        assert_eq!(p.endpoint & 0x80, 0, "and it is OUT, which is what playing is");
    }

    /// ** THE NUMBER THAT DECIDES WHETHER ANYTHING WILL SOUND AT ALL.
    ///
    /// 48000 Hz / 1000 = 48 samples per millisecond, x2 channels x2 bytes = 192
    /// bytes. Exactly `wMaxPacketSize`. That is not a coincidence: it is what
    /// the device sized its endpoint for.
    #[test]
    fn one_interval_has_to_fit_in_the_packet() {
        let p = find_playback(&headset()).unwrap();
        assert_eq!(p.bytes_per_interval(48000), 192);
        assert!(p.fits(48000));
        // And a rate that does not fit is said, not rounded.
        assert!(!p.fits(96000), "96 kHz would need 384 bytes and there are 192");
    }

    /// ** NO RESAMPLING, AND THAT IS A DECISION WITH A TEST.
    ///
    /// This headset does not offer 44100. Answering "close enough" would play a
    /// cat at the wrong speed, which is exactly the acceptance test of
    /// `AUDIO_MAESTRO.md`.
    #[test]
    fn a_rate_the_device_does_not_have_is_not_accepted() {
        let p = find_playback(&headset()).unwrap();
        assert!(!p.accepts(44100), "it is not in the list and there is no resampling");
        assert!(p.accepts(48000));
        // And asking for it falls back to what the device DOES have, said out
        // loud by returning a different number.
        assert_eq!(p.best_rate(44100), Some(48000));
    }

    /// A device with no audio in it answers `None`, and that is an answer.
    #[test]
    fn a_device_without_streaming_says_so() {
        // A lone HID interface: class 3, no audio anywhere.
        let hid = [9u8, 2, 18, 0, 1, 1, 0, 0x80, 50,
                   9, DESC_INTERFACE, 0, 0, 1, 3, 1, 1, 0];
        assert!(find_playback(&hid).is_none());
    }

    /// ** A ZERO `bLength` MUST NOT SPIN FOREVER.
    ///
    /// It is data from outside. The device that sends it is broken, and a broken
    /// device must not be able to hang the enumeration -- that is a hang at boot
    /// with no message.
    #[test]
    fn a_broken_descriptor_does_not_hang() {
        let mut d = headset().to_vec();
        d[9] = 0; // bLength of the first interface = 0
        assert!(find_playback(&d).is_none());
    }

    /// A truncated descriptor is read up to where it reaches and no further.
    /// Reading past it would be reading somebody else's memory and calling the
    /// result a sample rate.
    #[test]
    fn a_truncated_descriptor_is_not_invented() {
        let d = headset();
        for cut in 1..d.len() {
            let _ = find_playback(&d[..cut]);
        }
    }
}
