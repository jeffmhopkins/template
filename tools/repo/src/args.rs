//! A small argument parser: positional arguments plus `--flag value` and `--switch` options.

use std::collections::BTreeMap;

use anyhow::{Result, bail};

/// Parsed arguments.
#[derive(Debug, Default)]
pub struct Args {
    pub positional: Vec<String>,
    values: BTreeMap<String, String>,
    switches: Vec<String>,
}

impl Args {
    /// Parses `args`. `with_value` lists the flags that take a value, `switches` those that
    /// don't; any other `--flag` is an error.
    pub fn parse(args: &[String], with_value: &[&str], switches: &[&str]) -> Result<Self> {
        let mut out = Self::default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if let Some(name) = a.strip_prefix("--") {
                let (name, inline) = match name.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_owned())),
                    None => (name, None),
                };
                if with_value.contains(&name) {
                    let v = match inline {
                        Some(v) => v,
                        None => match it.next() {
                            Some(v) => v.clone(),
                            None => bail!("`--{name}` needs a value"),
                        },
                    };
                    if out.values.insert(name.to_owned(), v).is_some() {
                        bail!("`--{name}` given twice");
                    }
                } else if switches.contains(&name) && inline.is_none() {
                    out.switches.push(name.to_owned());
                } else {
                    bail!("unknown option `--{name}`");
                }
            } else {
                out.positional.push(a.clone());
            }
        }
        Ok(out)
    }

    /// The value of `--name`, if given.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// The value of `--name`, or an error naming `usage`.
    pub fn required(&self, name: &str, usage: &str) -> Result<&str> {
        match self.value(name) {
            Some(v) => Ok(v),
            None => bail!("missing `--{name}`\nusage: {usage}"),
        }
    }

    /// Whether `--name` was given.
    pub fn switch(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name)
    }

    /// Exactly `n` positional arguments, or an error naming `usage`.
    pub fn exactly(&self, n: usize, usage: &str) -> Result<&[String]> {
        if self.positional.len() != n {
            bail!("usage: {usage}");
        }
        Ok(&self.positional)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_values_switches_and_positionals() {
        let a = Args::parse(
            &strings(&["x", "--by", "agent:a", "--note=n n", "--draft", "y"]),
            &["by", "note"],
            &["draft"],
        )
        .unwrap();
        assert_eq!(a.positional, vec!["x", "y"]);
        assert_eq!(a.value("by"), Some("agent:a"));
        assert_eq!(a.value("note"), Some("n n"));
        assert!(a.switch("draft"));
        assert!(!a.switch("other"));
    }

    #[test]
    fn rejects_unknown_missing_and_repeated() {
        assert!(Args::parse(&strings(&["--nope"]), &[], &[]).is_err());
        assert!(Args::parse(&strings(&["--by"]), &["by"], &[]).is_err());
        assert!(Args::parse(&strings(&["--by", "a", "--by", "b"]), &["by"], &[]).is_err());
        let a = Args::parse(&strings(&["a"]), &[], &[]).unwrap();
        assert!(a.exactly(2, "u").is_err());
        assert!(a.required("by", "u").is_err());
    }
}
