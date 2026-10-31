//! UI-timer conversion for the core-owned pass-through deadlines.

use edutools_core::state::{PassThroughDeadlines, PassThroughModeSource};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PassThroughTimerPlan {
    pub idle_ms: Option<u32>,
    pub maximum_hold_ms: Option<u32>,
    pub poll_key: Option<u32>,
}

impl PassThroughTimerPlan {
    pub fn from_core(
        now: f64,
        source: Option<PassThroughModeSource>,
        deadlines: PassThroughDeadlines,
    ) -> Self {
        Self {
            idle_ms: deadlines
                .idle
                .map(|deadline| deadline_delay_ms(now, deadline)),
            maximum_hold_ms: deadlines
                .maximum_hold
                .map(|deadline| deadline_delay_ms(now, deadline)),
            poll_key: match source {
                Some(PassThroughModeSource::Key(key)) => Some(key),
                _ => None,
            },
        }
    }
}

pub fn deadline_delay_ms(now: f64, deadline: f64) -> u32 {
    let remaining_ms = ((deadline - now).max(0.0) * 1_000.0).ceil();
    remaining_ms.clamp(1.0, f64::from(u32::MAX)) as u32
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyPollDecision {
    Continue,
    DisableDuringTrustWindow,
    EndPassThrough,
}

pub fn key_poll_decision(key_is_down: bool, elapsed: f64, trust_window: f64) -> KeyPollDecision {
    if key_is_down {
        KeyPollDecision::Continue
    } else if elapsed <= trust_window {
        KeyPollDecision::DisableDuringTrustWindow
    } else {
        KeyPollDecision::EndPassThrough
    }
}

#[cfg(test)]
mod tests {
    use super::{key_poll_decision, KeyPollDecision, PassThroughTimerPlan};
    use edutools_core::state::{AppState, PassThroughDefaults, PassThroughModeSource};

    #[test]
    fn pass_through_timer_mapping_and_reset_table() {
        let mut state = AppState::new();
        state.set_annotation_mode_enabled(true);
        assert!(state.set_pass_through_mode_enabled(true, 100.0, true));
        let cases = [
            (
                "timed initial",
                100.0,
                PassThroughTimerPlan {
                    idle_ms: Some(10_000),
                    maximum_hold_ms: None,
                    poll_key: None,
                },
            ),
            (
                "timed remaining",
                104.25,
                PassThroughTimerPlan {
                    idle_ms: Some(5_750),
                    maximum_hold_ms: None,
                    poll_key: None,
                },
            ),
        ];
        for (name, now, expected) in cases {
            assert_eq!(
                PassThroughTimerPlan::from_core(
                    now,
                    state.pass_through_mode_source(),
                    state.pass_through_deadlines(),
                ),
                expected,
                "{name}"
            );
        }

        assert!(state.record_pass_through_interaction(108.0));
        assert_eq!(
            PassThroughTimerPlan::from_core(
                108.0,
                state.pass_through_mode_source(),
                state.pass_through_deadlines(),
            )
            .idle_ms,
            Some(3_000)
        );
        assert!(!state.expire_pass_through(111.0, true));
        assert_eq!(state.pass_through_deadlines().idle, Some(114.0));
        assert!(state.expire_pass_through(114.0, false));
        assert_eq!(state.pass_through_mode_source(), None);

        assert!(state.begin_key_pass_through(0x55, 200.0));
        assert_eq!(
            PassThroughTimerPlan::from_core(
                200.0,
                state.pass_through_mode_source(),
                state.pass_through_deadlines(),
            ),
            PassThroughTimerPlan {
                idle_ms: None,
                maximum_hold_ms: Some(30_000),
                poll_key: Some(0x55),
            }
        );
        assert_eq!(
            state.pass_through_mode_source(),
            Some(PassThroughModeSource::Key(0x55))
        );
        assert!(!state.expire_pass_through(229.999, false));
        assert!(state.expire_pass_through(230.0, false));
        assert!(state.begin_key_pass_through(0x55, 300.0));
        assert!(state.end_key_pass_through(0x55));
    }

    #[test]
    fn key_polling_decision_table() {
        let cases = [
            (true, 0.5, KeyPollDecision::Continue),
            (
                false,
                PassThroughDefaults::KEY_STATE_POLLING_TRUST_WINDOW,
                KeyPollDecision::DisableDuringTrustWindow,
            ),
            (
                false,
                PassThroughDefaults::KEY_STATE_POLLING_TRUST_WINDOW + 0.001,
                KeyPollDecision::EndPassThrough,
            ),
        ];
        for (down, elapsed, expected) in cases {
            assert_eq!(
                key_poll_decision(
                    down,
                    elapsed,
                    PassThroughDefaults::KEY_STATE_POLLING_TRUST_WINDOW,
                ),
                expected
            );
        }
        assert_eq!(PassThroughDefaults::KEY_MAXIMUM_HOLD_DURATION, 30.0);
    }
}
