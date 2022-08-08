use clap::Parser;
use ignore::overrides::OverrideBuilder;
use ignore::WalkState::Continue;
use ignore::{DirEntry, WalkBuilder, WalkParallel};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::{env, process, slice, thread};

type ErrorBox = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, ErrorBox>;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about)]
struct Args {
    /// Glob to match
    #[clap(short, long, required = true)]
    glob: Vec<String>,

    /// Path to search
    #[clap(default_value = ".")]
    paths: Vec<String>,

    /// Do not modify files
    #[clap(short = 'n', long)]
    dry_run: bool,

    /// Don't read ignore files
    #[clap(long)]
    no_ignore: bool,

    /// Include hidden files
    #[clap(long)]
    hidden: bool,

    /// List all included files
    #[clap(long)]
    list: bool,
}

enum FileResult {
    UpdatedFile(DirEntry),
    UpToDateFile(DirEntry),
    FileError(DirEntry, ErrorBox),
    UnknownError(ErrorBox),
}

fn main() {
    let args: Args = Args::parse();

    for glob in &args.glob {
        println!("glob: {}", glob);
    }

    for path in &args.paths {
        println!("path: {}", path);
    }

    if args.dry_run {
        println!("DRY RUN");
    }

    if let Err(msg) = run(&args) {
        eprintln!("{}", msg);
        process::exit(1);
    }
}

fn run(args: &Args) -> Result<()> {
    let walker = build_walker(&args)?;

    let (tx, rx) = mpsc::channel::<FileResult>();
    let args2 = args.clone();

    let printer = thread::spawn(move || {
        print_results(rx, &args2);
    });

    walker.run(|| {
        let tx = tx.clone();

        Box::new(move |entry| {
            match entry {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        let result = match process(&entry, args.dry_run) {
                            Ok(true) => FileResult::UpdatedFile(entry),
                            Ok(false) => FileResult::UpToDateFile(entry),
                            Err(err) => FileResult::FileError(entry, err),
                        };

                        tx.send(result).unwrap();
                    }
                }
                Err(msg) => {
                    tx.send(FileResult::UnknownError(msg.into())).unwrap();
                }
            }

            Continue
        })
    });

    drop(tx);
    printer.join().unwrap();

    Ok(())
}

fn build_walker(args: &Args) -> Result<WalkParallel> {
    let mut builder = WalkBuilder::new(&args.paths[0]);
    for path in &args.paths[1..] {
        builder.add(path);
    }

    if !args.glob.is_empty() {
        let mut override_builder = OverrideBuilder::new(env::current_dir()?);

        for glob in &args.glob {
            override_builder.add(&glob)?;
        }

        builder.overrides(override_builder.build()?);
    }

    if args.no_ignore {
        builder
            .ignore(false)
            .git_ignore(false)
            .git_global(false)
            .parents(false);
    }

    if args.hidden {
        builder.hidden(false);
    }

    Ok(builder.build_parallel())
}

fn process(entry: &DirEntry, dry_run: bool) -> Result<bool> {
    let mut file = File::options()
        .read(true)
        .write(!dry_run)
        .open(entry.path())?;

    if let Err(err) = file.seek(SeekFrom::End(-1)) {
        return if file.seek(SeekFrom::End(0))? == 0 {
            Ok(false) // Empty file
        } else {
            Err(err.into())
        };
    }

    let mut byte = 0u8;
    file.read_exact(slice::from_mut(&mut byte))?;

    if byte == b'\n' {
        return Ok(false);
    }

    if dry_run {
        return Ok(true);
    }

    #[cfg(windows)]
    const NEWLINE: &[u8] = b"\r\n";
    #[cfg(not(windows))]
    const NEWLINE: &[u8] = b"\n";

    file.write_all(NEWLINE)?;
    file.flush()?;

    Ok(true)
}

fn print_results(rx: Receiver<FileResult>, args: &Args) {
    println!();

    let mut file_count = 0;
    let mut updated_count = 0;
    let mut error_count = 0;

    while let Ok(result) = rx.recv() {
        match result {
            FileResult::UpdatedFile(entry) => {
                file_count += 1;
                updated_count += 1;
                println!("updated: {}", entry.path().display());
            }
            FileResult::UpToDateFile(entry) => {
                file_count += 1;
                if args.list {
                    println!("   read: {}", entry.path().display());
                }
            }
            FileResult::FileError(entry, err) => {
                file_count += 1;
                error_count += 1;
                eprintln!("  error: {}: {}", entry.path().display(), err);
            }
            FileResult::UnknownError(err) => {
                error_count += 1;
                eprintln!("  error: {}", err);
            }
        };
    }

    if file_count != 0 {
        println!();
    }

    println!("total files: {}", file_count);
    println!(
        "{}: {}",
        if args.dry_run {
            "files to be updated"
        } else {
            "updated files"
        },
        updated_count
    );

    if error_count != 0 {
        println!("error count: {}", error_count);
    }
}
