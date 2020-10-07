use chrono::prelude::*;
use std::path::PathBuf;
use std::process;
use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use std::io::Write;
use std::fs::OpenOptions;
use clap::{Arg, App, SubCommand};

const PKG_NAME: &str = "Track";
const TRACK_VERSION: &str = env!("CARGO_PKG_VERSION");
const TRACK_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

fn main() {
  let mut app = App::new(PKG_NAME)
    .version(TRACK_VERSION)
    .author(clap::crate_authors!(", "))
    .about(TRACK_DESCRIPTION)
    .arg(Arg::with_name("file")
         .short("f")
         .long("file")
         .value_name("FILE")
         .help("Path to track file")
         .takes_value(true))
    .subcommand(SubCommand::with_name("add")
                .about("add a new entry")
                .arg(Arg::with_name("category")
                     .required(true)
                     .index(1)
                     .help("the category for the entry"))
                .arg(Arg::with_name("value")
                     .required(true)
                     .index(2)
                     .help("the value for the entry")))
    .subcommand(SubCommand::with_name("test")
                .about("controls testing features")
                .arg(Arg::with_name("debug")
                     .short("d")
                     .help("print debug information verbosely")));
  let matches = app.clone().get_matches();

  let mut default_track_file = dirs::home_dir().expect("Unable to get home directory");
  default_track_file.push(".track");
  let track_file = match matches.value_of("file") {
    Some(f) => PathBuf::from(f),
    None => default_track_file
  };

  let mut track = Track::new(track_file);
  track.load();
  
  match matches.subcommand() {
    ("add", Some(m)) => {
      track.add_entry(m.value_of("category").expect("no category passed in CLI"),
                      m.value_of("value").expect("no value passed in CLI"));
    },
    _ => {
      app.print_help().expect("Unable to print help");
    }
  }
}

struct Track {
  track_file: PathBuf,
  entries: Vec<Entry>,
}

impl Track {
  fn new(track_file: PathBuf) -> Track {
    if !track_file.is_file() {
      println!("Track file does not exist, exiting.");
      process::exit(0);
    }
    
    Track {
      track_file,
      entries: vec![],
    }
  }

  fn load(&mut self) {
    self.entries = self.get_entries();
  }

  fn get_entries(&self) -> Vec<Entry> {
    let f = File::open(&self.track_file)
      .expect(format!("Unable to open {}", &self.track_file.to_str().unwrap()).as_str());
    let reader = BufReader::new(f);

    let entries = vec![];

    for line in reader.lines() {
      let l = line.unwrap();
      if l == "" { continue; }
    }
    
    entries
  }

  fn add_entry(&self, category: &str, value: &str) {
    let local: DateTime<Local> = Local::now();
    let mut file = OpenOptions::new().append(true).open(&self.track_file)
      .expect(format!("Unable to open {}", &self.track_file.to_str().unwrap()).as_str());
    let entry = Entry{ date: local, category: String::from(category), value: String::from(value) };
    file.write(entry.to_string().as_bytes()).expect("Write to track file failed");
  }
}

struct Entry {
  date: DateTime<Local>,
  category: String,
  value: String
}

impl ToString for Entry {
    #[inline]
    fn to_string(&self) -> String {
      let s = format!("[{}] {}:{}",
                      self.date.format("%Y-%m-%d %H:%M:%S %z"),
                      self.category,
                      self.value);
      s
    }
}
