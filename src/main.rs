#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate anyhow;

use chrono::prelude::*;
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
use structopt::StructOpt;

use std::env;
use telegram_bot::*;
use tokio::stream::StreamExt;

use anyhow::{Context, Result};

#[derive(Debug, StructOpt)]
#[structopt(name = "track")]
enum Opt {
    #[structopt(help = "Add an entry to the track file.")]
    Add {
        #[structopt(short, long, index = 1)]
        category: String,

        #[structopt(short, long, index = 2)]
        info: String,
    },

    #[structopt(help = "Query the track file.")]
    Query {
        #[structopt(short, long, index = 1)]
        category: String,

        #[structopt(short, long, index = 2, default_value = "7")]
        range: i64,
    },

    #[structopt(help = "Start telegram bot.")]
    Bot {},
}

async fn run() -> Result<()> {
    let mut track_file = dirs::home_dir().expect("Unable to get home directory");
    track_file.push(".track");
    let mut track = Track::new(track_file)?;

    let opt = Opt::from_args();

    match opt {
        Opt::Add { category, info } => {
            track.add_entry(&category, &info)?;
        }
        Opt::Query { category, range } => {
            track.load()?;
            track.query(&category, range)?;
        }
        Opt::Bot {} => {
            track.telegram_bot().await?;
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run().await?;
    Ok(())
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
        let mut print_date: bool;
        let mut print_category: bool;

        for (date, entries) in self
            .entries
            .iter()
            .filter(|e| e.categories.contains(&categories) && e.date.date() > min_date)
            .group_by(|e| e.date.date())
            .into_iter()
        {
            print_date = true;
            for (category, cat_entries) in entries
                .sorted_by(|e1, e2| e1.categories.cmp(&e2.categories))
                .group_by(|e| e.categories.to_string())
                .into_iter()
            {
                print_category = true;
                let entry_infos = cat_entries.collect::<Vec<&Entry>>();
                let entry_info_agg: EntryInfoAggregate = Entry::aggregate(entry_infos);
                for (log, count) in &entry_info_agg.logs {
                    println!(
                        "{0: <12} {1: <15} {2: <15}",
                        if print_date {
                            print_date = false;
                            date.format("%d %b %Y").to_string()
                        } else {
                            String::new()
                        },
                        if print_category {
                            print_category = false;
                            category.to_string()
                        } else {
                            String::new()
                        },
                        format!(
                            "{}{}",
                            log,
                            if *count != 1 as i64 {
                                format!("x{}", count)
                            } else {
                                String::new()
                            }
                        )
                    );
                }
                for (unit, total) in &entry_info_agg.quantities {
                    println!(
                        "{0: <12} {1: <10} {2: <30}",
                        if print_date {
                            print_date = false;
                            date.format("%d %b %Y").to_string()
                        } else {
                            String::new()
                        },
                        if print_category {
                            print_category = false;
                            category.to_string()
                        } else {
                            String::new()
                        },
                        format!("{}{} ", total, unit)
                    );
                }
            }
        }
        Ok(())
    }

    fn add_entry(&self, categories: &str, info: &str) -> Result<()> {
        let local: DateTime<Local> = Local::now();
        let file = OpenOptions::new().append(true).open(&self.track_file)?;
        let entry = Entry {
            date: local,
            categories: String::from(categories).to_lowercase(),
            info: EntryInfo::from(info)?,
        };
        writeln!(&file, "{}", entry.to_string())?;

        Ok(())
    }

    async fn telegram_bot(&self) -> Result<()> {
        let token = env::var("TELEGRAM_BOT_TOKEN").with_context(|| "TELEGRAM_BOT_TOKEN not set")?;
        let api = Api::new(token);

        // Fetch new updates via long poll method
        let mut stream = api.stream();
        while let Some(update) = stream.next().await {
            // If the received update contains a new message...
            let update = update?;
            if let UpdateKind::Message(message) = update.kind {
                if let MessageKind::Text { ref data, .. } = message.kind {
                    let first_space = data.find(' ');
                    let res = match first_space {
                        Some(v) => {
                            let category = &data[0..v];
                            let value = &data[v..].trim();

                            if category.is_empty() {
                                Err(anyhow!("Invalid entry: category is empty"))
                            } else if value.is_empty() {
                                Err(anyhow!("Invalid entry: value is empty"))
                            } else {
                                self.add_entry(category, value)
                                    .with_context(|| "Failed to add entry")
                            }
                        }
                        None => Err(anyhow!("Invalid entry")),
                    };

                    match res {
                        Ok(_) => api.send(message.text_reply("Saved!")).await?,
                        Err(e) => {
                            api.send(message.text_reply(format!("Errored! {}", e)))
                                .await?
                        }
                    };
                }
            }
        }
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
            Ok(EntryInfo::L(String::from(s)))
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
