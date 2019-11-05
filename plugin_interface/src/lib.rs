#[derive(PartialEq, Debug)]
pub struct PuszClipEntry {
    pub label : String,
    pub content : String,
}

#[derive(Debug)]
pub struct PuszActionEntry {
    pub label : String,
    pub action_context : Box<dyn std::any::Any>,
}

impl ::std::cmp::PartialEq for PuszActionEntry {
    fn eq(&self, other: &Self) -> bool {
        self.label.eq(&other.label)
    }
}

#[derive(PartialEq, Debug)]
pub enum PuszEntry {
    Display(PuszClipEntry),
    Action(PuszActionEntry),
}

#[derive(PartialEq, Debug)]
pub struct PuszRow {
    pub main_entry : PuszClipEntry,
    pub additional_entries : Vec<PuszEntry>,
}

#[derive(PartialEq, Debug)]
pub enum PluginResult {
    None,
    Error(String),
    Ok(Vec<PuszRow>),
}

pub trait Plugin : ::std::fmt::Debug {
    fn query(&mut self, query : &str) -> PluginResult;
    fn name(&self) -> &'static str;
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
