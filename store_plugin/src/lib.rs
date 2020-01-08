#[macro_use]
extern crate tantivy;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::Index;
use tantivy::ReloadPolicy;
use tantivy::directory::MmapDirectory;

use plugin_interface;
use plugin_interface::{PluginResult, PuszRow, PuszRowBuilder, PuszRowIdentifier, PluginEvent, PluginSettings};

#[derive(Debug)]
struct StorePlugin {
}

impl plugin_interface::Plugin for StorePlugin {
    fn query(&mut self, query: &str) -> PluginResult {
        PluginResult::None
    }

    fn name(&self) -> &'static str {
        "store"
    }
}

fn test() {
    println!("wut?");
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);

    let schema = schema_builder.build();

    let path = "clipboard_index";
    ::std::fs::create_dir_all(path);
    let dir = tantivy::directory::MmapDirectory::open(path).expect("dir?");
    let previously_existed = Index::exists(&dir);

    let index = Index::open_or_create(dir, schema.clone()).expect("couldnt create_in_dir");
    let mut index_writer = index.writer(3_000_000).expect("writer");

    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    if !previously_existed {
        let mut old_man_doc = Document::default();
        old_man_doc.add_text(title, "The Old Man and the Sea");
        old_man_doc.add_text(
            body,
            "He was an old man who fished alone in a skiff in the Gulf Stream and \
         he had gone eighty-four days now without taking a fish.",
        );

        index_writer.add_document(old_man_doc);

        let mut old_man_doc = Document::default();
        old_man_doc.add_text(title, "https://www.youtube.com/");
        old_man_doc.add_text(title, "https://www.wut.com/");
        old_man_doc.add_text(
            body,
            "A website to watch videos and stuff",
        );

        index_writer.add_document(old_man_doc);

        index_writer.commit();

        println!("ok injected");
    }

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into().expect("reader?");
    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![title, body]);

    let query = query_parser.parse_query("****y*t****").expect("parse?");

    let top_docs = searcher.search(&query, &TopDocs::with_limit(10)).expect("search?");

    for (_score, doc_address) in top_docs {
        let retrieved_doc = searcher.doc(doc_address).expect("doc?");

        println!("all: {:?}", retrieved_doc.get_all(title));
        println!("{}", schema.to_json(&retrieved_doc));
    }

    println!("ok done!");
}

#[no_mangle]
pub extern "C" fn load(plugin_interface_version : &str) -> Result<Box<dyn plugin_interface::Plugin>, String> {

    test();
    if plugin_interface_version == plugin_interface::COMMON_INTERFACE_VERSION {
        Ok(Box::new(StorePlugin {}))
    } else {
        Err(format!("compatible with: {} but your version is: {}", plugin_interface::COMMON_INTERFACE_VERSION, plugin_interface_version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_interface::*;
}
