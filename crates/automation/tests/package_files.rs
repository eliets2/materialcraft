use materialcraft_automation::{AuthorizedWorkspace, Filesystem, Headless};
use materialcraft_engine::{Session, core::Package};
use serde_json::json;

#[test]
fn reopening_relative_paths_activates_the_existing_document_and_preserves_edits() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().to_path_buf();
    std::fs::create_dir_all(&root).unwrap();

    for extension in ["sbs", "mcraft"] {
        let workspace = AuthorizedWorkspace::new(Some(&root), Some(&root)).unwrap();
        let mut app = Headless::new(Session::new(Package::new("Source")), Filesystem::Workspace(workspace.clone()));
        let filename = format!("package.{extension}");
        app.save(Some(&filename)).unwrap();
        let mut reopened = Headless::new(Session::new(Package::new("Empty")), Filesystem::Workspace(workspace));
        reopened.open(&filename).unwrap();
        let document = reopened.session.document_id;
        reopened.command_run("folder.create", json!({"name": "Unsaved"})).unwrap();
        let history = reopened.session.history();

        reopened.open(&filename).unwrap();

        assert_eq!(reopened.session.document_id, document);
        assert_eq!(reopened.session.open_packages().len(), 2);
        assert_eq!(reopened.session.path.as_deref(), Some(filename.as_str()));
        assert!(reopened.session.modified);
        assert!(reopened.session.pkg.folder_paths().contains("Unsaved"));
        assert_eq!(reopened.session.history(), history);

        reopened.command_run("file.newPackage", json!({"name": "Other"})).unwrap();
        reopened.open(&filename).unwrap();

        assert_eq!(reopened.session.document_id, document);
        assert_eq!(reopened.session.open_packages().len(), 3);
        assert!(reopened.session.modified);
        assert!(reopened.session.pkg.folder_paths().contains("Unsaved"));
        assert_eq!(reopened.session.history(), history);

        reopened.save(None).unwrap();

        assert!(!reopened.session.modified);
        assert!(root.join(&filename).is_file());
        assert_eq!(reopened.session.path.as_deref(), Some(filename.as_str()));
    }
}

#[test]
fn both_formats_save_and_reopen_using_separate_workspace_capabilities() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().to_path_buf();
    let input = root.join("input");
    let output = root.join("output");
    std::fs::create_dir_all(&input).unwrap();
    std::fs::create_dir_all(&output).unwrap();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
    std::fs::write(input.join("shape.svg"), svg).unwrap();
    let workspace = AuthorizedWorkspace::new(Some(&input), Some(&output)).unwrap();
    let mut app = Headless::new(Session::new(Package::new("Package")), Filesystem::Workspace(workspace));
    app.import_bitmap("shape.svg", None).unwrap();
    for extension in ["mcraft", "sbs"] {
        let filename = format!("package.{extension}");
        app.save(Some(&filename)).unwrap();
        app.save(None).unwrap();

        assert!(output.join(&filename).is_file());

        if extension == "mcraft" {
            assert_eq!(std::fs::read_dir(&output).unwrap().count(), 1);
        }
        let workspace = AuthorizedWorkspace::new(Some(&output), Some(&output)).unwrap();
        let mut reopened = Headless::new(Session::new(Package::new("Empty")), Filesystem::Workspace(workspace));
        reopened.open(&filename).unwrap();

        assert_eq!(reopened.session.pkg.resources[0].bytes.as_slice(), svg);

        reopened.save(None).unwrap();
    }

    assert_eq!(std::fs::read_dir(&input).unwrap().count(), 1);
    assert!(app.save(Some("../escape.sbs")).is_err());
    assert!(!root.join("escape.sbs").exists());

    // Release workspace directory handles before Windows removes the test tree
    drop(app);
}

#[test]
fn scoped_publication_resolves_linked_graphs_and_images_using_the_granted_read_root() {
    use materialcraft_engine::core::{Graph, GraphInstance, GraphPort, Resource, ResourceBytes, Value};
    use materialcraft_engine::{formats, image};

    let directory = tempfile::tempdir().unwrap();
    let dir = directory.path();
    let input = dir.join("input");
    let output = dir.join("published");
    std::fs::create_dir_all(input.join("parts")).unwrap();
    std::fs::create_dir_all(&output).unwrap();

    let image = image::Image::filled(2, 2, image::Kind::Color, [0.25, 0.5, 0.75, 1.0]);
    let pixels = formats::encode(&image, "png", formats::Depth::U8, false).unwrap();
    let mut helpers = Package::new("Helpers");
    helpers.resources.push(Resource::new("finish", "finish.png", ResourceBytes::new(pixels.clone())));
    let mut finish = Graph::new("Finish");
    let bitmap = finish.add_node("bitmap", [0.0; 2]);
    finish.node_mut(bitmap).unwrap().params.insert("resource".into(), Value::Text("finish".into()));
    let finish_output = finish.add_node("output", [0.0; 2]);
    finish.node_mut(finish_output).unwrap().params.insert("identifier".into(), Value::Text("basecolor".into()));
    finish.connect(bitmap, "output", finish_output, "input").unwrap();
    helpers.graphs.push(finish);
    let helper = formats::project::ProjectFiles::build(&helpers, "finish.sbs", false).unwrap();
    formats::project::write_files(&input.join("parts/finish.sbs"), &helper).unwrap();
    std::fs::write(dir.join("outside.sbs"), helper.xml().unwrap()).unwrap();

    for (project, dependency) in [("surface.sbs", "parts/finish.sbs"), ("missing.sbs", "../outside.sbs")] {
        let mut graph = Graph::new("Oak");
        let call = graph.add_node("graph_instance", [0.; 2]);
        graph.node_mut(call).unwrap().instance = Some(GraphInstance {
            package: dependency.into(),
            graph: "Finish".into(),
            inputs: vec![],
            outputs: vec![GraphPort { identifier: "basecolor".into(), type_code: 2 }],
            defaults: Default::default(),
            label: String::new(),
        });
        let node = graph.add_node("output", [200., 0.]);
        graph.node_mut(node).unwrap().params.extend([("identifier".into(), Value::Text("color".into())), ("usage".into(), Value::Enum(0))]);
        graph.connect(call, "basecolor", node, "input").unwrap();
        let mut package = Package::new("Surface");
        package.graphs.push(graph);
        std::fs::write(input.join(project), formats::sbs::write_sbs(&package).unwrap()).unwrap();
        let fs = AuthorizedWorkspace::new(Some(&input), Some(&output)).unwrap();
        let mut app = Headless::new(Session::new(Package::new("Empty")), Filesystem::Workspace(fs));
        app.load(project).unwrap();
        let result = app.export_material(&json!({"path":format!("{project}.mcmat")}));
        if project == "surface.sbs" {
            assert!(result.is_ok(), "{result:?}");
            let (pkg, entry) = formats::material::read_material(&std::fs::read(output.join("surface.sbs.mcmat")).unwrap()).unwrap();
            let entry = pkg.graph(entry).unwrap();
            assert_eq!(entry.name, "Oak");
            assert_eq!(pkg.graphs.len(), 2);
            let instance = entry.instances().next().unwrap();
            assert!(instance.package.is_empty());
            assert_eq!(entry.instances().count(), 1);
            assert_eq!(pkg.graph_by_name(&instance.graph).unwrap().1.name, "Finish");
            assert_eq!(pkg.resources.len(), 1);
            assert_eq!(pkg.resources[0].bytes.as_slice(), pixels);

            // Rebase links from the actual destination under the granted write root
            app.save(Some("saved.sbs")).unwrap();
            let xml = std::fs::read_to_string(output.join("saved.sbs")).unwrap();
            assert!(xml.contains("../input/parts/finish.sbs"), "{xml}");
            assert_eq!(app.session.path.as_deref(), Some("saved.sbs"));
            let workspace = AuthorizedWorkspace::new(Some(dir), Some(&output)).unwrap();
            let mut reopened = Headless::new(Session::new(Package::new("Empty")), Filesystem::Workspace(workspace));
            reopened.load("published/saved.sbs").unwrap();
            let helper_path = input.join("parts/finish.sbs").canonicalize().unwrap().to_string_lossy().into_owned();
            assert!(reopened.session.pkg.dependencies.contains_key(&helper_path));
            assert!(reopened.session.pkg.dependencies.values().any(|dependency| dependency.name == "Helpers"));
            app.save(Some("new/sub/saved.sbs")).unwrap();
            let xml = std::fs::read_to_string(output.join("new/sub/saved.sbs")).unwrap();
            assert!(xml.contains("../../../input/parts/finish.sbs"), "{xml}");
            reopened.load("published/new/sub/saved.sbs").unwrap();
            assert!(reopened.session.pkg.dependencies.contains_key(&helper_path));
            #[cfg(unix)]
            {
                std::fs::create_dir_all(output.join("nested/deep")).unwrap();
                std::os::unix::fs::symlink("nested/deep", output.join("alias")).unwrap();
                app.save(Some("alias/new/linked.sbs")).unwrap();
                let xml = std::fs::read_to_string(output.join("nested/deep/new/linked.sbs")).unwrap();
                assert!(xml.contains("../../../../input/parts/finish.sbs"), "{xml}");
                reopened.load("published/nested/deep/new/linked.sbs").unwrap();
                assert!(reopened.session.pkg.dependencies.values().any(|dependency| dependency.name == "Helpers"));
            }
        } else {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("the instanced graph does not exist"), "{error}");
            let outside = dir.join("outside.sbs").canonicalize().unwrap().to_string_lossy().into_owned();
            assert!(!app.session.pkg.dependencies.contains_key(&outside));
            assert!(!app.session.pkg.dependencies.values().any(|dependency| dependency.name == "Helpers"));
            assert!(!output.join("missing.sbs.mcmat").exists());
        }
    }
}

/// Ends a helper process when the test ends, however it ends: a PowerShell pipe server left running after a failed assertion held the build's output open for hours.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Remove a test directory on every exit path, including panics and early returns.
struct RemoveDirOnDrop(std::path::PathBuf);

impl Drop for RemoveDirOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn read_input_image_rejects_a_fifo_without_blocking() {
    if cfg!(unix) {
        let root = std::env::temp_dir().join(format!("materialcraft-workspace-fifo-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let fifo = root.join("input.fifo");
        let status = std::process::Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
        assert!(status.success(), "mkfifo failed");
        let thread_root = root.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let workspace = AuthorizedWorkspace::new(Some(&thread_root), Some(&thread_root)).unwrap();
            let _ = tx.send(workspace.read_input_image("input.fifo").is_err());
        });
        match rx.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(rejected) => assert!(rejected, "read_input_image must reject a FIFO"),
            Err(_) => panic!("read_input_image blocked on a FIFO instead of rejecting it"),
        }
        let _ = std::fs::remove_dir_all(root);
    } else if cfg!(windows) {
        let root = std::env::temp_dir().join(format!("materialcraft-workspace-fifo-{}", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let _cleanup = RemoveDirOnDrop(root.clone());
        let pipe_name = format!("materialcraft-fifo-{}", std::process::id());
        let pipe_path = format!(r"\\.\pipe\{}", pipe_name);
        let ready_path = root.join("pipe.ready");
        let _ = std::fs::remove_file(&ready_path);
        let _server = KillOnDrop(std::process::Command::new("powershell")
            .env("MATERIALCRAFT_PIPE_READY", ready_path.display().to_string())
            .args(["-NoProfile", "-Command", &format!(
                "$p = New-Object System.IO.Pipes.NamedPipeServerStream('{}', [System.IO.Pipes.PipeDirection]::InOut); [System.IO.File]::WriteAllText($env:MATERIALCRAFT_PIPE_READY, 'ready'); $p.WaitForConnection(); Start-Sleep -Seconds 5; $p.Dispose()",
                pipe_name
            )])
            .spawn()
            .expect("powershell"));
        for _ in 0..100 {
            if ready_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(ready_path.exists(), "named pipe was not created");
        let thread_root = root.clone();
        let pipe = pipe_path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut app = Headless::new(Session::new(Package::new("Source")), Filesystem::TrustedLocal);
            let _ = app.command_run("graph.node.add", json!({"def": "input_grayscale", "params": {"identifier": "curvature"}}));
            let output = thread_root.join("out.mcmat");
            let res = app.export_material(&json!({"path": output.to_str().unwrap(), "size": 16, "inputs": {"curvature": pipe}}));
            let _ = tx.send(res.is_err());
        });
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(rejected) => assert!(rejected, "read_input_image must reject a FIFO"),
            Err(_) => panic!("read_input_image blocked on a FIFO instead of rejecting it"),
        }
        drop(_server);
        drop(_cleanup);
    }
}

#[test]
fn read_input_image_rejects_a_fifo_without_blocking_review_regression_1() {
    if cfg!(unix) {
        let root = std::env::temp_dir().join(format!("materialcraft-workspace-fifo-review-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let fifo = root.join("input.fifo");
        let _ = std::fs::remove_file(&fifo);
        let status = std::process::Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
        assert!(status.success(), "mkfifo failed");
        let thread_root = root.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let workspace = AuthorizedWorkspace::new(Some(&thread_root), Some(&thread_root)).unwrap();
            let _ = tx.send(workspace.read_input_image("input.fifo").map_err(|error| error.to_string()));
        });
        match rx.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(Ok(_)) => panic!("read_input_image must reject a FIFO"),
            Ok(Err(error)) => assert!(error.contains("not a regular file"), "read_input_image must reject a FIFO: {error}"),
            Err(_) => panic!("read_input_image blocked on a FIFO instead of rejecting it"),
        }
        let _ = std::fs::remove_dir_all(root);
    }
    // Windows has no FIFO. The named-pipe variant that stood here leaked a PowerShell pipe server whenever an assertion failed first, and the leftover held a
    // build's output pipe open for hours; the O_NONBLOCK open this test guards is Unix-only, so on Windows the test does nothing.
}

#[test]
fn read_input_image_rejects_a_fifo_without_blocking_review_regression_2() {
    if cfg!(windows) {
        let root = std::env::temp_dir().join(format!("materialcraft-workspace-fifo-review2b-{}-O'Neil", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let _ = std::fs::remove_file(root.join("pipe.ready"));
        let pipe_name = format!("materialcraft-fifo-review2b-{}", std::process::id());
        let pipe_path = format!(r"\\.\pipe\{}", pipe_name);
        let ready_path = root.join("pipe.ready");
        let _cleanup = RemoveDirOnDrop(root.clone());
        let _server = KillOnDrop(std::process::Command::new("powershell")
            .env("MATERIALCRAFT_PIPE_READY", ready_path.display().to_string())
            .args(["-NoProfile", "-Command", &format!(
                "$p = New-Object System.IO.Pipes.NamedPipeServerStream('{}', [System.IO.Pipes.PipeDirection]::InOut); [System.IO.File]::WriteAllText($env:MATERIALCRAFT_PIPE_READY, 'ready'); $p.WaitForConnection(); Start-Sleep -Seconds 5; $p.Dispose()",
                pipe_name
            )])
            .spawn()
            .expect("powershell"));
        for _ in 0..100 {
            if ready_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(ready_path.exists(), "named pipe was not created");
        let thread_root = root.clone();
        let pipe = pipe_path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut app = Headless::new(Session::new(Package::new("Source")), Filesystem::TrustedLocal);
            let _ = app.command_run("graph.node.add", json!({"def": "input_grayscale", "params": {"identifier": "curvature"}}));
            let output = thread_root.join("out.mcmat");
            let res = app.export_material(&json!({"path": output.to_str().unwrap(), "size": 16, "inputs": {"curvature": pipe}}));
            let _ = tx.send(res.is_err());
        });
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(rejected) => assert!(rejected, "read_input_image must reject a FIFO"),
            Err(_) => panic!("read_input_image blocked on a FIFO instead of rejecting it"),
        }
        drop(_server);
        drop(_cleanup);
    }
}

#[test]
fn read_input_image_rejects_a_fifo_without_blocking_review_regression_2_review_regression_1() {
    if cfg!(windows) {
        let root = std::env::temp_dir().join(format!("materialcraft-workspace-fifo-review2-{}-O'Neil", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let _cleanup = RemoveDirOnDrop(root.clone());
        let pipe_name = format!("materialcraft-fifo-review2-{}", std::process::id());
        let pipe_path = format!(r"\\.\pipe\{}", pipe_name);
        let ready_path = root.join("pipe.ready");
        let _ = std::fs::remove_file(&ready_path);
        let _server = KillOnDrop(std::process::Command::new("powershell")
            .env("MATERIALCRAFT_PIPE_READY", ready_path.display().to_string())
            .args(["-NoProfile", "-Command", &format!(
                "$p = New-Object System.IO.Pipes.NamedPipeServerStream('{}', [System.IO.Pipes.PipeDirection]::InOut); [System.IO.File]::WriteAllText($env:MATERIALCRAFT_PIPE_READY, 'ready'); $p.WaitForConnection(); Start-Sleep -Seconds 5; $p.Dispose()",
                pipe_name
            )])
            .spawn()
            .expect("powershell"));
        for _ in 0..100 {
            if ready_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(ready_path.exists(), "named pipe was not created");
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let thread_root = root.clone();
        let pipe = pipe_path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut app = Headless::new(materialcraft_engine::sample_session(), Filesystem::TrustedLocal);
            let _ = app.command_run("graph.node.add", json!({"def": "input_grayscale", "params": {"identifier": "curvature"}}));
            let output = thread_root.join("out.mcmat");
            let res = app.export_material(&json!({"path": output.to_str().unwrap(), "size": 16, "inputs": {"curvature": pipe}}));
            let _ = tx.send(res.map_err(|error| error.to_string()));
        });
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(Ok(_)) => panic!("read_input_image must reject a FIFO"),
            // A Windows named pipe opens as a regular file, so the read succeeds and the failure
            // surfaces when the bytes are decoded; the call must still fail instead of hanging.
            Ok(Err(_)) => {}
            Err(_) => panic!("read_input_image blocked on a FIFO instead of rejecting it"),
        }
        drop(_server);
        drop(_cleanup);
    }
}
