use std::collections::BTreeMap;
use std::env;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug)]
pub struct Options {
    values: BTreeMap<String, String>,
}

impl Options {
    /// Parses the current process arguments as named values.
    ///
    /// # Errors
    ///
    /// Returns an error for positional, missing, or duplicate values.
    pub fn from_env() -> Result<Self, String> {
        let mut values = BTreeMap::new();
        let mut args = env::args().skip(1);
        while let Some(name) = args.next() {
            let Some(name) = name.strip_prefix("--") else {
                return Err(format!("expected a named option, found `{name}`"));
            };
            let value = args
                .next()
                .ok_or_else(|| format!("option `--{name}` needs a value"))?;
            if values.insert(name.to_owned(), value).is_some() {
                return Err(format!("option `--{name}` was supplied more than once"));
            }
        }
        Ok(Self { values })
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Rejects options outside the workload's accepted set.
    ///
    /// # Errors
    ///
    /// Returns an error naming the first unknown option.
    pub fn only(&self, names: &[&str]) -> Result<(), String> {
        if let Some(name) = self
            .values
            .keys()
            .find(|name| !names.contains(&name.as_str()))
        {
            return Err(format!("unknown option `--{name}`"));
        }
        Ok(())
    }

    /// Parses a required option value.
    ///
    /// # Errors
    ///
    /// Returns an error when the option is absent or malformed.
    pub fn required<T>(&self, name: &str) -> Result<T, String>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.values
            .get(name)
            .ok_or_else(|| format!("required option `--{name}` is missing"))?
            .parse()
            .map_err(|error| format!("invalid value for `--{name}`: {error}"))
    }

    /// Parses an optional value, using `default` when it is absent.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied value is malformed.
    pub fn optional<T>(&self, name: &str, default: T) -> Result<T, String>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.text(name).map_or(Ok(default), |value| {
            value
                .parse()
                .map_err(|error| format!("invalid value for `--{name}`: {error}"))
        })
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
}
