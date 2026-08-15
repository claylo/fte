//! Filename template expansion for `fte split`.

use std::fmt;

/// Values a filename template can reference.
#[derive(Debug, Clone, Copy)]
pub struct Vars<'a> {
    /// Source document stem.
    pub book: &'a str,
    /// Chapter index; 0 is the frontmatter file.
    pub n: usize,
    /// GFM slug of the chapter title.
    pub slug: &'a str,
    /// Raw chapter title.
    pub title: &'a str,
    /// Source spine-entry stem.
    pub src: &'a str,
    /// Zero-padding width applied to `n`.
    pub pad: usize,
}

/// A template that could not be expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// The template names a token that does not exist.
    UnknownToken(String),
    /// A `{` was opened and never closed.
    Unterminated,
}

impl fmt::Display for TemplateError {
    // keep in sync with cmd/split.rs: both arms are matched by substring
    // ("unknown template token", "unterminated") to classify a template
    // failure as Kind::ConfigError.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownToken(token) => write!(
                f,
                "unknown template token `{{{token}}}`; \
                 expected one of book, n, slug, title, src"
            ),
            Self::Unterminated => write!(f, "unterminated `{{` in template"),
        }
    }
}

impl std::error::Error for TemplateError {}

/// Expand a filename template.
///
/// An unrecognized token is an error rather than a literal passthrough: a
/// typo'd `{chapter}` should fail loudly, not write a file with braces in its
/// name.
///
/// # Errors
///
/// Returns [`TemplateError`] for unknown tokens and unterminated braces.
pub fn render(tpl: &str, vars: &Vars<'_>) -> Result<String, TemplateError> {
    let mut out = String::with_capacity(tpl.len());
    let mut rest = tpl;

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or(TemplateError::Unterminated)?;
        let token = &after[..close];

        match token {
            "book" => out.push_str(vars.book),
            "n" => out.push_str(&format!("{:0width$}", vars.n, width = vars.pad)),
            "slug" => out.push_str(vars.slug),
            "title" => out.push_str(vars.title),
            "src" => out.push_str(vars.src),
            other => return Err(TemplateError::UnknownToken(other.to_string())),
        }

        rest = &after[close + 1..];
    }

    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars<'static> {
        Vars {
            book: "oconnor-2022",
            n: 3,
            slug: "walking-in-the-dark",
            title: "Walking in the Dark",
            src: "ch03",
            pad: 2,
        }
    }

    #[test]
    fn every_token_expands() {
        assert_eq!(
            render("{book}/{n}-{slug}.md", &vars()).unwrap(),
            "oconnor-2022/03-walking-in-the-dark.md"
        );
        assert_eq!(render("{title}", &vars()).unwrap(), "Walking in the Dark");
        assert_eq!(render("{src}.md", &vars()).unwrap(), "ch03.md");
    }

    #[test]
    fn padding_is_configurable() {
        for (pad, expected) in [(1, "3"), (2, "03"), (3, "003"), (4, "0003")] {
            let v = Vars { pad, ..vars() };
            assert_eq!(render("{n}", &v).unwrap(), expected);
        }
    }

    #[test]
    fn the_flat_convention_expands() {
        assert_eq!(
            render("{book}-ch{n}.md", &vars()).unwrap(),
            "oconnor-2022-ch03.md"
        );
    }

    #[test]
    fn literal_text_passes_through() {
        assert_eq!(render("plain.md", &vars()).unwrap(), "plain.md");
        assert_eq!(render("", &vars()).unwrap(), "");
    }

    #[test]
    fn an_unknown_token_is_an_error() {
        let err = render("{chapter}.md", &vars()).unwrap_err();
        assert!(matches!(err, TemplateError::UnknownToken(ref t) if t == "chapter"));
        assert!(err.to_string().contains("chapter"));
    }

    #[test]
    fn an_unterminated_token_is_an_error() {
        assert!(matches!(
            render("{book.md", &vars()).unwrap_err(),
            TemplateError::Unterminated
        ));
    }
}
