use std::io::{Read, Write};

use crate::v3::atom::action::{Action, ActionData};

#[derive(Debug, Clone)]
pub enum Identifier {
    Input = 0,
    Repeat = 1,
    Special = 2,
}

impl TryFrom<u16> for Identifier {
    type Error = &'static str;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Input),
            1 => Ok(Self::Repeat),
            2 => Ok(Self::Special),
            _ => Err("Invalid section identifier"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SpecialType {
    Restart,
    RestartFull,
    Death,
    Tps,
    Bugpoint,
}

impl TryFrom<u8> for SpecialType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Restart,
            1 => Self::RestartFull,
            2 => Self::Death,
            3 => Self::Tps,
            4 => Self::Bugpoint,
            _ => return Err("Invalid special section type"),
        })
    }
}

pub enum Section {
    Input {
        player_actions: Vec<Action>,
        count_exp: u16,
        delta_size: u16,
    },
    Repeat {
        player_actions: Vec<Action>,
        repeats_exp: u16,
        count_exp: u16,
        delta_size: u16,
    },
    Special {
        action: Action,
        delta_size: u16,
    },
}

impl Section {
    pub fn from_run(run: Vec<Action>, delta_size: u16) -> Self {
        Self::Input {
            player_actions: run,
            count_exp: 0,
            delta_size,
        }
    }

    /// This function reads a section directly into an `Action` vector.
    ///
    /// The function does not return `Section` because `Section`s are only relevant when writing.
    pub fn read<R: Read>(reader: &mut R, actions: &mut Vec<Action>) -> Result<(), std::io::Error> {
        let mut buf2 = [0u8; 2];

        reader.read_exact(&mut buf2)?;
        let initial_header = u16::from_le_bytes(buf2);

        let id = Identifier::try_from(initial_header >> 14).unwrap(); // TODO handle error

        match id {
            Identifier::Input => {
                let delta_size = (initial_header >> 12) & 0b11;
                let count_exp = (initial_header >> 8) & 0b1111;

                let byte_size: usize = 1 << delta_size;
                let length: u64 = 1 << count_exp;

                let mut state_buf = [0u8; 8];

                for _ in 0..length {
                    reader.read_exact(&mut state_buf[..byte_size])?;
                    Self::handle_state(u64::from_le_bytes(state_buf), actions);
                }
            }
            Identifier::Repeat => {
                let delta_size = (initial_header >> 12) & 0b11;
                let count_exp = (initial_header >> 8) & 0b1111;
                let repeats_exp = (initial_header >> 3) & 0b11111;

                let byte_size: usize = 1 << delta_size;
                let length: usize = 1 << count_exp;
                let repeats: u64 = 1 << repeats_exp;

                let mut states = vec![0u64; length];

                let mut state_buf = [0u8; 8];

                for state in states.iter_mut() {
                    reader.read_exact(&mut state_buf[..byte_size])?;
                    *state = u64::from_le_bytes(state_buf);
                }

                for _ in 0..repeats {
                    for state in &states {
                        Self::handle_state(*state, actions);
                    }
                }
            }
            Identifier::Special => {
                let delta_size = (initial_header >> 8) & 0b11;
                let special_type =
                    SpecialType::try_from(((initial_header >> 10) as u8) & 0b1111).unwrap(); // TODO fix potentially erronious unwrap

                let mut buf8 = [0u8; 8];
                reader.read_exact(&mut buf8[..1 << delta_size])?;

                let frame_delta = u64::from_le_bytes(buf8);

                let previous_frame = if let Some(action) = actions.last() {
                    action.frame
                } else {
                    0
                };

                let frame = previous_frame + frame_delta;

                match special_type {
                    SpecialType::Tps => {
                        reader.read_exact(&mut buf8)?;

                        actions.push(Action::new(
                            frame,
                            ActionData::TPS(f64::from_le_bytes(buf8)),
                        ));
                    }
                    SpecialType::Restart => {
                        reader.read_exact(&mut buf8)?;

                        actions.push(Action::new(
                            frame,
                            ActionData::Restart {
                                seed: u64::from_le_bytes(buf8),
                            },
                        ));
                    }
                    SpecialType::RestartFull => {
                        reader.read_exact(&mut buf8)?;

                        actions.push(Action::new(
                            frame,
                            ActionData::RestartFull {
                                seed: u64::from_le_bytes(buf8),
                            },
                        ));
                    }
                    SpecialType::Death => {
                        reader.read_exact(&mut buf8)?;

                        actions.push(Action::new(
                            frame,
                            ActionData::Death {
                                seed: u64::from_le_bytes(buf8),
                            },
                        ));
                    }
                    SpecialType::Bugpoint => {
                        actions.push(Action::new(frame, ActionData::Bugpoint));
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_state(state: u64, actions: &mut Vec<Action>) {
        let previous_frame = if let Some(action) = actions.last() {
            action.frame
        } else {
            0
        };

        let frame_delta = state >> 4;
        let frame = previous_frame + frame_delta;

        let button: u8 = ((state >> 2) & 0b11) as u8;
        let holding = (state & 0b1) != 0;
        let player2 = (state & 0b10) != 0;

        // see v3::atom::action::Action::button
        match button {
            // "swift"
            0 => {
                actions.extend_from_slice(&[
                    Action::new(
                        frame,
                        ActionData::Jump {
                            holding: true,
                            player2,
                        },
                    ),
                    Action::new(
                        frame,
                        ActionData::Jump {
                            holding: false,
                            player2,
                        },
                    ),
                ]);
            }
            1 => {
                actions.push(Action::new(frame, ActionData::Jump { holding, player2 }));
            }
            2 => {
                actions.push(Action::new(frame, ActionData::Left { holding, player2 }));
            }
            3 => {
                actions.push(Action::new(frame, ActionData::Right { holding, player2 }));
            }
            _ => unreachable!(),
        }
    }

    fn weak_eq(player_actions: &[Action], idx_1: usize, idx_2: usize) -> bool {
        let first_action = &player_actions[idx_1];
        assert!(first_action.data.is_player());

        let second_action = &player_actions[idx_2];
        assert!(second_action.data.is_player());

        first_action.delta == second_action.delta
            && first_action.data.holding() == second_action.data.holding()
            && first_action.data.player2() == second_action.data.player2()
            && first_action.button() == second_action.button()
    }

    fn distribute_inputs_to_sections(
        sections: &mut Vec<Section>,
        actions: &mut Vec<Action>,
        delta_size: u16,
    ) {
        let mut i = 0;

        while i < actions.len() {
            let count = largest_power_of_two(actions.len() - i);

            sections.push(Section::Input {
                player_actions: actions[i..i + count].to_vec(),
                count_exp: exponent_of_two(count as u32),
                delta_size,
            });

            i += count;
        }

        actions.clear();
    }

    pub fn run_length_encode(self) -> Vec<Section> {
        const MAX_CLUSTER_SIZE: usize = 64;

        let (actions, delta_size) = match self {
            Section::Input {
                player_actions,
                delta_size,
                ..
            } => (player_actions, delta_size),
            _ => panic!("run_length_encode called with non-Input section"),
        };

        let mut new_sections = Vec::new();
        let mut free_inputs = Vec::new();

        let mut idx = 0;

        let actions_len = actions.len();
        while idx < actions_len {
            let mut found_repetitions = false;
            let mut best_cluster = 0;
            let mut best_cluster_repetitions = 0;
            let mut best_cluster_score = 0;

            let mut cluster = 1;

            while cluster <= MAX_CLUSTER_SIZE && cluster <= actions_len {
                if idx + cluster >= actions_len {
                    break;
                }

                let mut offset = 1;

                loop {
                    let start = idx + offset * cluster;
                    let end = idx + offset * (cluster + 1);

                    if end >= actions_len
                        || (start + cluster) > actions_len
                        || (idx + cluster) > actions_len
                    {
                        break;
                    }

                    let mut all_equal = true;
                    for j in 0..cluster {
                        if !Self::weak_eq(&actions, idx + j, start + j) {
                            all_equal = false;
                            break;
                        }
                    }

                    if !all_equal {
                        break;
                    }

                    offset += 1;
                }

                offset -= 1;

                if offset > 1 {
                    offset = largest_power_of_two(offset);
                    let score = cluster as u64 * (offset as u64 - 1);

                    if score > best_cluster_score {
                        found_repetitions = true;
                        best_cluster_score = score;
                        best_cluster = cluster;
                        best_cluster_repetitions = offset;
                    }
                }

                cluster <<= 1;
            }

            if found_repetitions {
                Self::distribute_inputs_to_sections(
                    &mut new_sections,
                    &mut free_inputs,
                    delta_size,
                );

                new_sections.push(Section::Repeat {
                    delta_size,
                    repeats_exp: exponent_of_two(best_cluster_repetitions as u32),
                    count_exp: exponent_of_two(best_cluster as u32),
                    player_actions: actions[idx..idx + best_cluster].to_vec(),
                });

                idx += best_cluster * best_cluster_repetitions;
            } else {
                free_inputs.push(actions[idx].to_owned());
                idx += 1;
            }
        }

        Self::distribute_inputs_to_sections(&mut new_sections, &mut free_inputs, delta_size);

        new_sections
    }

    fn get_real_delta_size(&self) -> u64 {
        let delta_size = match self {
            Section::Input { delta_size, .. }
            | Section::Repeat { delta_size, .. }
            | Section::Special { delta_size, .. } => *delta_size,
        };

        assert!(delta_size <= 3);

        return 1 << (delta_size as u64);
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        match self {
            Section::Input {
                player_actions,
                count_exp,
                delta_size,
            } => {
                writer.write_all(&((count_exp << 8) | (delta_size << 12)).to_le_bytes())?;

                let byte_size = self.get_real_delta_size();

                for action in player_actions {
                    let state = action.prepare_state(byte_size);
                    writer.write_all(&state.to_le_bytes()[..byte_size as usize])?;
                }
            }
            Section::Repeat {
                player_actions,
                repeats_exp,
                count_exp,
                delta_size,
            } => {
                writer.write_all(
                    &((Identifier::Repeat as u16) << 14
                        | (delta_size << 12)
                        | (count_exp << 8)
                        | (repeats_exp << 3))
                        .to_le_bytes(),
                )?;

                let byte_size = self.get_real_delta_size();

                for action in player_actions {
                    let state = action.prepare_state(byte_size);
                    writer.write_all(&state.to_le_bytes()[..byte_size as usize])?;
                }
            }
            Section::Special { action, delta_size } => {
                let special_type = match action.data {
                    ActionData::Restart { .. } => SpecialType::Restart,
                    ActionData::RestartFull { .. } => SpecialType::RestartFull,
                    ActionData::Death { .. } => SpecialType::Death,
                    ActionData::TPS(..) => SpecialType::Tps,
                    ActionData::Bugpoint => SpecialType::Bugpoint,
                    _ => panic!("Special section with non-special action"),
                };

                writer.write_all(
                    &(((Identifier::Special as u16) << 14)
                        | ((special_type as u16) << 10)
                        | (delta_size << 8))
                        .to_le_bytes(),
                )?;

                writer.write_all(
                    &action.delta.to_le_bytes()[..self.get_real_delta_size() as usize],
                )?;

                match action.data {
                    ActionData::Restart { seed }
                    | ActionData::RestartFull { seed }
                    | ActionData::Death { seed } => {
                        writer.write_all(&seed.to_le_bytes())?;
                    }
                    ActionData::TPS(tps) => {
                        writer.write_all(&tps.to_le_bytes())?;
                    }
                    ActionData::Bugpoint => {}
                    _ => unreachable!(),
                }
            }
        }

        Ok(())
    }
}

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
