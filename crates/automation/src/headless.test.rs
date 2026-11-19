#[cfg(test)]
mod end_merge_tests {
    use crate::headless::{Filesystem, Headless, RenderOptions};
    use serde_json::json;

    #[cfg(unix)]
    #[test]
    fn trusted_reads_are_bounded_and_regular_files_only() {
        let dir = std::env::temp_dir().join(format!("materialcraft-read-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let small = dir.join("small.txt");
        std::fs::write(&small, b"ok").unwrap();
        assert_eq!(crate::headless::read_file(&Filesystem::TrustedLocal, small.to_str().unwrap()).unwrap(), b"ok");

        let directory = dir.join("directory");
        std::fs::create_dir(&directory).unwrap();
        let err = crate::headless::read_file(&Filesystem::TrustedLocal, directory.to_str().unwrap()).unwrap_err().to_string();
        assert!(err.contains("not a regular file"), "{err}");

        let huge = dir.join("huge.bin");
        std::fs::File::create(&huge).unwrap().set_len(crate::workspace::MAX_READ_BYTES + 1).unwrap();
        let err = crate::headless::read_file(&Filesystem::TrustedLocal, huge.to_str().unwrap()).unwrap_err().to_string();
        assert!(err.contains("too large"), "{err}");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn trusted_reads_reject_named_pipes_without_waiting_for_a_writer() {
        use std::os::unix::fs::OpenOptionsExt;
        use std::time::Duration;

        let path = std::env::temp_dir().join(format!("materialcraft-read-pipe-{}", std::process::id()));
        assert!(std::process::Command::new("mkfifo").arg(&path).status().unwrap().success());
        let reader_path = path.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let result = crate::headless::read_file(&Filesystem::TrustedLocal, reader_path.to_str().unwrap());
            sender.send(result).unwrap();
        });

        let result = receiver.recv_timeout(Duration::from_secs(2));
        if result.is_err() {
            // Release a blocked reader so a regression cannot strand the test process
            let writer = std::fs::OpenOptions::new().write(true).custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32).open(&path).unwrap();
            drop(writer);
        }
        reader.join().unwrap();
        std::fs::remove_file(path).unwrap();

        let error = result.expect("a named pipe must be rejected before waiting for a writer").unwrap_err().to_string();
        assert!(error.contains("not a regular file"), "{error}");
    }

    #[test]
    fn trusted_input_reads_are_not_capped_at_the_sandbox_input_limit() {
        let dir = std::env::temp_dir().join(format!("materialcraft-trusted-input-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let large = dir.join("large.bin");
        let size = crate::budgets::MAX_INPUT_ENCODED_BYTES as u64 + 1;
        std::fs::File::create(&large).unwrap().set_len(size).unwrap();
        let h = Headless::new(Headless::default_session(), Filesystem::TrustedLocal);
        let bytes = h.read_input_image(large.to_str().unwrap()).expect("a trusted input above the sandbox cap must still load");
        assert_eq!(bytes.len() as u64, size);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn two_param_requests_are_two_undo_steps() {
        let mut h = Headless::new(Headless::default_session(), Filesystem::TrustedLocal);
        h.command_run("file.new", json!({"template": "empty"})).unwrap();
        let id = h.command_run("graph.node.add", json!({"def": "levels"})).unwrap()["id"].as_u64().unwrap();
        let sets = |h: &Headless| h.session.history().iter().filter(|l| *l == "Set parameter").count();
        h.command_run("graph.node.param", json!({"id": id, "param": "in_low", "value": [0.3, 0.3, 0.3, 0.3]})).unwrap();
        h.command_run("graph.node.param", json!({"id": id, "param": "in_low", "value": [0.4, 0.4, 0.4, 0.4]})).unwrap();
        assert_eq!(sets(&h), 2, "separate requests must not merge");
    }

    #[test]
    fn same_name_custom_mesh_reload_rebuilds_the_headless_scene() {
        let mut headless = Headless::new(materialcraft_engine::new_session(), Filesystem::Denied);
        let first_mesh = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let second_mesh = b"v 0 0 0\nv 2 0 0\nv 0 1 0\nf 1 2 3\n";
        let options = RenderOptions { width: Some(16), height: Some(16), ..RenderOptions::default() };

        headless.session.load_mesh_bytes("same.obj", first_mesh).unwrap();
        headless.render_3d(&options, 16);
        let first_positions = headless.scene.as_ref().unwrap().1.mesh.positions.clone();
        let previous_mesh = std::sync::Arc::downgrade(&headless.scene.as_ref().unwrap().1.mesh);

        headless.session.load_mesh_bytes("same.obj", second_mesh).unwrap();
        headless.render_3d(&options, 16);

        assert_ne!(headless.scene.as_ref().unwrap().1.mesh.positions, first_positions);
        assert!(previous_mesh.upgrade().is_none(), "the previous headless scene must be released");
    }
}
