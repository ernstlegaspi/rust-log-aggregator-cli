use clap::Parser;

use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

// Total entries: 45
// Errors: 18
// Warnings: 8
// Info: 19

// Top errors:
// - "Connection failed: too many connections" (3 occurrences)
// - "Disk full: cannot write to transaction log" (2 occurrences)
// - "Database connection lost" (1 occurrence)

// Files processed: app.log, api.log, database.log

// Filtering - Let users specify what log entries to include (by level, keywords, time range, etc.)
// Aggregating - Count and summarize the data (totals by severity, most common errors, patterns)
// Multi-file processing - Handle multiple log files simultaneously using threads

#[derive(Parser)]
#[command(name = "log-agg")]
#[command(about = "A Log Aggregator CLI", long_about = None)]
struct CLI {
    #[arg(short, long, required = true, value_name = "FILE", num_args = 1..6)]
    files: Vec<PathBuf>,

    #[arg(short = 'p', long, value_name = "FILTER")]
    filter: Option<String>,

    // output = json html txt
    #[arg(short, long, value_name = "OUTPUT")]
    output: Option<String>,

    #[command(subcommand)]
    actions: Option<Actions>,
}

#[derive(Parser)]
enum Actions {
    /// Manage configuration settings (config --help)
    Config {
        /// Print log
        #[arg(long)]
        print: bool,
    },
}

// add feature if user wants to print the log

fn main() {
    let cli = CLI::parse();

    let mut handles = vec![];

    let aggregate_count = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
    let contents = Arc::new(Mutex::new(String::new()));
    let filter = Arc::new(cli.filter);

    for path in cli.files.clone() {
        let contents = Arc::clone(&contents);
        let filter = Arc::clone(&filter);
        let aggregate_count = Arc::clone(&aggregate_count);

        let handle = thread::spawn(move || {
            let path = PathBuf::from("logs").join(path);

            if !path.exists() {
                println!("{:?} does not exist.", path);
                return;
            }

            if !path.is_file() {
                println!("{:?} is not a file", path);
                return;
            }

            let file = match File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Unable to open file: {:?}\n Error: {e:?}", path);
                    return;
                }
            };

            let mut file_contents = String::new();
            let mut reader = BufReader::new(&file);

            if let Err(e) = reader.read_to_string(&mut file_contents) {
                eprintln!("Error reading in {:?}: {e:?}", path);
                return;
            }

            {
                let mut guard = aggregate_count.lock().unwrap();

                for contents in file_contents.lines() {
                    let c = contents.to_lowercase();

                    if c.contains("error") || c.contains("err") {
                        guard
                            .entry("Errors".to_string())
                            .and_modify(|d| *d += 1)
                            .or_insert(1);
                    } else if c.contains("warn") || c.contains("warning") {
                        guard
                            .entry("Warnings".to_string())
                            .and_modify(|d| *d += 1)
                            .or_insert(1);
                    } else if c.contains("info") {
                        guard
                            .entry("Info".to_string())
                            .and_modify(|d| *d += 1)
                            .or_insert(1);
                    }
                }
            }

            let file_contents: String = match filter.as_ref() {
                Some(v) => {
                    let contents: String = file_contents
                        .lines()
                        .filter(|c| c.to_lowercase().contains(v.as_str()))
                        .map(|c| {
                            let mut content = c.to_string();
                            content.push('\n');

                            content
                        })
                        .collect();

                    contents
                }
                None => file_contents,
            };

            let mut contents = contents.lock().unwrap();
            contents.push_str(&file_contents);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!();

    {
        let aggregate_count = aggregate_count.lock().unwrap();
        for (k, v) in &*aggregate_count {
            println!("{k}: {}", v);
        }
    }

    println!();

    if let Some(v) = cli.actions {
        match v {
            Actions::Config { print } => {
                if print {
                    let contents = contents.lock().unwrap();
                    println!("{contents}");
                }
            }
        }
    }

    print!("Files processed: ");
    for (i, e) in cli.files.iter().enumerate() {
        if let Some(v) = e.file_name() {
            print!("{:?}", v);
        }

        if i != cli.files.len() - 1 {
            print!(", ");
        }
    }

    println!();
}
