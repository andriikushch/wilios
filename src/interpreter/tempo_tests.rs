use super::Tempo;

#[test]
fn duration_ms() {
    assert_eq!(500, (Tempo { bpm: 120 }).duration_ms(1, 4, false).unwrap());
    assert_eq!(1000, (Tempo { bpm: 120 }).duration_ms(2, 4, false).unwrap());
    assert_eq!(250, (Tempo { bpm: 120 }).duration_ms(1, 8, false).unwrap());
    assert_eq!(750, (Tempo { bpm: 120 }).duration_ms(1, 4, true).unwrap());
    assert_eq!(750, (Tempo { bpm: 120 }).duration_ms(3, 8, false).unwrap());
}

#[test]
fn duration_ms_division_by_zero() {
    assert!((Tempo { bpm: 120 }).duration_ms(4, 0, false).is_err());
}
