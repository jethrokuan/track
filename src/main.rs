#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;

use chrono::prelude::*;
use clap::{App, Arg, SubCommand};
use regex::Regex;
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
          Clap(::clap::Error) #[cfg(feature = "application")];
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
                    Arg::with_name("category")
                        .required(true)
                        .index(1)
                        .help("the category for the entry"),
                )
                .arg(
                    Arg::with_name("value")
                        .required(true)
                        .index(2)
                        .help("the value for the entry"),
                ),
        );
    let matches = app.clone().get_matches();

    let mut default_track_file = dirs::home_dir().expect("Unable to get home directory");
    default_track_file.push(".track");
    let track_file = match matches.value_of("file") {
        Some(f) => PathBuf::from(f),
        None => default_track_file,
    };

    let mut track = Track::new(track_file).chain_err(|| "unable to initialize track")?;
    track.load().chain_err(|| "unable to load track")?;

    match matches.subcommand() {
        ("add", Some(m)) => {
            track.add_entry(
                m.value_of("category")
                    .chain_err(|| "no category passed in CLI")?,
                m.value_of("value").chain_err(|| "no value passed in CLI")?,
            )?;
        }
        _ => {
            app.print_help().chain_err(|| "could not print help")?;
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
            std::fs::File::create(&track_file).chain_err(|| "Unable to create track file")?;
        }

        Ok(Track {
            track_file,
            entries: vec![],
        })
    }

    fn load(&mut self) -> Result<()> {
        self.get_entries().chain_err(|| "Unable to load entries")?;
        Ok(())
    }

    fn get_entries(&mut self) -> Result<()> {
        let f = File::open(&self.track_file)
            .chain_err(|| format!("Unable to open {}", &self.track_file.to_str().unwrap()))?;
        let reader = BufReader::new(f);

        let mut entries = vec![];

        for line in reader.lines() {
            let l = line.unwrap();
            if l == "" {
                continue;
            }
            let entry: Entry =
                Entry::from(&l).chain_err(|| format!("Unable to parse entry {}", l))?;
            entries.push(entry);
        }

        self.entries = entries;

        Ok(())
    }

    fn add_entry(&self, category: &str, value: &str) -> Result<()> {
        let local: DateTime<Local> = Local::now();
        let file = OpenOptions::new()
            .append(true)
            .open(&self.track_file)
            .expect(format!("Unable to open {}", &self.track_file.to_str().unwrap()).as_str());
        let entry = Entry {
            date: local,
            category: String::from(category),
            value: String::from(value),
        };
        write!(&file, "{}\n", entry.to_string()).chain_err(|| "write to track file failed")?;

        Ok(())
    }
}

#[derive(Debug)]
struct Entry {
    date: DateTime<Local>,
    category: String,
    value: String,
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
                let date = DateTime::parse_from_rfc3339(date)
                    .chain_err(|| "Could not parse timestamp")?
                    .with_timezone(&Local);

                let category = c.get(2).unwrap().as_str().to_string();
                let value = c.get(3).unwrap().as_str().to_string();
                Ok(Entry {
                    date,
                    category,
                    value,
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
            self.category,
            self.value
        );
        s
    }
}
