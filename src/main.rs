use clap::Parser;

use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result},
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
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

    #[arg(long, value_name = "FILTER")]
    filter: Option<String>,

    #[arg(short = 'p', long)]
    print: bool,
}

#[derive(Eq, Hash, PartialEq)]
enum LogLevel {
    Error,
    Info,
    Warning,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            LogLevel::Error => write!(f, "Errors"),
            LogLevel::Warning => write!(f, "Warnings"),
            LogLevel::Info => write!(f, "Info"),
        }
    }
}

fn main() {
    let start = Instant::now();
    let cli = CLI::parse();

    let mut handles = vec![];

    let aggregate_count = Arc::new(Mutex::new(HashMap::<LogLevel, usize>::new()));
    let contents = Arc::new(Mutex::new(Vec::<String>::new()));
    let filter = Arc::new(cli.filter.as_ref().map(|v| v.to_lowercase()));
    let top_errors = Arc::new(Mutex::new(HashMap::<String, usize>::new()));

    for path in cli.files.clone() {
        let contents = Arc::clone(&contents);
        let filter = Arc::clone(&filter);
        let aggregate_count = Arc::clone(&aggregate_count);
        let top_errors = Arc::clone(&top_errors);

        let handle = thread::spawn(move || {
            if !path.exists() {
                eprintln!("Error: File does not exist {:?}", path);
                return;
            }

            if !path.is_file() {
                eprintln!("Error: is not a file {:?}", path);
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

            let mut filtered_contents = Vec::<&str>::new();
            let mut local_count = HashMap::<LogLevel, usize>::new();
            let mut errors_collection = HashMap::<String, usize>::new();

            for line in file_contents.lines() {
                let content = line.to_lowercase();

                if content.contains("error") {
                    *local_count.entry(LogLevel::Error).or_insert(0) += 1;

                    if let Some((_, after_error)) = content.split_once("error") {
                        let error_msg = after_error.trim_start_matches(&[' ', ':', '-'][..]).trim();
                        if !error_msg.is_empty() {
                            *errors_collection.entry(error_msg.to_string()).or_insert(0) += 1;
                        }
                    }
                } else if content.contains("warn") || content.contains("warning") {
                    *local_count.entry(LogLevel::Warning).or_insert(0) += 1;
                } else if content.contains("info") {
                    *local_count.entry(LogLevel::Info).or_insert(0) += 1;
                }

                match filter.as_ref() {
                    Some(v) => {
                        if content.contains(v) {
                            filtered_contents.push(line);
                        }
                    }
                    None => {
                        filtered_contents.push(line);
                    }
                }
            }

            {
                let mut guard = aggregate_count.lock().unwrap();
                for (k, v) in local_count {
                    guard.entry(k).and_modify(|d| *d += v).or_insert(v);
                }
            }

            {
                let mut contents = contents.lock().unwrap();
                contents.push(filtered_contents.join("\n"));
            }

            {
                let mut top_errors = top_errors.lock().unwrap();
                for (k, v) in errors_collection {
                    top_errors.entry(k).and_modify(|d| *d += v).or_insert(v);
                }
            }
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

    println!("\nTop errors:");
    {
        let top_errors = top_errors.lock().unwrap();
        let mut sorted_errors: Vec<_> = top_errors.iter().map(|v| v).collect();
        sorted_errors.sort_unstable_by(|a, b| b.1.cmp(a.1));

        for (k, v) in sorted_errors.iter().take(5) {
            println!("- \"{k}\" ({v} occurences)");
        }
    }

    println!();

    {
        if cli.print {
            let contents = contents.lock().unwrap();
            println!("{}", contents.join("\n"));
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

    let duration = start.elapsed();
    println!("\nLooped through the data in {:?}", duration);
}
