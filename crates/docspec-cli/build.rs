//! Build script for `docspec-cli` crate.
//!
//! Generates man page and shell completions at build time using `clap_mangen` and `clap_complete`.

use std::env;
use std::fs;

include!("src/args.rs");

fn main() -> Result<(), Box<dyn core::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Generate man page
    let cmd = <Cli as clap::CommandFactory>::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buf = Vec::new();
    man.render(&mut buf)?;
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
