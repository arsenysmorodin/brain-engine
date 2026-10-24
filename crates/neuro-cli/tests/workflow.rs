use neuro_domain::{Parameters, Scenario};
use neuro_engine::evaluate;
use std::{fs, path::PathBuf, process::Command};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("neurolab-test-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_neuro-cli"))
}

#[test]
fn saved_scenario_cli_and_shared_app_path_agree() {
    let temp = Temp::new();
    let input = temp.0.join("scenario.json");
    let output = temp.0.join("results.csv");
    let json = temp.0.join("results.json");
    let scenario = Scenario {
        parameters: Parameters { a: 3.0, b: 6.0 },
        ..Scenario::default()
    };
    scenario.save(&input).unwrap();
    assert_eq!(Scenario::load(&input).unwrap(), scenario);
    let shared = evaluate(&Scenario::load(&input).unwrap()).unwrap();
    let result = cli()
        .arg("run")
        .arg(&input)
        .arg("--csv")
        .arg(&output)
        .arg("--json")
        .arg(&json)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read_to_string(&output).unwrap(), shared.csv());
    let result: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        result["current"]["p_a"].as_f64().unwrap(),
        shared.current.p_a
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(json).unwrap()).unwrap(),
        result
    );
}

#[test]
fn corrupted_unsupported_and_unwritable_files_are_clear_errors() {
    let temp = Temp::new();
    let input = temp.0.join("bad.json");
    for (text, message) in [
        ("{bad", "Повреждённый JSON"),
        ("{\"format_version\":99}", "версия"),
    ] {
        fs::write(&input, text).unwrap();
        let result = cli().arg("run").arg(&input).output().unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(message));
    }
    let valid = temp.0.join("valid.json");
    Scenario::default().save(&valid).unwrap();
    let missing_parent = temp.0.join("missing/result.csv");
    let result = cli()
        .arg("run")
        .arg(&valid)
        .arg("--csv")
        .arg(missing_parent)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Ошибка файла"));
    let before = fs::read(&valid).unwrap();
    let result = cli()
        .arg("run")
        .arg(&valid)
        .arg("--csv")
        .arg(&valid)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(fs::read(&valid).unwrap(), before);
}

#[test]
fn creation_is_usable_and_refuses_accidental_overwrite() {
    let temp = Temp::new();
    let path = temp.0.join("new.json");
    assert!(cli().arg("new").arg(&path).status().unwrap().success());
    assert!(Scenario::load(&path).is_ok());
    assert!(
        !cli()
            .arg("new")
            .arg(&path)
            .output()
            .unwrap()
            .status
            .success()
    );
}
