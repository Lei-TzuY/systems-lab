//! 3GPP Rel-17 Non-Terrestrial Networks (NTN) Polarization & Doppler Tracking Engine.
//!
//! Implements 3GPP TS 38.300 §16.14, TS 38.101-5, TS 38.211 §5.3 / §6.4, and ITU-R P.618-13:
//! - Dual circular polarization (RHCP / LHCP) modeling and Axial Ratio ($AR$).
//! - Cross-Polarization Discrimination ($XPD = 20 \log_{10} \frac{AR + 1}{AR - 1}\text{ dB}$) calculation.
//! - Polarization mismatch efficiency ($\eta_{pol}$) and cross-polarization leakage power.
//! - LEO/MEO/GEO Keplerian orbital kinematics, radial velocity, and line-of-sight Doppler shift.
//! - Maximum Doppler drift rate ($\dot{f}_{D, max}$) at closest approach / zenith pass.
//! - 2nd-order Doppler Frequency-Locked Loop (FLL) servo tracking frequency and chirp rate.
//! - Autonomous UE Uplink Pre-compensation maintaining subcarrier orthogonality ($\le 1\%$ of SCS).
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::fmt;

/// Speed of light in vacuum ($c$) in meters per second.
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// Earth gravitational parameter ($G M_E$) in $\text{m}^3 / \text{s}^2$.
pub const EARTH_GRAVITATIONAL_PARAM: f64 = 3.986_004_418e14;

/// Mean Earth radius ($R_E$) in meters.
pub const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

/// Maximum permissible residual Doppler error as a fraction of Subcarrier Spacing (1%).
pub const MAX_RESIDUAL_DOPPLER_SCS_RATIO: f64 = 0.01;

/// Polarization sense for satellite space-ground radio links.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolarizationSense {
    /// Right-Hand Circular Polarization.
    Rhcp,
    /// Left-Hand Circular Polarization.
    Lhcp,
    /// Linear Horizontal Polarization.
    LinearHorizontal,
    /// Linear Vertical Polarization.
    LinearVertical,
}

/// Errors raised during NTN polarization and Doppler tracking.
#[derive(Debug, Clone, PartialEq)]
pub enum NtnPolarizationError {
    InvalidAxialRatio(f64),
    InvalidAltitude(f64),
    InvalidCarrierFrequency(f64),
    InvalidSubcarrierSpacing(f64),
    SubcarrierOrthogonalityLost {
        residual_hz: f64,
        scs_hz: f64,
        ratio: f64,
    },
}

impl fmt::Display for NtnPolarizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NtnPolarizationError::InvalidAxialRatio(ar) => {
                write!(f, "Invalid axial ratio {:.3} (must be >= 1.0)", ar)
            }
            NtnPolarizationError::InvalidAltitude(h) => {
                write!(
                    f,
                    "Invalid satellite orbital altitude {:.1} km (must be > 0)",
                    h / 1000.0
                )
            }
            NtnPolarizationError::InvalidCarrierFrequency(fc) => {
                write!(
                    f,
                    "Invalid carrier frequency {:.1} MHz (must be > 0)",
                    fc / 1e6
                )
            }
            NtnPolarizationError::InvalidSubcarrierSpacing(scs) => {
                write!(
                    f,
                    "Invalid subcarrier spacing {:.1} kHz (must be > 0)",
                    scs / 1e3
                )
            }
            NtnPolarizationError::SubcarrierOrthogonalityLost {
                residual_hz,
                scs_hz,
                ratio,
            } => {
                write!(
                    f,
                    "Residual Doppler {:.1} Hz exceeds {:.2}% of SCS {:.1} kHz (ratio: {:.4})",
                    residual_hz,
                    MAX_RESIDUAL_DOPPLER_SCS_RATIO * 100.0,
                    scs_hz / 1e3,
                    ratio
                )
            }
        }
    }
}

impl std::error::Error for NtnPolarizationError {}

/// Circular and Linear Polarization Tracker (ITU-R P.618-13).
#[derive(Debug, Clone, PartialEq)]
pub struct PolarizationTracker {
    pub sense: PolarizationSense,
    /// Axial Ratio ($AR \ge 1.0$): ratio of major axis to minor axis of polarization ellipse.
    pub axial_ratio: f64,
}

impl PolarizationTracker {
    pub fn new(sense: PolarizationSense, axial_ratio: f64) -> Result<Self, NtnPolarizationError> {
        if axial_ratio < 1.0 {
            return Err(NtnPolarizationError::InvalidAxialRatio(axial_ratio));
        }
        Ok(Self { sense, axial_ratio })
    }

    /// Computes Cross-Polarization Discrimination ($XPD$) in dB:
    /// $$XPD = 20 \log_{10} \left( \frac{AR + 1}{AR - 1} \right)\text{ dB}$$
    /// Returns 100.0 dB if $AR = 1.0$ (perfect circular polarization).
    pub fn cross_polarization_discrimination_db(&self) -> f64 {
        if (self.axial_ratio - 1.0).abs() < 1e-6 {
            return 100.0;
        }
        let ar = self.axial_ratio;
        let ratio = (ar + 1.0) / (ar - 1.0);
        20.0 * ratio.log10()
    }

    /// Computes polarization coupling efficiency ($\eta_{pol}$) with an incoming wave.
    /// - If senses match (e.g. RHCP to RHCP): high efficiency ($\approx 1.0$).
    /// - If senses are orthogonal (e.g. RHCP to LHCP): near zero ($XPD$ isolation).
    pub fn coupling_efficiency(&self, incoming: &PolarizationTracker) -> f64 {
        let ar1 = self.axial_ratio;
        let ar2 = incoming.axial_ratio;

        let same_sense = self.sense == incoming.sense;
        let num = 4.0 * ar1 * ar2;
        let den = (ar1 * ar1 + 1.0) * (ar2 * ar2 + 1.0);
        let factor = num / den;

        if same_sense {
            (0.5 * (1.0 + factor)).clamp(0.0, 1.0)
        } else {
            (0.5 * (1.0 - factor)).clamp(0.0, 1.0)
        }
    }

    /// Computes polarization mismatch loss in dB:
    /// $$\Delta L_{pol} = -10 \log_{10}(\eta_{pol})$$
    pub fn polarization_mismatch_loss_db(&self, incoming: &PolarizationTracker) -> f64 {
        let eff = self.coupling_efficiency(incoming);
        if eff <= 1e-10 {
            return 100.0; // Complete cross-polarization isolation
        }
        -10.0 * eff.log10()
    }
}

/// Satellite Orbit Kinematics and Doppler Calculator.
#[derive(Debug, Clone, PartialEq)]
pub struct SatelliteKinematics {
    pub altitude_m: f64,
    pub carrier_freq_hz: f64,
    pub orbital_velocity_m_s: f64,
}

impl SatelliteKinematics {
    pub fn new(altitude_m: f64, carrier_freq_hz: f64) -> Result<Self, NtnPolarizationError> {
        if altitude_m <= 0.0 {
            return Err(NtnPolarizationError::InvalidAltitude(altitude_m));
        }
        if carrier_freq_hz <= 0.0 {
            return Err(NtnPolarizationError::InvalidCarrierFrequency(
                carrier_freq_hz,
            ));
        }

        // Orbital radius: R_E + h
        let r_orbit = EARTH_RADIUS_METERS + altitude_m;
        // Circular orbital velocity: v = sqrt(GM / R)
        let orbital_velocity_m_s = (EARTH_GRAVITATIONAL_PARAM / r_orbit).sqrt();

        Ok(Self {
            altitude_m,
            carrier_freq_hz,
            orbital_velocity_m_s,
        })
    }

    /// Computes theoretical line-of-sight Doppler frequency shift in Hz:
    /// $$\Delta f_D = - \frac{f_c}{c} \cdot v_{radial}$$
    /// where $v_{radial} > 0$ denotes satellite approaching ground UE (blue-shift $\Delta f_D > 0$),
    /// and $v_{radial} < 0$ denotes receding satellite (red-shift $\Delta f_D < 0$).
    #[inline]
    pub fn doppler_shift_hz(&self, radial_velocity_m_s: f64) -> f64 {
        -(self.carrier_freq_hz / SPEED_OF_LIGHT_M_S) * radial_velocity_m_s
    }

    /// Computes theoretical radial velocity and Doppler shift for a ground-track distance $x$
    /// (meters along track, $x < 0$ approaching, $x = 0$ closest approach/zenith, $x > 0$ receding).
    pub fn doppler_at_ground_distance(&self, ground_track_x_m: f64) -> (f64, f64) {
        let slant_range =
            (ground_track_x_m * ground_track_x_m + self.altitude_m * self.altitude_m).sqrt();
        let radial_velocity = self.orbital_velocity_m_s * (ground_track_x_m / slant_range);
        let doppler_hz = self.doppler_shift_hz(radial_velocity);
        (radial_velocity, doppler_hz)
    }

    /// Maximum Doppler drift rate (frequency derivative / chirp rate) at closest approach (zenith):
    /// $$\dot{f}_{D, max} = - \frac{f_c}{c} \cdot \frac{v_{orb}^2}{h}$$
    pub fn max_doppler_drift_rate_hz_s(&self) -> f64 {
        let accel = (self.orbital_velocity_m_s * self.orbital_velocity_m_s) / self.altitude_m;
        -(self.carrier_freq_hz / SPEED_OF_LIGHT_M_S) * accel
    }
}

/// 2nd-Order Frequency-Locked Loop (FLL) Doppler Servo Tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct DopplerFllServo {
    pub estimated_doppler_hz: f64,
    pub estimated_drift_rate_hz_s: f64,
    pub alpha: f64, // Proportional loop gain
    pub beta: f64,  // Integral loop gain
    pub last_update_time_s: f64,
}

impl DopplerFllServo {
    /// Creates a new FLL servo with loop gains tuned for dynamic LEO tracking.
    pub fn new(alpha: f64, beta: f64) -> Self {
        Self {
            estimated_doppler_hz: 0.0,
            estimated_drift_rate_hz_s: 0.0,
            alpha,
            beta,
            last_update_time_s: 0.0,
        }
    }

    /// Updates the servo with a new frequency measurement at timestamp `time_s`.
    pub fn update(&mut self, measured_doppler_hz: f64, time_s: f64) {
        let dt = (time_s - self.last_update_time_s).max(1e-4);
        self.last_update_time_s = time_s;

        // Innovation / Frequency error
        let freq_err = measured_doppler_hz - self.estimated_doppler_hz;

        // 2nd-order loop filter updates:
        // 1. Drift rate (acceleration) integral update
        self.estimated_drift_rate_hz_s += self.beta * (freq_err / dt);

        // 2. Frequency state update (propagation + proportional correction)
        self.estimated_doppler_hz += self.estimated_drift_rate_hz_s * dt + self.alpha * freq_err;
    }

    /// Computes Autonomous UE Uplink Pre-compensation frequency shift:
    /// $$f_{pre} = - \left(\hat{f}_D + \hat{\dot{f}}_D \cdot \frac{T_{RTT}}{2}\right)$$
    /// Pre-compensates both current Doppler and propagation drift over one-way transit delay.
    pub fn compute_uplink_precompensation(&self, rtt_seconds: f64) -> f64 {
        let one_way_delay = rtt_seconds * 0.5;
        -(self.estimated_doppler_hz + self.estimated_drift_rate_hz_s * one_way_delay)
    }
}

/// Telemetry and Subcarrier Orthogonality Compliance Metrics.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NtnDopplerMetrics {
    pub measured_doppler_hz: f64,
    pub estimated_doppler_hz: f64,
    pub residual_doppler_error_hz: f64,
    pub doppler_drift_rate_hz_s: f64,
    pub uplink_precompensation_hz: f64,
    pub subcarrier_spacing_hz: f64,
    pub residual_scs_ratio: f64,
    pub polarization_xpd_db: f64,
    pub polarization_loss_db: f64,
}

impl NtnDopplerMetrics {
    /// Validates whether residual Doppler error complies with subcarrier orthogonality bounds:
    /// $$\frac{\Delta f_{residual}}{\Delta f_{SCS}} \le 1.0\%$$
    pub fn check_subcarrier_orthogonality(&self) -> Result<(), NtnPolarizationError> {
        if self.subcarrier_spacing_hz <= 0.0 {
            return Err(NtnPolarizationError::InvalidSubcarrierSpacing(
                self.subcarrier_spacing_hz,
            ));
        }
        if self.residual_scs_ratio > MAX_RESIDUAL_DOPPLER_SCS_RATIO {
            return Err(NtnPolarizationError::SubcarrierOrthogonalityLost {
                residual_hz: self.residual_doppler_error_hz,
                scs_hz: self.subcarrier_spacing_hz,
                ratio: self.residual_scs_ratio,
            });
        }
        Ok(())
    }
}
