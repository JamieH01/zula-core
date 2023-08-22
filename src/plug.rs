use std::{ops::{DerefMut, Deref}, path::Path, ffi::OsStr};

use libloading::Library;

use crate::ShellState;

pub trait Plugin {
    fn init(&self) -> Box<dyn Plugin>;
    fn name(&self) -> &str;
    unsafe fn call(&self, state: *mut ShellState) {}
}

pub struct PluginHook {
    pub hook: libloading::Library,
    pub obj: Box<dyn Plugin>,
    pub path: String,
}

impl Deref for PluginHook {
    type Target = Box<dyn Plugin>;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl PluginHook {
    pub unsafe fn new<A: AsRef<OsStr>>(path:A) -> Result<Self, libloading::Error> {
        let str_path = OsStr::new(&path).to_str().map(|s| s.to_owned()).unwrap_or("".to_owned());
        let hook =  Library::new(path)?;
        let obj = hook.get::<libloading::Symbol<fn() -> Box<dyn Plugin>>>(b"init")?();
        Ok(Self { hook, obj, path:str_path})

    }
}


