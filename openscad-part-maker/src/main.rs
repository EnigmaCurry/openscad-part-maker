use std::io::Write;
use std::net::SocketAddr;

use clap::ArgMatches;
use clap_complete::shells::Shell;

mod cli;
mod prelude;
mod scad_params;
mod server;

use prelude::*;

fn main() {
    let cmd = cli::app();
    let matches = cmd.clone().get_matches();

    // Configure logging:
    let log_level = determine_log_level(&matches, std::env::var("RUST_LOG").ok());
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::from_str(&log_level).unwrap_or(log::LevelFilter::Info))
        .format_timestamp(None)
        .init();
    debug!("logging initialized.");

    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    let exit_code = run_once_with_serve(
        cmd,
        matches,
        |m| run_server_from_matches(m),
        &mut stdout,
        &mut stderr,
    );
    std::process::exit(exit_code);
}

fn generate_completion_script_to(shell: clap_complete::shells::Shell, out: &mut dyn Write) {
    clap_complete::generate(shell, &mut cli::app(), env!("CARGO_BIN_NAME"), out)
}

fn run_server_from_matches(sub_matches: &ArgMatches) -> anyhow::Result<()> {
    let addr_str = sub_matches
        .get_one::<String>("listen")
        .expect("listen has default");
    let addr: SocketAddr = addr_str.parse()?;

    let tile_scad_path = sub_matches
        .get_one::<String>("input-scad")
        .expect("required")
        .into();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(server::run(addr, tile_scad_path))
}

/// Decide the effective log level using the same precedence as main():
/// 1) --verbose forces debug
/// 2) --log LEVEL
/// 3) RUST_LOG env var
/// 4) fallback "info"
fn determine_log_level(matches: &ArgMatches, env_rust_log: Option<String>) -> String {
    let log_level = if matches.get_flag("verbose") {
        Some("debug".to_string())
    } else {
        matches.get_one::<String>("log").cloned()
    };
    let log_level = log_level.or(env_rust_log);
    log_level.unwrap_or_else(|| "info".to_string())
}

/// Testable core of main() that doesn't touch global logger or exit().
/// `serve_fn` is injected so unit tests can cover serve ok/error paths.
fn run_once_with_serve<F>(
    mut cmd: clap::Command,
    matches: clap::ArgMatches,
    serve_fn: F,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32
where
    F: Fn(&ArgMatches) -> anyhow::Result<()>,
{
    // Print help if no subcommand is given:
    if matches.subcommand_name().is_none() {
        let help = cmd.render_help().to_string();
        let _ = writeln!(stdout, "{help}");
        return 0;
    }

    // Handle the subcommands:
    let _ = writeln!(stderr, "");
    let exit_code = match matches.subcommand() {
        Some(("hello", sub_matches)) => {
            let name = sub_matches.get_one::<String>("NAME").unwrap();
            let _ = writeln!(stdout, "Hello, {name}!");
            0
        }
        Some(("completions", sub_matches)) => {
            if let Some(shell) = sub_matches.get_one::<String>("shell") {
                match shell.as_str() {
                    "bash" => generate_completion_script_to(Shell::Bash, stdout),
                    "zsh" => generate_completion_script_to(Shell::Zsh, stdout),
                    "fish" => generate_completion_script_to(Shell::Fish, stdout),
                    shell => {
                        let _ = writeln!(stderr, "Unsupported shell: {shell}");
                    }
                }
                0
            } else {
                let _ = writeln!(
                    stderr,
                    "### Instructions to enable tab completion for {}",
                    env!("CARGO_BIN_NAME")
                );
                let _ = writeln!(stderr, "");
                let _ = writeln!(stderr, "### Bash (put this in ~/.bashrc:)");
                let _ = writeln!(
                    stderr,
                    "  source <({} completions bash)",
                    env!("CARGO_BIN_NAME")
                );
                let _ = writeln!(stderr, "");
                let _ = writeln!(stderr, "### To make an alias (eg. 'h'), add this too:");
                let _ = writeln!(stderr, "  alias h={}", env!("CARGO_BIN_NAME"));
                let _ = writeln!(
                    stderr,
                    "  complete -F _{} -o bashdefault -o default h",
                    env!("CARGO_BIN_NAME")
                );
                let _ = writeln!(stderr, "");
                let _ = writeln!(
                    stderr,
                    "### If you don't use Bash, you can also use Fish or Zsh:"
                );
                let _ = writeln!(stderr, "### Fish (put this in ~/.config/fish/config.fish");
                let _ = writeln!(
                    stderr,
                    "  {} completions fish | source)",
                    env!("CARGO_BIN_NAME")
                );
                let _ = writeln!(stderr, "### Zsh (put this in ~/.zshrc)");
                let _ = writeln!(
                    stderr,
                    "  autoload -U compinit; compinit; source <({} completions zsh)",
                    env!("CARGO_BIN_NAME")
                );
                1
            }
        }
        Some(("serve", sub_matches)) => {
            if let Err(err) = serve_fn(sub_matches) {
                let _ = writeln!(stderr, "Server error: {err:?}");
                1
            } else {
                0
            }
        }
        _ => 1,
    };

    let _ = writeln!(stderr, "");
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_log_level_precedence_verbose_wins() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker", "-v", "hello"])
            .unwrap();
        let lvl = determine_log_level(&matches, Some("warn".into()));
        assert_eq!(lvl, "debug");
    }

    #[test]
    fn determine_log_level_uses_log_flag_over_env() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker", "--log", "error", "hello"])
            .unwrap();
        let lvl = determine_log_level(&matches, Some("warn".into()));
        assert_eq!(lvl, "error");
    }

    #[test]
    fn determine_log_level_uses_env_when_no_flags() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker", "hello"])
            .unwrap();
        let lvl = determine_log_level(&matches, Some("trace".into()));
        assert_eq!(lvl, "trace");
    }

    #[test]
    fn determine_log_level_defaults_to_info() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker", "hello"])
            .unwrap();
        let lvl = determine_log_level(&matches, None);
        assert_eq!(lvl, "info");
    }

    #[test]
    fn run_once_no_subcommand_prints_help_and_exits_zero() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker"])
            .unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_once_with_serve(cmd, matches, |_| Ok(()), &mut out, &mut err);
        assert_eq!(code, 0);
        let out_s = String::from_utf8(out).unwrap();
        assert!(out_s.contains("openscad-part-maker"));
        assert!(out_s.contains("serve"));
    }

    #[test]
    fn run_once_hello_defaults_bob() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker", "hello"])
            .unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_once_with_serve(cmd, matches, |_| Ok(()), &mut out, &mut err);
        assert_eq!(code, 0);
        assert_eq!(String::from_utf8(out).unwrap().trim(), "Hello, Bob!");
    }

    #[test]
    fn run_once_hello_custom_name() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker", "hello", "Ryan"])
            .unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_once_with_serve(cmd, matches, |_| Ok(()), &mut out, &mut err);
        assert_eq!(code, 0);
        assert!(String::from_utf8(out).unwrap().contains("Hello, Ryan!"));
    }

    #[test]
    fn run_once_completions_without_shell_exits_one_and_prints_instructions() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker", "completions"])
            .unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_once_with_serve(cmd, matches, |_| Ok(()), &mut out, &mut err);
        assert_eq!(code, 1);
        let err_s = String::from_utf8(err).unwrap();
        assert!(err_s.contains("Instructions to enable tab completion"));
        assert!(err_s.contains("Bash"));
    }

    #[test]
    fn run_once_completions_with_shell_exits_zero() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from(["openscad-part-maker", "completions", "bash"])
            .unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_once_with_serve(cmd, matches, |_| Ok(()), &mut out, &mut err);
        assert_eq!(code, 0);
        // output is a completion script; just check it's non-empty
        assert!(!out.is_empty());
    }

    #[test]
    fn run_once_serve_success_path_exits_zero() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from([
                "openscad-part-maker",
                "serve",
                "--input-scad",
                "/tmp/input.scad",
            ])
            .unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_once_with_serve(cmd, matches, |_| Ok(()), &mut out, &mut err);
        assert_eq!(code, 0);
    }

    #[test]
    fn run_once_serve_error_path_exits_one_and_reports() {
        let cmd = cli::app();
        let matches = cmd
            .clone()
            .try_get_matches_from([
                "openscad-part-maker",
                "serve",
                "--input-scad",
                "/tmp/input.scad",
            ])
            .unwrap();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_once_with_serve(cmd, matches, |_| anyhow::bail!("boom"), &mut out, &mut err);
        assert_eq!(code, 1);
        let err_s = String::from_utf8(err).unwrap();
        assert!(err_s.contains("Server error"));
        assert!(err_s.contains("boom"));
    }
}
