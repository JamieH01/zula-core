use std::{
    collections::HashMap,
    env,
    error::Error,
    fmt::Display,
    io::{self, stdin, stdout, ErrorKind, Stdin, Stdout},
    ops::Deref,
    process::Command,
};

use termion::raw::{IntoRawMode, RawTerminal};

pub struct ShellState {
    cwd: String,
    pub header: fn(state: &ShellState) -> String,
    pub history: Vec<String>,
    pub config: Config,

    pub stdin: Stdin,
    pub stdout: RawTerminal<Stdout>,
}
pub struct Config {
    pub aliases: HashMap<String, String>,
    pub hotkeys: HashMap<char, String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
            hotkeys: HashMap::new(),
        }
    }
}

impl ShellState {
    pub fn new() -> Result<Self, ZulaError> {
        let cwd = match dirs::home_dir().map(|s| s.to_string_lossy().to_string()) {
            Some(s) => s,
            None => env::current_dir()?.to_string_lossy().to_string(),
        };

        Ok(Self {
            cwd,
            header: {
                |state| {
                    format!(
                        "\x1b[38;5;93mzula\x1b[38;5;5m @ \x1b[38;5;93m{} \x1b[0m-> ",
                        state.get_cwd()
                    )
                }
            },
            config: Config::new(),
            history: vec![],

            stdin: stdin(),
            stdout: stdout().into_raw_mode()?,
        })
    }

    pub fn get_cwd(&self) -> &str {
        &self.cwd
    }
    pub fn set_cwd(&mut self, path: &str) -> Result<(), ZulaError> {
        env::set_current_dir(path).map_err(|_| ZulaError::InvalidDir)?;
        self.cwd = env::current_dir().map(|s| s.to_string_lossy().to_string())?;
        Ok(())
    }

    pub fn get_header(&self) -> String {
        let mut head = (self.header)(self);
        head.push_str("\x1b[0m");
        head
    }

    pub fn exec(
        &mut self,
        cmd: impl AsRef<str>,
        args: &[impl AsRef<str>],
    ) -> Result<(), ZulaError> {
        if cmd.as_ref() == "cd" {
            match args.get(0) {
                Some(targ) => return self.set_cwd(targ.as_ref()),
                None => return Err(ZulaError::CommandEmpty),
            }
        }

        let mut exec = Command::new(cmd.as_ref());

        for e in args {
            exec.arg(e.as_ref());
        }

        let init = exec.spawn();

        let mut proc = match init {
            Ok(c) => c,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                { Err(ZulaError::InvalidCmd(cmd.as_ref().to_owned())) }?
            }
            Err(e) => { Err(Into::<ZulaError>::into(e)) }?,
        };
        proc.wait()?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum ZulaError {
    Io(io::Error),
    InvalidCmd(String),
    CommandEmpty,
    InvalidDir,
    RecursiveAlias,
    Opaque(Box<dyn Error>),
}

impl From<io::Error> for ZulaError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<Box<dyn Error>> for ZulaError {
    fn from(value: Box<(dyn std::error::Error + 'static)>) -> Self {
        Self::Opaque(value)
    }
}

impl Display for ZulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => {
                write!(f, "io error: {e}\r\n")
            }
            Self::InvalidCmd(cmd) => write!(f, "unknown command: {cmd}\r\n"),
            Self::CommandEmpty => write!(f, "command not given\r\n"),
            Self::InvalidDir => write!(f, "directory does not exist\r\n"),
            Self::RecursiveAlias => write!(f, "recursive alias called\r\n"),
            Self::Opaque(e) => write!(f, "external error: {e}\r\n"),
        }
    }
}

impl Error for ZulaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        #[allow(unreachable_patterns)]
        match self {
            Self::Io(e) => Some(e),
            Self::Opaque(e) => Some(e.deref()),
            _ => None,
        }
    }
}
