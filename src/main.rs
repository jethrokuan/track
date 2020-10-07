#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;
use ansi_term::Style;
use chrono::prelude::*;
use clap::{App, Arg, SubCommand};
use itertools::Itertools;
use regex::Regex;
use std::collections::HashMap;
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
          Num(::std::num::ParseIntError);
          Float(::std::num::ParseFloatError);
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
        .subcommand(
            SubCommand::with_name("query")
                .about("query entries")
                .arg(
                    Arg::with_name("categories")
                        .required(true)
                        .index(1)
                        .help("the categories for the entry"),
                )
                .arg(
                    Arg::with_name("range")
                        .index(2)
                        .help("the range to query for"),
                ),
        );
    let matches = app.clone().get_matches();

    let mut default_track_file = dirs::home_dir().expect("Unable to get home directory");
    default_track_file.push(".track");
    let track_file = match matches.value_of("file") {
        Some(f) => PathBuf::from(f),
        None => default_track_file,
    };

    let mut track = Track::new(track_file)?;

    match matches.subcommand() {
        ("query", Some(m)) => {
            track.load()?;
            let categories = m.value_of("categories").unwrap();
            let range = m.value_of("range").unwrap_or("7");
            let range = range.parse::<i64>()?;
            track.query(categories, range)?;
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

    fn query(&self, categories: &str, range: i64) -> Result<()> {
        let now: Date<Local> = Local::now().date();
        let min_date: Date<Local> = now
            .checked_sub_signed(chrono::Duration::days(range))
            .unwrap();

        for (date, entries) in self
            .entries
            .iter()
            .filter(|e| e.categories.contains(&categories) && e.date.date() > min_date)
            .group_by(|e| e.date.date())
            .into_iter()
        {
            print!(
                "{}",
                Style::new()
                    .bold()
                    .paint(date.format("%d %b %Y").to_string())
            );
            for (pos, (category, cat_entries)) in entries
                .sorted_by(|e1, e2| e1.categories.cmp(&e2.categories))
                .group_by(|e| e.categories.to_string())
                .into_iter()
                .enumerate()
            {
                let entry_infos = cat_entries.collect::<Vec<&Entry>>();
                let entry_info_agg: EntryInfoAggregate = Entry::aggregate(entry_infos);
                if pos != 0 {
                    print!("           ");
                }
                println!(
                    "\t{}\t{}",
                    Style::new().underline().paint(category),
                    entry_info_agg
                );
            }
        }
        Ok(())
    }

    fn add_entry(&self, categories: &str, info: &str) -> Result<()> {
        let local: DateTime<Local> = Local::now();
        let file = OpenOptions::new().append(true).open(&self.track_file)?;
        let entry = Entry {
            date: local,
            categories: String::from(categories),
            info: EntryInfo::from(info)?,
        };
        write!(&file, "{}\n", entry.to_string())?;

        Ok(())
    }
}

#[derive(Debug)]
struct Entry {
    date: DateTime<Local>,
    categories: String,
    info: EntryInfo,
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}:{}",
            self.date.to_rfc3339(),
            self.categories,
            self.info
        )
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
                let categories = String::from(categories);
                let info = c.get(3).unwrap().as_str();
                let info = EntryInfo::from(info)?;
                Ok(Entry {
                    date,
                    categories,
                    info,
                })
            }

            None => bail!("Could not parse line for entry"),
        }
    }

    fn aggregate(entries: Vec<&Entry>) -> EntryInfoAggregate {
        let mut logs: HashMap<String, i64> = HashMap::new();
        let mut quantities: HashMap<String, f32> = HashMap::new();

        for entry in entries.iter() {
            match &entry.info {
                EntryInfo::Q(q) => {
                    let unit = q.unit.clone();
                    *quantities.entry(unit).or_insert(0.0) += q.quantity;
                }
                EntryInfo::L(l) => {
                    let l = l.clone();
                    *logs.entry(l).or_insert(0) += 1;
                }
            }
        }

        EntryInfoAggregate { logs, quantities }
    }
}

#[derive(Debug)]
enum EntryInfo {
    Q(Quantity),
    L(String),
}

impl EntryInfo {
    fn from(s: &str) -> Result<EntryInfo> {
        lazy_static! {
            static ref QUANTITY_RE: Regex = Regex::new(r"^([-+]?[0-9]*\.*[0-9]*)+(.*)$").unwrap();
        }
        let caps = QUANTITY_RE.captures(s.trim()).unwrap();
        let quantity = caps.get(1).unwrap().as_str();
        let unit = caps.get(2).unwrap().as_str().to_string();
        if quantity.is_empty() {
            return Ok(EntryInfo::L(String::from(s)));
        } else {
            let quantity = quantity.parse::<f32>()?;
            Ok(EntryInfo::Q(Quantity { quantity, unit }))
        }
    }
}

impl fmt::Display for EntryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryInfo::Q(q) => write!(f, "{}{}", q.quantity, q.unit),
            EntryInfo::L(s) => write!(f, "{}", s),
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

struct EntryInfoAggregate {
    logs: HashMap<String, i64>,
    quantities: HashMap<String, f32>,
}

impl fmt::Display for EntryInfoAggregate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        for (log, count) in &self.logs {
            s.push_str(log);
            if *count != 1 as i64 {
                s.push_str(format!("x{}", count).as_str());
            }
        }
        for (unit, total) in &self.quantities {
            s.push_str(format!("{}{} ", total, unit).as_str());
        }
        write!(f, "{}", s)
    }
}
