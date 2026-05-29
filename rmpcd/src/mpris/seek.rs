#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekPlan {
    Absolute { secs: u32, seeked_us: i64 },
    Relative { delta_secs: i64, seeked_us: i64 },
    Next,
    Ignore,
}

pub fn plan_set_position(
    current_id: Option<u32>,
    requested_id: Option<u32>,
    position_us: i64,
    duration_us: i64,
) -> SeekPlan {
    if duration_us <= 0 || position_us < 0 || position_us > duration_us {
        return SeekPlan::Ignore;
    }
    match (current_id, requested_id) {
        (Some(cur), Some(req)) if cur == req => {}
        _ => return SeekPlan::Ignore,
    }
    SeekPlan::Absolute { secs: (position_us / 1_000_000) as u32, seeked_us: position_us }
}

pub fn plan_relative_seek(elapsed_us: i64, duration_us: i64, offset_us: i64) -> SeekPlan {
    if duration_us <= 0 {
        return SeekPlan::Ignore;
    }
    let target_us = elapsed_us.saturating_add(offset_us);
    if target_us >= duration_us {
        return SeekPlan::Next;
    }
    SeekPlan::Relative { delta_secs: offset_us / 1_000_000, seeked_us: target_us.max(0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEC: i64 = 1_000_000;

    #[test]
    fn set_position_valid_seeks_absolute() {
        assert_eq!(plan_set_position(Some(7), Some(7), 90 * SEC, 200 * SEC), SeekPlan::Absolute {
            secs: 90,
            seeked_us: 90 * SEC
        });
    }

    #[test]
    fn set_position_at_exact_end_is_allowed() {
        assert_eq!(plan_set_position(Some(7), Some(7), 200 * SEC, 200 * SEC), SeekPlan::Absolute {
            secs: 200,
            seeked_us: 200 * SEC
        });
    }

    #[test]
    fn set_position_negative_is_ignored() {
        assert_eq!(plan_set_position(Some(7), Some(7), -1, 200 * SEC), SeekPlan::Ignore);
    }

    #[test]
    fn set_position_beyond_duration_is_ignored() {
        assert_eq!(plan_set_position(Some(7), Some(7), 200 * SEC + 1, 200 * SEC), SeekPlan::Ignore);
    }

    #[test]
    fn set_position_mismatched_track_is_ignored() {
        assert_eq!(plan_set_position(Some(7), Some(8), 10 * SEC, 200 * SEC), SeekPlan::Ignore);
    }

    #[test]
    fn set_position_no_current_song_is_ignored() {
        assert_eq!(plan_set_position(None, Some(7), 10 * SEC, 200 * SEC), SeekPlan::Ignore);
    }

    #[test]
    fn set_position_unparsable_track_is_ignored() {
        assert_eq!(plan_set_position(Some(7), None, 10 * SEC, 200 * SEC), SeekPlan::Ignore);
    }

    #[test]
    fn set_position_no_duration_is_ignored() {
        assert_eq!(plan_set_position(Some(7), Some(7), 0, 0), SeekPlan::Ignore);
    }

    #[test]
    fn relative_forward_within_bounds() {
        assert_eq!(plan_relative_seek(10 * SEC, 200 * SEC, 30 * SEC), SeekPlan::Relative {
            delta_secs: 30,
            seeked_us: 40 * SEC
        });
    }

    #[test]
    fn relative_backward_keeps_sign() {
        assert_eq!(plan_relative_seek(60 * SEC, 200 * SEC, -30 * SEC), SeekPlan::Relative {
            delta_secs: -30,
            seeked_us: 30 * SEC
        });
    }

    #[test]
    fn relative_backward_past_start_clamps_seeked_to_zero() {
        assert_eq!(plan_relative_seek(10 * SEC, 200 * SEC, -30 * SEC), SeekPlan::Relative {
            delta_secs: -30,
            seeked_us: 0
        });
    }

    #[test]
    fn relative_forward_past_end_is_next() {
        assert_eq!(plan_relative_seek(190 * SEC, 200 * SEC, 30 * SEC), SeekPlan::Next);
    }

    #[test]
    fn relative_reaching_exact_end_is_next() {
        assert_eq!(plan_relative_seek(170 * SEC, 200 * SEC, 30 * SEC), SeekPlan::Next);
    }

    #[test]
    fn relative_no_duration_is_ignored() {
        assert_eq!(plan_relative_seek(0, 0, 5 * SEC), SeekPlan::Ignore);
    }

    #[test]
    fn relative_sub_second_offset_truncates_delta() {
        assert_eq!(plan_relative_seek(10 * SEC, 200 * SEC, 2_500_000), SeekPlan::Relative {
            delta_secs: 2,
            seeked_us: 12_500_000
        });
    }
}
