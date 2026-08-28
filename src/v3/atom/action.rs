mod section;

use std::io::{Read, Write};

use crate::v3::atom::{
    AtomError,
    action::section::{Section, largest_power_of_two},
};

#[derive(Debug, Clone, PartialEq)]
pub enum ActionData {
    Reserved,
    Jump { holding: bool, player2: bool },
    Left { holding: bool, player2: bool },
    Right { holding: bool, player2: bool },
    Restart { seed: u64 },
    RestartFull { seed: u64 },
    Death { seed: u64 },
    TPS(f64),
    Bugpoint,
}

impl ActionData {
    pub fn is_player(&self) -> bool {
        matches!(
            self,
            ActionData::Jump { .. } | ActionData::Left { .. } | ActionData::Right { .. }
        )
    }

    pub fn holding(&self) -> bool {
        match self {
            ActionData::Jump {
                holding,
                player2: _,
            }
            | ActionData::Left {
                holding,
                player2: _,
            }
            | ActionData::Right {
                holding,
                player2: _,
            } => *holding,
            _ => false,
        }
    }

    pub fn player2(&self) -> bool {
        match self {
            ActionData::Jump {
                holding: _,
                player2,
            }
            | ActionData::Left {
                holding: _,
                player2,
            }
            | ActionData::Right {
                holding: _,
                player2,
            } => *player2,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub frame: u64,
    pub data: ActionData,
    /// This field is used internally when writing SLC3.
    delta: u64,
    /// This field is used internally when writing SLC3.
    swift: bool,
}

impl Action {
    pub fn new(frame: u64, data: ActionData) -> Self {
        Action {
            frame,
            data,
            swift: false,
            delta: 0,
        }
    }

    pub(crate) fn minimum_size(&self) -> u8 {
        let offset = if self.data.is_player() || self.data == ActionData::Reserved {
            4
        } else {
            8
        };

        let one_byte_threshold: u64 = 1 << offset;
        let two_byte_threshold: u64 = 1 << (offset + 8);
        let four_byte_threshold: u64 = 1 << (offset + 24);

        if self.delta < one_byte_threshold {
            0
        } else if self.delta < two_byte_threshold {
            1
        } else if self.delta < four_byte_threshold {
            2
        } else {
            3
        }
    }

    pub(crate) fn button(&self) -> u8 {
        if self.swift {
            return 0;
        }

        return match self.data {
            ActionData::Jump { .. } => 1,
            ActionData::Left { .. } => 2,
            ActionData::Right { .. } => 3,
            _ => panic!("button called with non-player and non-swift input"),
        };
    }

    pub(crate) fn prepare_state(&self, byte_size: u64) -> u64 {
        let byte_mask = if byte_size == 8 {
            (-1i64).cast_unsigned()
        } else {
            (1 << (byte_size * 8)) - 1
        };

        byte_mask
            & ((self.delta << 4)
                | ((self.button() as u64) << 2)
                | ((self.data.player2() as u64) << 1)
                | (self.data.holding() as u64))
    }
}

#[derive(Debug, Clone)]
pub struct ActionAtom {
    pub flags: u8,
    pub actions: Vec<Action>,
}

impl ActionAtom {
    pub const ID: u32 = 1;

    pub fn read<R: Read>(reader: &mut R, flags: u8, size: u64) -> Result<Self, std::io::Error> {
        let size_known = size != 0;

        if size_known && size < std::mem::size_of::<u64>() as u64 {
            todo!();
        }

        let mut action_count_buf = [0u8; 8];
        reader.read_exact(&mut action_count_buf)?;

        let actions_count = u64::from_le_bytes(action_count_buf);

        let mut actions = Vec::with_capacity(actions_count as usize);

        while (actions.len() as u64) < actions_count {
            Section::read(reader, &mut actions)?;
        }

        Ok(ActionAtom { flags, actions })
    }

    fn calculate_deltas(&mut self) {
        let mut previous_frame = 0;
        for action in &mut self.actions {
            action.delta = action.frame - previous_frame;
            previous_frame = action.frame;
        }
    }

    pub fn write<W: Write>(&mut self, writer: &mut W) -> Result<(), AtomError> {
        self.calculate_deltas();

        writer.write_all(&Self::ID.to_le_bytes())?;

        let mut body = Vec::new();

        body.write_all(&(self.actions.len() as u64).to_le_bytes())?;
        for section in self.prepare_sections() {
            section.write(&mut body)?;
        }

        let flags_and_size = ((self.flags as u64) << 56) | (body.len() as u64);
        writer.write_all(&flags_and_size.to_le_bytes())?;
        writer.write_all(&body)?;

        Ok(())
    }

    fn swift_compatible(actions: &[Action], i: usize) -> bool {
        let previous = &actions[i - 1];
        let current = &actions[i];

        current.delta == 0
            && !current.data.holding()
            && previous.data.holding() != current.data.holding()
            && previous.data.player2() == current.data.player2()
            && matches!(previous.data, ActionData::Jump { .. })
            && matches!(current.data, ActionData::Jump { .. })
    }

    fn prepare_sections(&self) -> Vec<Section> {
        const MAX_SECTION_ACTIONS: usize = 1 << 16;

        let mut sections = Vec::new();
        let mut i = 0;

        while i < self.actions.len() {
            let current_action = &self.actions[i];

            if !current_action.data.is_player() {
                sections.push(Section::Special {
                    action: current_action.to_owned(),
                    delta_size: current_action.minimum_size() as u16,
                });
                i += 1;
                continue;
            }

            let mut pure_count = 1;
            let mut swifts = 0;
            let mut pure_swifts = 0;

            let start = i;

            let min_size = current_action.minimum_size();

            let mut run = vec![current_action.to_owned()];

            while i < (self.actions.len() - 1) && pure_count < MAX_SECTION_ACTIONS {
                let next = &self.actions[i + 1];

                if !next.data.is_player() || next.minimum_size() != min_size {
                    break;
                }

                i += 1;
                run.push(self.actions[i].to_owned());

                if Self::swift_compatible(&self.actions, i) {
                    let last = run.len() - 1;
                    run[last - 1].swift = true;
                    run[last].swift = true;
                    swifts += 1;
                } else {
                    pure_count += 1;
                }

                if largest_power_of_two(pure_count) == pure_count {
                    pure_swifts = swifts;
                }
            }

            let keep = largest_power_of_two(pure_count) + pure_swifts;
            run.truncate(keep);
            run.retain(|a| a.data.holding() || !a.swift);

            i = start + keep;

            sections.extend(Section::from_run(run, min_size as u16).run_length_encode());
        }

        sections
    }
}
