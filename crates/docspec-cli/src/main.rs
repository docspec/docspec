#![warn(missing_docs)]
//! `docspec` CLI entry point. Dispatches to subcommand handlers.

mod args;
mod commands;
mod conversions;
mod error;
mod format;

use std::ffi::OsStr;
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

/// Explicit `always`/`never` win; `auto` colors only on a TTY with `NO_COLOR`
/// unset or empty (<https://no-color.org>).
fn use_color(choice: ColorChoice, no_color: Option<&OsStr>, stderr_is_tty: bool) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => stderr_is_tty && no_color.is_none_or(OsStr::is_empty),
    }
}

fn write_error(err: &CliError, color: ColorChoice) {
    let use_color = use_color(
        color,
        std::env::var_os("NO_COLOR").as_deref(),
        std::io::stderr().is_terminal(),
    );
    let msg = if use_color {
        format!("\x1b[1;31merror:\x1b[0m {err}\n")
    } else {
        format!("error: {err}\n")
    };
    let write_result = std::io::stderr().write_all(msg.as_bytes());
    drop(write_result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_color_matrix() {
        use ColorChoice::{Always, Auto, Never};
        let cases: &[(ColorChoice, Option<&str>, bool, bool)] = &[
            (Always, None, false, true),
            (Always, Some(""), false, true),
            (Always, Some("1"), false, true),
            (Never, None, true, false),
            (Never, Some(""), true, false),
            (Never, Some("1"), true, false),
            (Auto, None, true, true),
            (Auto, Some(""), true, true),
            (Auto, Some("1"), true, false),
            (Auto, Some("0"), true, false),
            (Auto, None, false, false),
            (Auto, Some(""), false, false),
        ];
        for &(choice, no_color, tty, expected) in cases {
            assert_eq!(
                use_color(choice, no_color.map(OsStr::new), tty),
                expected,
                "{choice:?} NO_COLOR={no_color:?} tty={tty}"
            );
        }
    }
}
