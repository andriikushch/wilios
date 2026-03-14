#[derive(Clone)]
pub struct Tempo {
    pub bpm: u32,
}

impl Tempo {
    fn ms_per_beat(&self) -> f32 {
        60_000.0 / self.bpm as f32
    }

    pub fn duration_ms(&self, beats: usize, division: usize, dotted: bool) -> Result<u64, String> {
        if self.bpm == 0 {
            return Err("Tempo (BPM) cannot be zero".to_string());
        }
        if division == 0 {
            return Err("Duration division cannot be zero".to_string());
        }
        let mut b = beats as f32 * (4.0 / division as f32);

        if dotted {
            b *= 1.5;
        }

        Ok((b * self.ms_per_beat()) as u64)
    }
}

#[cfg(test)]
#[path = "tempo_tests.rs"]
mod tempo_tests;
