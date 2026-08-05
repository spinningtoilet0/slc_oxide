use slc_oxide::Replay;
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::PathBuf;

/*
#[test]
fn test_macro_files_roundtrip() {
    let macro_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("macros");

    let entries = fs::read_dir(&macro_dir).expect("Failed to read macros directory");

    for entry in entries {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("slc") {
            let file_data = fs::read(&path).expect("Failed to read file");

            let mut reader = Cursor::new(&file_data);
            let replay = Replay::read(&mut reader)
                .expect("Failed to parse replay")
                .to_generic_replay();

            let mut v2_buffer = Vec::new();

            replay
                .write_v2(&mut v2_buffer, &[])
                .expect("Failed to write v2 replay");

            let mut reader2 = Cursor::new(&v2_buffer);
            let replay2 = Replay::read(&mut reader2).expect("Failed to re-parse replay");

            let replay2 = match replay2 {
                Replay::V2(x) => x,
                Replay::V3(_) => panic!("wrong format!"),
            };

            assert_eq!(replay.tps, replay2.tps);
            assert_eq!(replay.actions.len(), replay2.inputs.len());
            for (i, (input1, input2)) in replay.actions.iter().zip(&replay2.inputs).enumerate() {
                assert_eq!(
                    input1.frame, input2.frame,
                    "V2 roundtrip: frame mismatch at action {}",
                    i
                );
                assert_eq!(
                    input1.delta, input2.delta,
                    "V2 roundtrip: delta mismatch at action {}",
                    i
                );
                assert_eq!(
                    input1.data, input2.data,
                    "V2 roundtrip: data mismatch at action {}",
                    i
                );
            }

            let mut v3_buffer = Vec::new();

            replay
                .write_v3(&mut v3_buffer, 0, 0)
                .expect("Failed to write v3 replay");

            let mut reader3 = BufReader::new(Cursor::new(&v3_buffer));
            let replay3 = Replay::read(&mut reader3)
                .expect("Failed to parse v3 replay")
                .to_generic_replay();

            assert_eq!(replay.tps, replay3.tps);
            assert_eq!(replay.actions.len(), replay3.actions.len());
            for (i, (input1, input3)) in replay.actions.iter().zip(&replay3.actions).enumerate() {
                assert_eq!(
                    input1.frame, input3.frame,
                    "V3 roundtrip: frame mismatch at action {}",
                    i
                );
                assert_eq!(
                    input1.delta, input3.delta,
                    "V3 roundtrip: delta mismatch at action {}",
                    i
                );
                assert_eq!(
                    input1.data, input3.data,
                    "V3 roundtrip: data mismatch at action {}",
                    i
                );
            }
        }
    }
}
*/
