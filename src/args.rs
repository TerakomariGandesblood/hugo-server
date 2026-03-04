use std::io;
use std::net::Ipv4Addr;
use std::path::PathBuf;

use anyhow::Result;
use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Style};
use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::Generator;
use clap_verbosity_flag::Verbosity;
use supports_color::Stream;
use url::Url;

#[derive(Parser)]
#[command(version, about, long_about = None, styles = get_styles())]
pub struct Args {
    /// The Git URL for the Hugo project
    pub repo_url: Url,

    /// Path where the Hugo project is cloned into
    #[arg(long, default_value = String::from("web"))]
    pub repo_dst: PathBuf,

    /// Listening Address
    #[arg(long, default_value_t = Ipv4Addr::new(127, 0, 0, 1))]
    pub host: Ipv4Addr,

    /// Listening port
    #[arg(long, default_value_t = 8001)]
    pub port: u16,

    /// Generate shell completion to standard output
    #[arg(long, value_enum)]
    pub completion: Option<Shell>,

    #[command(flatten)]
    pub verbose: Verbosity,
}

const HEADER: Style = AnsiColor::Green.on_default().bold();
const USAGE: Style = AnsiColor::Green.on_default().bold();
const LITERAL: Style = AnsiColor::Cyan.on_default().bold();
const PLACEHOLDER: Style = AnsiColor::Cyan.on_default();
const ERROR: Style = AnsiColor::Red.on_default().bold();
const VALID: Style = AnsiColor::Cyan.on_default().bold();
const INVALID: Style = AnsiColor::Yellow.on_default().bold();
const HELP_STYLES: Styles = Styles::styled()
    .header(HEADER)
    .usage(USAGE)
    .literal(LITERAL)
    .placeholder(PLACEHOLDER)
    .error(ERROR)
    .valid(VALID)
    .invalid(INVALID);

fn get_styles() -> Styles {
    if supports_color::on(Stream::Stdout).is_some() {
        HELP_STYLES
    } else {
        Styles::plain()
    }
}

#[must_use]
#[derive(Clone, ValueEnum)]
pub enum Shell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
    Nushell,
}

impl Shell {
    fn to_clap_type(&self) -> Box<dyn Generator> {
        match self {
            Self::Bash => Box::new(clap_complete::Shell::Bash),
            Self::Elvish => Box::new(clap_complete::Shell::Elvish),
            Self::Fish => Box::new(clap_complete::Shell::Fish),
            Self::PowerShell => Box::new(clap_complete::Shell::PowerShell),
            Self::Zsh => Box::new(clap_complete::Shell::Zsh),
            Self::Nushell => Box::new(clap_complete_nushell::Nushell),
        }
    }
}

pub fn generate_completion(shell: Shell) -> Result<()> {
    let mut cmd = Args::command();
    let bin_name = cmd.get_name().to_string();

    cmd.set_bin_name(bin_name);
    cmd.build();

    shell.to_clap_type().generate(&cmd, &mut io::stdout());

    Ok(())
}
