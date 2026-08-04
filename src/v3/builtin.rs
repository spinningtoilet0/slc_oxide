use std::io::{Read, Write};

use super::action::{Action, ActionType};
use super::atom::{Atom, AtomError, AtomId};
use super::section::{Section, largest_power_of_two};

#[derive(Debug)]
pub struct ActionAtom {
    pub actions: Vec<Action>,
}

impl ActionAtom {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn add_player_action(
        &mut self,
        frame: u64,
        action_type: ActionType,
        holding: bool,
        player2: bool,
    ) -> Result<(), AtomError> {
        if !matches!(
            action_type,
            ActionType::Jump | ActionType::Left | ActionType::Right
        ) {
            return Err(AtomError::InvalidActionType(action_type));
        }
        let previous_frame = self.actions.last().map(|a| a.frame).unwrap_or(0);
        let delta = frame
            .checked_sub(previous_frame)
            .ok_or(AtomError::NonMonotonicFrame {
                previous: previous_frame,
                frame,
            })?;
        if delta > (u64::MAX >> 4) {
            return Err(AtomError::PlayerDeltaTooLarge(delta));
        }
        self.actions.push(Action::player(
            previous_frame,
            delta,
            action_type,
            holding,
            player2,
        ));
        Ok(())
    }

    pub fn add_death_action(
        &mut self,
        frame: u64,
        action_type: ActionType,
        seed: u64,
    ) -> Result<(), AtomError> {
        if !matches!(
            action_type,
            ActionType::Restart | ActionType::RestartFull | ActionType::Death
        ) {
            return Err(AtomError::InvalidActionType(action_type));
        }
        let previous_frame = self.actions.last().map(|a| a.frame).unwrap_or(0);
        let delta = frame
            .checked_sub(previous_frame)
            .ok_or(AtomError::NonMonotonicFrame {
                previous: previous_frame,
                frame,
            })?;
        self.actions
            .push(Action::death(previous_frame, delta, action_type, seed));
        Ok(())
    }

    pub fn add_tps_action(&mut self, frame: u64, tps: f64) -> Result<(), AtomError> {
        if !tps.is_finite() || tps <= 0.0 {
            return Err(AtomError::InvalidTPS(tps));
        }
        let previous_frame = self.actions.last().map(|a| a.frame).unwrap_or(0);
        let delta = frame
            .checked_sub(previous_frame)
            .ok_or(AtomError::NonMonotonicFrame {
                previous: previous_frame,
                frame,
            })?;
        self.actions
            .push(Action::tps_change(previous_frame, delta, tps));
        Ok(())
    }

    pub fn add_bugpoint_action(&mut self, frame: u64) -> Result<(), AtomError> {
        let previous_frame = self.actions.last().map(|a| a.frame).unwrap_or(0);
        let delta = frame
            .checked_sub(previous_frame)
            .ok_or(AtomError::NonMonotonicFrame {
                previous: previous_frame,
                frame,
            })?;
        self.actions.push(Action::bugpoint(previous_frame, delta));
        Ok(())
    }

    pub fn clear(&mut self) {
        self.actions.clear();
    }

    pub fn clip_actions(&mut self, frame: u64) {
        self.actions.retain(|a| a.frame < frame);
    }

    fn swift_compatible(actions: &[Action], i: usize) -> bool {
        if i == 0 {
            return false;
        }
        actions[i].delta() == 0
            && !actions[i].holding
            && actions[i - 1].holding != actions[i].holding
            && actions[i - 1].player2 == actions[i].player2
            && actions[i - 1].action_type == actions[i].action_type
            && actions[i].action_type == ActionType::Jump
    }

    fn can_join(actions: &[Action], count: usize, i: usize) -> bool {
        const MAX_SECTION_ACTIONS: usize = 1 << 16;
        i < actions.len() - 1
            && count < MAX_SECTION_ACTIONS
            && actions[i + 1].is_player()
            && actions[i + 1].minimum_size() == actions[i].minimum_size()
    }

    fn prepare_sections(
        actions: &mut [Action],
        sections: &mut Vec<Section>,
    ) -> Result<(), AtomError> {
        Self::validate_actions(actions)?;

        let mut i = 0;
        while i < actions.len() {
            if !actions[i].is_player() {
                let section = Section::special(&actions[i])?;
                sections.push(section);
                i += 1;
                continue;
            }

            let mut pure_count = 1;
            let mut swifts = 0;
            let mut pure_swifts = 0;
            let start = i;
            let min_size = actions[i].minimum_size();

            while Self::can_join(actions, pure_count, i) {
                i += 1;

                if Self::swift_compatible(actions, i) {
                    actions[i - 1].swift = true;
                    actions[i].swift = true;
                    swifts += 1;
                } else {
                    pure_count += 1;
                }

                if largest_power_of_two(pure_count) == pure_count {
                    pure_swifts = swifts;
                }
            }

            let count = largest_power_of_two(pure_count);
            i = start + count + pure_swifts;

            let mut section = Section::player_from_range(actions, start, i);
            section.delta_size = min_size as u16;

            let real_sections = section.run_length_encode();
            sections.extend(real_sections);
        }

        Ok(())
    }

    fn validate_actions(actions: &[Action]) -> Result<(), AtomError> {
        let mut previous_frame = 0;

        for action in actions {
            let expected_delta =
                action
                    .frame
                    .checked_sub(previous_frame)
                    .ok_or(AtomError::NonMonotonicFrame {
                        previous: previous_frame,
                        frame: action.frame,
                    })?;

            if action.delta() != expected_delta {
                return Err(AtomError::InconsistentFrameDelta {
                    expected: expected_delta,
                    actual: action.delta(),
                });
            }
            if action.is_player() && action.delta() > (u64::MAX >> 4) {
                return Err(AtomError::PlayerDeltaTooLarge(action.delta()));
            }
            if action.action_type == ActionType::TPS
                && (!action.tps.is_finite() || action.tps <= 0.0)
            {
                return Err(AtomError::InvalidTPS(action.tps));
            }

            previous_frame = action.frame;
        }

        Ok(())
    }
}

impl Atom for ActionAtom {
    const ID: AtomId = AtomId::Action;

    fn read<R: Read>(reader: &mut R, size: usize) -> Result<Self, AtomError> {
        if size < 8 {
            return Err(AtomError::ActionAtomTooSmall);
        }

        let mut buf8 = [0u8; 8];
        reader.read_exact(&mut buf8)?;
        let count =
            usize::try_from(u64::from_le_bytes(buf8)).map_err(|_| AtomError::AtomTooLarge)?;

        let mut actions = Vec::with_capacity(count.min(4096));

        while actions.len() < count {
            Section::read(reader, &mut actions, count)?;
        }

        Ok(Self { actions })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), AtomError> {
        writer.write_all(&(self.actions.len() as u64).to_le_bytes())?;

        let mut sections = Vec::new();
        let mut actions_copy = self.actions.clone();

        Self::prepare_sections(&mut actions_copy, &mut sections)?;

        for section in &sections {
            section.write(writer)?;
        }

        Ok(())
    }
}

impl Default for ActionAtom {
    fn default() -> Self {
        Self::new()
    }
}
