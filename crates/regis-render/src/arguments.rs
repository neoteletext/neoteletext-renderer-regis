//! The command line.

/// How to call the program.
pub(crate) const USAGE: &str =
    "usage: regis-render [--font FILE] [--page 0|1] [--macrograph-reports] INPUT OUTPUT.png|OUTPUT.ppm";

/// What the command line asked for.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Arguments {
    pub(crate) font: Option<String>,
    /// The page to write; the displayed one if not given.
    pub(crate) page: Option<usize>,
    pub(crate) macrograph_reports: bool,
    pub(crate) input: String,
    pub(crate) output: String,
}

impl Arguments {
    pub(crate) fn parse(mut words: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut arguments = Self::default();
        let mut files = Vec::new();
        while let Some(word) = words.next() {
            match word.as_str() {
                "--font" => arguments.font = Some(words.next().ok_or("--font needs a file")?),
                "--page" => {
                    let number = words.next().ok_or("--page needs a number")?;
                    arguments.page = Some(number.parse().map_err(|_| format!("--page: {number} is not a number"))?);
                }
                "--macrograph-reports" => arguments.macrograph_reports = true,
                option if option.starts_with("--") => return Err(format!("unknown option {option}")),
                _ => files.push(word),
            }
        }
        let [input, output] = <[String; 2]>::try_from(files).map_err(|_| "an input and an output file are needed")?;
        arguments.input = input;
        arguments.output = output;
        Ok(arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::Arguments;

    fn parse(words: &[&str]) -> Result<Arguments, String> {
        Arguments::parse(words.iter().map(ToString::to_string))
    }

    #[test]
    fn options_and_two_files_are_read() {
        let arguments = parse(&["--font", "f", "--page", "1", "in", "out.png"]).unwrap();
        assert_eq!(arguments.font.as_deref(), Some("f"));
        assert_eq!(arguments.page, Some(1));
        assert_eq!((arguments.input.as_str(), arguments.output.as_str()), ("in", "out.png"));
    }

    #[test]
    fn mistakes_are_refused() {
        assert!(parse(&["in"]).is_err());
        assert!(parse(&["--bogus", "in", "out"]).is_err());
        assert!(parse(&["--page", "x", "in", "out"]).is_err());
        assert!(parse(&["--font"]).is_err());
    }
}
