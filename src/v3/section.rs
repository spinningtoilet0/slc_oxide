use std::io::{Read, Write};
use thiserror::Error;

use crate::v3::{Action, ActionType};

pub(crate) fn exponent_of_two(n: u32) -> u16 {
    if n == 0 {
        return 0;
    }
    let exp = 31 - n.leading_zeros();
    exp.min(15) as u16
}

pub(crate) fn largest_power_of_two(n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    1 << exponent_of_two(n as u32)
}

#[derive(Debug, Error)]
pub enum SectionError {
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Invalid section identifier")]
    InvalidIdentifier,
    #[error("Invalid button type")]
    InvalidButton,
    #[error("Section expands beyond the declared action count")]
    ActionCountExceeded,
    #[error("Action frame overflow")]
    FrameOverflow,
    #[error("Invalid TPS value: {0}")]
    InvalidTPS(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionIdentifier {
    Input,
    Repeat,
    Special,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialType {
    Restart,
    RestartFull,
    Death,
    TPS,
    Bugpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Swift,
    Jump,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct PlayerInput {
    pub frame: u64,
    pub delta: u64,
    pub button: Button,
    pub holding: bool,
    pub player2: bool,
}

impl PlayerInput {
    pub fn from_action(action: &Action) -> Self {
        let button = if action.swift() {
            Button::Swift
        } else {
            match action.action_type {
                ActionType::Jump => Button::Jump,
                ActionType::Left => Button::Left,
                ActionType::Right => Button::Right,
                _ => Button::Jump,
            }
        };

        Self {
            frame: action.frame,
            delta: action.delta(),
            button,
            holding: action.holding,
            player2: action.player2,
        }
    }

    pub fn from_state(prev_frame: u64, state: u64) -> Result<Self, SectionError> {
        let delta = state >> 4;
        let frame = prev_frame
            .checked_add(delta)
            .ok_or(SectionError::FrameOverflow)?;
        let button_val = (state >> 2) & 0b11;
        let button = match button_val {
            0 => Button::Swift,
            1 => Button::Jump,
            2 => Button::Left,
            3 => Button::Right,
            _ => Button::Swift,
        };
        let holding = (state & 0b1) == 0b1;
        let player2 = (state & 0b10) == 0b10;

        Ok(Self {
            frame,
            delta,
            button,
            holding,
            player2,
        })
    }

    pub fn prepare_state(&self, byte_size: u8) -> u64 {
        let byte_mask = if byte_size == 8 {
            u64::MAX
        } else {
            (1u64 << (byte_size as u64 * 8)) - 1
        };

        byte_mask
            & ((self.delta << 4)
                | ((self.button as u64) << 2)
                | ((self.player2 as u64) << 1)
                | self.holding as u64)
    }

    pub fn weak_eq(&self, other: &PlayerInput) -> bool {
        self.delta == other.delta
            && self.holding == other.holding
            && self.player2 == other.player2
            && self.button == other.button
    }
}

pub struct Section {
    pub id: SectionIdentifier,
    pub delta_size: u16,
    pub player_inputs: Vec<PlayerInput>,
    pub marked_for_removal: bool,
    count_exp: u16,
    repeats_exp: u16,
    special_type: SpecialType,
    seed: u64,
    tps: f64,
    special: Option<Action>,
}

impl Section {
    pub fn player_from_range(actions: &[Action], start: usize, end: usize) -> Self {
        let mut player_inputs = Vec::new();
        let mut count = 0u32;

        for action in &actions[start..end] {
            if action.holding || !action.swift() {
                player_inputs.push(PlayerInput::from_action(action));
                count += 1;
            }
        }

        let count_exp = exponent_of_two(count);

        Self {
            id: SectionIdentifier::Input,
            delta_size: 0,
            player_inputs,
            marked_for_removal: false,
            count_exp,
            repeats_exp: 0,
            special_type: SpecialType::Restart,
            seed: 0,
            tps: 240.0,
            special: None,
        }
    }

    pub fn special(action: &Action) -> Result<Self, SectionError> {
        if action.action_type == ActionType::TPS && (!action.tps.is_finite() || action.tps <= 0.0) {
            return Err(SectionError::InvalidTPS(action.tps));
        }

        let special_type = match action.action_type {
            ActionType::TPS => SpecialType::TPS,
            ActionType::Death => SpecialType::Death,
            ActionType::Restart => SpecialType::Restart,
            ActionType::RestartFull => SpecialType::RestartFull,
            ActionType::Bugpoint => SpecialType::Bugpoint,
            _ => return Err(SectionError::InvalidIdentifier),
        };

        Ok(Self {
            id: SectionIdentifier::Special,
            delta_size: action.minimum_size() as u16,
            player_inputs: Vec::new(),
            marked_for_removal: false,
            count_exp: 0,
            repeats_exp: 0,
            special_type,
            seed: action.seed,
            tps: action.tps,
            special: Some(action.clone()),
        })
    }

    pub fn real_delta_size(&self) -> u64 {
        1u64 << self.delta_size as u64
    }

    pub fn input_count(&self) -> u64 {
        1u64 << self.count_exp as u64
    }

    pub fn repeat_count(&self) -> u64 {
        1u64 << self.repeats_exp as u64
    }

    pub fn run_length_encode(&self) -> Vec<Section> {
        let mut new_sections = Vec::new();
        let mut free_inputs = Vec::new();

        const MAX_CLUSTER_SIZE: usize = 64;

        let n = self.player_inputs.len();
        let mut idx = 0;

        while idx < n {
            let mut found_any_repetitions = false;
            let mut best_cluster = 0;
            let mut best_cluster_repetitions = 0;
            let mut best_cluster_score = 0i64;

            let mut cluster = 1;
            while cluster <= MAX_CLUSTER_SIZE && cluster <= n {
                if idx + cluster >= n {
                    break;
                }

                let mut offset = 1;
                loop {
                    let start = idx + offset * cluster;
                    let end = idx + (offset + 1) * cluster;

                    if end > n {
                        break;
                    }

                    let all_equal = (0..cluster).all(|j| {
                        self.player_inputs[idx + j].weak_eq(&self.player_inputs[start + j])
                    });

                    if !all_equal {
                        break;
                    }

                    offset += 1;
                }

                offset = offset.saturating_sub(1);
                if offset <= 1 {
                    cluster <<= 1;
                    continue;
                }

                offset = largest_power_of_two(offset);

                let score = (cluster as i64) * ((offset as i64) - 1);
                if score > best_cluster_score {
                    found_any_repetitions = true;
                    best_cluster_score = score;
                    best_cluster = cluster;
                    best_cluster_repetitions = offset;
                }

                cluster <<= 1;
            }

            if found_any_repetitions {
                distribute_inputs_to_sections(&mut new_sections, &mut free_inputs, self.delta_size);

                let repeat_section = Section {
                    id: SectionIdentifier::Repeat,
                    delta_size: self.delta_size,
                    player_inputs: self.player_inputs[idx..idx + best_cluster].to_vec(),
                    marked_for_removal: false,
                    count_exp: exponent_of_two(best_cluster as u32),
                    repeats_exp: exponent_of_two(best_cluster_repetitions as u32),
                    special_type: SpecialType::Restart,
                    seed: 0,
                    tps: 240.0,
                    special: None,
                };

                new_sections.push(repeat_section);
                idx += best_cluster * best_cluster_repetitions;
            } else {
                free_inputs.push(self.player_inputs[idx].clone());
                idx += 1;
            }
        }

        distribute_inputs_to_sections(&mut new_sections, &mut free_inputs, self.delta_size);

        new_sections
    }

    pub fn read<R: Read>(
        reader: &mut R,
        actions: &mut Vec<Action>,
        max_actions: usize,
    ) -> Result<(), SectionError> {
        let mut buf2 = [0u8; 2];
        reader.read_exact(&mut buf2)?;
        let initial_header = u16::from_le_bytes(buf2);

        let id = (initial_header >> 14) as u8;
        let id = match id {
            0 => SectionIdentifier::Input,
            1 => SectionIdentifier::Repeat,
            2 => SectionIdentifier::Special,
            _ => return Err(SectionError::InvalidIdentifier),
        };

        match id {
            SectionIdentifier::Input => {
                let delta_size = (initial_header >> 12) & 0b11;
                let count_exp = (initial_header >> 8) & 0b1111;

                let byte_size = 1u64 << delta_size;
                let length = 1u64 << count_exp;

                let mut previous_frame = actions.last().map(|a| a.frame).unwrap_or(0);

                for _ in 0..length {
                    let state = read_n_bytes(reader, byte_size as usize)?;
                    let p = PlayerInput::from_state(previous_frame, state)?;
                    let added_actions = if p.button == Button::Swift { 2 } else { 1 };
                    ensure_action_capacity(actions.len(), added_actions, max_actions)?;

                    if p.button == Button::Swift {
                        actions.push(Action::player(
                            previous_frame,
                            p.delta,
                            ActionType::Jump,
                            true,
                            p.player2,
                        ));
                        actions.last_mut().unwrap().swift = true;
                        actions.push(Action::player(
                            p.frame,
                            0,
                            ActionType::Jump,
                            false,
                            p.player2,
                        ));
                        actions.last_mut().unwrap().swift = true;
                    } else {
                        let action_type = match p.button {
                            Button::Jump => ActionType::Jump,
                            Button::Left => ActionType::Left,
                            Button::Right => ActionType::Right,
                            _ => ActionType::Jump,
                        };
                        actions.push(Action::player(
                            previous_frame,
                            p.delta,
                            action_type,
                            p.holding,
                            p.player2,
                        ));
                    }

                    previous_frame = actions.last().unwrap().frame;
                }
            }
            SectionIdentifier::Repeat => {
                let delta_size = (initial_header >> 12) & 0b11;
                let count_exp = (initial_header >> 8) & 0b1111;
                let repeats_exp = (initial_header >> 3) & 0b11111;

                let byte_size = 1u64 << delta_size;
                let length = 1u64 << count_exp;
                let repeats = 1u64 << repeats_exp;

                let mut inputs = Vec::new();
                let mut prev_input_frame = 0u64;

                for _ in 0..length {
                    let state = read_n_bytes(reader, byte_size as usize)?;
                    let p = PlayerInput::from_state(prev_input_frame, state)?;
                    prev_input_frame = p.frame;
                    inputs.push(p);
                }

                let actions_per_repeat = inputs.iter().try_fold(0usize, |count, input| {
                    count.checked_add(if input.button == Button::Swift { 2 } else { 1 })
                });
                let repeats =
                    usize::try_from(repeats).map_err(|_| SectionError::ActionCountExceeded)?;
                let added_actions = actions_per_repeat
                    .and_then(|count| count.checked_mul(repeats))
                    .ok_or(SectionError::ActionCountExceeded)?;
                ensure_action_capacity(actions.len(), added_actions, max_actions)?;

                for _ in 0..repeats {
                    let mut previous_frame = actions.last().map(|a| a.frame).unwrap_or(0);
                    for p in &inputs {
                        previous_frame
                            .checked_add(p.delta)
                            .ok_or(SectionError::FrameOverflow)?;
                        if p.button == Button::Swift {
                            actions.push(Action::player(
                                previous_frame,
                                p.delta,
                                ActionType::Jump,
                                true,
                                p.player2,
                            ));
                            actions.last_mut().unwrap().swift = true;
                            actions.push(Action::player(
                                previous_frame + p.delta,
                                0,
                                ActionType::Jump,
                                false,
                                p.player2,
                            ));
                            actions.last_mut().unwrap().swift = true;
                        } else {
                            let action_type = match p.button {
                                Button::Jump => ActionType::Jump,
                                Button::Left => ActionType::Left,
                                Button::Right => ActionType::Right,
                                _ => ActionType::Jump,
                            };
                            actions.push(Action::player(
                                previous_frame,
                                p.delta,
                                action_type,
                                p.holding,
                                p.player2,
                            ));
                        }
                        previous_frame = actions.last().unwrap().frame;
                    }
                }
            }
            SectionIdentifier::Special => {
                let delta_size = (initial_header >> 8) & 0b11;
                let special_type = (initial_header >> 10) & 0b1111;

                let byte_size = 1u64 << delta_size;
                let frame_delta = read_n_bytes(reader, byte_size as usize)?;

                let current_frame = actions.last().map(|a| a.frame).unwrap_or(0);
                current_frame
                    .checked_add(frame_delta)
                    .ok_or(SectionError::FrameOverflow)?;
                ensure_action_capacity(actions.len(), 1, max_actions)?;

                let special_type = match special_type {
                    0 => SpecialType::Restart,
                    1 => SpecialType::RestartFull,
                    2 => SpecialType::Death,
                    3 => SpecialType::TPS,
                    4 => SpecialType::Bugpoint,
                    _ => return Err(SectionError::InvalidIdentifier),
                };

                match special_type {
                    SpecialType::TPS => {
                        let mut buf8 = [0u8; 8];
                        reader.read_exact(&mut buf8)?;
                        let tps = f64::from_le_bytes(buf8);
                        if !tps.is_finite() || tps <= 0.0 {
                            return Err(SectionError::InvalidTPS(tps));
                        }
                        actions.push(Action::tps_change(current_frame, frame_delta, tps));
                    }
                    SpecialType::Restart | SpecialType::RestartFull | SpecialType::Death => {
                        let mut buf8 = [0u8; 8];
                        reader.read_exact(&mut buf8)?;
                        let seed = u64::from_le_bytes(buf8);
                        let action_type = match special_type {
                            SpecialType::Restart => ActionType::Restart,
                            SpecialType::RestartFull => ActionType::RestartFull,
                            SpecialType::Death => ActionType::Death,
                            _ => ActionType::Restart,
                        };
                        actions.push(Action::death(current_frame, frame_delta, action_type, seed));
                    }
                    SpecialType::Bugpoint => {
                        actions.push(Action::bugpoint(current_frame, frame_delta));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), SectionError> {
        if self.marked_for_removal {
            return Ok(());
        }

        match self.id {
            SectionIdentifier::Input => {
                let header = (self.count_exp << 8) | (self.delta_size << 12);
                writer.write_all(&header.to_le_bytes())?;

                let byte_size = self.real_delta_size();
                for input in &self.player_inputs {
                    let state = input.prepare_state(byte_size as u8);
                    write_n_bytes(writer, state, byte_size as usize)?;
                }
            }
            SectionIdentifier::Repeat => {
                let header = (SectionIdentifier::Repeat as u16) << 14
                    | self.delta_size << 12
                    | self.count_exp << 8
                    | self.repeats_exp << 3;
                writer.write_all(&header.to_le_bytes())?;

                let byte_size = self.real_delta_size();
                for input in &self.player_inputs {
                    let state = input.prepare_state(byte_size as u8);
                    write_n_bytes(writer, state, byte_size as usize)?;
                }
            }
            SectionIdentifier::Special => {
                let header = (SectionIdentifier::Special as u16) << 14
                    | (self.special_type as u16) << 10
                    | (self.delta_size << 8);
                writer.write_all(&header.to_le_bytes())?;

                let delta = self.special.as_ref().unwrap().delta();
                write_n_bytes(writer, delta, self.real_delta_size() as usize)?;

                match self.special_type {
                    SpecialType::Restart | SpecialType::RestartFull | SpecialType::Death => {
                        writer.write_all(&self.seed.to_le_bytes())?;
                    }
                    SpecialType::TPS => {
                        writer.write_all(&self.tps.to_le_bytes())?;
                    }
                    SpecialType::Bugpoint => {}
                }
            }
        }

        Ok(())
    }
}

fn ensure_action_capacity(
    current: usize,
    additional: usize,
    maximum: usize,
) -> Result<(), SectionError> {
    if current
        .checked_add(additional)
        .is_none_or(|total| total > maximum)
    {
        return Err(SectionError::ActionCountExceeded);
    }

    Ok(())
}

fn distribute_inputs_to_sections(
    sections: &mut Vec<Section>,
    inputs: &mut Vec<PlayerInput>,
    delta_size: u16,
) {
    let mut i = 0;
    while i < inputs.len() {
        let count = largest_power_of_two(inputs.len() - i);
        let section = Section {
            id: SectionIdentifier::Input,
            delta_size,
            player_inputs: inputs[i..i + count].to_vec(),
            marked_for_removal: false,
            count_exp: exponent_of_two(count as u32),
            repeats_exp: 0,
            special_type: SpecialType::Restart,
            seed: 0,
            tps: 240.0,
            special: None,
        };
        i += count;
        sections.push(section);
    }
    inputs.clear();
}

fn read_n_bytes<R: Read>(reader: &mut R, n: usize) -> Result<u64, SectionError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf[..n])?;
    Ok(u64::from_le_bytes(buf))
}

fn write_n_bytes<W: Write>(writer: &mut W, value: u64, n: usize) -> Result<(), SectionError> {
    let bytes = value.to_le_bytes();
    writer.write_all(&bytes[..n])?;
    Ok(())
}
