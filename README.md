zula-core is the core module of the [zula shell](https://crates.io/crates/zula).
`zula-core` contains the core functionality of the zula shell, and is required for writing
plugins. This api is experimental, and may introduce breaking changes.

# Plugin Guide
To create a plugin, first initialize a library crate.
```bash
cargo new my_plugin --lib
```
Set the crate type to `cdylib`, and add `zula-core` as a dependency.
```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
zula-core = "3.0.2"
```
Import the [`Plugin`] trait and implement it on your plugin type.
```rust
use zula-core::{Plugin, ShellState};
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
        println!("Hello, plugin!")
    }
}
```
Run `cargo build --release` to build your plugin. The library file should be in `target/release/lib<name>.so`. This is the file that you'll put in your plugins folder.

Thats it! Run `zula cfg` inside zula to check that its loaded, and run `plugin.<name>` to use it. Due to weird ownership relationships, `call` has to take a raw pointer, so use it responsibly.
