// ============================================================================
// File: client/src/input/action_buffer.rs
// ============================================================================
// ----------------------------------------------------------------------------
// EVENTS, ENUMS & ACTION BUFFER (Unified Input Pipeline)
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use spacetime_rts_logic::TacticalAbilityKind;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum VirtualAction {
    Primary,
    Secondary,
    Interact,
    Jump,
    UseAbility(TacticalAbilityKind),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionState {
    JustPressed,
    Pressed,
    JustReleased,
}

#[derive(Event, Debug, Clone)]
pub struct ActionEvent {
    pub action: VirtualAction,
    pub state: ActionState,
    pub cursor_pos: Option<Vec2>,
    pub is_over_ui: bool, 
}

/// A buffered player action intent with Time-To-Live (TTL) preventing "eaten inputs".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferedAction {
    pub action: VirtualAction,
    pub timestamp_seconds: f64,
    pub ttl_seconds: f32,
}

impl BufferedAction {
    pub fn new(action: VirtualAction, timestamp_seconds: f64, ttl_seconds: f32) -> Self {
        Self {
            action,
            timestamp_seconds,
            ttl_seconds,
        }
    }

    pub fn is_expired(&self, current_time: f64) -> bool {
        (current_time - self.timestamp_seconds) > (self.ttl_seconds as f64)
    }
}

/// Central action buffer retaining recent intent triggers across physics/cooldown frames.
#[derive(Resource, Default, Debug)]
pub struct ActionBuffer {
    pub actions: Vec<BufferedAction>,
}

impl ActionBuffer {
    pub fn push(&mut self, action: VirtualAction, current_time: f64, ttl_seconds: f32) {
        self.actions.push(BufferedAction::new(action, current_time, ttl_seconds));
    }

    pub fn pop_matching<F>(&mut self, current_time: f64, predicate: F) -> Option<VirtualAction>
    where
        F: Fn(VirtualAction) -> bool,
    {
        self.actions.retain(|item| !item.is_expired(current_time));
        if let Some(idx) = self.actions.iter().position(|item| predicate(item.action)) {
            Some(self.actions.remove(idx).action)
        } else {
            None
        }
    }

    pub fn prune(&mut self, current_time: f64) {
        self.actions.retain(|item| !item.is_expired(current_time));
    }
}

/// Locomotion feel settings (coyote time, jump buffer window, and movement speeds).
#[derive(Resource, Debug, Clone)]
pub struct LocomotionSettings {
    pub coyote_time_max: f32,
    pub jump_buffer_max: f32,
    pub horizontal_speed: f32,
    pub jump_impulse: f32,
}

impl Default for LocomotionSettings {
    fn default() -> Self {
        Self {
            coyote_time_max: 0.12,
            jump_buffer_max: 0.12,
            horizontal_speed: 15.0,
            jump_impulse: 10.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::LocomotionState;

    #[test]
    fn test_action_buffer_push_pop_and_ttl_expiration() {
        let mut buffer = ActionBuffer::default();
        let current_time = 100.0;

        buffer.push(VirtualAction::Jump, current_time, 0.12);
        buffer.push(VirtualAction::UseAbility(TacticalAbilityKind::PhaseDash), current_time, 0.12);
        assert_eq!(buffer.actions.len(), 2);

        // Pop jump
        let popped = buffer.pop_matching(current_time + 0.05, |a| a == VirtualAction::Jump);
        assert_eq!(popped, Some(VirtualAction::Jump));
        assert_eq!(buffer.actions.len(), 1);

        // Advancing time past TTL (0.12s) expires remaining actions
        let popped_expired = buffer.pop_matching(current_time + 0.20, |a| a == VirtualAction::UseAbility(TacticalAbilityKind::PhaseDash));
        assert_eq!(popped_expired, None);
        assert_eq!(buffer.actions.len(), 0);
    }

    #[test]
    fn test_coyote_time_grace_window() {
        let settings = LocomotionSettings::default();
        let mut loco = LocomotionState::default();

        // On ground: timer is 0
        loco.time_since_grounded = 0.0;
        let mut can_jump = loco.time_since_grounded <= settings.coyote_time_max;
        assert!(can_jump);

        // Falling off a ledge for 0.08s (within 0.12s coyote window): still can jump
        loco.time_since_grounded += 0.08;
        can_jump = loco.time_since_grounded <= settings.coyote_time_max;
        assert!(can_jump);

        // Falling off ledge past 0.12s: cannot coyote jump
        loco.time_since_grounded += 0.05; // 0.13s total
        can_jump = loco.time_since_grounded <= settings.coyote_time_max;
        assert!(!can_jump);
    }
}
