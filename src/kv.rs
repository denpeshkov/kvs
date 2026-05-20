use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{self, Path, PathBuf},
};

use postcard::{self, Error::DeserializeUnexpectedEnd};
use serde::{self, Deserialize, Serialize};

use crate::error::Result;

/// Maximum size of active log file before rotation in bytes.
pub const MAX_FILE_SIZE: u64 = 1024 * 1024;

/// The threshold of bytes that triggers the compaction process.
pub const COMPACTION_THRESHOLD: u64 = 4 * 1024 * 1024;

/// The `KvStore` stores string key/value pairs.
///
/// Key/value pairs are stored in a `HashMap` in memory and not persisted to
/// disk.
pub struct KvStore {
    /// Base directory path where all log files are stored.
    dir: PathBuf,
    /// Bytes in log files eligible for deletion during compaction.
    uncompacted_bytes: u64,
    // Identifier of the current active file.
    current_log_id: u64,
    /// Map of log IDs to their respective readers.
    readers: HashMap<u64, PosReader<io::BufReader<fs::File>>>,
    /// Writer for the active log file.
    writer: PosWriter<io::BufWriter<fs::File>>,
    /// In-memory index mapping keys to their latest position.
    index: HashMap<String, IndexEntry>,
}

impl KvStore {
    /// Creates a `KvStore`.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created, log files cannot be
    /// read, or log entries are corrupted.
    pub fn open<P: AsRef<path::Path>>(dir: P) -> Result<Self> {
        // TODO: add file lock

        let dir = dir.as_ref();
        fs::create_dir_all(dir)?;

        let mut log_ids = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let path = entry.path();
                if path.extension() == Some("log".as_ref())
                    && let Some(id) = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .and_then(|s| s.parse::<u64>().ok())
                {
                    log_ids.push(id);
                }
            }
        }
        log_ids.sort_unstable();
        let current_log_id = log_ids.last().copied().unwrap_or_default();

        let mut uncompacted_bytes = 0;
        let mut index = HashMap::new();
        for log_id in log_ids {
            let mut reader = create_reader(dir.join(format!("{log_id}.log")))?;
            for log_entry in LogIterator::new(&mut reader) {
                let LogEntry { cmd, value_position, value_size } = log_entry?;
                match cmd {
                    Command::Set { key, .. } => {
                        if let Some(old_cmd) =
                            index.insert(key, IndexEntry { log_id, value_position, value_size })
                        {
                            uncompacted_bytes += old_cmd.value_size;
                        }
                    }
                    Command::Remove { key } => {
                        if let Some(old_cmd) = index.remove(&key) {
                            uncompacted_bytes += old_cmd.value_size;
                        }
                        // The "remove" command itself can be compacted.
                        uncompacted_bytes += value_size;
                    }
                }
            }
        }

        let mut readers = HashMap::new();
        // Add a reader for the current log.
        let reader = create_reader(dir.join(format!("{current_log_id}.log")))?;
        readers.insert(current_log_id, reader);

        // Only add readers for logs referenced in the index.
        for ie in index.values() {
            if let std::collections::hash_map::Entry::Vacant(e) = readers.entry(ie.log_id) {
                let reader = create_reader(dir.join(format!("{}.log", ie.log_id)))?;
                e.insert(reader);
            }
        }

        let mut writer = create_writer(dir.join(format!("{current_log_id}.log")))?;
        writer.seek(SeekFrom::End(0))?; // Manually seek to the end, otherwise stream_positon returns 0 (even with append).

        Ok(Self {
            dir: dir.to_path_buf(),
            current_log_id,
            readers,
            writer,
            index,
            uncompacted_bytes,
        })
    }

    /// Sets the value of a string key to a string.
    ///
    /// If the key already exists, the previous value will be overwritten.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails or compaction/rotation encounters
    /// an I/O error.
    pub fn set(&mut self, key: String, value: String) -> Result<()> {
        if self.uncompacted_bytes > COMPACTION_THRESHOLD {
            self.compact_logs()?;
        } else if self.writer.stream_position()? > MAX_FILE_SIZE {
            self.rotate_active_log()?;
        }

        let cmd = Command::Set { key, value };

        let value_position = self.writer.stream_position()?;
        postcard::to_io(&cmd, &mut self.writer)?; // FIXME: optimize with size
        self.writer.flush()?;

        let Command::Set { key, .. } = cmd else { unreachable!() }; // Move out of enum variant.
        let value_size = self.writer.stream_position()? - value_position;
        if let Some(old_cmd) = self
            .index
            .insert(key, IndexEntry { log_id: self.current_log_id, value_position, value_size })
        {
            self.uncompacted_bytes += old_cmd.value_size;
        }
        Ok(())
    }

    /// Gets the string value of a given string key.
    ///
    /// Returns `None` if the given key does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the log file cannot be read or the entry is
    /// corrupted.
    #[allow(clippy::needless_pass_by_value)]
    #[allow(clippy::missing_panics_doc)]
    pub fn get(&mut self, key: String) -> Result<Option<String>> {
        let Some(entry) = self.index.get(&key) else {
            return Ok(None);
        };

        let reader = self.readers.get_mut(&entry.log_id).expect("reader must exist for log file");
        reader.seek(SeekFrom::Start(entry.value_position))?;

        #[allow(clippy::cast_possible_truncation)]
        let (cmd, _) = postcard::from_io((reader, &mut vec![0u8; entry.value_size as usize]))?;
        let Command::Set { value, .. } = cmd else {
            return Err(crate::KvsError::UnexpectedCommandType);
        };
        Ok(Some(value))
    }

    /// Remove a given key.
    ///
    /// # Errors
    ///
    /// Returns [`KvsError::KeyNotFound`](crate::KvsError::KeyNotFound) if the
    /// key does not exist, or an I/O error if the write fails.
    pub fn remove(&mut self, key: String) -> Result<()> {
        if let Some(old_cmd) = self.index.remove(&key) {
            let value_position = self.writer.stream_position()?;
            postcard::to_io(&Command::Remove { key }, &mut self.writer)?;
            self.writer.flush()?;

            let value_size = self.writer.stream_position()? - value_position;

            self.uncompacted_bytes += old_cmd.value_size + value_size;

            return Ok(());
        }
        Err(crate::KvsError::KeyNotFound)
    }

    fn compact_logs(&mut self) -> Result<()> {
        self.current_log_id += 1;

        let compacted_log_path = self.dir.join(format!("{}.log", self.current_log_id));
        let mut compaction_writer = create_writer(&compacted_log_path)?;
        let compaction_reader = create_reader(compacted_log_path)?;
        self.readers.insert(self.current_log_id, compaction_reader);

        let mut new_position = 0; // Position in the new (compacted) log file.
        // Copy live entries.
        for entry in self.index.values_mut() {
            let reader =
                self.readers.get_mut(&entry.log_id).expect("reader must exist for log file");
            reader.seek(SeekFrom::Start(entry.value_position))?;
            io::copy(&mut reader.take(entry.value_size), &mut compaction_writer)?;

            entry.value_position = new_position;
            new_position += entry.value_size;
            entry.log_id = self.current_log_id;
        }
        compaction_writer.flush()?;

        // Remove old log files.
        let old_log_ids: Vec<_> =
            self.readers.keys().filter(|&&log_id| log_id < self.current_log_id).copied().collect();
        for log_id in old_log_ids {
            self.readers.remove(&log_id);
            fs::remove_file(self.dir.join(format!("{log_id}.log")))?;
        }

        self.rotate_active_log()?;
        self.uncompacted_bytes = 0;

        Ok(())
    }

    fn rotate_active_log(&mut self) -> Result<()> {
        self.current_log_id += 1;
        let new_log_path = self.dir.join(format!("{}.log", self.current_log_id));

        self.readers.insert(self.current_log_id, create_reader(&new_log_path)?);
        self.writer = create_writer(&new_log_path)?;
        Ok(())
    }
}

fn create_reader(path: impl AsRef<Path>) -> Result<PosReader<io::BufReader<fs::File>>> {
    let f = fs::OpenOptions::new().create(true).read(true).append(true).open(path)?;
    PosReader::new(io::BufReader::new(f))
}

fn create_writer(path: impl AsRef<Path>) -> Result<PosWriter<io::BufWriter<fs::File>>> {
    let f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    PosWriter::new(io::BufWriter::new(f))
}

#[derive(Serialize, Deserialize, Debug)]
enum Command {
    Set { key: String, value: String },
    Remove { key: String },
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Set { key, value } => write!(f, "set {key} {value}"),
            Self::Remove { key } => write!(f, "rm {key}"),
        }
    }
}

#[derive(Debug)]
struct IndexEntry {
    /// Log ID containing the value.
    log_id: u64,
    /// Offset position of the value within the file.
    value_position: u64,
    /// Size of the value in bytes.
    value_size: u64,
}

impl Display for IndexEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id={} pos={} sz={}", self.log_id, self.value_position, self.value_size)
    }
}

#[derive(Debug)]
struct LogEntry {
    cmd: Command,
    value_position: u64,
    value_size: u64,
}

struct LogIterator<R> {
    reader: R,
    buf: [u8; 1024],
}

impl<R: io::Read + io::Seek> LogIterator<R> {
    const fn new(reader: R) -> Self {
        Self { reader, buf: [0; 1024] }
    }
}

impl<R: io::Read + io::Seek> Iterator for LogIterator<R> {
    type Item = Result<LogEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        let pos = match self.reader.stream_position() {
            Ok(pos) => pos,
            Err(err) => return Some(Err(err.into())),
        };

        let cmd: postcard::Result<(Command, _)> =
            postcard::from_io((&mut self.reader, &mut self.buf));

        match cmd {
            Ok((cmd, _)) => {
                let end_pos = match self.reader.stream_position() {
                    Ok(p) => p,
                    Err(err) => return Some(Err(err.into())),
                };
                Some(Ok(LogEntry { cmd, value_position: pos, value_size: end_pos - pos }))
            }
            Err(DeserializeUnexpectedEnd) => None,
            Err(err) => Some(Err(err.into())),
        }
    }
}

struct PosWriter<W> {
    writer: W,
    position: u64,
}

impl<W: io::Seek> PosWriter<W> {
    fn new(mut writer: W) -> Result<Self> {
        let position = writer.stream_position()?;
        Ok(Self { writer, position })
    }
}

impl<W: io::Write> io::Write for PosWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.writer.write(buf)?;
        self.position += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<W: io::Write + io::Seek> io::Seek for PosWriter<W> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.position = self.writer.seek(pos)?;
        Ok(self.position)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.position)
    }
}

struct PosReader<R> {
    reader: R,
    position: u64,
}

impl<R: io::Seek> PosReader<R> {
    fn new(mut reader: R) -> Result<Self> {
        let position = reader.stream_position()?;
        Ok(Self { reader, position })
    }
}

impl<R: io::Read> io::Read for PosReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.reader.read(buf)?;
        self.position += n as u64;
        Ok(n)
    }
}

impl<R: io::Seek> io::Seek for PosReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.position = self.reader.seek(pos)?;
        Ok(self.position)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.position)
    }
}
