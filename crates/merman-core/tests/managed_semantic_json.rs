use merman_core::ManagedSemanticJson;
use serde_json::{Value, json};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn chain(depth: usize) -> ManagedSemanticJson {
    let mut value = Value::String("leaf".to_owned());
    for _ in 0..depth {
        value = Value::Array(vec![value]);
    }
    value.into()
}

#[test]
fn compact_writer_preserves_scalar_representation_and_object_order() {
    let value = json!({
        "first": ["quote\"\\\n\t\u{0001}", "雪🦀", null, true],
        "numbers": [i64::MIN, u64::MAX, -0.0, 1.25, 1e30],
        "last": {}
    });
    let expected = serde_json::to_vec(&value).unwrap();
    let managed = ManagedSemanticJson::from(value);
    let mut actual = Vec::new();
    managed.write_json(&mut actual).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(serde_json::to_vec(&managed).unwrap(), expected);
}

#[test]
fn generic_serde_checks_actual_container_depth_before_serializing() {
    for depth in [127, 128] {
        let value = chain(depth);
        assert!(
            serde_json::to_vec(&value).is_ok(),
            "depth {depth} is supported"
        );
    }
    let error = serde_json::to_vec(&chain(129)).unwrap_err();
    assert!(error.to_string().contains("128"));
    // An empty container contributes to depth even though it has no child value.
    let mut empty = Value::Array(Vec::new());
    for _ in 0..128 {
        empty = Value::Array(vec![empty]);
    }
    assert!(serde_json::to_vec(&ManagedSemanticJson::from(empty)).is_err());
}

#[test]
fn object_equality_ignores_insertion_order_like_serde_json() {
    let left = ManagedSemanticJson::from(json!({"a": [1, 2], "b": "value"}));
    let right = ManagedSemanticJson::from(json!({"b": "value", "a": [1, 2]}));
    assert_eq!(left, right);
    assert_ne!(
        left,
        ManagedSemanticJson::from(json!({"a": [2, 1], "b": "value"}))
    );
}

#[test]
fn explicitly_unmanaged_extraction_preserves_the_payload() {
    let value = ManagedSemanticJson::from(json!({"value": [1, 2]}));
    assert_eq!(value.into_unmanaged_value(), json!({"value": [1, 2]}));
}

struct FailingWriter {
    remaining: usize,
}

impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("intentional partial-output failure"));
        }
        let count = bytes.len().min(self.remaining);
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn managed_json_lifecycle_child() {
    if std::env::var_os("MERMAN_MANAGED_JSON_LIFECYCLE_CHILD").is_none() {
        return;
    }
    std::thread::Builder::new()
        .name("managed-json-lifecycle".to_owned())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let value = chain(5_000);
            let cloned = value.clone();
            assert_eq!(value, cloned);
            let mut encoded = Vec::new();
            value.write_json(&mut encoded).unwrap();
            assert_eq!(encoded.len(), 10_006);
            assert!(encoded.starts_with(b"[[[["));
            assert!(encoded.ends_with(b"]]]]"));
            let diagnostic = format!("{value:?}");
            assert!(diagnostic.contains("leaf"));
            assert!(serde_json::to_vec(&value).is_err());
            assert!(
                value
                    .write_json(FailingWriter { remaining: 3_000 })
                    .is_err()
            );
            #[cfg(feature = "test-support")]
            {
                let control = merman_core::OperationControl::new();
                control.cancel_after_checkpoints(120);
                assert!(value.clone_controlled(&control).is_err());
                let control = merman_core::OperationControl::new();
                control.cancel_after_checkpoints(20);
                assert!(value.write_json_controlled(io::sink(), &control).is_err());
            }
            drop(cloned);
            drop(value);
            println!("managed-deep-clone-equality-export-debug-drop=done");
            io::stdout().flush().unwrap();
            let shallow = ManagedSemanticJson::from(json!({"still": "usable"}));
            shallow.write_json(io::sink()).unwrap();
            drop(shallow);
            println!("managed-after-small-operation=done");
            io::stdout().flush().unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn deep_managed_json_lifecycle_finishes_on_a_2_mib_worker() {
    let directory =
        std::env::temp_dir().join(format!("merman-managed-json-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let stdout_path = directory.join("stdout.txt");
    let stderr_path = directory.join("stderr.txt");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "managed_json_lifecycle_child", "--nocapture"])
        .env("MERMAN_MANAGED_JSON_LIFECYCLE_CHILD", "1")
        .stdout(Stdio::from(std::fs::File::create(&stdout_path).unwrap()))
        .stderr(Stdio::from(std::fs::File::create(&stderr_path).unwrap()))
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < Duration::from_secs(60) => {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "managed JSON child failed or timed out: {result:?}; {}",
                    directory.display()
                );
            }
        }
    };
    let stdout = std::fs::read_to_string(stdout_path).unwrap();
    let stderr = std::fs::read_to_string(stderr_path).unwrap();
    assert!(
        status.success(),
        "{status}; stdout={stdout}; stderr={stderr}"
    );
    assert!(stdout.contains("managed-deep-clone-equality-export-debug-drop=done"));
    assert!(stdout.contains("managed-after-small-operation=done"));
}
