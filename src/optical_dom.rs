//! Digital Optical Monitoring & Transceiver Telemetry (SFF-8472 / SFF-8636 - DOM).
//!
//! Models physical layer optical transceiver diagnostics (Temperature, Voltage,
//! Tx Bias Current, Tx/Rx Optical Power in dBm/mW, and Alarm/Warning Thresholds).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransceiverFormFactor {
    SfpPlus10G,
    Qsfp28_100G,
    QsfpDd400G,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpticalThresholds {
    pub temp_high_alarm_c: f32,
    pub temp_low_alarm_c: f32,
    pub tx_power_high_alarm_dbm: f32,
    pub tx_power_low_alarm_dbm: f32,
    pub rx_power_high_alarm_dbm: f32,
    pub rx_power_low_alarm_dbm: f32, // Receiver sensitivity limit
}

impl Default for OpticalThresholds {
    fn default() -> Self {
        OpticalThresholds {
            temp_high_alarm_c: 75.0,
            temp_low_alarm_c: -5.0,
            tx_power_high_alarm_dbm: 2.0,
            tx_power_low_alarm_dbm: -10.0,
            rx_power_high_alarm_dbm: 2.0,
            rx_power_low_alarm_dbm: -18.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpticalDiagnostics {
    pub port_name: String,
    pub form_factor: TransceiverFormFactor,
    pub temperature_c: f32,
    pub supply_voltage_v: f32,
    pub tx_bias_current_ma: f32,
    pub tx_power_dbm: f32,
    pub rx_power_dbm: f32,
    pub thresholds: OpticalThresholds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpticalAlarmStatus {
    pub temp_alarm: bool,
    pub rx_los: bool, // Loss of Signal
    pub tx_fault: bool,
    pub rx_power_low: bool,
    pub tx_power_low: bool,
}

impl OpticalDiagnostics {
    pub fn new(
        port_name: &str,
        form_factor: TransceiverFormFactor,
        temperature_c: f32,
        voltage: f32,
        tx_bias_ma: f32,
        tx_power_dbm: f32,
        rx_power_dbm: f32,
    ) -> Self {
        OpticalDiagnostics {
            port_name: port_name.to_string(),
            form_factor,
            temperature_c,
            supply_voltage_v: voltage,
            tx_bias_current_ma: tx_bias_ma,
            tx_power_dbm,
            rx_power_dbm,
            thresholds: OpticalThresholds::default(),
        }
    }

    /// Converts dBm to optical milliwatts (mW)
    pub fn dbm_to_mw(dbm: f32) -> f32 {
        // P(mW) = 10^(P(dBm)/10)
        // Approximate 10^(x) via exp(x * ln(10))
        (dbm / 10.0 * std::f32::consts::LN_10).exp()
    }

    /// Calculates link optical attenuation (path loss) in dB
    pub fn link_attenuation_db(&self) -> f32 {
        self.tx_power_dbm - self.rx_power_dbm
    }

    /// Calculates receiver power safety margin before reaching sensitivity threshold
    pub fn rx_optical_margin_db(&self) -> f32 {
        self.rx_power_dbm - self.thresholds.rx_power_low_alarm_dbm
    }

    /// Evaluates transceiver alarm and warning bits
    pub fn evaluate_alarms(&self) -> OpticalAlarmStatus {
        let temp_alarm = self.temperature_c > self.thresholds.temp_high_alarm_c
            || self.temperature_c < self.thresholds.temp_low_alarm_c;
        let rx_power_low = self.rx_power_dbm < self.thresholds.rx_power_low_alarm_dbm;
        let rx_los = self.rx_power_dbm <= -30.0;
        let tx_power_low = self.tx_power_dbm < self.thresholds.tx_power_low_alarm_dbm;
        let tx_fault = self.tx_bias_current_ma <= 0.0 || self.tx_bias_current_ma > 80.0;

        OpticalAlarmStatus {
            temp_alarm,
            rx_los,
            tx_fault,
            rx_power_low,
            tx_power_low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optical_dom_diagnostics_and_alarms() {
        let port_dom = OpticalDiagnostics::new(
            "Ethernet1/1",
            TransceiverFormFactor::Qsfp28_100G,
            38.5,
            3.31,
            35.2,
            -1.5,
            -8.5,
        );

        assert_eq!(port_dom.link_attenuation_db(), 7.0);
        assert_eq!(port_dom.rx_optical_margin_db(), 9.5); // -8.5 - (-18.0) = 9.5 dB margin

        let alarms = port_dom.evaluate_alarms();
        assert!(!alarms.temp_alarm);
        assert!(!alarms.rx_los);
        assert!(!alarms.tx_fault);
        assert!(!alarms.rx_power_low);
    }

    #[test]
    fn test_optical_loss_of_signal_alarm() {
        let port_dom = OpticalDiagnostics::new(
            "Ethernet1/2",
            TransceiverFormFactor::SfpPlus10G,
            42.0,
            3.28,
            30.0,
            -2.0,
            -35.0, // Dark fiber (No light)
        );

        let alarms = port_dom.evaluate_alarms();
        assert!(alarms.rx_los);
        assert!(alarms.rx_power_low);
    }
}
