#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::path::Path;
use std::path::PathBuf;

use teleport_build::{Config, ContractBundle, GenerateError};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let exit_code = run_cli(
        &args,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    std::process::exit(exit_code);
}

fn run_cli(
    args: &[String],
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
) -> i32 {
    match run(args, stdout) {
        Ok(()) => 0,
        Err(err) => {
            let _ = writeln!(stderr, "{err}");
            1
        }
    }
}

fn run(args: &[String], stdout: &mut dyn std::io::Write) -> Result<(), CliError> {
    let command = args.first().ok_or_else(|| CliError::Usage(usage()))?;

    match command.as_str() {
        "generate-ts" => generate_ts(&args[1..], stdout),
        "-h" | "--help" | "help" => {
            write_line(stdout, &usage())?;
            Ok(())
        }
        other => Err(CliError::Usage(format!(
            "unknown command `{other}`\n\n{}",
            usage()
        ))),
    }
}

fn generate_ts(args: &[String], stdout: &mut dyn std::io::Write) -> Result<(), CliError> {
    let mut input = None;
    let mut output = None;
    let mut client_import_path = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--input" => {
                i += 1;
                input = Some(required_value(args, i, "--input")?.into());
            }
            "--output" => {
                i += 1;
                output = Some(required_value(args, i, "--output")?.into());
            }
            "--client-import" => {
                i += 1;
                client_import_path = Some(required_value(args, i, "--client-import")?.to_owned());
            }
            "-h" | "--help" => {
                write_line(stdout, &generate_ts_usage())?;
                return Ok(());
            }
            flag => {
                return Err(CliError::Usage(format!(
                    "unknown flag `{flag}`\n\n{}",
                    generate_ts_usage()
                )));
            }
        }

        i += 1;
    }

    let input: PathBuf = input.ok_or_else(|| CliError::Usage(generate_ts_usage()))?;
    let output: PathBuf = output.ok_or_else(|| CliError::Usage(generate_ts_usage()))?;

    let bundle = read_contract(&input)?;
    let mut config = Config::new(&output);
    if let Some(client_import_path) = client_import_path {
        config.client_import_path = Some(client_import_path);
    }

    teleport_build::write_from_contract(&config, &bundle).map_err(CliError::Generate)?;
    write_line(
        stdout,
        &format!("Generated TypeScript bindings in {}", output.display()),
    )?;

    Ok(())
}

fn required_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, CliError> {
    args.get(index)
        .map(std::string::String::as_str)
        .ok_or_else(|| {
            CliError::Usage(format!(
                "missing value for {flag}\n\n{}",
                generate_ts_usage()
            ))
        })
}

fn read_contract(path: &Path) -> Result<ContractBundle, CliError> {
    let bytes = std::fs::read(path).map_err(|source| CliError::ReadContract {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_slice(&bytes).map_err(|source| CliError::ParseContract {
        path: path.to_path_buf(),
        source,
    })
}

fn write_line(stdout: &mut dyn std::io::Write, message: &str) -> Result<(), CliError> {
    writeln!(stdout, "{message}").map_err(CliError::WriteOutput)
}

fn usage() -> String {
    format!(
        "Usage:\n  teleport-cli <command> [options]\n\nCommands:\n  generate-ts   {}\n",
        generate_ts_usage().replace('\n', "\n                ")
    )
}

fn generate_ts_usage() -> String {
    "generate-ts --input <teleport.contract.json> --output <dir> [--client-import <path>]"
        .to_owned()
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    WriteOutput(std::io::Error),
    ReadContract {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseContract {
        path: PathBuf,
        source: serde_json::Error,
    },
    Generate(GenerateError),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(msg) => write!(f, "{msg}"),
            Self::WriteOutput(source) => write!(f, "failed to write CLI output: {source}"),
            Self::ReadContract { path, source } => {
                write!(f, "failed to read contract {}: {source}", path.display())
            }
            Self::ParseContract { path, source } => {
                write!(f, "failed to parse contract {}: {source}", path.display())
            }
            Self::Generate(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Usage(_) => None,
            Self::WriteOutput(source) => Some(source),
            Self::ReadContract { source, .. } => Some(source),
            Self::ParseContract { source, .. } => Some(source),
            Self::Generate(err) => Some(err),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::fs;
    use std::io;

    use tempfile::tempdir;

    use super::*;

    fn run_capture(args: &[&str]) -> (i32, String, String) {
        let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit_code = run_cli(&args, &mut stdout, &mut stderr);

        (
            exit_code,
            String::from_utf8(stdout).expect("stdout should be utf-8"),
            String::from_utf8(stderr).expect("stderr should be utf-8"),
        )
    }

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini.contract.json")
    }

    #[test]
    fn help_command_prints_usage() {
        let (exit_code, stdout, stderr) = run_capture(&["help"]);

        assert_eq!(exit_code, 0);
        assert_eq!(stderr, "");
        assert!(stdout.contains("teleport-cli <command> [options]"));
        assert!(stdout.contains("generate-ts"));
    }

    #[test]
    fn empty_invocation_returns_usage_error() {
        let (exit_code, stdout, stderr) = run_capture(&[]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains("Usage:"));
    }

    #[test]
    fn unknown_command_returns_usage_error() {
        let (exit_code, stdout, stderr) = run_capture(&["bogus"]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains("unknown command `bogus`"));
        assert!(stderr.contains("generate-ts"));
    }

    #[test]
    fn generate_ts_help_prints_subcommand_usage() {
        let (exit_code, stdout, stderr) = run_capture(&["generate-ts", "--help"]);

        assert_eq!(exit_code, 0);
        assert_eq!(stderr, "");
        assert_eq!(stdout, format!("{}\n", generate_ts_usage()));
    }

    #[test]
    fn generate_ts_requires_input_and_output_flags() {
        let (exit_code, stdout, stderr) = run_capture(&["generate-ts"]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains(&generate_ts_usage()));

        let (exit_code, stdout, stderr) = run_capture(&[
            "generate-ts",
            "--input",
            fixture_path()
                .to_str()
                .expect("fixture path should be utf-8"),
        ]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains(&generate_ts_usage()));
    }

    #[test]
    fn generate_ts_rejects_unknown_flags_and_missing_values() {
        let (exit_code, stdout, stderr) = run_capture(&["generate-ts", "--bogus"]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains("unknown flag `--bogus`"));

        let (exit_code, stdout, stderr) = run_capture(&["generate-ts", "--input"]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains("missing value for --input"));
    }

    #[test]
    fn generate_ts_writes_bindings_from_contract_fixture() {
        let tempdir = tempdir().expect("create tempdir");
        let output_dir = tempdir.path().join("generated");
        let output_dir_str = output_dir.to_str().expect("output path should be utf-8");
        let fixture_path = fixture_path();
        let fixture_path_str = fixture_path.to_str().expect("fixture path should be utf-8");

        let (exit_code, stdout, stderr) = run_capture(&[
            "generate-ts",
            "--input",
            fixture_path_str,
            "--output",
            output_dir_str,
            "--client-import",
            "~/rpc-client",
        ]);

        assert_eq!(exit_code, 0);
        assert_eq!(stderr, "");
        assert_eq!(
            stdout,
            format!(
                "Generated TypeScript bindings in {}\n",
                output_dir.display()
            )
        );

        let client = fs::read_to_string(output_dir.join("client.ts")).expect("read client.ts");
        assert!(client.contains("from \"~/rpc-client\""));
        assert!(client.contains("export function users_getUser"));

        let types = fs::read_to_string(output_dir.join("types.ts")).expect("read types.ts");
        assert!(types.contains("export type UserId = string;"));
        assert!(types.contains("export type User = { id: string; name: string };"));

        let errors = fs::read_to_string(output_dir.join("errors.ts")).expect("read errors.ts");
        assert!(errors.contains("import type { AppError } from \"~/rpc-client\";"));
        assert!(errors.contains("export type GetUserError = AppError<GetUserErrorDetail>;"));

        let index = fs::read_to_string(output_dir.join("index.ts")).expect("read index.ts");
        assert!(index.contains("export * from \"./types\";"));
        assert!(index.contains("export * from \"./errors\";"));
        assert!(index.contains("export * from \"./client\";"));
    }

    #[test]
    fn generate_ts_reports_missing_input_file() {
        let tempdir = tempdir().expect("create tempdir");
        let missing_input = tempdir.path().join("missing.contract.json");
        let output_dir = tempdir.path().join("generated");

        let (exit_code, stdout, stderr) = run_capture(&[
            "generate-ts",
            "--input",
            missing_input.to_str().expect("path should be utf-8"),
            "--output",
            output_dir.to_str().expect("path should be utf-8"),
        ]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains(&format!(
            "failed to read contract {}",
            missing_input.display()
        )));
    }

    #[test]
    fn generate_ts_reports_invalid_contract_json() {
        let tempdir = tempdir().expect("create tempdir");
        let invalid_contract = tempdir.path().join("invalid.contract.json");
        fs::write(&invalid_contract, "{ this is not valid json }").expect("write invalid contract");
        let output_dir = tempdir.path().join("generated");

        let (exit_code, stdout, stderr) = run_capture(&[
            "generate-ts",
            "--input",
            invalid_contract.to_str().expect("path should be utf-8"),
            "--output",
            output_dir.to_str().expect("path should be utf-8"),
        ]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains(&format!(
            "failed to parse contract {}",
            invalid_contract.display()
        )));
    }

    #[test]
    fn generate_ts_propagates_output_directory_errors() {
        let tempdir = tempdir().expect("create tempdir");
        let output_file = tempdir.path().join("not-a-directory");
        fs::write(&output_file, "occupied").expect("write output file");
        let fixture_path = fixture_path();

        let (exit_code, stdout, stderr) = run_capture(&[
            "generate-ts",
            "--input",
            fixture_path.to_str().expect("fixture path should be utf-8"),
            "--output",
            output_file.to_str().expect("output path should be utf-8"),
        ]);

        assert_eq!(exit_code, 1);
        assert_eq!(stdout, "");
        assert!(stderr.contains("failed to create output directory"));
    }

    #[test]
    fn stdout_write_failures_are_reported_to_stderr() {
        struct FailingWriter;

        impl io::Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("broken pipe"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let args = vec!["help".to_owned()];
        let mut stdout = FailingWriter;
        let mut stderr = Vec::new();

        let exit_code = run_cli(&args, &mut stdout, &mut stderr);

        assert_eq!(exit_code, 1);
        assert_eq!(
            String::from_utf8(stderr).expect("stderr should be utf-8"),
            "failed to write CLI output: broken pipe\n"
        );
    }
}
