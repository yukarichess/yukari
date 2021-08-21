use std::io::{self};
use std::io::prelude::*;

/// Communication helper that helps to encapsulate IO to/from the GUI/driver
/// allowing us to add logging properly
pub struct Comms {
    input: Box<dyn BufRead>,
    output: Box<dyn Write>
}

impl Comms {
    /// Creates a comms that uses the stdin for input and stdout for output
    pub fn stdio() -> Self {
        Self {
            input: Box::new(io::stdin_locked()),
            output: Box::new(io::stdout())
        }
    }

    /// Reads a message from the input, messages are seperated by newlines
    /// and read sequentially (hopefully) without any buffering to avoid blocking
    /// Returns true for "read successful" and false for EOF
    /// Note: Probably this will be made non-public since the generic split message handler is better
    pub fn read_line(&mut self, buf: &mut String) -> bool {
        // Read a line of input
        buf.clear();
        loop {
            match self.input.read_line(buf) {
                // Ok(0) means EOF
                Ok(count) if count == 0 => return false,
                // We want to try again since an empty buffer isn't helpful
                Ok(_) if buf.trim().len() == 0 => continue,
                // any other case means that we have meaningful non-zero data
                Ok(_) => return true,
                // Error means something went wrong since read_line handles blocking for more input
                Err(_) => return false
            }
        }
    }

    /// Pushes a message out to the output (note: unstable might change type of buf)
    /// # Panics
    /// Panics when we can't actually send the whole message for whatever reason
    pub fn send_message<T: AsRef<[u8]>>(&mut self, buf: T) {
        self.output.write_all(buf.as_ref()).unwrap();
    }
}