#[derive(Default)]
pub struct Framer {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
}

impl Framer {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(pos) = self.read_line_end() {
            let line = {
                let bytes: Vec<u8> = self.buffer[..pos].to_vec();
                self.buffer.drain(..=pos);
                String::from_utf8_lossy(&bytes)
                    .trim_end_matches('\r')
                    .to_owned()
            };
            if line.is_empty() {
                if !self.data_lines.is_empty() {
                    events.push(self.data_lines.join("\n"));
                    self.data_lines.clear();
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("data:") {
                self.data_lines
                    .push(rest.strip_prefix(' ').unwrap_or(rest).to_owned());
            }
        }
        events
    }

    fn read_line_end(&self) -> Option<usize> {
        self.buffer.iter().position(|byte| *byte == b'\n')
    }
}

#[cfg(test)]
mod tests {
    use super::Framer;

    #[test]
    fn splits_events_across_chunks() {
        let mut framer = Framer::default();
        assert!(framer.push(b"data: {\"a").is_empty());
        assert_eq!(framer.push(b"\": 1}\n\n"), vec!["{\"a\": 1}"]);
        assert_eq!(framer.push(b"data: [DONE]\n\n"), vec!["[DONE]"]);
    }

    #[test]
    fn ignores_comments_and_event_lines() {
        let mut framer = Framer::default();
        assert_eq!(
            framer.push(b": ping\nevent: x\ndata: hel lo\n\n"),
            vec!["hel lo"]
        );
    }
}
