use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Language {
    #[default]
    En,
    Ru,
}

impl Language {
    pub fn text(self, english: &'static str, russian: &'static str) -> &'static str {
        match self {
            Self::En => english,
            Self::Ru => russian,
        }
    }

    pub fn toggle(&mut self) {
        *self = match self {
            Self::En => Self::Ru,
            Self::Ru => Self::En,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn language_toggle_switches_both_ways() {
        let mut language = Language::En;
        language.toggle();
        assert_eq!(language, Language::Ru);
        language.toggle();
        assert_eq!(language, Language::En);
    }
}
