use std::io::Cursor;

use slc_oxide::{Replay, v3::atom::Atom};

#[test]
fn v3_round_trip() {
    let input = std::fs::read("tests/macros/v3/markforremoval.slc").unwrap();
    let replay = Replay::read(&mut Cursor::new(input.clone())).unwrap();

    let mut x = match replay {
        Replay::V2(_) => panic!("wrong replay type"),
        Replay::V3(x) => x,
    };

    assert_eq!(x.metadata.tps, 12900.0);
    assert_eq!(x.metadata.seed, 18113763409179897008);
    assert_eq!(x.metadata.version, 2);
    assert_eq!(x.metadata.build, 81);
    assert_eq!(x.metadata.randomness_algorithm, 0);

    assert_eq!(x.registry.atoms.len(), 1);

    match &x.registry.atoms[0] {
        Atom::Opaque(_) => panic!("parsed non-action atom"),
        Atom::Action(actions) => {
            assert_eq!(actions.flags, 0);
            assert_eq!(actions.actions.len(), 2121706);
        }
    };

    let mut buffer = Vec::new();
    x.write(&mut buffer).unwrap();

    assert!(buffer == input);
}
