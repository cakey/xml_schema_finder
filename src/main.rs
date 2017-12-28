extern crate quick_xml;
extern crate indicatif;
extern crate clap;
use clap::{Arg, App};

use quick_xml::reader::Reader;
use quick_xml::events::{Event, BytesStart};

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

    let num_events = matches.value_of("num_events").unwrap().parse::<u64>().unwrap();

    let debug = matches.is_present("debug");

    match get_schema(file, num_events, debug) {
    Ok(()) => {}
        Err(e) => {
            println!("Error in dedupe: {}", e);
        }
    }
}

fn add_attributes(schema: &mut ElementSchema, e: &BytesStart) -> Result<(),quick_xml::errors::Error> {
    for a in e.attributes() {
        if a.is_ok() {
            let at = a?;
            let key = str::from_utf8(&at.key).unwrap().to_string();
            let mut attribute = schema.attributes.entry(key).or_insert(
                HashSet::new()
            );
            if attribute.len() < 5 {
                let mut value = str::from_utf8(&at.value).unwrap().to_string();
                truncate_next_with_ellipses(&mut value, 20);
                attribute.insert(value);
            }
        }
    }
    Ok(())
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
    bar.set_style(
        ProgressStyle::default_bar().template("[{elapsed}] {wide_bar} {percent}%"),
    );

    let mut event_count = 0;
    let mut buf = Vec::new();
    let mut ident = 0;
    let mut idents = Vec::new();
    let mut s = String::new();
    for _ in 0..50 {
        idents.push(s.clone());
        s.push_str("\t");
    }

    let root_str = "__Root__";
    let mut elements:HashMap<String,ElementSchema> = HashMap::new();


    let mut others = 0;
    let root_element = ElementSchema{
        name: root_str.to_string(),
        sub_elements: HashSet::new(),
        attributes: HashMap::new(),
        examples: HashSet::new(),
    };
    elements.insert(root_str.to_string(), root_element);
    let mut element_stack= Vec::new();
    element_stack.push(root_str.to_string());
    loop {
        if event_count >= max_events {
            break;
        }
        event_count += 1;
        if event_count % 1000 == 0 {
            if (event_count as f32 / max_events as f32) > (reader.buffer_position() as f32 / file_size as f32) {
                bar.set_position(((event_count as f32 / max_events as f32) * file_size as f32) as u64 + 1);
            } else {
                bar.set_position(reader.buffer_position() as u64);
            }

        }
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {

                let name = str::from_utf8(&e.name()).unwrap();
                {
                    let last = element_stack.last().unwrap();
                    let last_e = elements.get_mut(last).unwrap();
                    last_e.sub_elements.insert(name.to_string());
                }
                let mut schema = elements.entry(name.to_string()).or_insert(ElementSchema{
                    name: name.to_string(),
                    sub_elements: HashSet::new(),
                    attributes: HashMap::new(),
                    examples: HashSet::new()
                });
                element_stack.push(name.to_string());

                add_attributes(&mut schema, e)?;

                if debug {
                    println!("{}start: {}", idents[ident], e.unescape_and_decode(&reader)?);
                    ident += 1;
                }
                // attributes ?
            },
            Ok(Event::End(_)) => {
                element_stack.pop();
                if debug {
                    ident -= 1;
                    println!("{}end", idents[ident]);
                }
            }
            Ok(Event::Empty(ref e)) => {

                let name = str::from_utf8(&e.name()).unwrap();
                {
                    let last = element_stack.last().unwrap();
                    let last_e = elements.get_mut(last).unwrap();
                    last_e.sub_elements.insert(name.to_string());
                }
                let mut schema = elements.entry(name.to_string()).or_insert(ElementSchema{
                    name: name.to_string(),
                    sub_elements: HashSet::new(),
                    attributes: HashMap::new(),
                    examples: HashSet::new()
                });

                add_attributes(&mut schema, e)?;

                if debug {
                    println!("{}empty: {}", idents[ident], e.unescape_and_decode(&reader)?);
                }
            }
            Ok(Event::Text(ref e)) => {
                let last = element_stack.last().unwrap();
                let last_e = elements.get_mut(last).unwrap();
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
            },
            Err(e) => {
                println!("Error: {:?}", e);
                break;
            }
            Ok(Event::Eof) => {
                break;
            }
            f => {
                println!("Other: {:?}", f);
                others += 1;

            }
        }
        buf.clear();
    }
    // TODO: handle circular dependencies
    for elem in elements.iter() {
        println!("{:?}", elem);
    }

    Ok(())
}