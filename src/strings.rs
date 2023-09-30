use std::borrow::Cow;


impl crate::Shell {
    /// Unconditionally replaces all sequences of `$VAR` with a value for `VAR`.
    pub fn expand_vars<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let regex = crate::static_regex!(r#"\$(\w+)"#);
        regex.replace_all(text, move |captures: &regex::Captures| {
            self.get_var_or_env(&captures[1]).unwrap_or_default()
        })
    }
}
