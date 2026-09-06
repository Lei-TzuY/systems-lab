//! O-RAN WG4 Open Fronthaul delay management (O-RAN.WG4.CUS-Plane, delay management annex).
//!
//! Every fronthaul message is timed against the air-interface instant of the OFDM symbol
//! it belongs to. The O-DU transmits inside a window `[T1a_min, T1a_max]` ahead of that
//! instant, the fronthaul network adds `T12` (downlink) or `T34` (uplink), and the O-RU
//! therefore expects the packet inside `[T2a_min, T2a_max]`. A packet arriving outside
//! the reception window is discarded: too early and the O-RU has nowhere to buffer it,
//! too late and the symbol has already gone out over the antenna.
//!
//! `T12` and `T34` are the very quantities the eCPRI one-way delay measurement of
//! [`crate::ecpri`] reports, and the windows here are what an IEEE 802.1CM profile
//! (see [`crate::tsn_8021cm_fronthaul`]) has to fit inside.

use std::fmt;

/// Nominal 5G NR radio frame duration.
pub const NR_FRAME_DURATION_NS: i64 = 10_000_000;
/// Nominal 5G NR subframe duration.
pub const NR_SUBFRAME_DURATION_NS: i64 = 1_000_000;
/// OFDM symbols per slot with normal cyclic prefix.
pub const NR_SYMBOLS_PER_SLOT: i64 = 14;

/// Errors raised by fronthaul window configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelayMgmtError {
    /// A window's minimum is not below its maximum.
    InvertedWindow { min_ns: i64, max_ns: i64 },
    /// The network delay spread consumed the whole window: nothing can ever be on time.
    WindowCollapsed { t2a_min_ns: i64, t2a_max_ns: i64 },
    /// The derived arrival window and the O-RU's supported window do not overlap.
    IncompatibleWindows {
        derived_min_ns: i64,
        derived_max_ns: i64,
        supported_min_ns: i64,
        supported_max_ns: i64,
    },
}

impl fmt::Display for DelayMgmtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DelayMgmtError::InvertedWindow { min_ns, max_ns } => write!(
                f,
                "Fronthaul window minimum {} ns is not below maximum {} ns",
                min_ns, max_ns
            ),
            DelayMgmtError::WindowCollapsed {
                t2a_min_ns,
                t2a_max_ns,
            } => write!(
                f,
                "Reception window collapsed: T2a_min {} ns >= T2a_max {} ns",
                t2a_min_ns, t2a_max_ns
            ),
            DelayMgmtError::IncompatibleWindows {
                derived_min_ns,
                derived_max_ns,
                supported_min_ns,
                supported_max_ns,
            } => write!(
                f,
                "Derived window [{}, {}] ns does not overlap the O-RU window [{}, {}] ns",
                derived_min_ns, derived_max_ns, supported_min_ns, supported_max_ns
            ),
        }
    }
}

impl std::error::Error for DelayMgmtError {}

/// Which fronthaul message stream a timing window applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FronthaulWindowKind {
    /// C-Plane scheduling messages for downlink symbols (`T1a_*_cp_dl`).
    CPlaneDownlink,
    /// C-Plane scheduling messages for uplink symbols (`T1a_*_cp_ul`).
    CPlaneUplink,
    /// U-Plane IQ data for downlink symbols (`T1a_*_up`).
    UPlaneDownlink,
}

impl FronthaulWindowKind {
    pub fn label(&self) -> &'static str {
        match self {
            FronthaulWindowKind::CPlaneDownlink => "C-Plane downlink (T1a_cp_dl)",
            FronthaulWindowKind::CPlaneUplink => "C-Plane uplink (T1a_cp_ul)",
            FronthaulWindowKind::UPlaneDownlink => "U-Plane downlink (T1a_up)",
        }
    }
}

/// Minimum and maximum one-way transport delay of the fronthaul network.
///
/// `T12` is O-DU to O-RU, `T34` is O-RU to O-DU. The spread between minimum and
/// maximum is the packet delay variation the windows have to absorb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkDelayBudget {
    pub t12_min_ns: i64,
    pub t12_max_ns: i64,
    pub t34_min_ns: i64,
    pub t34_max_ns: i64,
}

impl NetworkDelayBudget {
    pub fn new(
        t12_min_ns: i64,
        t12_max_ns: i64,
        t34_min_ns: i64,
        t34_max_ns: i64,
    ) -> Result<Self, DelayMgmtError> {
        if t12_min_ns > t12_max_ns {
            return Err(DelayMgmtError::InvertedWindow {
                min_ns: t12_min_ns,
                max_ns: t12_max_ns,
            });
        }
        if t34_min_ns > t34_max_ns {
            return Err(DelayMgmtError::InvertedWindow {
                min_ns: t34_min_ns,
                max_ns: t34_max_ns,
            });
        }
        Ok(NetworkDelayBudget {
            t12_min_ns,
            t12_max_ns,
            t34_min_ns,
            t34_max_ns,
        })
    }

    /// Symmetric budget, as produced by a pair of eCPRI one-way delay measurements.
    pub fn symmetric(min_ns: i64, max_ns: i64) -> Result<Self, DelayMgmtError> {
        NetworkDelayBudget::new(min_ns, max_ns, min_ns, max_ns)
    }

    /// Downlink packet delay variation: the jitter the reception window must absorb.
    pub fn t12_variation_ns(&self) -> i64 {
        self.t12_max_ns - self.t12_min_ns
    }

    /// Uplink packet delay variation.
    pub fn t34_variation_ns(&self) -> i64 {
        self.t34_max_ns - self.t34_min_ns
    }

    /// Worst-case round trip across the fronthaul network.
    pub fn round_trip_max_ns(&self) -> i64 {
        self.t12_max_ns + self.t34_max_ns
    }

    /// Asymmetry between the two directions at their maxima.
    pub fn asymmetry_ns(&self) -> i64 {
        self.t12_max_ns - self.t34_max_ns
    }
}

/// O-DU transmission window: how far ahead of the air time the O-DU may send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OduTransmissionWindow {
    pub kind: FronthaulWindowKind,
    pub t1a_min_ns: i64,
    pub t1a_max_ns: i64,
}

impl OduTransmissionWindow {
    pub fn new(
        kind: FronthaulWindowKind,
        t1a_min_ns: i64,
        t1a_max_ns: i64,
    ) -> Result<Self, DelayMgmtError> {
        if t1a_min_ns >= t1a_max_ns {
            return Err(DelayMgmtError::InvertedWindow {
                min_ns: t1a_min_ns,
                max_ns: t1a_max_ns,
            });
        }
        Ok(OduTransmissionWindow {
            kind,
            t1a_min_ns,
            t1a_max_ns,
        })
    }

    pub fn width_ns(&self) -> i64 {
        self.t1a_max_ns - self.t1a_min_ns
    }
}

/// O-RU reception window, expressed as an advance ahead of the symbol's air time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OruReceptionWindow {
    pub kind: FronthaulWindowKind,
    pub t2a_min_ns: i64,
    pub t2a_max_ns: i64,
}

impl OruReceptionWindow {
    pub fn new(
        kind: FronthaulWindowKind,
        t2a_min_ns: i64,
        t2a_max_ns: i64,
    ) -> Result<Self, DelayMgmtError> {
        if t2a_min_ns >= t2a_max_ns {
            return Err(DelayMgmtError::WindowCollapsed {
                t2a_min_ns,
                t2a_max_ns,
            });
        }
        Ok(OruReceptionWindow {
            kind,
            t2a_min_ns,
            t2a_max_ns,
        })
    }

    pub fn width_ns(&self) -> i64 {
        self.t2a_max_ns - self.t2a_min_ns
    }

    /// Classifies a packet that arrived `advance_ns` before its symbol's air time.
    pub fn classify(&self, advance_ns: i64) -> WindowVerdict {
        if advance_ns > self.t2a_max_ns {
            WindowVerdict::TooEarly {
                by_ns: advance_ns - self.t2a_max_ns,
            }
        } else if advance_ns < self.t2a_min_ns {
            WindowVerdict::TooLate {
                by_ns: self.t2a_min_ns - advance_ns,
            }
        } else {
            WindowVerdict::OnTime {
                margin_ns: (advance_ns - self.t2a_min_ns).min(self.t2a_max_ns - advance_ns),
            }
        }
    }

    pub fn accepts(&self, advance_ns: i64) -> bool {
        matches!(self.classify(advance_ns), WindowVerdict::OnTime { .. })
    }
}

/// Where a packet fell relative to the reception window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowVerdict {
    /// Inside the window; `margin_ns` is the distance to the nearer edge.
    OnTime { margin_ns: i64 },
    /// Arrived further ahead of the air time than `T2a_max`: the O-RU cannot buffer it yet.
    TooEarly { by_ns: i64 },
    /// Arrived closer to the air time than `T2a_min`: the symbol can no longer be prepared.
    TooLate { by_ns: i64 },
}

impl WindowVerdict {
    pub fn is_on_time(&self) -> bool {
        matches!(self, WindowVerdict::OnTime { .. })
    }
}

/// Envelope of arrival advances a given O-DU transmission window can produce.
///
/// A larger transport delay leaves a smaller remaining advance, so the maximum advance
/// pairs with the minimum delay and vice versa:
/// `T2a_max = T1a_max - T12_min`, `T2a_min = T1a_min - T12_max`.
///
/// The envelope is always wider than the transmission window by exactly the downlink
/// delay variation: that spread is what the O-RU's buffers have to absorb.
pub fn derive_reception_window(
    tx: &OduTransmissionWindow,
    budget: &NetworkDelayBudget,
) -> Result<OruReceptionWindow, DelayMgmtError> {
    let t2a_min_ns = tx.t1a_min_ns - budget.t12_max_ns;
    let t2a_max_ns = tx.t1a_max_ns - budget.t12_min_ns;
    OruReceptionWindow::new(tx.kind, t2a_min_ns, t2a_max_ns)
}

/// The O-DU transmission window demanded by an O-RU's advertised reception window.
///
/// This is the direction O-RAN deployments actually configure: the O-RU publishes what
/// it can buffer and the O-DU works backwards through the network delay with
/// `T1a_min = T2a_min + T12_max`, `T1a_max = T2a_max + T12_min`.
///
/// The result inverts, and [`DelayMgmtError::InvertedWindow`] is returned, when the
/// network's delay variation exceeds the width of the O-RU's reception window: no
/// transmission schedule can then keep every packet inside the window.
pub fn derive_transmission_window(
    rx: &OruReceptionWindow,
    budget: &NetworkDelayBudget,
) -> Result<OduTransmissionWindow, DelayMgmtError> {
    OduTransmissionWindow::new(
        rx.kind,
        rx.t2a_min_ns + budget.t12_max_ns,
        rx.t2a_max_ns + budget.t12_min_ns,
    )
}

/// O-RU uplink transmission window `Ta3`, measured as a delay after the air time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OruUplinkWindow {
    pub ta3_min_ns: i64,
    pub ta3_max_ns: i64,
}

/// O-DU uplink reception window `Ta4`, measured as a delay after the air time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OduReceptionWindow {
    pub ta4_min_ns: i64,
    pub ta4_max_ns: i64,
}

impl OduReceptionWindow {
    pub fn width_ns(&self) -> i64 {
        self.ta4_max_ns - self.ta4_min_ns
    }

    /// Classifies uplink data that reached the O-DU `delay_ns` after the air time.
    pub fn classify(&self, delay_ns: i64) -> WindowVerdict {
        if delay_ns < self.ta4_min_ns {
            WindowVerdict::TooEarly {
                by_ns: self.ta4_min_ns - delay_ns,
            }
        } else if delay_ns > self.ta4_max_ns {
            WindowVerdict::TooLate {
                by_ns: delay_ns - self.ta4_max_ns,
            }
        } else {
            WindowVerdict::OnTime {
                margin_ns: (delay_ns - self.ta4_min_ns).min(self.ta4_max_ns - delay_ns),
            }
        }
    }
}

/// Uplink counterpart of [`derive_reception_window`]: `Ta4 = Ta3 + T34`.
///
/// Both bounds add here because uplink windows run forward from the air time.
pub fn derive_odu_reception_window(
    ul: &OruUplinkWindow,
    budget: &NetworkDelayBudget,
) -> Result<OduReceptionWindow, DelayMgmtError> {
    if ul.ta3_min_ns >= ul.ta3_max_ns {
        return Err(DelayMgmtError::InvertedWindow {
            min_ns: ul.ta3_min_ns,
            max_ns: ul.ta3_max_ns,
        });
    }
    Ok(OduReceptionWindow {
        ta4_min_ns: ul.ta3_min_ns + budget.t34_min_ns,
        ta4_max_ns: ul.ta3_max_ns + budget.t34_max_ns,
    })
}

/// The window an O-RU advertises in its M-Plane delay management capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OruWindowCapability {
    pub supported_min_ns: i64,
    pub supported_max_ns: i64,
}

/// Overlap between the window the O-DU's configuration produces and what the O-RU supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowCompatibility {
    pub usable_min_ns: i64,
    pub usable_max_ns: i64,
    /// Width of the overlap; the delay variation the link may still absorb.
    pub usable_width_ns: i64,
}

/// Intersects the derived reception window with the O-RU's advertised capability.
pub fn check_window_compatibility(
    derived: &OruReceptionWindow,
    capability: &OruWindowCapability,
) -> Result<WindowCompatibility, DelayMgmtError> {
    let usable_min_ns = derived.t2a_min_ns.max(capability.supported_min_ns);
    let usable_max_ns = derived.t2a_max_ns.min(capability.supported_max_ns);
    if usable_min_ns >= usable_max_ns {
        return Err(DelayMgmtError::IncompatibleWindows {
            derived_min_ns: derived.t2a_min_ns,
            derived_max_ns: derived.t2a_max_ns,
            supported_min_ns: capability.supported_min_ns,
            supported_max_ns: capability.supported_max_ns,
        });
    }
    Ok(WindowCompatibility {
        usable_min_ns,
        usable_max_ns,
        usable_width_ns: usable_max_ns - usable_min_ns,
    })
}

/// Air-interface instant of one OFDM symbol, in nanoseconds from the start of frame 0.
///
/// Symbol duration is modelled as an equal share of the slot; a real cyclic prefix is
/// slightly longer on the first symbol of every half subframe.
pub fn nr_symbol_air_time_ns(
    frame_id: u16,
    subframe_id: u8,
    slot_id: u8,
    symbol_id: u8,
    numerology: u8,
) -> i64 {
    let slots_per_subframe = 1i64 << numerology;
    let slot_duration_ns = NR_SUBFRAME_DURATION_NS / slots_per_subframe;
    let symbol_duration_ns = slot_duration_ns / NR_SYMBOLS_PER_SLOT;
    frame_id as i64 * NR_FRAME_DURATION_NS
        + subframe_id as i64 * NR_SUBFRAME_DURATION_NS
        + slot_id as i64 * slot_duration_ns
        + symbol_id as i64 * symbol_duration_ns
}

/// Counts how fronthaul arrivals fall against a reception window.
#[derive(Debug, Clone)]
pub struct OranDelayManager {
    pub window: OruReceptionWindow,
    pub on_time: u64,
    pub too_early: u64,
    pub too_late: u64,
    min_advance_ns: Option<i64>,
    max_advance_ns: Option<i64>,
    worst_margin_ns: Option<i64>,
}

impl OranDelayManager {
    pub fn new(window: OruReceptionWindow) -> Self {
        OranDelayManager {
            window,
            on_time: 0,
            too_early: 0,
            too_late: 0,
            min_advance_ns: None,
            max_advance_ns: None,
            worst_margin_ns: None,
        }
    }

    /// Records one packet given the air time of its symbol and when it actually arrived.
    pub fn observe(&mut self, air_time_ns: i64, arrival_ns: i64) -> WindowVerdict {
        self.observe_advance(air_time_ns - arrival_ns)
    }

    /// Records one packet already expressed as an advance ahead of its air time.
    pub fn observe_advance(&mut self, advance_ns: i64) -> WindowVerdict {
        let verdict = self.window.classify(advance_ns);
        match verdict {
            WindowVerdict::OnTime { margin_ns } => {
                self.on_time += 1;
                self.worst_margin_ns = Some(match self.worst_margin_ns {
                    Some(worst) => worst.min(margin_ns),
                    None => margin_ns,
                });
            }
            WindowVerdict::TooEarly { .. } => self.too_early += 1,
            WindowVerdict::TooLate { .. } => self.too_late += 1,
        }
        self.min_advance_ns = Some(
            self.min_advance_ns
                .map_or(advance_ns, |v| v.min(advance_ns)),
        );
        self.max_advance_ns = Some(
            self.max_advance_ns
                .map_or(advance_ns, |v| v.max(advance_ns)),
        );
        verdict
    }

    pub fn observed(&self) -> u64 {
        self.on_time + self.too_early + self.too_late
    }

    /// Fraction of packets the O-RU was able to use.
    pub fn on_time_ratio(&self) -> f64 {
        let total = self.observed();
        if total == 0 {
            return 0.0;
        }
        self.on_time as f64 / total as f64
    }

    /// Spread of observed arrival times: the packet delay variation actually seen.
    pub fn observed_variation_ns(&self) -> Option<i64> {
        Some(self.max_advance_ns? - self.min_advance_ns?)
    }

    /// Smallest margin any accepted packet had to a window edge.
    pub fn worst_margin_ns(&self) -> Option<i64> {
        self.worst_margin_ns
    }

    /// The narrowest window that would have accepted every packet observed so far.
    pub fn required_window(&self) -> Option<(i64, i64)> {
        Some((self.min_advance_ns?, self.max_advance_ns?))
    }

    /// Whether the configured window covers everything seen, and by what margin.
    pub fn headroom_ns(&self) -> Option<i64> {
        let (min, max) = self.required_window()?;
        Some((min - self.window.t2a_min_ns).min(self.window.t2a_max_ns - max))
    }

    pub fn reset(&mut self) {
        self.on_time = 0;
        self.too_early = 0;
        self.too_late = 0;
        self.min_advance_ns = None;
        self.max_advance_ns = None;
        self.worst_margin_ns = None;
    }
}
