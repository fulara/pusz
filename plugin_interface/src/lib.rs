#[derive(Debug)]
pub enum PluginResultEntry {
    Clip{label : String, content : String},
}

#[derive(Debug)]
pub enum PluginResult {
    None,
    Error(String),
    Ok(Vec<PluginResultEntry>),
}

pub trait Plugin : ::std::fmt::Debug {
    fn query(&mut self, query : &str) -> PluginResult;
}

pub const COMMON_INTERFACE_VERSION : &'static str = "";
pub type LoadFn = extern "C" fn(&str) -> Result<Box<dyn Plugin>, String>;

#[no_mangle]
extern "C" fn load_example(plugin_interface_version : &str) -> Result<Box<dyn Plugin>, String> {
    if plugin_interface_version != COMMON_INTERFACE_VERSION {
        Err("unsupported common interface, update your plugin!".to_string())
    } else {
        panic!("this is example, cant load this.");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
