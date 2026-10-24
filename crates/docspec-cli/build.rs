//! Build script for `docspec-cli` crate.
//!
//! Generates man page and shell completions at build time using `clap_mangen` and `clap_complete`.

use std::env;
use std::fs;

include!("src/args.rs");

/// The EXIT STATUS section of the man page. Keep in step with
/// `Cli::after_long_help` in `src/args.rs` and the README's "Exit status" table.
const EXIT_STATUS_ROFF: &str = r#".SH "EXIT STATUS"
.TP
.B 0
Success.
.TP
.B 2
Command used incorrectly: unknown flag or value, missing subcommand, input format not given and not detectable, input and output are the same file.
.TP
.B 65
The input document is malformed or holds something that cannot be converted (EX_DATAERR).
.TP
.B 66
The input file does not exist (EX_NOINPUT).
.TP
.B 69
The HTTP server cannot listen on the given address (EX_UNAVAILABLE).
.TP
.B 70
Internal DocSpec error (EX_SOFTWARE).
.TP
.B 71
The operating system refused a resource (EX_OSERR).
.TP
.B 73
The output file cannot be created (EX_CANTCREAT).
.TP
.B 74
I/O failed while reading or writing (EX_IOERR).
.TP
.B 77
Permission denied (EX_NOPERM).
.PP
Codes above 2 follow
.IR sysexits (3).
"#;

fn main() -> Result<(), Box<dyn core::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Generate man page
    // `--help` carries the exit list as after_long_help; the man page gets a
    // proper EXIT STATUS section below instead of clap_mangen's EXTRA copy.
    let cmd = <Cli as clap::CommandFactory>::command().after_long_help(None::<&str>);
    let man = clap_mangen::Man::new(cmd);
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    // clap_mangen has no exit-status section; append the conventional one.
    buf.extend_from_slice(EXIT_STATUS_ROFF.as_bytes());
    fs::write(out_dir.join("docspec.1"), buf)?;

    // Generate shell completions
    let mut cmd_completions = <Cli as clap::CommandFactory>::command();
    clap_complete::generate_to(
        clap_complete::Shell::Bash,
        &mut cmd_completions,
        "docspec",
        &out_dir,
    )?;
    clap_complete::generate_to(
        clap_complete::Shell::Zsh,
        &mut cmd_completions,
        "docspec",
        &out_dir,
    )?;
    clap_complete::generate_to(
        clap_complete::Shell::Fish,
        &mut cmd_completions,
        "docspec",
        &out_dir,
    )?;

    Ok(())
}
