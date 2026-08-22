#[derive(Debug, Clone, Copy)]
pub enum PitchName {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl PitchName {
    pub fn from_string(letter: char) -> PitchName {
        match letter {
            'C' => PitchName::C,
            'D' => PitchName::D,
            'E' => PitchName::E,
            'F' => PitchName::F,
            'G' => PitchName::G,
            'A' => PitchName::A,
            'B' => PitchName::B,
            _ => panic!("Unexpected pitch"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Accidental {
    Natural,
    Sharp,
    Flat,
}

impl Accidental {
    pub fn from_int(accidental: isize) -> Accidental {
        match accidental {
            -1 => Accidental::Flat,
            0 => Accidental::Natural,
            1 => Accidental::Sharp,
            _ => panic!("invalid accidental"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Pitch {
    pub name: PitchName,
    pub accidental: Accidental,
}

pub fn note_frequency(p: Pitch, octave: u8) -> f32 {
    let a4 = 440.0;
    let a4_note = 9 + 12 * 4;
    let n = pitch_to_semitone(p) + 12 * octave as i32;
    a4 * 2f32.powf((n - a4_note) as f32 / 12.0)
}

pub fn pitch_to_semitone(p: Pitch) -> i32 {
    let base = match p.name {
        PitchName::C => 0,
        PitchName::D => 2,
        PitchName::E => 4,
        PitchName::F => 5,
        PitchName::G => 7,
        PitchName::A => 9,
        PitchName::B => 11,
    };
    match p.accidental {
        Accidental::Natural => base,
        Accidental::Sharp => base + 1,
        Accidental::Flat => base - 1,
    }
}
