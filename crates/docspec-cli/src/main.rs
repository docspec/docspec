#![warn(missing_docs)]
//! `docspec` CLI entry point. Dispatches to subcommand handlers.

mod args;
mod commands;
mod conversions;
mod error;
mod format;

use std::io::{IsTerminal as _, Write as _};
use std::process::ExitCode;

use clap::Parser as _;

use crate::args::{Cli, ColorChoice, Commands};
use crate::error::CliError;

/// Main entry point.
fn main() -> ExitCode {
    let cli = Cli::parse();
    let color = command_color(&cli.command);

    // CRITICAL: Sentry guard MUST live for the entire process lifetime.
    // Initialized only for the http subcommand; convert stays silent.
    #[cfg(feature = "http")]
    let _telemetry_guard = match &cli.command {
        Commands::Http(_) => Some(docspec_http::init_telemetry()),
        _ => None,
    };

    let result = match cli.command {
        Commands::Convert(args) => commands::convert::run(args),
        #[cfg(feature = "http")]
        Commands::Http(args) => commands::http::run(args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            write_error(&err, color);
            ExitCode::FAILURE
        }
    }
}

fn command_color(command: &Commands) -> ColorChoice {
    match command {
        Commands::Convert(args) => args.color,
        #[cfg(feature = "http")]
        Commands::Http(_) => ColorChoice::Auto,
    }
}

fn write_error(err: &CliError, color: ColorChoice) {
    let use_color = if std::env::var("NO_COLOR").is_ok() {
        false
    } else {
        match color {
            ColorChoice::Always => true,
            ColorChoice::Auto => std::io::stderr().is_terminal(),
            ColorChoice::Never => false,
        }
    };
    let msg = if use_color {
        format!("\x1b[1;31merror:\x1b[0m {err}\n")
    } else {
        format!("error: {err}\n")
    };
    let write_result = std::io::stderr().write_all(msg.as_bytes());
    drop(write_result);
}
