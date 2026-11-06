//! Build script for `docspec-cli` crate.
//!
//! Generates man page and shell completions at build time using `clap_mangen` and `clap_complete`.

use std::env;
use std::fs;

include!("src/args.rs");

/// The EXIT STATUS section of the man page, rendered from [`EXIT_STATUSES`].
fn exit_status_roff() -> String {
    let rows = EXIT_STATUSES
        .iter()
        .map(|(code, name, meaning)| {
            let sysexit = if name.starts_with("EX_") {
                format!(" ({name})")
            } else {
                String::new()
            };
            format!(".TP\n.B {code}\n{meaning}{sysexit}.\n")
        })
        .collect::<Vec<_>>()
        .concat();
    format!(".SH \"EXIT STATUS\"\n{rows}.PP\nCodes above 2 follow\n.IR sysexits (3).\n")
}

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
    buf.extend_from_slice(exit_status_roff().as_bytes());
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
