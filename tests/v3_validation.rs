use slc_oxide::replay::GenericReplay;
use slc_oxide::v3::atom::{AtomError, AtomVariant, OpaqueAtom};
use slc_oxide::v3::builtin::ActionAtom;
use slc_oxide::v3::replay::ReplayError as V3ReplayError;
use slc_oxide::v3::section::SectionError;
use slc_oxide::v3::{Action, ActionType, Metadata, Replay as V3Replay};
use slc_oxide::{Replay, ReplayError};
use std::io::Cursor;

const ATOMS_OFFSET: usize = 8 + 2 + 64;

fn raw_replay(atom_id: u32, size_and_flags: u64, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"SLC3RPLY");
    bytes.extend_from_slice(&64u16.to_le_bytes());
    Metadata::new(240.0, 0, 1).write(&mut bytes).unwrap();
    bytes.extend_from_slice(&atom_id.to_le_bytes());
    bytes.extend_from_slice(&size_and_flags.to_le_bytes());
    bytes.extend_from_slice(body);
    bytes.push(0xCC);
    bytes
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("invalid fixture hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn read_error(bytes: Vec<u8>) -> V3ReplayError {
    match V3Replay::read(&mut Cursor::new(bytes)) {
        Ok(_) => panic!("malformed replay unexpectedly parsed"),
        Err(error) => error,
    }
}

#[test]
fn writes_measured_action_atom_size() {
    let mut actions = ActionAtom::new();
    actions
        .add_player_action(10, ActionType::Jump, true, false)
        .unwrap();

    let mut replay = V3Replay::new(Metadata::new(240.0, 0, 1));
    replay.add_atom(AtomVariant::Action(actions));

    let mut bytes = Vec::new();
    replay.write(&mut bytes).unwrap();

    let size_offset = ATOMS_OFFSET + 4;
    let written_size =
        u64::from_le_bytes(bytes[size_offset..size_offset + 8].try_into().unwrap()) as usize;
    let actual_size = bytes.len() - (ATOMS_OFFSET + 12) - 1;

    assert_eq!(written_size, actual_size);
    assert_eq!(written_size, 11);
}

#[test]
fn matches_cpp_reference_fixture() {
    let mut metadata = Metadata::new(240.0, 123, 42);
    metadata.randomness_algorithm = 7;

    let mut actions = ActionAtom::new();
    actions
        .add_player_action(10, ActionType::Jump, true, false)
        .unwrap();
    actions.add_bugpoint_action(15).unwrap();

    let mut replay = V3Replay::new(metadata);
    replay.add_atom(AtomVariant::Action(actions));

    let mut bytes = Vec::new();
    replay.write(&mut bytes).unwrap();

    let cpp_fixture = hex_bytes(
        "534c433352504c5940000000000000006e407b00000000000000020000002a00000007000000000000000000000000000000000000000000000000000000000000000000000000000000010000000e0000000000000002000000000000000000a5009005cc",
    );
    assert_eq!(bytes, cpp_fixture);
}

#[test]
fn opaque_atoms_preserve_id_flags_and_body() {
    let opaque = OpaqueAtom::new(0xfeed_beef, 0xA5, vec![1, 2, 3, 4]);
    let mut replay = V3Replay::new(Metadata::new(240.0, 0, 1));
    replay.add_atom(AtomVariant::Opaque(opaque.clone()));
    replay.add_atom(AtomVariant::Opaque(OpaqueAtom::new(0, 0, vec![9, 8, 7])));

    let mut bytes = Vec::new();
    replay.write(&mut bytes).unwrap();

    let packed_size = u64::from_le_bytes(
        bytes[ATOMS_OFFSET + 4..ATOMS_OFFSET + 12]
            .try_into()
            .unwrap(),
    );
    assert_eq!(packed_size >> 56, u64::from(opaque.flags));
    assert_eq!(packed_size & 0x00ff_ffff_ffff_ffff, 4);

    let decoded = V3Replay::read(&mut Cursor::new(&bytes)).unwrap();
    let AtomVariant::Opaque(decoded_opaque) = &decoded.atoms.atoms[0] else {
        panic!("expected opaque atom");
    };
    assert_eq!(decoded_opaque, &opaque);
    let AtomVariant::Opaque(decoded_null) = &decoded.atoms.atoms[1] else {
        panic!("expected opaque null atom");
    };
    assert_eq!(decoded_null.id, 0);
    assert_eq!(decoded_null.body, [9, 8, 7]);

    let mut rewritten = Vec::new();
    decoded.write(&mut rewritten).unwrap();
    assert_eq!(rewritten, bytes);
}

#[test]
fn metadata_v2_and_bugpoint_round_trip() {
    let mut metadata = Metadata::new(360.0, 123, 42);
    metadata.randomness_algorithm = 7;

    let mut actions = ActionAtom::new();
    actions.add_bugpoint_action(99).unwrap();

    let mut replay = V3Replay::new(metadata);
    replay.add_atom(AtomVariant::Action(actions));

    let mut bytes = Vec::new();
    replay.write(&mut bytes).unwrap();
    let decoded = V3Replay::read(&mut Cursor::new(bytes)).unwrap();

    assert_eq!(decoded.metadata.version, 2);
    assert_eq!(decoded.metadata.randomness_algorithm, 7);

    let AtomVariant::Action(decoded_actions) = &decoded.atoms.atoms[0] else {
        panic!("expected action atom");
    };
    assert_eq!(decoded_actions.actions.len(), 1);
    assert_eq!(decoded_actions.actions[0].action_type, ActionType::Bugpoint);
    assert_eq!(decoded_actions.actions[0].frame, 99);
}

#[test]
fn rejects_invalid_action_construction() {
    let mut actions = ActionAtom::new();
    assert!(matches!(
        actions.add_player_action(1, ActionType::TPS, false, false),
        Err(AtomError::InvalidActionType(ActionType::TPS))
    ));
    assert!(matches!(
        actions.add_death_action(1, ActionType::Jump, 0),
        Err(AtomError::InvalidActionType(ActionType::Jump))
    ));
    assert!(matches!(
        actions.add_tps_action(1, 0.0),
        Err(AtomError::InvalidTPS(_))
    ));
    assert!(matches!(
        actions.add_tps_action(1, f64::NAN),
        Err(AtomError::InvalidTPS(value)) if value.is_nan()
    ));

    actions
        .add_player_action(10, ActionType::Jump, true, false)
        .unwrap();
    assert!(matches!(
        actions.add_bugpoint_action(9),
        Err(AtomError::NonMonotonicFrame {
            previous: 10,
            frame: 9
        })
    ));

    let mut oversized = ActionAtom::new();
    assert!(matches!(
        oversized.add_player_action(u64::MAX, ActionType::Jump, true, false),
        Err(AtomError::PlayerDeltaTooLarge(_))
    ));

    let mut inconsistent = ActionAtom::new();
    inconsistent
        .add_player_action(10, ActionType::Jump, true, false)
        .unwrap();
    inconsistent.actions[0].frame = 11;
    let mut replay = V3Replay::new(Metadata::new(240.0, 0, 1));
    replay.add_atom(AtomVariant::Action(inconsistent));
    assert!(matches!(
        replay.write(&mut Vec::new()),
        Err(V3ReplayError::AtomError(
            AtomError::InconsistentFrameDelta {
                expected: 11,
                actual: 10
            }
        ))
    ));

    let mut invalid_direct_tps = ActionAtom::new();
    invalid_direct_tps
        .actions
        .push(Action::tps_change(0, 1, -1.0));
    let mut replay = V3Replay::new(Metadata::new(240.0, 0, 1));
    replay.add_atom(AtomVariant::Action(invalid_direct_tps));
    assert!(matches!(
        replay.write(&mut Vec::new()),
        Err(V3ReplayError::AtomError(AtomError::InvalidTPS(_)))
    ));
}

#[test]
fn rejects_invalid_atom_boundaries() {
    assert!(matches!(
        read_error(raw_replay(1, 0, &[])),
        V3ReplayError::AtomError(AtomError::ActionAtomTooSmall)
    ));

    assert!(matches!(
        read_error(raw_replay(1, 8, &[])),
        V3ReplayError::AtomError(AtomError::AtomBodyOutOfBounds)
    ));

    let mut truncated_header = raw_replay(0, 0, &[]);
    truncated_header.truncate(ATOMS_OFFSET);
    truncated_header.push(0x42);
    truncated_header.push(0xCC);
    assert!(matches!(
        read_error(truncated_header),
        V3ReplayError::AtomError(AtomError::TruncatedAtomHeader)
    ));
}

#[test]
fn rejects_sections_that_exceed_the_declared_action_count() {
    let mut swift_body = Vec::new();
    swift_body.extend_from_slice(&1u64.to_le_bytes());
    swift_body.extend_from_slice(&0u16.to_le_bytes());
    swift_body.push(0);

    assert!(matches!(
        read_error(raw_replay(1, swift_body.len() as u64, &swift_body)),
        V3ReplayError::AtomError(AtomError::SectionError(SectionError::ActionCountExceeded))
    ));

    let mut repeat_body = Vec::new();
    repeat_body.extend_from_slice(&1u64.to_le_bytes());
    let repeat_header = (1u16 << 14) | (31u16 << 3);
    repeat_body.extend_from_slice(&repeat_header.to_le_bytes());
    repeat_body.push(1u8 << 2);

    assert!(matches!(
        read_error(raw_replay(1, repeat_body.len() as u64, &repeat_body)),
        V3ReplayError::AtomError(AtomError::SectionError(SectionError::ActionCountExceeded))
    ));

    let huge_count = u64::MAX.to_le_bytes();
    assert!(matches!(
        read_error(raw_replay(1, huge_count.len() as u64, &huge_count)),
        V3ReplayError::AtomError(AtomError::SectionError(SectionError::IOError(_)))
    ));
}

#[test]
fn generic_v3_conversion_honors_meta_contract_and_reports_errors() {
    let direct = V3Replay::new(Metadata::new(240.0, 0, 1));
    let mut bytes = Vec::new();

    direct
        .to_generic_replay()
        .write_v2(&mut bytes, &[67, 69, 41])
        .unwrap();

    let decoded = Replay::read(&mut Cursor::new(bytes)).unwrap();

    let decoded = match decoded {
        Replay::V2(x) => x,
        Replay::V3(_) => panic!("incorrect replay format"),
    };

    assert_eq!(decoded.meta, &[67, 69, 41]);

    let mut invalid_tps = GenericReplay {
        tps: 240.0,
        actions: Vec::new(),
    };

    invalid_tps.add_action(slc_oxide::Action {
        frame: 1,
        data: slc_oxide::ActionData::TPS(-1.0),
    });

    assert!(matches!(
        invalid_tps.write_v3(&mut Vec::new(), Metadata::new(240.0, 0, 0)),
        Err(ReplayError::V3Error(V3ReplayError::AtomError(
            AtomError::InvalidTPS(_)
        )))
    ));
}
