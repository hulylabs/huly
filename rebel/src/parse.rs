// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use std::str::CharIndices;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParserError<E> {
    #[error("end of input")]
    EndOfInput,
    #[error("unexpected character: `{0}`")]
    UnexpectedChar(char),
    #[error("integer overflow")]
    IntegerOverflow,
    #[error("unexpected error")]
    UnexpectedError,
    #[error("collector error: `{0}`")]
    CollectorError(E),
    #[error("empty word")]
    EmptyWord,
}

#[derive(Debug, PartialEq)]
pub enum WordKind {
    Word,
    SetWord,
    GetWord,
}

pub trait Collector {
    type Error;

    fn string(&mut self, string: &str) -> Result<(), Self::Error>;
    fn word(&mut self, kind: WordKind, word: &str) -> Result<(), Self::Error>;
    fn integer(&mut self, value: i32) -> Result<(), Self::Error>;
    fn begin_block(&mut self) -> Result<(), Self::Error>;
    fn end_block(&mut self) -> Result<(), Self::Error>;

    fn begin_path(&mut self) -> Result<(), Self::Error>;
    fn end_path(&mut self) -> Result<(), Self::Error>;
}

pub struct Parser<'a, C>
where
    C: Collector,
{
    input: &'a str,
    cursor: CharIndices<'a>,
    collector: &'a mut C,
    in_path: bool,
}

impl<'a, C> Parser<'a, C>
where
    C: Collector,
{
    pub fn new(input: &'a str, collector: &'a mut C) -> Self {
        Self {
            input,
            collector,
            cursor: input.char_indices(),
            in_path: false,
        }
    }

    pub fn parse_block(&mut self) -> Result<(), ParserError<C::Error>> {
        self.collector
            .begin_block()
            .map_err(ParserError::CollectorError)?;
        self.parse()?;
        self.collector
            .end_block()
            .map_err(ParserError::CollectorError)
    }

    fn skip_whitespace(&mut self) -> Option<(usize, char)> {
        while let Some((pos, char)) = self.cursor.next() {
            if char.is_ascii_whitespace() {
                continue;
            } else if char == ';' {
                // Skip comment until newline
                for (_, c) in self.cursor.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            } else {
                return Some((pos, char));
            }
        }
        None
    }

    fn parse_string(&mut self, pos: usize) -> Result<Option<char>, ParserError<C::Error>> {
        let _start_pos = pos + 1; // Skip the opening quote
        let mut result = String::new();
        let mut escaped = false;

        while let Some((_, char)) = self.cursor.next() {
            if escaped {
                // Handle escape sequences
                let escaped_char = match char {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    _ => return Err(ParserError::UnexpectedChar(char)),
                };
                result.push(escaped_char);
                escaped = false;
            } else if char == '\\' {
                escaped = true;
            } else if char == '"' {
                // End of string
                return self
                    .collector
                    .string(&result)
                    .map(|()| None)
                    .map_err(ParserError::CollectorError);
            } else {
                result.push(char);
            }
        }

        // If we get here, we never found the closing quote
        Err(ParserError::EndOfInput)
    }

    fn collect_word(
        &mut self,
        symbol: &str,
        kind: WordKind,
        consumed: Option<char>,
    ) -> Result<Option<char>, C::Error> {
        if let Some('/') = consumed {
            if self.in_path == false {
                self.in_path = true;
                self.collector.begin_path()?;
            }
        }
        self.collector.word(kind, symbol).map(|_| consumed)
    }

    fn parse_word(&mut self, start_pos: usize) -> Result<Option<char>, ParserError<C::Error>> {
        let mut kind = WordKind::Word;

        let consumed = loop {
            match self.cursor.next() {
                Some((pos, char)) => match char {
                    ':' => {
                        if pos == start_pos {
                            kind = WordKind::GetWord;
                        } else {
                            kind = WordKind::SetWord;
                            break Some(char);
                        }
                    }
                    ']' | '/' => break Some(char),
                    c if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '?' => {}
                    c if c.is_ascii_whitespace() => break Some(char),
                    _ => return Err(ParserError::UnexpectedChar(char)),
                },
                None => break None,
            }
        };

        let pos = self.cursor.offset() - if consumed.is_some() { 1 } else { 0 };
        if pos == start_pos {
            return Err(ParserError::EmptyWord);
        }
        let symbol = self
            .input
            .get(start_pos..pos)
            .ok_or(ParserError::UnexpectedError)?;

        self.collect_word(symbol, kind, consumed)
            .map_err(ParserError::CollectorError)
    }

    fn parse_number(&mut self, char: char) -> Result<Option<char>, ParserError<C::Error>> {
        let mut value: i32 = 0;
        let mut is_negative = false;
        let mut has_digits = false;
        let mut consumed = None;

        match char {
            '+' => {}
            '-' => {
                is_negative = true;
            }
            c if c.is_ascii_digit() => {
                value = c.to_digit(10).ok_or(ParserError::UnexpectedError)? as i32;
                has_digits = true;
            }
            _ => return Err(ParserError::UnexpectedChar(char)),
        }

        for (_, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_digit() => {
                    has_digits = true;
                    let digit = c.to_digit(10).ok_or(ParserError::UnexpectedError)? as i32;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(digit))
                        .ok_or(ParserError::IntegerOverflow)?;
                }
                ']' => {
                    consumed = Some(char);
                    break;
                }
                _ => break,
            }
        }
        if !has_digits {
            return Err(ParserError::EndOfInput);
        }
        if is_negative {
            value = value.checked_neg().ok_or(ParserError::IntegerOverflow)?;
        }
        self.collector
            .integer(value)
            .map(|_| consumed)
            .map_err(ParserError::CollectorError)
    }

    fn process_block_end(&mut self, consumed: Option<char>) -> Result<(), C::Error> {
        match consumed {
            Some('/') => {}
            _ => {
                if self.in_path {
                    self.in_path = false;
                    self.collector.end_path()?;
                }
            }
        }
        if let Some(']') = consumed {
            self.collector.end_block()?;
        }
        Ok(())
    }

    fn parse(&mut self) -> Result<(), ParserError<C::Error>> {
        while let Some((pos, char)) = self.skip_whitespace() {
            let consumed = match char {
                '[' => self
                    .collector
                    .begin_block()
                    .map(|()| None)
                    .map_err(ParserError::CollectorError)?,
                ']' => Some(char),
                '"' => self.parse_string(pos)?,
                c if c.is_ascii_alphabetic() => self.parse_word(pos)?,
                c if c.is_ascii_digit() || c == '+' || c == '-' => self.parse_number(c)?,
                _ => return Err(ParserError::UnexpectedChar(char)),
            };
            self.process_block_end(consumed)
                .map_err(ParserError::CollectorError)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Collector, Parser, WordKind};

    #[derive(PartialEq, Debug)]
    struct TestCollector {
        pub strings: Vec<String>,
        pub words: Vec<(WordKind, String)>,
        pub integers: Vec<i32>,
    }

    impl Collector for TestCollector {
        type Error = ();

        fn string(&mut self, string: &str) -> Result<(), Self::Error> {
            self.strings.push(string.to_string());
            Ok(())
        }

        fn word(&mut self, kind: WordKind, word: &str) -> Result<(), Self::Error> {
            self.words.push((kind, word.to_string()));
            Ok(())
        }

        fn integer(&mut self, value: i32) -> Result<(), Self::Error> {
            self.integers.push(value);
            Ok(())
        }

        fn begin_block(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn end_block(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn begin_path(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn end_path(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn test_comments_are_ignored() {
        let input = r#"
                ; this is a comment
                word1 ; this is a comment
                "string" ; another comment
                123 ; numeric comment
                ; full line comment
                word2
            "#;

        let mut collector = TestCollector {
            strings: vec![],
            words: vec![],
            integers: vec![],
        };

        let mut parser = Parser::new(input, &mut collector);
        parser.parse().unwrap();

        assert_eq!(
            collector.words,
            vec![
                (WordKind::Word, "word1".to_string()),
                (WordKind::Word, "word2".to_string()),
            ]
        );
        assert_eq!(collector.strings, vec!["string"]);
        assert_eq!(collector.integers, vec![123]);
    }

    #[test]
    fn test_escaped_characters_in_strings() {
        let input = r#"
            "Hello\nWorld"
            "Tab\tCharacter"
            "Quotes: \"quoted\""
            "Backslash: \\"
            "Carriage Return: \r"
            "Mixed: \t\r\n\"\\"
        "#;

        let mut collector = TestCollector {
            strings: vec![],
            words: vec![],
            integers: vec![],
        };

        let mut parser = Parser::new(input, &mut collector);
        parser.parse().unwrap();

        assert_eq!(
            collector.strings,
            vec![
                "Hello\nWorld",
                "Tab\tCharacter",
                "Quotes: \"quoted\"",
                "Backslash: \\",
                "Carriage Return: \r",
                "Mixed: \t\r\n\"\\"
            ]
        );
    }

    #[test]
    fn test_string_with_escaped_quotes() {
        let input = r#""This string has \"escaped quotes\"" "#;

        let mut collector = TestCollector {
            strings: vec![],
            words: vec![],
            integers: vec![],
        };

        let mut parser = Parser::new(input, &mut collector);
        parser.parse().unwrap();

        assert_eq!(
            collector.strings,
            vec!["This string has \"escaped quotes\""]
        );
    }

    #[test]
    fn test_string_with_escaped_newlines() {
        let input = r#""Line1\nLine2\nLine3""#;

        let mut collector = TestCollector {
            strings: vec![],
            words: vec![],
            integers: vec![],
        };

        let mut parser = Parser::new(input, &mut collector);
        parser.parse().unwrap();

        assert_eq!(collector.strings, vec!["Line1\nLine2\nLine3"]);
    }
}
