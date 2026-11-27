//! Shared subprocess and deadline assertions for semantic tree tests.

pub(crate) fn run_lifecycle_child(test: &'static str, worker: impl FnOnce() + Send + 'static) {
    const CASE_ENV: &str = "MERMAN_CORE_TREE_UNIT_LIFECYCLE_CASE";
    if std::env::var(CASE_ENV).as_deref() == Ok(test) {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                println!("phase=tree-worker status=begin case={test} stack_bytes=2097152");
                worker();
                println!("phase=tree-worker status=done case={test}");
            })
            .expect("spawn 2 MiB tree worker")
            .join()
            .expect("tree lifecycle worker completes");
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(CASE_ENV, test)
        .spawn()
        .expect("spawn tree lifecycle child");
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("tree child observation failed: {error}");
            }
        }
        if started.elapsed() > std::time::Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("tree lifecycle child timed out: {test}");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(
        status.success(),
        "tree lifecycle child failed: {test}, status={status}"
    );
}

pub(crate) fn assert_in_progress_export_deadline(json: &crate::ManagedSemanticJson) {
    struct Writer {
        control: crate::OperationControl,
        written: usize,
    }
    impl std::io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.written += bytes.len();
            if self.written >= 1_024 {
                self.control.set_deadline(std::time::Duration::ZERO);
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let control = crate::OperationControl::new();
    let mut writer = Writer {
        control: control.clone(),
        written: 0,
    };
    assert!(matches!(
        json.write_json_controlled(&mut writer, &control),
        Err(crate::OperationCancelled {
            reason: crate::CancelReason::DeadlineExceeded,
            ..
        })
    ));
    assert!(
        writer.written >= 1_024,
        "deadline is installed after useful export work"
    );
}
