#![doc = r#"`zula-core` contains the core functionality of the zula shell, and is required for writing
plugins. This api is experimental, and may introduce breaking changes.

# Plugin Guide
To create a plugin, first initialize a library crate.
```sh
cargo new my_plugin --lib
```
Set the crate type to `cdylib`, and add `zula-core` as a dependency.
```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
zula-core = "4.0.0"
```
Import the [`Plugin`] trait and implement it on your plugin type.
```
use zula_core::{Plugin, ShellState};
use std::error::Error;

pub struct MyPlugin;

impl Plugin for MyPlugin {
    //since this function is called across abi boundaries, its important to include no_mangle so
    //that rustc leaves the symbol as-is and can be called properly.
    #[no_mangle]
    fn init(&self) -> Box<dyn Plugin> {
        Box::new(Self)
    }
    fn name(&self) -> &str {
        "my_plugin"
    }
    fn call(&self, state: *mut ShellState) -> Result<(), Box<dyn Error>> {
        println!("Hello, plugin!");
        Ok(())
    }
}
```
Run `cargo build --release` to build your plugin. The library file should be in `target/release/lib<name>.so`. This is the file that you'll put in your plugins folder.

Thats it! Run `zula cfg` inside zula to check that its loaded, and run `plugin.<name>` to use it. Due to weird ownership relationships, `call` has to take a raw pointer, so use it responsibly.
"#]

use std::{
    collections::HashMap,
    env,
    error::Error,
    ffi::OsStr,
    fmt::Display,
    io::{self, stdin, stdout, ErrorKind, Stdin, Stdout},
    ops::Deref,
    process::Command,
};

use termion::raw::{IntoRawMode, RawTerminal};

mod plug;
pub use plug::{Plugin, PluginHook};

#[repr(C)]
///The core shell state object. This api is WIP, and may become more locked down in the future.
pub struct ShellState {
    cwd: String,
    pub header: fn(state: &ShellState) -> String,
    pub history: Vec<String>,
    pub config: Config,

    pub stdin: Stdin,
    pub stdout: RawTerminal<Stdout>,
}
///Holds configuration info.
pub struct Config {
    pub aliases: HashMap<String, String>,
    pub hotkeys: HashMap<char, String>,
    plugins: HashMap<String, PluginHook>,
    pub safety: bool, 
}


impl Config {
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
            hotkeys: HashMap::new(),
            plugins: HashMap::new(),
            safety: false
        }
    }
}

impl ShellState {
    ///Initializes a new shell. Do not use this if making plugins.
    pub fn new() -> Result<Self, ZulaError> {
        let cwd = env::current_dir()?.to_string_lossy().to_string();

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

    ///Get the current working directory of the shell.
    pub fn get_cwd(&self) -> &str {
        &self.cwd
    }
    ///Set the current working directory of the shell. Will error if the path is not found.
    pub fn set_cwd(&mut self, path: &str) -> Result<(), ZulaError> {
        env::set_current_dir(path).map_err(|_| ZulaError::InvalidDir)?;
        self.cwd = env::current_dir().map(|s| s.to_string_lossy().to_string())?;
        Ok(())
    }

    ///Returns the header or "status bar."
    pub fn get_header(&self) -> String {
        let mut head = (self.header)(self);
        head.push_str("\x1b[0m");
        head
    }

    ///Execute a command. Does no proccessing such as aliases, chaining, and quoting.
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
    ///Attempt to load a plugin from a path.
    pub fn load_plugin(&mut self, path: impl AsRef<OsStr>) -> Result<(), libloading::Error> {
        let plug = unsafe { PluginHook::new(path) }?;
        self.config.plugins.insert(plug.name().to_owned(), plug);
        Ok(())
    }
    ///Returns a hook to the given plugin if it exists.
    pub fn plugin_lookup(&self, name: &str) -> Result<&PluginHook, ZulaError> {
        self.config
            .plugins
            .get(name)
            .ok_or(ZulaError::InvalidPlugin)
    }
    ///Returns an iterator over the currently loaded plugin names.
    pub fn plugin_names(&self) -> std::collections::hash_map::Keys<'_, String, PluginHook> {
        self.config.plugins.keys()
    }
}

#[derive(Debug)]
///The zula shell error type. All errors can be converted to the `Opaque` variant.
pub enum ZulaError {
    Io(io::Error),
    InvalidCmd(String),
    CommandEmpty,
    InvalidDir,
    RecursiveAlias,
    InvalidPlugin,
    LibErr(libloading::Error),
    Opaque(Box<dyn Error + Send + Sync>),
}

impl From<io::Error> for ZulaError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<libloading::Error> for ZulaError {
    fn from(value: libloading::Error) -> Self {
        Self::LibErr(value)
    }
}
impl From<Box<dyn Error + Send + Sync>> for ZulaError {
    fn from(value: Box<(dyn std::error::Error + Send + Sync + 'static)>) -> Self {
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
            Self::InvalidPlugin => write!(f, "plugin not found\r\n"),
            Self::LibErr(e) => write!(f, "lib error: {e}\r\n"),
            Self::Opaque(e) => write!(f, "external error: {e}\r\n"),
        }
    }
}

impl Error for ZulaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        #[allow(unreachable_patterns)]
        match self {
            Self::Io(e) => Some(e),
            Self::LibErr(e) => Some(e),
            Self::Opaque(e) => Some(e.deref()),
            _ => None,
        }
    }
}
