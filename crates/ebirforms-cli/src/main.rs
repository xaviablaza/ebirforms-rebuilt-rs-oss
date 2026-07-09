use ebirforms_core::{
    build_submission_package, decrypt_payload, encrypt_payload, parse_and_apply_receipt,
    poll_receipt_directory, run_due_jobs_dry_run, run_due_jobs_live, sha256_hex, submit_with_store,
    AppStateStore, DryRunTransport, JobMode, JobStore, SftpTransport, SubmissionStore, SubmitMode,
    TaxpayerProfile, Theme,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage(program: &str) {
    eprintln!("Usage:");
    eprintln!("  {program} encrypt <plaintext.xml> <encrypted.xml>");
    eprintln!("  {program} decrypt <encrypted.xml> <plaintext.xml>");
    eprintln!("  {program} render --form 1601C --input <input.json> --out <plaintext.xml>");
    eprintln!("  {program} package --form 1601C --input <input.json> --out <upload.xml> [--manifest <manifest.json>]");
    eprintln!("  {program} diff-fixture --form 1601C --input <input.json> --fixture <synthetic_encrypted.xml>");
    eprintln!("  {program} submit --form 1601C --input <input.json> --dry-run [--records <submissions.json>]");
    eprintln!("  {program} submit --form 1601C --input <input.json> --live --confirm [--records <submissions.json>]");
    eprintln!("  {program} queue --form 1601C --input <input.json> --dry-run [--db <jobs.sqlite>] [--max-attempts <n>]");
    eprintln!("  {program} queue --form 1601C --input <input.json> --live --confirm [--db <jobs.sqlite>] [--max-attempts <n>]");
    eprintln!("  {program} run-queue --dry-run [--db <jobs.sqlite>] [--records <submissions.json>] [--limit <n>]");
    eprintln!("  {program} run-queue --live --confirm [--db <jobs.sqlite>] [--records <submissions.json>] [--limit <n>]");
    eprintln!("  {program} jobs [--db <jobs.sqlite>]");
    eprintln!("  {program} receipt-match --receipt <receipt.txt> [--records <submissions.json>]");
    eprintln!("  {program} receipt-poll --receipt-dir <dir> [--records <submissions.json>]");
    eprintln!("  {program} profiles [--state <app-state.json>]");
    eprintln!("  {program} profile-create --profile-id <id> --tin <tin> --email <email> --name <taxpayer> [--rdo <code>] [--address <addr>] [--zip <zip>] [--state <app-state.json>]");
    eprintln!("  {program} settings --theme <light|dark|system> [--state <app-state.json>]");
    eprintln!("  {program} lock-init --pin <pin> [--state <app-state.json>]");
    eprintln!("  {program} unlock-check --pin <pin> [--state <app-state.json>]");
    eprintln!("  {program} serve [--addr 127.0.0.1:8765] [--db <jobs.sqlite>] [--records <submissions.json>] [--state <app-state.json>]");
}

#[derive(Debug, Default)]
struct Args {
    form: Option<String>,
    input: Option<PathBuf>,
    out: Option<PathBuf>,
    manifest: Option<PathBuf>,
    fixture: Option<PathBuf>,
    receipt: Option<PathBuf>,
    receipt_dir: Option<PathBuf>,
    records: Option<PathBuf>,
    db: Option<PathBuf>,
    state: Option<PathBuf>,
    profile_id: Option<String>,
    tin: Option<String>,
    email: Option<String>,
    name: Option<String>,
    rdo: Option<String>,
    address: Option<String>,
    zip: Option<String>,
    theme: Option<String>,
    pin: Option<String>,
    limit: Option<usize>,
    max_attempts: Option<u32>,
    addr: Option<String>,
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
        "receipt-match" => run_receipt_match(parse_flags(&argv[2..])),
        "receipt-poll" => run_receipt_poll(parse_flags(&argv[2..])),
        "profiles" => run_profiles(parse_flags(&argv[2..])),
        "profile-create" => run_profile_create(parse_flags(&argv[2..])),
        "settings" => run_settings(parse_flags(&argv[2..])),
        "lock-init" => run_lock_init(parse_flags(&argv[2..])),
        "unlock-check" => run_unlock_check(parse_flags(&argv[2..])),
        "serve" => run_serve(parse_flags(&argv[2..])),
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
            "--receipt" => {
                i += 1;
                parsed.receipt = Some(PathBuf::from(
                    args.get(i).ok_or("--receipt requires a value")?,
                ));
            }
            "--receipt-dir" => {
                i += 1;
                parsed.receipt_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--receipt-dir requires a value")?,
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
            "--state" => {
                i += 1;
                parsed.state = Some(PathBuf::from(
                    args.get(i).ok_or("--state requires a value")?,
                ));
            }
            "--profile-id" => {
                i += 1;
                parsed.profile_id =
                    Some(args.get(i).ok_or("--profile-id requires a value")?.clone());
            }
            "--tin" => {
                i += 1;
                parsed.tin = Some(args.get(i).ok_or("--tin requires a value")?.clone());
            }
            "--email" => {
                i += 1;
                parsed.email = Some(args.get(i).ok_or("--email requires a value")?.clone());
            }
            "--name" => {
                i += 1;
                parsed.name = Some(args.get(i).ok_or("--name requires a value")?.clone());
            }
            "--rdo" => {
                i += 1;
                parsed.rdo = Some(args.get(i).ok_or("--rdo requires a value")?.clone());
            }
            "--address" => {
                i += 1;
                parsed.address = Some(args.get(i).ok_or("--address requires a value")?.clone());
            }
            "--zip" => {
                i += 1;
                parsed.zip = Some(args.get(i).ok_or("--zip requires a value")?.clone());
            }
            "--theme" => {
                i += 1;
                parsed.theme = Some(args.get(i).ok_or("--theme requires a value")?.clone());
            }
            "--pin" => {
                i += 1;
                parsed.pin = Some(args.get(i).ok_or("--pin requires a value")?.clone());
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
            "--addr" => {
                i += 1;
                parsed.addr = Some(args.get(i).ok_or("--addr requires a value")?.clone());
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

fn app_state_path(args: &Args) -> PathBuf {
    args.state
        .clone()
        .unwrap_or_else(|| PathBuf::from(".ebirforms/app-state.json"))
}

fn print_json<T: serde::Serialize>(value: T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&value).map_err(|err| err.to_string())?;
    println!("{json}");
    Ok(())
}

fn run_receipt_match(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let receipt_path = args
        .receipt
        .as_deref()
        .ok_or("receipt-match requires --receipt")?;
    let receipt_text = fs::read_to_string(receipt_path)
        .map_err(|err| format!("failed to read receipt {}: {err}", receipt_path.display()))?;
    let store = SubmissionStore::new(submission_records_path(&args));
    let record = parse_and_apply_receipt(&store, &receipt_text).map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(&record).map_err(|err| err.to_string())?;
    println!("{json}");
    eprintln!("submission record store: {}", store.path().display());
    Ok(())
}

fn run_receipt_poll(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let receipt_dir = args
        .receipt_dir
        .as_deref()
        .ok_or("receipt-poll requires --receipt-dir")?;
    let store = SubmissionStore::new(submission_records_path(&args));
    let report = poll_receipt_directory(&store, receipt_dir).map_err(|err| err.to_string())?;
    print_json(report)
}

fn run_profiles(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let store = AppStateStore::new(app_state_path(&args));
    print_json(store.list_profiles().map_err(|err| err.to_string())?)
}

fn run_profile_create(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let mut profile = TaxpayerProfile::new(
        args.profile_id
            .clone()
            .ok_or("profile-create requires --profile-id")?,
        args.tin.clone().ok_or("profile-create requires --tin")?,
        args.email
            .clone()
            .ok_or("profile-create requires --email")?,
        args.name.clone().ok_or("profile-create requires --name")?,
    );
    profile.rdo_code = args.rdo.clone();
    profile.registered_address = args.address.clone();
    profile.zip_code = args.zip.clone();
    let store = AppStateStore::new(app_state_path(&args));
    let saved = store
        .upsert_profile(profile)
        .map_err(|err| err.to_string())?;
    print_json(saved)
}

fn run_settings(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let store = AppStateStore::new(app_state_path(&args));
    if let Some(theme) = args.theme.as_deref() {
        let theme = Theme::parse(theme).map_err(|err| err.to_string())?;
        print_json(store.set_theme(theme).map_err(|err| err.to_string())?)
    } else {
        print_json(store.settings().map_err(|err| err.to_string())?)
    }
}

fn run_lock_init(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let pin = args.pin.as_deref().ok_or("lock-init requires --pin")?;
    let store = AppStateStore::new(app_state_path(&args));
    print_json(store.set_master_pin(pin).map_err(|err| err.to_string())?)
}

fn run_unlock_check(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let pin = args.pin.as_deref().ok_or("unlock-check requires --pin")?;
    let store = AppStateStore::new(app_state_path(&args));
    let unlocked = store
        .verify_master_pin(pin)
        .map_err(|err| err.to_string())?;
    print_json(serde_json::json!({ "unlocked": unlocked }))
}

fn run_serve(args: Result<Args, String>) -> Result<(), String> {
    let args = args?;
    let addr = args
        .addr
        .clone()
        .unwrap_or_else(|| "127.0.0.1:8765".to_string());
    let job_store = JobStore::open(job_db_path(&args)).map_err(|err| err.to_string())?;
    let submission_store = SubmissionStore::new(submission_records_path(&args));
    let app_state_store = AppStateStore::new(app_state_path(&args));
    let listener =
        TcpListener::bind(&addr).map_err(|err| format!("failed to bind {addr}: {err}"))?;
    eprintln!("ebirforms local IPC listening on http://{addr}");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) =
                    handle_http_connection(stream, &job_store, &submission_store, &app_state_store)
                {
                    eprintln!("ipc request failed: {err}");
                }
            }
            Err(err) => eprintln!("ipc accept failed: {err}"),
        }
    }
    Ok(())
}

fn handle_http_connection(
    mut stream: TcpStream,
    job_store: &JobStore,
    submission_store: &SubmissionStore,
    app_state_store: &AppStateStore,
) -> Result<(), String> {
    let mut buffer = [0_u8; 64 * 1024];
    let read = stream
        .read(&mut buffer)
        .map_err(|err| format!("read failed: {err}"))?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let (head, body) = request
        .split_once("\r\n\r\n")
        .ok_or("malformed HTTP request")?;
    let mut lines = head.lines();
    let request_line = lines.next().ok_or("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing method")?;
    let target = parts.next().ok_or("missing path")?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = parse_query(query);

    let response = match (method, path) {
        ("GET", "/health") => json_response(200, serde_json::json!({"ok": true})),
        ("GET", "/jobs") => json_result(job_store.list().map_err(|err| err.to_string())),
        ("GET", "/submissions") => {
            json_result(submission_store.load().map_err(|err| err.to_string()))
        }
        ("GET", "/profiles") => json_result(
            app_state_store
                .list_profiles()
                .map_err(|err| err.to_string()),
        ),
        ("GET", "/settings") => {
            json_result(app_state_store.settings().map_err(|err| err.to_string()))
        }
        ("POST", "/profiles") => {
            let profile: TaxpayerProfile = serde_json::from_str(body)
                .map_err(|err| format!("failed to parse POST /profiles body: {err}"))?;
            json_result(
                app_state_store
                    .upsert_profile(profile)
                    .map_err(|err| err.to_string()),
            )
        }
        ("POST", "/settings/theme") => {
            let body: Value = serde_json::from_str(body)
                .map_err(|err| format!("failed to parse POST /settings/theme body: {err}"))?;
            let theme = body
                .get("theme")
                .and_then(Value::as_str)
                .ok_or("POST /settings/theme requires JSON field `theme`")?;
            let theme = Theme::parse(theme).map_err(|err| err.to_string())?;
            json_result(
                app_state_store
                    .set_theme(theme)
                    .map_err(|err| err.to_string()),
            )
        }
        ("POST", "/lock/init") => {
            let body: Value = serde_json::from_str(body)
                .map_err(|err| format!("failed to parse POST /lock/init body: {err}"))?;
            let pin = body
                .get("pin")
                .and_then(Value::as_str)
                .ok_or("POST /lock/init requires JSON field `pin`")?;
            json_result(
                app_state_store
                    .set_master_pin(pin)
                    .map_err(|err| err.to_string()),
            )
        }
        ("POST", "/lock/check") => {
            let body: Value = serde_json::from_str(body)
                .map_err(|err| format!("failed to parse POST /lock/check body: {err}"))?;
            let pin = body
                .get("pin")
                .and_then(Value::as_str)
                .ok_or("POST /lock/check requires JSON field `pin`")?;
            json_result(
                app_state_store
                    .verify_master_pin(pin)
                    .map(|unlocked| serde_json::json!({ "unlocked": unlocked }))
                    .map_err(|err| err.to_string()),
            )
        }
        ("POST", "/jobs") => {
            let form = query.get("form").map(String::as_str).unwrap_or("1601C");
            let mode = match query.get("mode").map(String::as_str) {
                Some("live") => JobMode::Live,
                _ => JobMode::DryRun,
            };
            let max_attempts = query
                .get("max_attempts")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(3);
            let input: Value = serde_json::from_str(body)
                .map_err(|err| format!("failed to parse POST /jobs body: {err}"))?;
            json_result(
                job_store
                    .enqueue(form, &input, mode, max_attempts)
                    .map_err(|err| err.to_string()),
            )
        }
        ("POST", "/run-queue") => {
            let limit = query
                .get("limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1);
            let mode = query.get("mode").map(String::as_str).unwrap_or("dry_run");
            let result = match mode {
                "dry_run" => run_due_jobs_dry_run(job_store, submission_store, limit),
                "live" => run_due_jobs_live(job_store, submission_store, limit),
                other => {
                    return write_http_response(
                        &mut stream,
                        json_response(
                            400,
                            serde_json::json!({"error": format!("unknown mode {other}")}),
                        ),
                    )
                }
            };
            json_result(result.map_err(|err| err.to_string()))
        }
        _ => json_response(
            404,
            serde_json::json!({"error": format!("unsupported route {method} {path}")}),
        ),
    };

    write_http_response(&mut stream, response)
}

fn json_result<T: serde::Serialize>(result: Result<T, String>) -> HttpResponse {
    match result {
        Ok(value) => json_response(200, value),
        Err(error) => json_response(500, serde_json::json!({"error": error})),
    }
}

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

fn json_response<T: serde::Serialize>(status: u16, value: T) -> HttpResponse {
    let body = serde_json::to_vec_pretty(&value)
        .unwrap_or_else(|_| b"{\"error\":\"serialization failed\"}".to_vec());
    HttpResponse { status, body }
}

fn write_http_response(stream: &mut TcpStream, response: HttpResponse) -> Result<(), String> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Internal Server Error",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        reason,
        response.body.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.write_all(&response.body))
        .map_err(|err| format!("write failed: {err}"))
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 2;
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8(out).ok()
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
