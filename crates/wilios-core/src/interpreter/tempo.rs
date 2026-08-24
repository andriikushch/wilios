#[derive(Clone)]
pub struct Tempo {
    pub bpm: u32,
}

impl Tempo {
    pub fn duration_ms(&self, beats: usize, division: usize, dotted: bool) -> Result<u64, String> {
        if self.bpm == 0 {
            return Err("Tempo (BPM) cannot be zero".to_string());
        }
        let b = crate::time::beats_from_duration(
            beats as i64,
            division as i64,
            dotted,
            "Tempo::duration_ms",
        )
        .map_err(|e| e.to_string())?;
        crate::time::beats_delta_to_ms(b, self.bpm).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "tempo_tests.rs"]
mod tempo_tests;
