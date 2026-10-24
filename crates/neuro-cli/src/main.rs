use neuro_domain::{Scenario, atomic_write};
use neuro_engine::evaluate;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("Ошибка: {error}");
        std::process::exit(1);
    }
}
fn next_path(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("После {flag} нужен путь"))
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        help();
        return Ok(());
    };
    if command == "--help" || command == "-h" {
        help();
        return Ok(());
    }
    let path = next_path(&mut args, &command)?;
    match command.as_str() {
        "new" => {
            if args.next().is_some() {
                return Err("Лишний аргумент команды new".into());
            }
            if path.exists() {
                return Err("Файл уже существует; укажите новый путь".into());
            }
            Scenario::default().save(&path)?;
            println!("Создан illustrative-сценарий: {}", path.display());
        }
        "run" => {
            let mut csv = None;
            let mut json = None;
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--csv" if csv.is_none() => csv = Some(next_path(&mut args, &flag)?),
                    "--json" if json.is_none() => json = Some(next_path(&mut args, &flag)?),
                    _ => return Err(format!("Неизвестный или повторный аргумент: {flag}").into()),
                }
            }
            for output in [&csv, &json].into_iter().flatten() {
                if same_path(&path, output) {
                    return Err("Экспорт не должен перезаписывать входной сценарий".into());
                }
            }
            if let (Some(csv), Some(json)) = (&csv, &json)
                && same_path(csv, json)
            {
                return Err("CSV и JSON должны иметь разные пути".into());
            }
            let evaluation = evaluate(&Scenario::load(&path)?)?;
            if let Some(csv) = csv {
                evaluation.export_csv(csv)?;
            }
            let text = serde_json::to_string_pretty(&evaluation)?;
            if let Some(json) = json {
                atomic_write(&json, format!("{text}\n").as_bytes())?;
            }
            println!("{text}");
        }
        _ => return Err(format!("Неизвестная команда: {command}. Используйте --help").into()),
    }
    Ok(())
}
fn same_path(a: &Path, b: &Path) -> bool {
    fn normalized(path: &Path) -> PathBuf {
        if let Ok(path) = path.canonicalize() {
            return path;
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf())
            .join(path.file_name().unwrap_or_default())
    }
    normalized(a) == normalized(b)
}
fn help() {
    println!(
        "NeuroLab — равновесное связывание абстрактных лигандов (illustrative)\n\n  neuro-cli new scenario.json\n  neuro-cli run scenario.json [--csv results.csv] [--json results.json]\n\nРезультат JSON выводится в stdout. Ошибки: stderr и код 1. Дозировок и прогнозов эффектов нет."
    );
}
