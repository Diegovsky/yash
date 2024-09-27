#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct History {
    past_lines: Vec<String>,
    draft_line: Option<String>,
    index: usize,
}

impl History {
    pub fn from_lines(lines: Vec<String>) -> Self {
        Self {
            past_lines: lines,
            ..Default::default()
        }
    }
    pub fn push(&mut self, line: impl Into<String>) {
        let line = line.into();
        if !line.is_empty() {
            self.past_lines.push(line);
        }
    }
    pub fn unselect(&mut self) {
        self.draft_line = None;
        self.index = 0;
    }
    fn get_line<'a, 'b>(&self, index: usize) -> Option<&str> {
        if index == 0 {
            return self.draft_line.as_deref();
        } else {
            self.past_lines
                .get(self.past_lines.len().checked_sub(index)?)
        }
        .map(String::as_ref)
    }
    pub fn scroll(&mut self, last_prompt: &str, offset: isize) -> Option<&str> {
        if self.index == 0 {
            self.draft_line = Some(last_prompt.into());
        }
        let new_index = (self.index as isize + offset) as usize;
        if self.get_line(new_index).is_some() {
            self.index = new_index;
        }
        self.get_line(self.index)
    }
    pub fn lines(&self) -> &[String] {
        &self.past_lines
    }
}
