use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::Parser;

#[derive(Parser)]
#[command(name = "bastion-ingest", about = "Convert PDF to Markdown")]
struct Cli {
    #[arg(long, value_name = "PDF")]
    input: PathBuf,

    #[arg(long, value_name = "DIR")]
    output_dir: PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Validate input.
    if !cli.input.exists() {
        eprintln!("error: input file not found: {}", cli.input.display());
        return ExitCode::from(3);
    }
    if cli.input.extension().map_or(true, |e| e != "pdf") {
        eprintln!("error: input must be a .pdf file");
        return ExitCode::from(3);
    }

    // Create output directory.
    if let Err(e) = std::fs::create_dir_all(&cli.output_dir) {
        eprintln!("error: cannot create output dir: {e}");
        return ExitCode::from(2);
    }

    // Copy PDF to output directory (skip if input is already in place).
    let dest_pdf = cli.output_dir.join("original.pdf");
    let already_in_place = cli.input.canonicalize().ok() == dest_pdf.canonicalize().ok();
    if !already_in_place {
        if let Err(e) = std::fs::copy(&cli.input, &dest_pdf) {
            eprintln!("error: cannot copy PDF: {e}");
            return ExitCode::from(2);
        }
    }

    // Convert via pdftotext.
    let output = Command::new("pdftotext")
        .args(["-layout", "-enc", "UTF-8"])
        .arg(&dest_pdf)
        .arg("-") // output to stdout
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: cannot run pdftotext: {e}");
            eprintln!("hint: install poppler-utils (apt/brew install poppler-utils)");
            return ExitCode::from(1);
        }
    };

    if !output.status.success() {
        eprintln!("error: pdftotext failed");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return ExitCode::from(1);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let page_count = text.chars().filter(|&c| c == '\x0C').count() + 1; // \f = form feed

    // Write markdown output.
    let filename = cli
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into());

    let md_content =
        format!("---\nsource: {filename}\nconverted_by: bastion-ingest\n---\n\n{text}");

    let output_path = cli.output_dir.join("original.md");
    if let Err(e) = std::fs::write(&output_path, md_content.as_bytes()) {
        eprintln!("error: cannot write output: {e}");
        return ExitCode::from(2);
    }

    // Print JSON result to stdout.
    let result = serde_json::json!({
        "output_path": output_path.to_string_lossy(),
        "page_count": page_count
    });
    println!("{result}");

    ExitCode::SUCCESS
}
