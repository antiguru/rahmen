//! The installed configuration must work without files from the source checkout.

use std::process::Command;

use pyo3::prelude::*;
use pyo3::types::PyList;
use rahmen::config::Settings;
use rahmen::provider::StatusLineFormatter;

#[test]
fn shipped_config_formats_metadata_outside_checkout() {
    const CHILD: &str = "RAHMEN_PACKAGED_CONFIG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        assert!(!std::path::Path::new("postprocess.py").exists());
        Python::attach(|py| {
            let sys = py.import("sys").unwrap();
            assert!(
                !sys.getattr("modules")
                    .unwrap()
                    .contains("postprocess")
                    .unwrap()
            );
            assert!(
                py.import("importlib.util")
                    .unwrap()
                    .call_method1("find_spec", ("postprocess",))
                    .unwrap()
                    .is_none()
            );
        });
        let settings: Settings = config::Config::builder()
            .add_source(config::File::with_name("rahmen"))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();
        if let Some(paths) = settings.py_path {
            Python::attach(|py| {
                let paths_list = py
                    .import("sys")
                    .unwrap()
                    .getattr("path")
                    .unwrap()
                    .cast_into::<PyList>()
                    .unwrap();
                for path in paths {
                    paths_list.insert(0, path).unwrap();
                }
            });
        }
        let formatter = StatusLineFormatter::new(
            settings.status_line.into_iter(),
            settings.py_postprocess,
            settings.separator.unwrap_or_else(|| ", ".to_owned()),
        )
        .expect("installed defaults should construct without an optional Python module");
        assert_eq!(
            formatter.format("tagged.jpg").unwrap(),
            "Zurich, 2026-06-11, by Moritz"
        );
        return;
    }

    let directory = std::env::temp_dir().join(format!(
        "rahmen-packaged-config-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::copy(source.join("rahmen.toml"), directory.join("rahmen.toml")).unwrap();
    std::fs::copy(
        source.join("rahmen-exiv2/testdata/tagged.jpg"),
        directory.join("tagged.jpg"),
    )
    .unwrap();
    // A fresh process isolates both Python's module cache and the working directory.
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "shipped_config_formats_metadata_outside_checkout",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env_remove("PYTHONPATH")
        .current_dir(&directory)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(
        result.status.success(),
        "installed configuration failed outside checkout:\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
