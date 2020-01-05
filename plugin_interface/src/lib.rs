#[macro_use]
extern crate derive_builder;

#[macro_use]
extern crate maplit;

use std::any::Any;
use std::collections::BTreeMap;

#[derive(PartialEq, Clone, Debug)]
pub struct PuszRowIdentifier {
    pub plugin_id : &'static str,

    pub identifier :  String,
    pub data : Option<String>,
}

impl PuszRowIdentifier {
    pub fn new(plugin_id : &'static str, identifier : String) -> Self {
        Self {
            plugin_id,
            identifier,
            data : None,
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum PuszAction {
    SetClipboard,
    OpenBrowserIfLink,
    CustomAction,
}

#[derive(PartialEq, Eq, Clone, Debug, PartialOrd, Ord)]
pub enum SpecialKey {
    Return,
}

#[derive(PartialEq, Eq, Clone, Debug, PartialOrd, Ord)]
pub enum PuszEvent {
    Click,
    DoubleClick,
    SpecialKeyPress(SpecialKey),
    //CompountAction(Vec<PuszEvent>) ?
}

#[derive(PartialEq, Clone, Debug)]
pub struct PuszEntry {
    // TODO: consider having multiple actions?
    pub actions : BTreeMap<PuszEvent, PuszAction>,
    pub label : String,
    pub content : String,
}

//already had panics unexpected due to builder, eh purge it out?
#[derive(PartialEq, Clone, Debug, Builder)]
pub struct PuszRow {
    pub main_entry : PuszEntry,

    #[builder(default)]
    pub additional_entries : Vec<PuszEntry>,

    pub identifier : PuszRowIdentifier,
    pub is_removable : bool,
}

impl PuszRowBuilder {
    pub fn new(content : String, identifier : PuszRowIdentifier) -> PuszRowBuilder {
        PuszRowBuilder {
            main_entry : Some(PuszEntry {
                actions : btreemap!(PuszEvent::Click => PuszAction::SetClipboard),
                label : content.clone(),
                content,
            }),
            additional_entries : None,
            identifier : Some(identifier),

            is_removable : Some(false),

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
    fn query_return(&mut self, query: &str) -> PluginResult {
        self.query(query)
    }

    //invoked on pressing return
    fn action_request(&mut self, query : &str) -> PluginResult {
        self.query(query)
    }
    fn name(&self) -> &'static str;

//    fn id(&self) -> PuszRowIdentifier {
//        PuszRowIdentifier::Plugin(self.name())
//    }

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
