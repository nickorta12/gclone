use std::{error, fmt, fs, path::Path, process::Command};

use directories::{BaseDirs, ProjectDirs};
use log::debug;
use tokei::{LanguageType, Languages};
use url::{Host, Url};

mod cli;

fn main() -> eyre::Result<()> {
    let cli = cli::args();
    color_eyre::install()?;
    env_logger::builder()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    let project_dirs = ProjectDirs::from("", "", "gclone").unwrap();
    let base_dirs = BaseDirs::new().unwrap();
    let url: Url = cli.url.into();
    debug!("URL: {:#?}", url);

    let location = RepoLocation::from_url(url)?;
    debug!("Location: {:?}", location);

    let parent_dir = {
        let mut parent_dir = project_dirs.cache_dir().join(location.host);

        if let Some(parent) = location.parent {
            parent_dir = parent_dir.join(parent);
        }

        parent_dir
    };

    let repo_dir = parent_dir.join(location.repo);
    let symlink_dir = base_dirs.home_dir().join("repos");
    if repo_dir.exists() {
        if let Ok(true) =
            inquire::Confirm::new(&format!("{} already exists, reclone?", repo_dir.display()))
                .with_default(false)
                .prompt()
        {
            fs::remove_dir(&repo_dir)?;
        } else {
            symlink(&repo_dir, &symlink_dir, cli.lang, cli.name)?;
            return Ok(());
        }
    }
    fs::create_dir_all(&parent_dir)?;
    clone_repo(&location.url, &parent_dir, &repo_dir)?;
    symlink(&repo_dir, &symlink_dir, cli.lang, cli.name)?;

    Ok(())
}

fn clone_repo(url: &Url, parent_dir: &Path, repo_dir: &Path) -> eyre::Result<()> {
    println!("Cloning {} to {}", url.as_str(), repo_dir.display());
    let status = Command::new("git")
        .arg("clone")
        .arg(url.as_str())
        .arg(repo_dir)
        .current_dir(parent_dir)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(eyre::eyre!("git clone command failed"))
    }
}

fn get_top_language(repo_dir: &Path) -> String {
    let mut languages = Languages::new();
    languages.get_statistics(&[repo_dir], &[], &Default::default());

    let mut top_language_fallback: Option<LanguageType> = None;
    let mut top_fallback_code = 0;

    let mut top_language_type: Option<LanguageType> = None;
    let mut top_code = 0;
    for (language_type, language) in languages {
        debug!("Found language: {language_type}");
        let code = language.summarise().code;
        if code > top_code {
            if matches!(
                language_type,
                LanguageType::Json
                    | LanguageType::Yaml
                    | LanguageType::Html
                    | LanguageType::Css
                    | LanguageType::Markdown
            ) && code > top_fallback_code
            {
                top_language_fallback = Some(language_type);
                top_fallback_code = code;
            } else {
                top_language_type = Some(language_type);
                top_code = code;
            }
        }
    }

    if top_language_type.is_none() && top_language_fallback.is_some() {
        top_language_type = top_language_fallback;
    }

    top_language_type.map_or("other".to_string(), |t| {
        t.to_string()
            .to_lowercase()
            .replace(' ', "")
            .replace('_', "-")
            .replace("c#", "csharp")
    })
}

fn symlink(
    repo_dir: &Path,
    symlink_dir: &Path,
    lang: Option<String>,
    name: Option<String>,
) -> eyre::Result<()> {
    if !symlink_dir.exists() {
        fs::create_dir(&symlink_dir)?;
    }
    let lang = lang.unwrap_or_else(|| get_top_language(repo_dir));
    let lang_dir = symlink_dir.join(lang);
    if !lang_dir.exists() {
        fs::create_dir(&lang_dir)?;
    }

    let dest_dir = if let Some(name) = name {
        lang_dir.join(name)
    } else {
        lang_dir.join(
            repo_dir
                .file_name()
                .ok_or(eyre::eyre!("unable to get file name of repo"))?,
        )
    };
    if dest_dir.exists() {
        println!(
            "Repo {} already linked at {}",
            repo_dir.display(),
            dest_dir.display()
        );
        return Ok(());
    }
    println!("Linking {} to {}", repo_dir.display(), dest_dir.display());
    std::os::unix::fs::symlink(repo_dir, &dest_dir)?;

    Ok(())
}

#[derive(Debug, Clone)]
enum LocationPart {
    Parent,
    InvalidHost,
}

#[derive(Debug, Clone)]
struct LocationParsingError(LocationPart);

impl error::Error for LocationParsingError {}

impl fmt::Display for LocationParsingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            LocationPart::Parent => f.write_str("unable to parse parent portion of url"),
            LocationPart::InvalidHost => f.write_str("invalid hostname"),
        }
    }
}

impl LocationParsingError {
    fn parent() -> Self {
        Self(LocationPart::Parent)
    }

    fn host() -> Self {
        Self(LocationPart::InvalidHost)
    }
}

#[derive(Debug)]
struct RepoLocation {
    url: Url,
    parent: Option<String>,
    repo: String,
    host: String,
}

impl RepoLocation {
    fn from_url(url: Url) -> Result<Self, LocationParsingError> {
        let mut segments = url
            .path_segments()
            .ok_or_else(LocationParsingError::parent)?;

        struct Partial {
            parent: Option<String>,
            repo: String,
        }

        let partial = match segments.next() {
            Some(parent) => match segments.next() {
                Some(repo) => Ok(Partial {
                    parent: Some(parent.to_owned()),
                    repo: repo.to_owned(),
                }),
                None => Ok(Partial {
                    parent: None,
                    repo: parent.to_owned(),
                }),
            },
            None => Err(LocationParsingError::parent()),
        }?;

        match url.host() {
            Some(Host::Domain(host)) => Ok(Self {
                parent: partial.parent,
                repo: partial.repo,
                host: host.replace("www.", ""),
                url,
            }),
            Some(_) => Err(LocationParsingError::host()),
            None => Err(LocationParsingError::host()),
        }
    }
}
