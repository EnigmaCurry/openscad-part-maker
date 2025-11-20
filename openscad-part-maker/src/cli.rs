use clap::{Arg, Command};

pub fn app() -> Command {
    Command::new("openscad-part-maker")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("log")
                .long("log")
                .global(true)
                .num_args(1)
                .value_name("LEVEL")
                .value_parser(["trace", "debug", "info", "warn", "error"])
                .help("Sets the log level, overriding the RUST_LOG environment variable."),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .global(true)
                .help("Sets the log level to debug.")
                .action(clap::ArgAction::SetTrue),
        )
        .subcommand(
            Command::new("hello")
                .about("Greeting")
                .arg(Arg::new("NAME").default_value("Bob")),
        )
        .subcommand(
            Command::new("completions")
                .about("Generates shell completions script (tab completion)")
                .arg(
                    Arg::new("shell")
                        .help("The shell to generate completions for")
                        .required(false)
                        .value_parser(["bash", "zsh", "fish"]),
                ),
        )
        .subcommand(
            Command::new("serve")
                .about("Run the HTTP API server")
                .arg(
                    Arg::new("listen")
                        .long("listen")
                        .value_name("ADDR")
                        .default_value("127.0.0.1:3000")
                        .help("Address to bind the HTTP server to"),
                )
                .arg(
                    Arg::new("input-scad")
                        .long("input-scad")
                        .value_name("PATH")
                        .required(true)
                        .help("Path to input.scad template file"),
                ),
        )
}
