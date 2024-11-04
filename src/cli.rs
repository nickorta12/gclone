use std::str::FromStr;

use clap::Parser;
use clap_verbosity_flag::Verbosity;
use url::Url;

#[derive(Debug, Clone)]
pub struct TryUrl(Url);

fn validate_fragment(frag: &str) -> bool {
    match frag.split_once('/') {
        Some((parent, repo)) => !(parent.is_empty() || repo.is_empty()),
        None => false,
    }
}

impl FromStr for TryUrl {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Url::parse(s) {
            Ok(url) => Ok(Self(url)),
            Err(_) if validate_fragment(s) => {
                Url::parse(&format!("https://www.github.com/{}", s)).map(|x| x.into())
            }
            Err(err) => Err(err),
        }
    }
}

impl From<Url> for TryUrl {
    fn from(url: Url) -> Self {
        Self(url)
    }
}

impl From<TryUrl> for Url {
    fn from(try_url: TryUrl) -> Self {
        try_url.0
    }
}

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(flatten)]
    pub verbose: Verbosity,
    /// Override language to use
    #[clap(short, long)]
    pub lang: Option<String>,
    /// Override name of folder in symlink dir
    #[clap(short, long)]
    pub name: Option<String>,
    pub url: TryUrl,
    /// No auto generation of name and lang
    #[clap(short, long)]
    pub raw: bool,
}

pub fn args() -> Cli {
    Cli::parse()
}
