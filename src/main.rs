use clap::Parser;
use ignore::overrides::OverrideBuilder;
use ignore::WalkState::Continue;
use ignore::{DirEntry, WalkBuilder, WalkParallel};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, process, slice};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser, Debug)]
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

    println!();

    if let Err(msg) = run(&args) {
        eprintln!("{}", msg);
        process::exit(1);
    }
}

fn run(args: &Args) -> Result<()> {
    let walker = build_walker(&args)?;

    let file_count = AtomicUsize::new(0);
    let updated_count = AtomicUsize::new(0);

    walker.run(|| {
        Box::new(|entry| {
            match entry {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        file_count.fetch_add(1, Ordering::Relaxed);

                        match process(&entry, args.dry_run) {
                            Ok(updated) => {
                                if updated {
                                    updated_count.fetch_add(1, Ordering::Relaxed);
                                    println!("updated: {}", entry.path().display());
                                } else if args.list {
                                    println!("   read: {}", entry.path().display());
                                }
                            }
                            Err(msg) => {
                                eprintln!("  error: {}: {}", entry.path().display(), msg);
                            }
                        }
                    }
                }
                Err(msg) => {
                    eprintln!("  error: {}", msg);
                }
            }

            Continue
        })
    });

    println!();
    println!("total files: {}", file_count.load(Ordering::Relaxed));
    println!(
        "{}: {}",
        if args.dry_run {
            "files to be updated"
        } else {
            "updated files"
        },
        updated_count.load(Ordering::Relaxed),
    );

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

    if file.seek(SeekFrom::End(0))? == 0 {
        return Ok(false);
    }

    let mut byte = 0u8;

    file.seek(SeekFrom::End(-1))?;
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

    file.seek(SeekFrom::End(0))?;
    file.write_all(NEWLINE)?;
    file.flush()?;

    Ok(true)
}
