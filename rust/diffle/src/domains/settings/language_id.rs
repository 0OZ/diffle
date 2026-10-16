//! A language diffle can start a language server for.

use std::fmt;

/// The value is the LSP `languageId` a server is told in `didOpen`, so it must be the spec's
/// spelling, not ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LanguageId {
    C,
    Cpp,
    Go,
    Haskell,
    Java,
    Javascript,
    JavascriptReact,
    Json,
    Jsonc,
    Lua,
    Nix,
    Ocaml,
    Php,
    Python,
    Ruby,
    Rust,
    ShellScript,
    Swift,
    Terraform,
    Toml,
    Typescript,
    TypescriptReact,
    Yaml,
    Zig,
}

impl LanguageId {
    pub const ALL: [Self; 24] = [
        Self::C,
        Self::Cpp,
        Self::Go,
        Self::Haskell,
        Self::Java,
        Self::Javascript,
        Self::JavascriptReact,
        Self::Json,
        Self::Jsonc,
        Self::Lua,
        Self::Nix,
        Self::Ocaml,
        Self::Php,
        Self::Python,
        Self::Ruby,
        Self::Rust,
        Self::ShellScript,
        Self::Swift,
        Self::Terraform,
        Self::Toml,
        Self::Typescript,
        Self::TypescriptReact,
        Self::Yaml,
        Self::Zig,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Go => "go",
            Self::Haskell => "haskell",
            Self::Java => "java",
            Self::Javascript => "javascript",
            Self::JavascriptReact => "javascriptreact",
            Self::Json => "json",
            Self::Jsonc => "jsonc",
            Self::Lua => "lua",
            Self::Nix => "nix",
            Self::Ocaml => "ocaml",
            Self::Php => "php",
            Self::Python => "python",
            Self::Ruby => "ruby",
            Self::Rust => "rust",
            Self::ShellScript => "shellscript",
            Self::Swift => "swift",
            Self::Terraform => "terraform",
            Self::Toml => "toml",
            Self::Typescript => "typescript",
            Self::TypescriptReact => "typescriptreact",
            Self::Yaml => "yaml",
            Self::Zig => "zig",
        }
    }

    /// The language named by `text`, in the spelling `as_str` gives.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|language| language.as_str() == text)
    }
}

impl fmt::Display for LanguageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_round_trips_through_its_wire_spelling() {
        for language in LanguageId::ALL {
            assert_eq!(LanguageId::parse(language.as_str()), Some(language));
        }
    }

    #[test]
    fn is_sorted_by_spelling_and_unknown_names_are_refused() {
        let spellings: Vec<_> = LanguageId::ALL
            .iter()
            .map(|language| language.as_str())
            .collect();
        let mut sorted = spellings.clone();
        sorted.sort_unstable();
        assert_eq!(spellings, sorted);
        assert_eq!(LanguageId::parse("klingon"), None);
    }
}
