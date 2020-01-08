use std::time::SystemTime;

use std::io::Write;
use std::fs::{
    metadata,
    Metadata
};

use serde::{Serialize, Deserialize};


use plugin_interface;
use plugin_interface::{PluginResult, PuszRow, PuszRowBuilder, PuszRowIdentifier, PluginEvent, PluginSettings};

const FILENAME : &'static str = "pusz.toml";

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
    fn add_entry(&mut self, text : &str, last_known_modification_time : SystemTime, ) -> SystemTime{
        for e in &mut self.clips {
            if e.text == text {
                e.last_use_timestamp = SystemTime::now();
                return last_known_modification_time;
            }
        }

        self.clips.push(DataEntry {text : text.to_owned(), last_use_timestamp : SystemTime::now() });

        save_data_model(FILENAME, Some(text.to_owned()), last_known_modification_time, self)
    }
}

fn load_data_model(file : &str) -> (DataModel, SystemTime) {
    use std::fs;

    if let Ok(contents) = fs::read_to_string(file) {
        let modification_date = metadata(file).expect("metadata").modified().expect("metadata");
        (toml::from_str(&contents).unwrap_or_default(), modification_date)
    } else {
        (DataModel::default(), SystemTime::now())
    }
}

fn save_data_model(file : &str, last_stored_entry : Option<String>, last_known_modification_date : SystemTime, model : &mut DataModel) -> SystemTime {
    use std::fs::File;

    if let Ok(metadata) = metadata(file) {
        if let Ok(modification_time) = metadata.modified() {
            if modification_time > last_known_modification_date {
                //then file was modified in the meantime.

                let (loaded_model, modification_time) = load_data_model(file);
                *model = loaded_model;

                if let Some(last_stored_entry) = last_stored_entry {
                    model.add_entry(&last_stored_entry, modification_time);
                }

                return modification_time;
            }
        }
    }

    let mut file = File::create(file).expect("couldnt create a file.");
    file.write_all(toml::to_string_pretty(model).expect("failed to serialize").as_bytes()).expect("couldnt dump data model.");

    SystemTime::now()
}

#[derive(Debug)]
struct ClipboardPlugin {
    data_model : DataModel,
    last_known_storage_modification_time : SystemTime,
}

impl plugin_interface::Plugin for ClipboardPlugin {
    fn query(&mut self, query: &str) -> PluginResult {
        use fuzzy_matcher::skim::fuzzy_match;
        let mut matched = self.data_model.clips.iter().filter_map(|e| fuzzy_match(&e.text, query).map(|match_score| (e, match_score))).collect::<Vec<_>>();

        matched.sort_by(|(_, score_a), (_, score_b)| score_b.cmp(score_a));

        let score_requirement = matched.first().map_or(0, |(_, score)| *score) * 0.5 as i64;

        ;
        let results : Vec<_> = matched.iter().filter(|(_, score)| *score >= score_requirement ).map(|(e, ..)| *e).map(| de : &DataEntry| {
            PuszRowBuilder::new(de.text.clone(), PuszRowIdentifier::new(self.name(), de.text.clone())).build().unwrap()
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
                self.last_known_storage_modification_time = self.data_model.add_entry(clipbard, self.last_known_storage_modification_time);
            },
        }
    }
}

#[no_mangle]
pub extern "C" fn load(plugin_interface_version : &str) -> Result<Box<dyn plugin_interface::Plugin>, String> {
    if plugin_interface_version == plugin_interface::COMMON_INTERFACE_VERSION {
        let (model, modification_time) = load_data_model(FILENAME);
        Ok(Box::new(ClipboardPlugin{ data_model : model, last_known_storage_modification_time : modification_time}))
    } else {
        Err(format!("compatible with: {} but your version is: {}", plugin_interface::COMMON_INTERFACE_VERSION, plugin_interface_version))
    }
}

#[no_mangle]
pub extern "C" fn introduce() -> Box<dyn plugin_interface::Plugin> {
//    let x = Box::new(ClipboardPlugin{});
//    Box::into_raw(x)
    let (model, modification_time) = load_data_model(FILENAME);
    Box::new(ClipboardPlugin{ data_model : model, last_known_storage_modification_time : modification_time})
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_interface::*;
}
