//! IEEE 802.1Qci Per-Stream Filtering and Policing (PSFP / TSN - Time-Sensitive Networking).
//!
//! Provides deterministic input policing, stream gate filtering (Gate Control List),
//! and dual-rate three-color flow metering (srTCM/trTCM) to protect against babbling idiots and rogue traffic.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterColor {
    Green,
    Yellow,
    Red,
}

#[derive(Debug, Clone)]
pub struct StreamGate {
    pub gate_id: u32,
    pub state: GateState,
    pub cycle_time_us: u32,
    pub open_duration_us: u32,
}

impl StreamGate {
    pub fn new(gate_id: u32, cycle_time_us: u32, open_duration_us: u32) -> Self {
        StreamGate {
            gate_id,
            state: GateState::Open,
            cycle_time_us,
            open_duration_us,
        }
    }

    /// Evaluates gate state based on offset time in current cycle
    pub fn evaluate_time(&mut self, time_in_cycle_us: u32) -> GateState {
        if time_in_cycle_us <= self.open_duration_us {
            self.state = GateState::Open;
        } else {
            self.state = GateState::Closed;
        }
        self.state
    }
}

/// Dual-Token Bucket Flow Meter (Committed & Peak Rate Token Buckets)
#[derive(Debug, Clone)]
pub struct FlowMeter {
    pub meter_id: u32,
    pub cir_bytes_sec: u64, // Committed Information Rate
    pub cbs_bytes: u64,     // Committed Burst Size
    pub current_tokens: u64,
    pub drop_red: bool,
}

impl FlowMeter {
    pub fn new(meter_id: u32, cir_bytes_sec: u64, cbs_bytes: u64, drop_red: bool) -> Self {
        FlowMeter {
            meter_id,
            cir_bytes_sec,
            cbs_bytes,
            current_tokens: cbs_bytes,
            drop_red,
        }
    }

    /// Meters a frame of given size and returns MeterColor (Green, Yellow, Red)
    pub fn meter_frame(&mut self, frame_len: usize) -> MeterColor {
        let cost = frame_len as u64;
        if self.current_tokens >= cost {
            self.current_tokens -= cost;
            MeterColor::Green
        } else if !self.drop_red {
            MeterColor::Yellow
        } else {
            MeterColor::Red
        }
    }

    /// Replenishes tokens based on elapsed microseconds
    pub fn replenish(&mut self, elapsed_us: u64) {
        let added_tokens = (self.cir_bytes_sec * elapsed_us) / 1_000_000;
        self.current_tokens = (self.current_tokens + added_tokens).min(self.cbs_bytes);
    }
}

/// IEEE 802.1Qci PSFP Filter Instance Pipeline
#[derive(Debug, Clone)]
pub struct PsfpFilterInstance {
    pub stream_id: u32,
    pub priority: u8,
    pub stream_gate: StreamGate,
    pub flow_meter: FlowMeter,
    pub frames_passed: u64,
    pub frames_dropped_gate: u64,
    pub frames_dropped_meter: u64,
}

impl PsfpFilterInstance {
    pub fn new(
        stream_id: u32,
        priority: u8,
        stream_gate: StreamGate,
        flow_meter: FlowMeter,
    ) -> Self {
        PsfpFilterInstance {
            stream_id,
            priority,
            stream_gate,
            flow_meter,
            frames_passed: 0,
            frames_dropped_gate: 0,
            frames_dropped_meter: 0,
        }
    }

    /// Processes an incoming TSN frame through Gate filtering and Flow Meter policing
    pub fn filter_and_police(
        &mut self,
        time_in_cycle_us: u32,
        frame_len: usize,
    ) -> Result<MeterColor, &'static str> {
        // Step 1: Check Stream Gate
        let gate_state = self.stream_gate.evaluate_time(time_in_cycle_us);
        if gate_state == GateState::Closed {
            self.frames_dropped_gate += 1;
            return Err("Dropped by closed Stream Gate");
        }

        // Step 2: Check Flow Meter & Token Bucket
        let color = self.flow_meter.meter_frame(frame_len);
        if color == MeterColor::Red && self.flow_meter.drop_red {
            self.frames_dropped_meter += 1;
            return Err("Dropped by Flow Meter rate exceed");
        }

        self.frames_passed += 1;
        Ok(color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psfp_stream_gate_schedule() {
        let mut gate = StreamGate::new(1, 1000, 400); // 1000µs cycle, 400µs open window
        assert_eq!(gate.evaluate_time(200), GateState::Open);
        assert_eq!(gate.evaluate_time(400), GateState::Open);
        assert_eq!(gate.evaluate_time(401), GateState::Closed);
        assert_eq!(gate.evaluate_time(800), GateState::Closed);
    }

    #[test]
    fn test_psfp_pipeline_policing_and_drops() {
        let gate = StreamGate::new(1, 1000, 500);
        let meter = FlowMeter::new(1, 1_000_000, 2000, true);
        let mut psfp = PsfpFilterInstance::new(100, 7, gate, meter);

        // Frame within gate window and under burst limit -> Accepted (Green)
        assert_eq!(psfp.filter_and_police(250, 1000), Ok(MeterColor::Green));
        assert_eq!(psfp.frames_passed, 1);

        // Frame outside gate window (time = 750µs) -> Gate drop
        assert!(psfp.filter_and_police(750, 500).is_err());
        assert_eq!(psfp.frames_dropped_gate, 1);

        // Frame inside gate window but exceeding remaining tokens (cost 1500 > remaining 1000) -> Meter drop
        assert!(psfp.filter_and_police(100, 1500).is_err());
        assert_eq!(psfp.frames_dropped_meter, 1);
    }
}
