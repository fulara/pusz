use std::time::SystemTime;

use std::io::Write;

use serde::{Serialize, Deserialize};

use plugin_interface;
use plugin_interface::{PluginResult, PuszRow, PuszRowBuilder, PuszRowIdentifier, PluginEvent, PluginSettings};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataEntry {
    text : String,
    last_use_timestamp : SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DataModel {
    clips : Vec<DataEntry>
}

impl DataModel {
    fn add_entry(&mut self, text : &str) {
        for e in &mut self.clips {
            if e.text == text {
                e.last_use_timestamp = SystemTime::now();
                return;
            }
        }

        self.clips.push(DataEntry {text : text.to_owned(), last_use_timestamp : SystemTime::now() });

        save_data_model("pusz.json", &self);
    }
}

fn load_data_model(file : &str) -> DataModel {
    use std::fs;

    let contents = fs::read_to_string(file).unwrap_or_default();

    serde_json::from_str(&contents).unwrap_or_default()
}

fn save_data_model(file : &str, model : &DataModel) {
    use std::fs::File;

    let mut file = File::create(file).expect("couldnt create a file.");
    file.write_all(serde_json::to_string_pretty(model).expect("failed to serialize").as_bytes()).expect("couldnt dump data model.");
}

#[derive(Debug)]
struct ClipboardPlugin {
    data_model : DataModel,
}

impl plugin_interface::Plugin for ClipboardPlugin {
    fn query(&mut self, query: &str) -> PluginResult {
        use fuzzy_matcher::skim::fuzzy_match;
        let mut matched = self.data_model.clips.iter().filter_map(|e| fuzzy_match(&e.text, query).map(|match_score| (e, match_score))).collect::<Vec<_>>();

        matched.sort_by(|(_, score_a), (_, score_b)| score_b.cmp(score_a));

        let score_requirement = matched.first().map_or(0, |(_, score)| *score) * 0.5 as i64;

        ;
        let results : Vec<_> = matched.iter().filter(|(_, score)| *score >= score_requirement ).map(|(e, ..)| *e).map(| de : &DataEntry| {
            PuszRowBuilder::new(de.text.clone(), PuszRowIdentifier::new(self.name())).build().unwrap()
        }).collect();

        PluginResult::Ok(results)
    }

    fn query_return(&mut self, query : &str) -> PluginResult {
        println!("clipboard got return query.");
        PluginResult::None
    }

    fn name(&self) -> &'static str {
        "clip"
    }

    fn settings(&self) -> PluginSettings {
        PluginSettings {
            interested_in_clipboard : true,
            requies_explicit_query : false,
        }
    }

    fn on_subscribed_event(&mut self, event: &PluginEvent) {
        match event {
            PluginEvent::Clipboard(clipbard) => {
                self.data_model.add_entry(clipbard);
            },
        }
    }
}

#[no_mangle]
pub extern "C" fn load(plugin_interface_version : &str) -> Result<Box<dyn plugin_interface::Plugin>, String> {
    if plugin_interface_version == plugin_interface::COMMON_INTERFACE_VERSION {
        Ok(Box::new(ClipboardPlugin{ data_model : load_data_model("pusz.json")}))
    } else {
        Err(format!("compatible with: {} but your version is: {}", plugin_interface::COMMON_INTERFACE_VERSION, plugin_interface_version))
    }
}

#[no_mangle]
pub extern "C" fn introduce() -> Box<dyn plugin_interface::Plugin> {
//    let x = Box::new(ClipboardPlugin{});
//    Box::into_raw(x)
    Box::new(ClipboardPlugin{ data_model : load_data_model("pusz.json")})
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_interface::*;
}
