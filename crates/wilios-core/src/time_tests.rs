use super::*;

#[test]
fn three_thirds_sum_to_exactly_one() {
    let third = beats_from_duration(1, 3, false, "ctx").unwrap();
    let mut total = Ratio::from_integer(0);
    for _ in 0..3 {
        total = checked_add(total, third, "ctx").unwrap();
    }
    assert_eq!(total, Ratio::from_integer(1));
    assert_eq!(*total.numer(), 1);
    assert_eq!(*total.denom(), 1);
}

#[test]
fn dotted_quarter_is_three_eighths() {
    let b = beats_from_duration(1, 4, true, "ctx").unwrap();
    assert_eq!(b, Ratio::new(3, 8));
}

#[test]
fn denominator_too_large_diagnostic() {
    let err = beats_from_duration(1, 1 << 25, false, "ctx").unwrap_err();
    match err {
        TimeError::DenominatorTooLarge { denom, .. } => assert_eq!(denom, 1 << 25),
        other => panic!("expected DenominatorTooLarge, got {other:?}"),
    }
}

#[test]
fn checked_mul_overflow_does_not_panic() {
    let big = Ratio::new(1, i64::MAX / 2);
    let err = checked_mul(big, big, "ctx").unwrap_err();
    assert!(matches!(
        err,
        TimeError::Overflow { .. } | TimeError::DenominatorTooLarge { .. }
    ));
}

#[test]
fn zero_division_error_text() {
    let err = beats_from_duration(4, 0, false, "ctx").unwrap_err();
    assert_eq!(err.to_string(), "Duration division cannot be zero");
}

#[test]
fn non_positive_beats_rejected() {
    assert!(matches!(
        beats_from_duration(0, 4, false, "ctx"),
        Err(TimeError::NonPositiveBeats { .. })
    ));
    assert!(matches!(
        beats_from_duration(-1, 4, false, "ctx"),
        Err(TimeError::NonPositiveBeats { .. })
    ));
}

#[test]
fn tempo_history_multi_segment() {
    let mut history = TempoHistory::new(120);
    // First whole note at 120bpm: 240_000/120 = 2000ms.
    history
        .record_tempo_change(Ratio::from_integer(1), 60)
        .unwrap();
    // Next half whole note at 60bpm: 240_000/60 * 1/2 = 2000ms.
    let ms = history.ms_at(Ratio::new(3, 2)).unwrap();
    assert_eq!(ms, 4000);
    // Earlier segment's value must not have been retroactively changed.
    let ms_at_change_point = history.ms_at(Ratio::from_integer(1)).unwrap();
    assert_eq!(ms_at_change_point, 2000);
}

#[test]
fn round_half_up_matches_existing_truncation_on_clean_fractions() {
    assert_eq!(beats_delta_to_ms(Ratio::new(1, 4), 120).unwrap(), 500);
    assert_eq!(beats_delta_to_ms(Ratio::new(1, 2), 120).unwrap(), 1000);
    assert_eq!(
        beats_delta_to_ms(Ratio::new(3, 8), 120).unwrap(), // dotted 1/4
        750
    );
}

#[test]
fn triplet_ms_no_longer_truncates_the_group() {
    // At 120bpm, three exact 1/3 durations must sum to exactly 2000ms,
    // not 1998ms (the old f32-truncation result).
    let third = beats_from_duration(1, 3, false, "ctx").unwrap();
    let mut position = Ratio::from_integer(0);
    for _ in 0..3 {
        position = checked_add(position, third, "ctx").unwrap();
    }
    let history = TempoHistory::new(120);
    assert_eq!(history.ms_at(position).unwrap(), 2000);
}

#[test]
fn rem_euclid_wraps_bar_position() {
    let bar_len = Ratio::from_integer(1); // 1 whole note per bar
    let pos = Ratio::new(5, 4); // 1 bar + a quarter note
    let wrapped = rem_euclid(pos, bar_len, "ctx").unwrap();
    assert_eq!(wrapped, Ratio::new(1, 4));
}
