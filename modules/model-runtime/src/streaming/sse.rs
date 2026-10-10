// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Bounded SSE frame decoder; handles split UTF-8, CRLF and multiline data.
use crate::{
    ErrorKind,
    errors::{ModelError, Result},
};
#[derive(Default)]
pub(crate) struct Decoder {
    pending: Vec<u8>,
    data: Vec<u8>,
    limit: usize,
    ended: bool,
}
impl Decoder {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            ..Self::default()
        }
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
        if self.ended || bytes.len() > self.limit.saturating_sub(self.pending.len()) {
            return Err(ModelError::new(ErrorKind::OutputLimit));
        }
        self.pending.extend_from_slice(bytes);
        let mut frames = Vec::new();
        while let Some(index) = self.pending.iter().position(|b| *b == b'\n') {
            let line = self.pending.drain(..=index).collect::<Vec<_>>();
            self.line(line.strip_suffix(b"\n").unwrap_or(&line), &mut frames)?;
        }
        Ok(frames)
    }
    pub fn finish(&mut self) -> Result<Vec<Vec<u8>>> {
        let mut frames = Vec::new();
        let line = std::mem::take(&mut self.pending);
        if !line.is_empty() {
            self.line(&line, &mut frames)?
        }
        // SSE requires a blank line to dispatch a frame. Truncated data is not
        // upgraded to a complete success event merely because the socket ends.
        self.ended = true;
        Ok(frames)
    }
    fn line(&mut self, line: &[u8], frames: &mut Vec<Vec<u8>>) -> Result<()> {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            if !self.data.is_empty() {
                self.data.pop();
                frames.push(std::mem::take(&mut self.data));
            }
        } else if let Some(data) = line.strip_prefix(b"data:") {
            let data = data.strip_prefix(b" ").unwrap_or(data);
            if data.len() + 1 > self.limit.saturating_sub(self.data.len()) {
                return Err(ModelError::new(ErrorKind::OutputLimit));
            }
            self.data.extend_from_slice(data);
            self.data.push(b'\n');
        }
        Ok(())
    }
}
