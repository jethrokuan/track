#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;

use chrono::prelude::*;
use clap::{App, Arg, SubCommand};
use regex::Regex;
use std::fmt;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;

const PKG_NAME: &str = "Track";
const TRACK_VERSION: &str = env!("CARGO_PKG_VERSION");
const TRACK_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

mod errors {
    error_chain! {
      foreign_links {
          Clap(::clap::Error);
          Io(::std::io::Error);
          Chrono(::chrono::format::ParseError);
      }
    }
}

use errors::*;

fn run() -> Result<()> {
    let mut app = App::new(PKG_NAME)
        .version(TRACK_VERSION)
        .author(clap::crate_authors!(", "))
        .about(TRACK_DESCRIPTION)
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .value_name("FILE")
                .help("Path to track file")
                .takes_value(true),
        )
        .subcommand(
            SubCommand::with_name("add")
                .about("add a new entry")
                .arg(
                    Arg::with_name("categories")
                        .required(true)
                        .index(1)
                        .help("the categories for the entry"),
                )
                .arg(
                    Arg::with_name("info")
                        .required(true)
                        .index(2)
                        .help("the info for the entry"),
                ),
        )
        .subcommand(SubCommand::with_name("read").about("read trackfile"));
    let matches = app.clone().get_matches();

    let mut default_track_file = dirs::home_dir().expect("Unable to get home directory");
    default_track_file.push(".track");
    let track_file = match matches.value_of("file") {
        Some(f) => PathBuf::from(f),
        None => default_track_file,
    };

    let mut track = Track::new(track_file)?;

    match matches.subcommand() {
        ("read", Some(_)) => {
            track.load()?;
            println!("{:?}", &track.entries);
        }
        ("add", Some(m)) => {
            track.add_entry(
                m.value_of("categories").unwrap(),
                m.value_of("info").unwrap(),
            )?;
        }
        _ => {
            app.print_help()?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(ref e) = run() {
        println!("error: {}", e);
        for e in e.iter().skip(1) {
            println!("caused by: {}", e);
        }
        if let Some(backtrace) = e.backtrace() {
            println!("backtrace: {:?}", backtrace);
        }
        std::process::exit(1);
    }
}

struct Track {
    track_file: PathBuf,
    entries: Vec<Entry>,
}

impl Track {
    fn new(track_file: PathBuf) -> Result<Track> {
        if !track_file.is_file() {
            std::fs::File::create(&track_file)?;
        }

        Ok(Track {
            track_file,
            entries: vec![],
        })
    }

    fn load(&mut self) -> Result<()> {
        self.get_entries()?;
        Ok(())
    }

    fn get_entries(&mut self) -> Result<()> {
        let f = File::open(&self.track_file)?;
        let reader = BufReader::new(f);

        let mut entries = vec![];

        for line in reader.lines() {
            let l = line?;
            if l == "" {
                continue;
            }
            let entry: Entry = Entry::from(&l)?;
            entries.push(entry);
        }

        self.entries = entries;

        Ok(())
    }

    fn add_entry(&self, categories: &str, info: &str) -> Result<()> {
        let local: DateTime<Local> = Local::now();
        let file = OpenOptions::new().append(true).open(&self.track_file)?;
        let entry = Entry {
            date: local,
            categories: Categories::from(categories),
            info: EntryInfo::from(info),
        };
        write!(&file, "{}\n", entry.to_string())?;

        Ok(())
    }
}

#[derive(Debug)]
struct Entry {
    date: DateTime<Local>,
    categories: Categories,
    info: EntryInfo,
}

#[derive(Debug)]
struct Categories {
    categories: Vec<String>,
}

impl Categories {
    fn from(s: &str) -> Categories {
        let categories = s.split(":").map(|s| String::from(s)).collect();
        Categories { categories }
    }
}

impl fmt::Display for Categories {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.categories.join(":"))
    }
}

#[derive(Debug)]
enum EntryInfo {
    Quantity,
    String,
}

impl EntryInfo {
    fn from(s: &str) -> EntryInfo {
        let first_char = l.chars().next().unwrap();
        if first_char.is_num() {
            EntryInfo::Quantity {
                quantity: 0.0,
                unit: String::from("m"),
            }
        } else {
            String::from(s)
        }
    }
}

#[derive(Debug)]
struct Quantity {
    quantity: f32,
    unit: String,
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.quantity, self.unit)
    }
}

impl Entry {
    fn from(s: &str) -> Result<Entry> {
        lazy_static! {
            static ref ENTRY_RE: Regex = Regex::new(r"^\[(.*)\] (.*):(.*)$").unwrap();
        }

        let caps = ENTRY_RE.captures(s);
        match caps {
            Some(c) => {
                let date = c.get(1).unwrap().as_str();
                let date = DateTime::parse_from_rfc3339(date)?.with_timezone(&Local);

                let categories = c.get(2).unwrap().as_str();
                let categories = Categories::from(categories);
                let info = c.get(3).unwrap().as_str().to_string();
                Ok(Entry {
                    date,
                    categories,
                    info,
                })
            }

            None => bail!("Could not parse line for entry"),
        }
    }
}

impl ToString for Entry {
    #[inline]
    fn to_string(&self) -> String {
        let s = format!(
            "[{}] {}:{}",
            self.date.to_rfc3339(),
            self.categories,
            self.info
        );
        s
    }
}
