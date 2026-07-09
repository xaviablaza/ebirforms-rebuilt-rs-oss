use ebirforms_core::{decrypt_payload, encrypt_payload};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn usage(program: &str) {
    eprintln!("Usage:");
    eprintln!("  {program} encrypt <plaintext.xml> <encrypted.xml>");
    eprintln!("  {program} decrypt <encrypted.xml> <plaintext.xml>");
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("ebirforms-cli");

    if args.len() != 4 || !matches!(args[1].as_str(), "encrypt" | "decrypt") {
        usage(program);
        return ExitCode::from(2);
    }

    let input = PathBuf::from(&args[2]);
    let output = PathBuf::from(&args[3]);

    let data = match fs::read(&input) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("failed to read {input:?}: {err}");
            return ExitCode::from(1);
        }
    };

    let result = match args[1].as_str() {
        "encrypt" => encrypt_payload(&data),
        "decrypt" => decrypt_payload(&data),
        _ => unreachable!(),
    };

    let output_bytes = match result {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("transform failed: {err}");
            return ExitCode::from(1);
        }
    };

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(err) = fs::create_dir_all(parent) {
                eprintln!("failed to create output directory {parent:?}: {err}");
                return ExitCode::from(1);
            }
        }
    }

    if let Err(err) = fs::write(&output, &output_bytes) {
        eprintln!("failed to write {output:?}: {err}");
        return ExitCode::from(1);
    }

    println!("wrote {} bytes to {}", output_bytes.len(), output.display());
    ExitCode::SUCCESS
}
