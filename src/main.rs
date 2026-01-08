use clap::Parser;

use std::{
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
#[command(name = "la")]
#[command(about = "A Log Aggregator CLI", long_about = None)]
struct CLI {
    #[arg(short, long, required = true, value_name = "FILE", num_args = 1..5)]
    files: Vec<PathBuf>,

    #[arg(short = 'p', long, value_name = "FILTER")]
    filter: Option<String>,

    #[arg(short, long, value_name = "OUTPUT")]
    output: Option<String>,
}

fn main() {
    let cli = CLI::parse();

    let mut handles = vec![];

    let contents = Arc::new(Mutex::new(String::new()));
    let entry_count = Arc::new(Mutex::new(0));
    let filter = Arc::new(cli.filter);

    for path in cli.files {
        let contents = Arc::clone(&contents);
        let entry_count = Arc::clone(&entry_count);
        let filter = Arc::clone(&filter);

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

            {
                let mut counter = entry_count.lock().unwrap();
                *counter += file_contents.lines().count();
            }

            let mut contents = contents.lock().unwrap();
            contents.push_str(&file_contents);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let entry_count = entry_count.lock().unwrap();
    println!("Total entries: {}", entry_count);

    let contents = contents.lock().unwrap();
    print!("{contents}");
}
