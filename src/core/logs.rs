use std::{
    fs::{File, OpenOptions},
    io::{self, IsTerminal, Write},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt,
};
#[derive(Clone)]
struct RotatingWriter(Arc<Mutex<LogFile>>);
struct LogFile {
    file: File,
    path: PathBuf,
    max: u64,
    backups: usize,
}
struct Guard<'a>(MutexGuard<'a, LogFile>);
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingWriter {
    type Writer = Guard<'a>;
    fn make_writer(&'a self) -> Self::Writer {
        Guard(self.0.lock().expect("log lock"))
    }
}
impl Write for Guard<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.file.metadata()?.len() + bytes.len() as u64 > self.0.max {
            self.0.file.flush()?;
            for index in (1..=self.0.backups).rev() {
                let old = if index == 1 {
                    self.0.path.clone()
                } else {
                    PathBuf::from(format!("{}.{}", self.0.path.display(), index - 1))
                };
                let new = PathBuf::from(format!("{}.{}", self.0.path.display(), index));
                if old.exists() {
                    std::fs::rename(old, new)?;
                }
            }
            self.0.file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.0.path)?;
        }
        self.0.file.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.file.flush()
    }
}
impl RotatingWriter {
    fn open(path: PathBuf, max: u64, backups: usize) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self(Arc::new(Mutex::new(LogFile {
            file,
            path,
            max,
            backups,
        }))))
    }
}
pub fn init(cli: bool, verbose: bool) -> io::Result<()> {
    let config = crate::core::config::Config::default();
    let file = RotatingWriter::open(config.log_file, config.log_max_bytes, config.log_backups)?;
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(file)
        .with_file(true)
        .with_line_number(true)
        .with_filter(LevelFilter::DEBUG);
    let console_level = if verbose {
        LevelFilter::DEBUG
    } else if cli {
        LevelFilter::WARN
    } else {
        LevelFilter::INFO
    };
    let color = std::env::var_os("NO_COLOR").is_none()
        && if cli {
            io::stderr().is_terminal()
        } else {
            io::stdout().is_terminal()
        };
    let writer = if cli {
        tracing_subscriber::fmt::writer::BoxMakeWriter::new(io::stderr)
    } else {
        tracing_subscriber::fmt::writer::BoxMakeWriter::new(io::stdout)
    };
    let console = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(color)
        .with_file(true)
        .with_line_number(true)
        .with_filter(console_level);
    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(console)
        .try_init();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_rotation() {
        let root =
            std::env::temp_dir().join(format!("hippo-log-{}", uuid::Uuid::new_v4().simple()));
        let path = root.join("fixture.log");
        let writer = RotatingWriter::open(path.clone(), 8, 3).unwrap();
        use tracing_subscriber::fmt::MakeWriter;
        for _ in 0..10 {
            writer.make_writer().write_all(b"fixture\n").unwrap();
        }
        assert!(path.exists());
        assert!(PathBuf::from(format!("{}.3", path.display())).exists());
        assert!(!PathBuf::from(format!("{}.4", path.display())).exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
