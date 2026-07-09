use ebirforms_core::{
    build_submission_package, decrypt_payload, encrypt_payload, run_due_jobs_dry_run,
    run_due_jobs_live, sha256_hex, submit_with_store, DryRunTransport, JobMode, JobStore,
    SftpTransport, SubmissionStore, SubmitMode,
};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage(program: &str) {
    eprintln!("Usage:");
    eprintln!("  {program} encrypt <plaintext.xml> <encrypted.xml>");
    eprintln!("  {program} decrypt <encrypted.xml> <plaintext.xml>");
    eprintln!("  {program} render --form 1601C --input <input.json> --out <plaintext.xml>");
    eprintln!("  {program} package --form 1601C --input <input.json> --out <upload.xml> [--manifest <manifest.json>]");
    eprintln!("  {program} diff-fixture --form 1601C --input <input.json> --fixture <official_encrypted.xml>");
    eprintln!("  {program} submit --form 1601C --input <input.json> --dry-run [--records <submissions.json>]");
    eprintln!("  {program} submit --form 1601C --input <input.json> --live --confirm [--records <submissions.json>]");
    eprintln!("  {program} queue --form 1601C --input <input.json> --dry-run [--db <jobs.sqlite>] [--max-attempts <n>]");
    eprintln!("  {program} queue --form 1601C --input <input.json> --live --confirm [--db <jobs.sqlite>] [--max-attempts <n>]");
    eprintln!("  {program} run-queue --dry-run [--db <jobs.sqlite>] [--records <submissions.json>] [--limit <n>]");
    eprintln!("  {program} run-queue --live --confirm [--db <jobs.sqlite>] [--records <submissions.json>] [--limit <n>]");
    eprintln!("  {program} jobs [--db <jobs.sqlite>]");
}

#[derive(Debug, Default)]
struct Args {
    form: Option<String>,
    input: Option<PathBuf>,
    out: Option<PathBuf>,
    manifest: Option<PathBuf>,
    fixture: Option<PathBuf>,
    records: Option<PathBuf>,
    db: Option<PathBuf>,
    limit: Option<usize>,
    max_attempts: Option<u32>,
    dry_run: bool,
    live: bool,
    confirm: bool,
}

fn main() -> ExitCode {
    let argv: Vec<String> = env::args().collect();
    let program = argv.first().map(String::as_str).unwrap_or("ebirforms-cli");
    let Some(command) = argv.get(1).map(String::as_str) else {
        usage(program);
        return ExitCode::from(2);
    };

    let result = match command {
        "encrypt" | "decrypt" => run_transform(command, &argv[2..]),
        "render" => run_render(parse_flags(&argv[2..])),
        "package" => run_package(parse_flags(&argv[2..])),
        "diff-fixture" => run_diff_fixture(parse_flags(&argv[2..])),
        "submit" => run_submit(parse_flags(&argv[2..])),
        "queue" => run_queue(parse_flags(&argv[2..])),
        "run-queue" => run_run_queue(parse_flags(&argv[2..])),
        "jobs" => run_jobs(parse_flags(&argv[2..])),
        _ => {
            usage(program);
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(1)
        }
    }
}

fn parse_flags(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--form" => {
                i += 1;
                parsed.form = Some(args.get(i).ok_or("--form requires a value")?.clone());
            }
            "--input" => {
                i += 1;
                parsed.input = Some(PathBuf::from(
                    args.get(i).ok_or("--input requires a value")?,
                ));
            }
            "--out" => {
                i += 1;
                parsed.out = Some(PathBuf::from(args.get(i).ok_or("--out requires a value")?));
            }
            "--manifest" => {
                i += 1;
                parsed.manifest = Some(PathBuf::from(
                    args.get(i).ok_or("--manifest requires a value")?,
                ));
            }
            "--fixture" => {
                i += 1;
                parsed.fixture = Some(PathBuf::from(
                    args.get(i).ok_or("--fixture requires a value")?,
                ));
            }
            "--records" => {
                i += 1;
                parsed.records = Some(PathBuf::from(
                    args.get(i).ok_or("--records requires a value")?,
                ));
            }
            "--db" => {
                i += 1;
                parsed.db = Some(PathBuf::from(args.get(i).ok_or("--db requires a value")?));
            }
            "--limit" => {
                i += 1;
                parsed.limit = Some(
                    args.get(i)
                        .ok_or("--limit requires a value")?
                        .parse()
                        .map_err(|_| "--limit must be a positive integer".to_string())?,
                );
            }
            "--max-attempts" => {
                i += 1;
                parsed.max_attempts = Some(
                    args.get(i)
                        .ok_or("--max-attempts requires a value")?
                        .parse()
                        .map_err(|_| "--max-attempts must be a positive integer".to_string())?,
                );
            }
            "--dry-run" => parsed.dry_run = true,
            "--live" => parsed.live = true,
            "--confirm" => parsed.confirm = true,
            other => return Err(format!("unknown flag or argument: {other}")),
        }
        i += 1;
    }
    Ok(parsed)
}

fn run_transform(command: &str, args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(format!("{command} requires <input> <output>"));
    }

    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let data =
        fs::read(&input).map_err(|err| format!("failed to read {}: {err}", input.display()))?;

    let output_bytes = match command {
        "encrypt" => encrypt_payload(&data).map_err(|err| err.to_string())?,
        "decrypt" => decrypt_payload(&data).map_err(|err| err.to_string())?,
        _ => unreachable!(),
    };

    write_bytes(&output, &output_bytes)?;
    println!("wrote {} bytes to {}", output_bytes.len(), output.display());
    Ok(())
}

fn run_render(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let form = args.form.as_deref().ok_or("render requires --form")?;
    let input = read_json(args.input.as_deref().ok_or("render requires --input")?)?;
    let out = args.out.as_deref().ok_or("render requires --out")?;
    let plaintext = ebirforms_core::render_form(form, &input).map_err(|err| err.to_string())?;
    write_bytes(out, plaintext.as_bytes())?;
    println!("wrote {} bytes to {}", plaintext.len(), out.display());
    Ok(())
}

fn run_package(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let form = args.form.as_deref().ok_or("package requires --form")?;
    let input_path = args.input.as_deref().ok_or("package requires --input")?;
    let out = args.out.as_deref().ok_or("package requires --out")?;
    let input = read_json(input_path)?;
    let package = build_submission_package(form, &input).map_err(|err| err.to_string())?;

    write_bytes(out, &package.payload)?;
    let manifest_json =
        serde_json::to_vec_pretty(&package.manifest).map_err(|err| err.to_string())?;
    if let Some(path) = args.manifest.as_deref() {
        write_bytes(path, &manifest_json)?;
        println!("wrote manifest to {}", path.display());
    }
    println!(
        "packaged {} bytes for {} (sha256 {}, remote path {})",
        package.payload.len(),
        package.manifest.filename,
        package.manifest.payload_sha256,
        package.manifest.remote_path
    );
    Ok(())
}

fn run_diff_fixture(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let form = args.form.as_deref().ok_or("diff-fixture requires --form")?;
    let input_path = args
        .input
        .as_deref()
        .ok_or("diff-fixture requires --input")?;
    let fixture_path = args
        .fixture
        .as_deref()
        .ok_or("diff-fixture requires --fixture")?;
    let input = read_json(input_path)?;
    let expected = fs::read(fixture_path)
        .map_err(|err| format!("failed to read fixture {}: {err}", fixture_path.display()))?;
    let package = build_submission_package(form, &input).map_err(|err| err.to_string())?;

    if package.payload == expected {
        println!(
            "fixture match: {} bytes, sha256 {}",
            package.payload.len(),
            package.manifest.payload_sha256
        );
        Ok(())
    } else {
        Err(format!(
            "fixture mismatch: generated len={} sha256={}, expected len={} sha256={}",
            package.payload.len(),
            package.manifest.payload_sha256,
            expected.len(),
            sha256_hex(&expected)
        ))
    }
}

fn run_submit(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let form = args.form.as_deref().ok_or("submit requires --form")?;
    let input_path = args.input.as_deref().ok_or("submit requires --input")?;
    if args.live && !(args.confirm && !args.dry_run) {
        return Err(
            "live submission requires --live --confirm and must not include --dry-run".to_string(),
        );
    }
    if !args.live && !args.dry_run {
        return Err(
            "submit is safe-by-default: pass --dry-run or explicitly pass --live --confirm"
                .to_string(),
        );
    }

    let input = read_json(input_path)?;
    let package = build_submission_package(form, &input).map_err(|err| err.to_string())?;

    let records_path = args
        .records
        .unwrap_or_else(|| PathBuf::from(".ebirforms/submissions.json"));
    let store = SubmissionStore::new(&records_path);

    if args.live {
        let mut transport = SftpTransport::from_env();
        let record = submit_with_store(&package, &store, &mut transport, SubmitMode::Live)
            .map_err(|err| err.to_string())?;
        let json = serde_json::to_string_pretty(&record).map_err(|err| err.to_string())?;
        println!("{json}");
    } else {
        let mut transport = DryRunTransport::new();
        let record = submit_with_store(&package, &store, &mut transport, SubmitMode::DryRun)
            .map_err(|err| err.to_string())?;
        let json = serde_json::to_string_pretty(&record).map_err(|err| err.to_string())?;
        println!("{json}");
    }

    eprintln!("submission record store: {}", store.path().display());
    Ok(())
}

fn run_queue(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let form = args.form.as_deref().ok_or("queue requires --form")?;
    let input_path = args.input.as_deref().ok_or("queue requires --input")?;
    let mode = requested_job_mode(&args, "queue")?;
    let input = read_json(input_path)?;
    let job_store = JobStore::open(job_db_path(&args)).map_err(|err| err.to_string())?;
    let job = job_store
        .enqueue(form, &input, mode, args.max_attempts.unwrap_or(3))
        .map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(&job).map_err(|err| err.to_string())?;
    println!("{json}");
    eprintln!("job store: {}", job_store.path().display());
    Ok(())
}

fn run_run_queue(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let mode = requested_job_mode(&args, "run-queue")?;
    let job_store = JobStore::open(job_db_path(&args)).map_err(|err| err.to_string())?;
    let submission_store = SubmissionStore::new(submission_records_path(&args));
    let limit = args.limit.unwrap_or(1);

    let jobs = match mode {
        JobMode::DryRun => run_due_jobs_dry_run(&job_store, &submission_store, limit),
        JobMode::Live => {
            if !args.confirm {
                return Err("run-queue live mode requires --live --confirm".to_string());
            }
            run_due_jobs_live(&job_store, &submission_store, limit)
        }
    }
    .map_err(|err| err.to_string())?;

    let json = serde_json::to_string_pretty(&jobs).map_err(|err| err.to_string())?;
    println!("{json}");
    eprintln!("job store: {}", job_store.path().display());
    eprintln!(
        "submission record store: {}",
        submission_store.path().display()
    );
    Ok(())
}

fn run_jobs(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let job_store = JobStore::open(job_db_path(&args)).map_err(|err| err.to_string())?;
    let jobs = job_store.list().map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(&jobs).map_err(|err| err.to_string())?;
    println!("{json}");
    eprintln!("job store: {}", job_store.path().display());
    Ok(())
}

fn requested_job_mode(args: &Args, command: &str) -> Result<JobMode, String> {
    if args.live && !(args.confirm && !args.dry_run) {
        return Err(format!(
            "{command} live mode requires --live --confirm and must not include --dry-run"
        ));
    }
    if args.live {
        Ok(JobMode::Live)
    } else if args.dry_run {
        Ok(JobMode::DryRun)
    } else {
        Err(format!(
            "{command} is safe-by-default: pass --dry-run or explicitly pass --live --confirm"
        ))
    }
}

fn job_db_path(args: &Args) -> PathBuf {
    args.db
        .clone()
        .unwrap_or_else(|| PathBuf::from(".ebirforms/jobs.sqlite"))
}

fn submission_records_path(args: &Args) -> PathBuf {
    args.records
        .clone()
        .unwrap_or_else(|| PathBuf::from(".ebirforms/submissions.json"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let data = fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_slice(&data)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
    }
    fs::write(path, bytes).map_err(|err| format!("failed to write {}: {err}", path.display()))
}
