use std::{
    error::Error,
    ffi::OsStr,
    ops::{Deref, DerefMut},
    path::Path, mem::ManuallyDrop,
};

use libloading::Library;

use crate::ShellState;

///The plugin trait that defines how a plugin object acts.
pub trait Plugin {
    ///The initializer function. Don't mind the `&self` parameter, its a technicallity for this
    ///trait to be abi-safe.
    fn init(&self) -> Box<dyn Plugin>;
    ///Return the display name of your plugin. Due to abi restrictions, this must be a function and
    ///not an associated constant.
    fn name(&self) -> &str;
    ///The "heart" of the plugin; this is called with the syntax `plugin.<name>`.
    fn call(&self, _state: *mut ShellState) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

///Represents a plugin object. Not very useful outside of internal functions.
pub struct PluginHook {
    hook: libloading::Library,
    obj: ManuallyDrop<Box<dyn Plugin>>,
    path: String,
}

impl Deref for PluginHook {
    type Target = Box<dyn Plugin>;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl PluginHook {
    pub unsafe fn new<S: AsRef<OsStr>>(path: S) -> Result<Self, libloading::Error> {
        let str_path = OsStr::new(&path)
            .to_str()
            .map(|s| s.to_owned())
            .unwrap_or("".to_owned());
        let hook = Library::new(path)?;
        let obj = hook.get::<libloading::Symbol<fn() -> Box<dyn Plugin>>>(b"init")?();
        Ok(Self {
            hook,
            obj: ManuallyDrop::new(obj),
            path: str_path,
        })
    }
}

impl Drop for PluginHook {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.obj) };
    }
}

#[cfg(test)]
mod tests {
    use crate::PluginHook;

    #[test]
    fn drop() {
        let hook = unsafe { PluginHook::new("/home/jamie/.config/zula/plugins/libtest_plugin.so") }.unwrap();
    }
}
