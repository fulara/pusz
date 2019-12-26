#[macro_use]
extern crate derive_builder;

#[derive(PartialEq, Debug, Clone)]
pub struct PuszClipEntry {
    pub label : String,
    pub content : String,
}

#[derive(Debug, Clone)]
pub struct PuszActionEntry {
    pub label : String,
//    pub action_context : Box<dyn std::any::Any>,
}

impl ::std::cmp::PartialEq for PuszActionEntry {
    fn eq(&self, other: &Self) -> bool {
        self.label.eq(&other.label)
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum PuszEntry {
    Display(PuszClipEntry),
    Action(PuszActionEntry),
}

//temporary until main api adopts api.
#[derive(PartialEq, Clone, Debug)]
pub enum PuszRowIdentifier {
    MainApi,
    Plugin(&'static str),
}

#[derive(PartialEq, Builder, Clone, Debug)]
pub struct PuszRow {
    pub main_entry : PuszClipEntry,
    #[builder(default)]
    pub additional_entries : Vec<PuszEntry>,

    pub identifier : PuszRowIdentifier,

    #[builder(default)]
    pub is_removable : bool,
}

impl PuszRowBuilder {
    pub fn new(content : String, identifier : PuszRowIdentifier) -> PuszRowBuilder {
        PuszRowBuilder {
            main_entry : Some(PuszClipEntry {
                label : content.clone(),
                content,
            }),
            additional_entries : None,
            identifier : Some(identifier),

            is_removable : None,

        }
    }
}


#[derive(PartialEq, Debug)]
pub enum PluginResult {
    None,
    Error(String),
    Ok(Vec<PuszRow>),
}

#[derive(PartialEq, Debug)]
pub struct PluginSettings {
    pub requies_explicit_query : bool,
    pub interested_in_clipboard : bool,
}

#[derive(PartialEq, Debug)]
// should this be renamed to voluntary/subscribed event?
pub enum PluginEvent {
    Clipboard(String), // images'n stuff in the future
}

pub trait Plugin : ::std::fmt::Debug {
    fn query(&mut self, query : &str) -> PluginResult;
    //invoked on pressing return
    fn action_request(&mut self, query : &str) -> PluginResult {
        self.query(query)
    }
    fn name(&self) -> &'static str;

    fn id(&self) -> PuszRowIdentifier {
        PuszRowIdentifier::Plugin(self.name())
    }

    fn settings(&self) -> PluginSettings {
        PluginSettings {
            requies_explicit_query : true,
            interested_in_clipboard : false,
        }
    }

    fn on_subscribed_event(&mut self, _event : &PluginEvent) {
        ()
    }
}

pub const COMMON_INTERFACE_VERSION : &'static str = "1";
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
