extern crate clap;
extern crate indicatif;
extern crate quick_xml;

use clap::{App, Arg};
use quick_xml::reader::Reader;
use quick_xml::events::{BytesStart, Event};
use indicatif::{ProgressBar, ProgressStyle};

use std::fs;
use std::str;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug)]
struct ElementSchema {
    name: String,
    sub_elements: HashSet<String>,
    attributes: HashMap<String, HashSet<String>>,
    examples: HashSet<String>,
}

impl ElementSchema {
    fn new(s: &str) -> ElementSchema {
        ElementSchema {
            name: s.to_string(),
            sub_elements: HashSet::new(),
            attributes: HashMap::new(),
            examples: HashSet::new(),
        }
    }
}

struct XMLSchema {
    elements: HashMap<String, ElementSchema>,
    root_name: String,
}

impl XMLSchema {
    fn new() -> XMLSchema {
        let mut elements = HashMap::new();
        let r = "__Root__";
        let root = ElementSchema::new(r);
        elements.insert(r.to_string(), root);
        XMLSchema {
            elements: elements,
            root_name: r.to_string(),
        }
    }
    fn root_string(&self) -> String {
        self.root_name.clone()
    }

    fn add_sub_element(&mut self, parent: &str, sub: &str) {
        self.elements
            .entry(sub.to_string())
            .or_insert_with(|| ElementSchema::new(sub));
        let last_e = self.elements.get_mut(parent).unwrap();
        last_e.sub_elements.insert(sub.to_string());
    }

    fn add_quick_xml_attributes(
        &mut self,
        name: &str,
        e: &BytesStart,
    ) -> Result<(), quick_xml::errors::Error> {
        // seemingly have to choose between two hashes and an allocation in the non insert case
        let schema = self.elements
            .entry(name.to_string())
            .or_insert_with(|| ElementSchema::new(name));

        for a in e.attributes() {
            if a.is_ok() {
                let at = a?;
                let key = str::from_utf8(&at.key).unwrap().to_string();
                let mut attribute = schema
                    .attributes
                    .entry(key)
                    .or_insert_with(|| HashSet::new());
                if attribute.len() < 5 {
                    let mut value = str::from_utf8(&at.value).unwrap().to_string();
                    truncate_next_with_ellipses(&mut value, 50);
                    attribute.insert(value);
                }
            }
        }
        Ok(())
    }
}

fn main() {
    let matches = App::new("XML Schema Finder")
        .version("1.0")
        .about("Finds the structure of a large xml document.")
        .arg(Arg::with_name("file")
            .short("f")
            .long("file")
            .value_name("FILE")
            .help("Sets the input file to use")
            .required(true)
            .takes_value(true))
        .arg(Arg::with_name("num_events")
            .short("n")
            .long("num_events")
            .value_name("NUM_EVENTS")
            .help("The max number of events to parse (e.g. to save time running against a 50GB file)")
            .default_value("10000000")
            .takes_value(true))
        .arg(Arg::with_name("debug")
            .short("d")
            .long("debug")
            .help("Print debugging"))
        .get_matches();

    let file = matches.value_of("file").unwrap();

    let num_events = matches
        .value_of("num_events")
        .unwrap()
        .parse::<u64>()
        .unwrap();

    let debug = matches.is_present("debug");

    match get_schema(file, num_events, debug) {
        Ok(()) => {}
        Err(e) => {
            println!("Error in dedupe: {}", e);
        }
    }
}

fn truncate_next_with_ellipses(s: &mut String, v1: usize) -> () {
    let len = s.len();
    let mut v = v1;
    while v < len && !s.is_char_boundary(v) {
        v += 1;
    }
    s.truncate(v);
    if v < len {
        s.push_str("...");
    }
}

fn get_schema(file: &str, max_events: u64, debug: bool) -> Result<(), quick_xml::errors::Error> {
    let mut reader = Reader::from_file(file)?;
    let file_size = fs::metadata(file)?.len();
    reader.trim_text(true);
    let bar = ProgressBar::new(file_size);
    bar.set_style(ProgressStyle::default_bar().template("[{elapsed}] {wide_bar} {percent}%"));

    let mut event_count = 0;
    let mut buf = Vec::new();
    let mut ident = 0;
    let mut idents = Vec::new();
    let mut s = String::new();
    for _ in 0..50 {
        idents.push(s.clone());
        s.push_str("\t");
    }

    let mut elements = XMLSchema::new();

    let mut element_stack = Vec::new();
    element_stack.push(elements.root_string());
    loop {
        if event_count >= max_events {
            break;
        }
        event_count += 1;
        if event_count % 1000 == 0 {
            if (event_count as f32 / max_events as f32)
                > (reader.buffer_position() as f32 / file_size as f32)
            {
                bar.set_position(
                    ((event_count as f32 / max_events as f32) * file_size as f32) as u64 + 1,
                );
            } else {
                bar.set_position(reader.buffer_position() as u64);
            }
        }
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = str::from_utf8(&e.name()).unwrap();

                elements.add_sub_element(element_stack.last().unwrap(), name);

                elements.add_quick_xml_attributes(name, e)?;

                element_stack.push(name.to_string());

                if debug {
                    println!(
                        "{}start: {}",
                        idents[ident],
                        e.unescape_and_decode(&reader)?
                    );
                    ident += 1;
                }
            }
            Ok(Event::End(_)) => {
                element_stack.pop();
                if debug {
                    ident -= 1;
                    println!("{}end", idents[ident]);
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = str::from_utf8(&e.name()).unwrap();
                elements.add_sub_element(element_stack.last().unwrap(), name);
                elements.add_quick_xml_attributes(name, e)?;

                if debug {
                    println!(
                        "{}empty: {}",
                        idents[ident],
                        e.unescape_and_decode(&reader)?
                    );
                }
            }
            Ok(Event::Text(ref e)) => {
                let last = element_stack.last().unwrap();
                let last_e = elements.elements.get_mut(last).unwrap();
                if last_e.examples.len() < 5 {
                    let mut text = e.unescape_and_decode(&reader)?;
                    truncate_next_with_ellipses(&mut text, 100);
                    last_e.examples.insert(text);
                }

                if debug {
                    let mut text = e.unescape_and_decode(&reader)?;
                    truncate_next_with_ellipses(&mut text, 1000);
                    println!("{}text: {}", idents[ident], text);
                }
            }
            Err(e) => {
                println!("Error: {:?}", e);
                break;
            }
            Ok(Event::Eof) => {
                break;
            }
            f => {
                println!("Other: {:?}", f);
            }
        }
        buf.clear();
    }
    // TODO: handle circular dependencies
    for elem in elements.elements.iter() {
        println!("{:?}", elem);
    }

    Ok(())
}
