//! The command line.

/// How to call the program.
pub(crate) const USAGE: &str = "usage: regis-render [--font FILE] [--teletext ENGINE] [--page 0|1] [--macrograph-reports] [--frames TICKS [--frame-step K] [--script FILE|-]] INPUT OUTPUT.png|OUTPUT.ppm";

/// What the command line asked for.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Arguments {
    pub(crate) font: Option<String>,
    /// The engine's checkout, to load the 55 provided sets from.
    pub(crate) teletext: Option<String>,
    /// The page to write; the displayed one if not given.
    pub(crate) page: Option<usize>,
    pub(crate) macrograph_reports: bool,
    /// The ticks to play into a sequence of images (animation.md,
    /// ANIM-18); a single picture if not given.
    pub(crate) frames: Option<u32>,
    /// An image is written every this many ticks, and always the last.
    pub(crate) frame_step: u32,
    /// The file of timed operations; `-` for standard input.
    pub(crate) script: Option<String>,
    pub(crate) input: String,
    pub(crate) output: String,
}

impl Arguments {
    pub(crate) fn parse(mut words: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut arguments = Self { frame_step: 1, ..Self::default() };
        let mut files = Vec::new();
        while let Some(word) = words.next() {
            match word.as_str() {
                "--font" => arguments.font = Some(words.next().ok_or("--font needs a file")?),
                "--teletext" => {
                    arguments.teletext = Some(words.next().ok_or("--teletext needs the engine's checkout")?);
                }
                "--page" => {
                    let number = words.next().ok_or("--page needs a number")?;
                    arguments.page = Some(number.parse().map_err(|_| format!("--page: {number} is not a number"))?);
                }
                "--macrograph-reports" => arguments.macrograph_reports = true,
                "--frames" => arguments.frames = Some(ticks(words.next(), "--frames")?),
                "--frame-step" => arguments.frame_step = ticks(words.next(), "--frame-step")?,
                "--script" => arguments.script = Some(words.next().ok_or("--script needs a file")?),
                option if option.starts_with("--") => return Err(format!("unknown option {option}")),
                _ => files.push(word),
            }
        }
        let [input, output] = <[String; 2]>::try_from(files).map_err(|_| "an input and an output file are needed")?;
        arguments.input = input;
        arguments.output = output;
        if arguments.frames.is_none() && (arguments.script.is_some() || arguments.frame_step != 1) {
            return Err("--script and --frame-step need --frames".into());
        }
        if arguments.frames.is_some() && arguments.page.is_some() {
            return Err("--frames plays the picture on show; --page cannot be given with it".into());
        }
        Ok(arguments)
    }
}

/// A count of ticks: 1 to 36,000 (ANIM-18).
fn ticks(word: Option<String>, option: &str) -> Result<u32, String> {
    let word = word.ok_or_else(|| format!("{option} needs a number of ticks"))?;
    let range = 1..=crate::play::MAX_TICKS;
    word.parse().ok().filter(|ticks| range.contains(ticks)).ok_or_else(|| format!("{option}: {word} is not 1 to 36000"))
}

#[cfg(test)]
mod tests {
    use super::Arguments;

    fn parse(words: &[&str]) -> Result<Arguments, String> {
        Arguments::parse(words.iter().map(ToString::to_string))
    }

    #[test]
    fn options_and_two_files_are_read() {
        let arguments = parse(&["--font", "f", "--teletext", "engine", "--page", "1", "in", "out.png"]).unwrap();
        assert_eq!(arguments.font.as_deref(), Some("f"));
        assert_eq!(arguments.teletext.as_deref(), Some("engine"));
        assert_eq!(arguments.page, Some(1));
        assert_eq!((arguments.input.as_str(), arguments.output.as_str()), ("in", "out.png"));
    }

    #[test]
    fn the_options_of_an_image_sequence_are_read() {
        let arguments = parse(&["--frames", "120", "--frame-step", "6", "--script", "-", "in", "out.png"]).unwrap();
        assert_eq!((arguments.frames, arguments.frame_step, arguments.script.as_deref()), (Some(120), 6, Some("-")));
        assert_eq!(parse(&["--frames", "36000", "in", "out.png"]).unwrap().frame_step, 1);
        for bad in [
            &["--frames", "0", "in", "out"][..],
            &["--frames", "36001", "in", "out"],
            &["--frames", "5", "--frame-step", "0", "in", "out"],
            &["--frame-step", "2", "in", "out"],
            &["--script", "s", "in", "out"],
            &["--frames", "5", "--page", "1", "in", "out"],
            &["--frames"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn mistakes_are_refused() {
        assert!(parse(&["in"]).is_err());
        assert!(parse(&["--bogus", "in", "out"]).is_err());
        assert!(parse(&["--page", "x", "in", "out"]).is_err());
        assert!(parse(&["--font"]).is_err());
        assert!(parse(&["--teletext"]).is_err());
    }
}
