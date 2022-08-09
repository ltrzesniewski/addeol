use crate::FileResult;
use ignore::DirEntry;
use std::io::Write;
use std::{fmt, io};
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};

pub struct Printer {
    stdout: StandardStream,
}

impl Printer {
    pub(crate) fn new() -> Printer {
        Printer {
            stdout: StandardStream::stdout(termcolor::ColorChoice::Auto),
        }
    }

    pub(crate) fn write_file_result(
        &mut self,
        result: &FileResult,
        dry_run: bool,
    ) -> io::Result<()> {
        match result {
            FileResult::UpdatedFile(ref entry) => {
                self.write_header(if dry_run { "to update" } else { "updated" }, Color::Green)?;
                self.write_file_path(entry)?;
            }
            FileResult::UpToDateFile(ref entry) => {
                self.write_header("up to date", Color::White)?;
                self.write_file_path(entry)?;
            }
            FileResult::FileError(ref entry, ref err) => {
                self.write_header("error", Color::Red)?;
                self.write_file_path(entry)?;

                self.stdout
                    .set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
                write!(&mut self.stdout, "{}", err)?;
            }
            FileResult::UnknownError(ref err) => {
                self.stdout
                    .set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_intense(true))?;
                write!(&mut self.stdout, "{}", err)?;
            }
        }

        self.writeln()?;
        Ok(())
    }

    fn write_header(&mut self, header: &str, color: Color) -> io::Result<()> {
        self.stdout
            .set_color(ColorSpec::new().set_fg(Some(color)))?;
        write!(&mut self.stdout, "{:>10}", header)?;
        self.stdout.set_color(&ColorSpec::new())?;
        write!(&mut self.stdout, ": ")?;
        Ok(())
    }

    fn write_file_path(&mut self, entry: &DirEntry) -> io::Result<()> {
        self.stdout
            .set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
        write!(&mut self.stdout, "{}", entry.path().display())?;
        Ok(())
    }

    pub fn writeln(&mut self) -> io::Result<()> {
        writeln!(&mut self.stdout)?;
        Ok(())
    }

    pub fn write_stat(&mut self, label: &str, stat: fmt::Arguments) -> io::Result<()> {
        self.stdout
            .set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
        write!(&mut self.stdout, "{:>20}", label)?;
        self.stdout.set_color(&ColorSpec::new())?;
        writeln!(&mut self.stdout, ": {}", stat)?;
        Ok(())
    }
}

impl Write for Printer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

impl Drop for Printer {
    fn drop(&mut self) {
        let _ = self.stdout.reset();
        let _ = self.stdout.flush();
    }
}
