// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, WordKind};
use std::str::CharIndices;

pub trait Collector {
    fn string(&mut self, string: &str) -> Result<(), CoreError>;
    fn word(&mut self, kind: WordKind, word: &str);
    fn integer(&mut self, value: i32);
    fn begin_block(&mut self);
    fn end_block(&mut self) -> Result<(), CoreError>;
}

pub struct Parser<'a, C>
where
    C: Collector,
{
    input: &'a str,
    cursor: CharIndices<'a>,
    collector: &'a mut C,
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
        }
    }

    fn skip_whitespace(&mut self) -> Option<(usize, char)> {
        for (pos, char) in self.cursor.by_ref() {
            if !char.is_ascii_whitespace() {
                return Some((pos, char));
            }
        }
        None
    }

    fn parse_string(&mut self, pos: usize) -> Result<(), CoreError> {
        let start_pos = pos + 1; // Skip the opening quote
        for (pos, char) in self.cursor.by_ref() {
            if char == '"' {
                return self.collector.string(
                    self.input
                        .get(start_pos..pos)
                        .ok_or(CoreError::EndOfInput)?,
                );
            }
        }
        Err(CoreError::EndOfInput)
    }

    fn parse_word(&mut self, start_pos: usize) -> Result<bool, CoreError> {
        for (pos, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {}
                ':' => {
                    self.collector.word(
                        WordKind::SetWord,
                        self.input
                            .get(start_pos..pos)
                            .ok_or(CoreError::EndOfInput)?,
                    );
                    return Ok(false);
                }
                c if c.is_ascii_whitespace() || c == ']' => {
                    self.collector.word(
                        WordKind::Word,
                        self.input
                            .get(start_pos..pos)
                            .ok_or(CoreError::EndOfInput)?,
                    );
                    return Ok(c == ']');
                }
                _ => return Err(CoreError::UnexpectedChar(char)),
            }
        }
        self.collector.word(
            WordKind::Word,
            self.input.get(start_pos..).ok_or(CoreError::EndOfInput)?,
        );
        Ok(false)
    }

    fn parse_number(&mut self, char: char) -> Result<bool, CoreError> {
        let mut value: i32 = 0;
        let mut is_negative = false;
        let mut has_digits = false;
        let mut end_of_block = false;

        match char {
            '+' => {}
            '-' => {
                is_negative = true;
            }
            c if c.is_ascii_digit() => {
                value = c.to_digit(10).ok_or(CoreError::InternalError)? as i32;
                has_digits = true;
            }
            _ => return Err(CoreError::UnexpectedChar(char)),
        }

        for (_, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_digit() => {
                    has_digits = true;
                    let digit = c.to_digit(10).ok_or(CoreError::InternalError)? as i32;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(digit))
                        .ok_or(CoreError::IntegerOverflow)?;
                }
                ']' => {
                    end_of_block = true;
                    break;
                }
                _ => break,
            }
        }
        if !has_digits {
            return Err(CoreError::EndOfInput);
        }
        if is_negative {
            value = value.checked_neg().ok_or(CoreError::IntegerOverflow)?;
        }
        self.collector.integer(value);
        Ok(end_of_block)
    }

    pub fn parse(&mut self) -> Result<(), CoreError> {
        while let Some((pos, char)) = self.skip_whitespace() {
            match char {
                '[' => self.collector.begin_block(),
                ']' => self.collector.end_block()?,
                '"' => self.parse_string(pos)?,
                c if c.is_ascii_alphabetic() => {
                    if self.parse_word(pos)? {
                        self.collector.end_block()?;
                    }
                }
                c if c.is_ascii_digit() || c == '+' || c == '-' => {
                    if self.parse_number(c)? {
                        self.collector.end_block()?;
                    }
                }
                _ => return Err(CoreError::UnexpectedChar(char)),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::WordKind;
    use std::cell::RefCell;

    // A simple test collector that records parser events
    struct TestCollector {
        events: RefCell<Vec<ParseEvent>>,
    }

    #[derive(Clone, Debug)]
    enum ParseEvent {
        String(String),
        Word(WordKind, String),
        Integer(i32),
        BeginBlock,
        EndBlock,
    }

    impl TestCollector {
        fn new() -> Self {
            Self {
                events: RefCell::new(Vec::new()),
            }
        }

        fn events(&self) -> Vec<ParseEvent> {
            self.events.borrow().clone()
        }
    }

    impl Collector for TestCollector {
        fn string(&mut self, string: &str) -> Result<(), CoreError> {
            self.events.borrow_mut().push(ParseEvent::String(string.to_string()));
            Ok(())
        }

        fn word(&mut self, kind: WordKind, word: &str) {
            self.events.borrow_mut().push(ParseEvent::Word(kind, word.to_string()));
        }

        fn integer(&mut self, value: i32) {
            self.events.borrow_mut().push(ParseEvent::Integer(value));
        }

        fn begin_block(&mut self) {
            self.events.borrow_mut().push(ParseEvent::BeginBlock);
        }

        fn end_block(&mut self) -> Result<(), CoreError> {
            self.events.borrow_mut().push(ParseEvent::EndBlock);
            Ok(())
        }
    }

    #[test]
    fn test_parse_simple_word() {
        let mut collector = TestCollector::new();
        let mut parser = Parser::new("hello", &mut collector);
        
        parser.parse().unwrap();
        
        let events = collector.events();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            ParseEvent::Word(kind, word) => {
                assert!(matches!(kind, WordKind::Word));
                assert_eq!(word, "hello");
            }
            _ => panic!("Expected Word event"),
        }
    }

    #[test]
    fn test_parse_set_word() {
        let mut collector = TestCollector::new();
        let mut parser = Parser::new("value:", &mut collector);
        
        parser.parse().unwrap();
        
        let events = collector.events();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            ParseEvent::Word(kind, word) => {
                assert!(matches!(kind, WordKind::SetWord));
                assert_eq!(word, "value");
            }
            _ => panic!("Expected SetWord event"),
        }
    }

    #[test]
    fn test_parse_integer() {
        let mut collector = TestCollector::new();
        let mut parser = Parser::new("42", &mut collector);
        
        parser.parse().unwrap();
        
        let events = collector.events();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            ParseEvent::Integer(value) => {
                assert_eq!(*value, 42);
            }
            _ => panic!("Expected Integer event"),
        }
    }

    #[test]
    fn test_parse_negative_integer() {
        let mut collector = TestCollector::new();
        let mut parser = Parser::new("-123", &mut collector);
        
        parser.parse().unwrap();
        
        let events = collector.events();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            ParseEvent::Integer(value) => {
                assert_eq!(*value, -123);
            }
            _ => panic!("Expected Integer event"),
        }
    }

    #[test]
    fn test_parse_string() {
        let mut collector = TestCollector::new();
        let mut parser = Parser::new("\"hello world\"", &mut collector);
        
        parser.parse().unwrap();
        
        let events = collector.events();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            ParseEvent::String(content) => {
                assert_eq!(content, "hello world");
            }
            _ => panic!("Expected String event"),
        }
    }

    #[test]
    fn test_parse_block() {
        let mut collector = TestCollector::new();
        let mut parser = Parser::new("[hello 42]", &mut collector);
        
        parser.parse().unwrap();
        
        let events = collector.events();
        assert_eq!(events.len(), 4);
        
        assert!(matches!(events[0], ParseEvent::BeginBlock));
        
        match &events[1] {
            ParseEvent::Word(kind, word) => {
                assert!(matches!(kind, WordKind::Word));
                assert_eq!(word, "hello");
            }
            _ => panic!("Expected Word event"),
        }
        
        match &events[2] {
            ParseEvent::Integer(value) => {
                assert_eq!(*value, 42);
            }
            _ => panic!("Expected Integer event"),
        }
        
        assert!(matches!(events[3], ParseEvent::EndBlock));
    }

    #[test]
    fn test_parse_complex_expression() {
        let mut collector = TestCollector::new();
        let mut parser = Parser::new(
            r#"[
                x: 10
                y: 20
                "result"
                [nested 30]
            ]"#, 
            &mut collector
        );
        
        parser.parse().unwrap();
        
        let events = collector.events();
        // Print the events for debugging
        println!("Parsed events: {:?}", events);
        assert_eq!(events.len(), 11); // Updated count: 11 events
        
        assert!(matches!(events[0], ParseEvent::BeginBlock));
        
        match &events[1] {
            ParseEvent::Word(kind, word) => {
                assert!(matches!(kind, WordKind::SetWord));
                assert_eq!(word, "x");
            }
            _ => panic!("Expected SetWord event"),
        }
        
        match &events[2] {
            ParseEvent::Integer(value) => {
                assert_eq!(*value, 10);
            }
            _ => panic!("Expected Integer event"),
        }
        
        match &events[3] {
            ParseEvent::Word(kind, word) => {
                assert!(matches!(kind, WordKind::SetWord));
                assert_eq!(word, "y");
            }
            _ => panic!("Expected SetWord event"),
        }
        
        match &events[4] {
            ParseEvent::Integer(value) => {
                assert_eq!(*value, 20);
            }
            _ => panic!("Expected Integer event"),
        }
        
        match &events[5] {
            ParseEvent::String(content) => {
                assert_eq!(content, "result");
            }
            _ => panic!("Expected String event"),
        }
        
        assert!(matches!(events[6], ParseEvent::BeginBlock));
        
        match &events[7] {
            ParseEvent::Word(kind, word) => {
                assert!(matches!(kind, WordKind::Word));
                assert_eq!(word, "nested");
            }
            _ => panic!("Expected Word event"),
        }
        
        match &events[8] {
            ParseEvent::Integer(value) => {
                assert_eq!(*value, 30);
            }
            _ => panic!("Expected Integer event"),
        }
        
        assert!(matches!(events[9], ParseEvent::EndBlock));
        assert!(matches!(events[10], ParseEvent::EndBlock));
    }
}
