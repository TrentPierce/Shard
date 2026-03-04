use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Shared ring-buffer of formatted log lines. Max 500 lines, oldest dropped first.
pub type LogBuffer = Arc<Mutex<VecDeque<String>>>;

pub fn new_log_buffer() -> LogBuffer {
    Arc::new(Mutex::new(VecDeque::with_capacity(500)))
}

/// Per-event writer — accumulates bytes and pushes a line on flush/drop.
pub struct LogEventWriter {
    buffer: LogBuffer,
    bytes: Vec<u8>,
}

impl Write for LogEventWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let text = String::from_utf8_lossy(&self.bytes);
        let line = text.trim_end_matches(['\n', '\r']).to_string();
        if !line.is_empty() {
            if let Ok(mut q) = self.buffer.lock() {
                if q.len() >= 500 {
                    q.pop_front();
                }
                q.push_back(line);
            }
        }
        self.bytes.clear();
        Ok(())
    }
}

impl Drop for LogEventWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// MakeWriter implementation — creates a LogEventWriter per tracing event.
pub struct LogMakeWriter(pub LogBuffer);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogMakeWriter {
    type Writer = LogEventWriter;
    fn make_writer(&'a self) -> Self::Writer {
        LogEventWriter { buffer: self.0.clone(), bytes: Vec::new() }
    }
}
