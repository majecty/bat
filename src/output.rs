use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use shell_words;

use app::PagingMode;
use errors::*;

pub enum OutputType {
    Pager(Child),
    Stdout(io::Stdout),
}

impl OutputType {
    pub fn from_mode(mode: PagingMode) -> Result<Self> {
        use self::PagingMode::*;
        Ok(match mode {
            Always => OutputType::try_pager(false)?,
            QuitIfOneScreen => OutputType::try_pager(true)?,
            _ => OutputType::stdout(),
        })
    }

    /// Try to launch the pager. Fall back to stdout in case of errors.
    fn try_pager(quit_if_one_screen: bool) -> Result<Self> {
        let pager = env::var("BAT_PAGER")
            .or_else(|_| env::var("PAGER"))
            .unwrap_or(String::from("less"));

        let pagerflags = shell_words::split(&pager)
            .chain_err(|| "Could not parse (BAT_)PAGER environment variable.")?;

        match pagerflags.split_first() {
            Some((pager_name, args)) => {
                let pager_path = PathBuf::from(pager_name);
                let is_less = pager_path.file_stem() == Some(&OsString::from("less"));

                let mut process = if is_less {
                    let mut p = Command::new(&pager_path);
                    if args.is_empty() {
                        p.args(vec!["--RAW-CONTROL-CHARS", "--no-init"]);
                        if quit_if_one_screen {
                            p.arg("--quit-if-one-screen");
                        }
                    }
                    p.env("LESSCHARSET", "UTF-8");
                    p
                } else {
                    Command::new(&pager_path)
                };

                Ok(process
                    .args(args)
                    .stdin(Stdio::piped())
                    .spawn()
                    .map(OutputType::Pager)
                    .unwrap_or_else(|_| OutputType::stdout()))
            }
            None => Ok(OutputType::stdout()),
        }
    }

    fn stdout() -> Self {
        OutputType::Stdout(io::stdout())
    }

    pub fn handle(&mut self) -> Result<&mut Write> {
        Ok(match *self {
            OutputType::Pager(ref mut command) => command
                .stdin
                .as_mut()
                .chain_err(|| "Could not open stdin for pager")?,
            OutputType::Stdout(ref mut handle) => handle,
        })
    }
}

impl Drop for OutputType {
    fn drop(&mut self) {
        if let OutputType::Pager(ref mut command) = *self {
            let _ = command.wait();
        }
    }
}
