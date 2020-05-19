mod lex {
    use super::{Token, TokenMetadata};
    use std::{iter::Enumerate, str::Lines};

    fn _consume_str<'a>(input: &'a str, what: &str) -> Option<&'a str> {
        if input.starts_with(what) {
            Some(&input[what.len()..])
        } else {
            None
        }
    }

    fn consume_any(input: &str, what: impl Fn(char) -> bool) -> (&str, &str, usize) {
        let pos = input.find(|c| !what(c)).unwrap_or_else(|| input.len());
        let (o, i) = input.split_at(pos);
        (o, i, pos)
    }

    pub fn consume_token(input: &String) -> (Token, &str, usize, usize) {
        let mut input = input.as_str();

        let start = input
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(input.chars().count());
        input = &input[start..];

        let mut chars = input.chars();
        let cur = match chars.next() {
            Some(c) => c,
            None => return (Token::End, input, start, start + 1),
        };
        match cur {
            ':' => {
                input = chars.as_str();
                if chars.next() == Some(':') {
                    (Token::DoubleColon, chars.as_str(), start, start + 2)
                } else {
                    (Token::Separator(cur), input, start, start + 1)
                }
            }
            ';' | ',' | '.' => (Token::Separator(cur), chars.as_str(), start, start + 1),
            '(' | ')' | '{' | '}' | '[' | ']' => {
                (Token::Paren(cur), chars.as_str(), start, start + 1)
            }
            '<' | '>' => {
                input = chars.as_str();
                let next = chars.next();
                if next == Some('=') {
                    (
                        Token::LogicalOperation(cur),
                        chars.as_str(),
                        start,
                        start + 1,
                    )
                } else if next == Some(cur) {
                    (Token::ShiftOperation(cur), chars.as_str(), start, start + 2)
                } else {
                    (Token::Paren(cur), input, start, start + 1)
                }
            }
            '0'..='9' => {
                let (number, rest, pos) =
                    consume_any(input, |c| (c >= '0' && c <= '9' || c == '.'));
                if let Some(_) = number.find('.') {
                    input = chars.as_str();

                    if (
                        chars.next().map(|c| c.to_lowercase().next().unwrap()),
                        chars.next().map(|c| c.to_lowercase().next().unwrap()),
                    ) == (Some('l'), Some('f'))
                    {
                        (
                            Token::Double(number.parse().unwrap()),
                            chars.as_str(),
                            start,
                            start + pos + 2,
                        )
                    } else {
                        (
                            Token::Float(number.parse().unwrap()),
                            input,
                            start,
                            start + pos,
                        )
                    }
                } else {
                    (
                        Token::Integral(number.parse().unwrap()),
                        rest,
                        start,
                        start + pos,
                    )
                }
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let (word, rest, pos) = consume_any(input, |c| c.is_alphanumeric() || c == '_');
                (Token::Word(String::from(word)), rest, start, start + pos)
            }
            '+' | '-' => {
                input = chars.as_str();
                match chars.next() {
                    Some('=') => (Token::OpAssign(cur), chars.as_str(), start, start + 2),
                    Some(next) if cur == next => {
                        (Token::Sufix(cur), chars.as_str(), start, start + 2)
                    }
                    _ => (Token::Operation(cur), input, start, start + 1),
                }
            }
            '%' | '^' => {
                input = chars.as_str();

                if chars.next() == Some('=') {
                    (Token::OpAssign(cur), chars.as_str(), start, start + 2)
                } else {
                    (Token::Operation(cur), input, start, start + 1)
                }
            }
            '!' => {
                input = chars.as_str();

                if chars.next() == Some('=') {
                    (
                        Token::LogicalOperation(cur),
                        chars.as_str(),
                        start,
                        start + 2,
                    )
                } else {
                    (Token::Operation(cur), input, start, start + 1)
                }
            }
            '*' => {
                input = chars.as_str();
                match chars.next() {
                    Some('=') => (Token::OpAssign(cur), chars.as_str(), start, start + 2),
                    Some('/') => (
                        Token::MultiLineCommentClose,
                        chars.as_str(),
                        start,
                        start + 2,
                    ),
                    _ => (Token::Operation(cur), input, start, start + 1),
                }
            }
            '/' => {
                input = chars.as_str();
                match chars.next() {
                    Some('=') => (Token::OpAssign(cur), chars.as_str(), start, start + 2),
                    Some('/') => (Token::LineComment, chars.as_str(), start, start + 2),
                    Some('*') => (
                        Token::MultiLineCommentOpen,
                        chars.as_str(),
                        start,
                        start + 2,
                    ),
                    _ => (Token::Operation(cur), input, start, start + 1),
                }
            }
            '=' | '&' | '|' => {
                input = chars.as_str();
                if chars.next() == Some(cur) {
                    (
                        Token::LogicalOperation(cur),
                        chars.as_str(),
                        start,
                        start + 2,
                    )
                } else {
                    (Token::Operation(cur), input, start, start + 1)
                }
            }
            '#' => {
                input = chars.as_str();
                if chars.next() == Some(cur) {
                    (Token::TokenPasting, chars.as_str(), start, start + 2)
                } else {
                    (Token::Preprocessor, input, start, start + 1)
                }
            }
            '~' => (Token::Operation(cur), chars.as_str(), start, start + 1),
            '?' => (Token::Selection, chars.as_str(), start, start + 1),
            _ => (Token::Unknown(cur), chars.as_str(), start, start + 1),
        }
    }

    #[derive(Clone)]
    pub struct Lexer<'a> {
        lines: Enumerate<Lines<'a>>,
        input: String,
        line: usize,
        offset: usize,
    }

    impl<'a> Lexer<'a> {
        pub fn new(input: &'a str) -> Self {
            let mut lines = input.lines().enumerate();
            let (line, input) = lines.next().unwrap_or((0, ""));
            let mut input = String::from(input);

            while input.chars().last() == Some('\\') {
                if let Some((_, next)) = lines.next() {
                    input.pop();
                    input.push_str(next);
                } else {
                    break;
                }
            }

            Lexer {
                lines,
                input,
                line,
                offset: 0,
            }
        }

        #[must_use]
        pub fn next(&mut self) -> TokenMetadata {
            let (token, rest, start, end) = consume_token(&self.input);

            if token == Token::End {
                match self.lines.next() {
                    Some((line, input)) => {
                        let mut input = String::from(input);

                        while input.chars().last() == Some('\\') {
                            if let Some((_, next)) = self.lines.next() {
                                input.pop();
                                input.push_str(next);
                            } else {
                                break;
                            }
                        }

                        self.input = input;
                        self.line = line;
                        self.offset = 0;
                        self.next()
                    }
                    None => TokenMetadata {
                        token: Token::End,
                        line: self.line,
                        chars: self.offset + start..end + self.offset,
                    },
                }
            } else {
                self.input = String::from(rest);
                let metadata = TokenMetadata {
                    token,
                    line: self.line,
                    chars: self.offset + start..end + self.offset,
                };
                self.offset += end;
                metadata
            }
        }

        #[must_use]
        pub fn peek(&mut self) -> TokenMetadata {
            self.clone().next()
        }
    }
}

use crate::FastHashMap;
use std::{
    fmt,
    ops::{Deref, Range},
};

#[derive(Debug, Clone)]
pub struct TokenMetadata {
    pub token: Token,
    pub line: usize,
    pub chars: Range<usize>,
}

impl Deref for TokenMetadata {
    type Target = Token;

    fn deref(&self) -> &Token {
        &self.token
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Separator(char),
    DoubleColon,
    Paren(char),
    Integral(usize),
    Float(f32),
    Double(f64),
    Word(String),
    Operation(char),
    OpAssign(char),
    LogicalOperation(char),
    ShiftOperation(char),
    Unknown(char),
    LineComment,
    MultiLineCommentOpen,
    MultiLineCommentClose,
    Preprocessor,
    End,
    Selection,
    Sufix(char),
    TokenPasting,
}

impl Token {
    pub fn type_to_string(&self) -> String {
        match self {
            Token::Separator(separator) => separator.to_string(),
            Token::DoubleColon => ":".to_string(),
            Token::Paren(paren) => paren.to_string(),
            Token::Integral(_) => "integer".to_string(),
            Token::Float(_) => "float".to_string(),
            Token::Double(_) => "double".to_string(),
            Token::Word(_) => "word".to_string(),
            Token::Operation(op) => op.to_string(),
            Token::OpAssign(op) => format!("{}=", op),
            Token::LogicalOperation(op) => format!("{}=", op),
            Token::ShiftOperation(op) => format!("{0}{0}", op),
            Token::Unknown(_) => "unknown".to_string(),
            Token::LineComment => "//".to_string(),
            Token::MultiLineCommentOpen => "/*".to_string(),
            Token::MultiLineCommentClose => "*/".to_string(),
            Token::Preprocessor => "#".to_string(),
            Token::End => "EOF".to_string(),
            Token::Selection => "?".to_string(),
            Token::Sufix(op) => format!("{0}{0}", op),
            Token::TokenPasting => "##".to_string(),
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Separator(sep) => write!(f, "{}", sep),
            Token::DoubleColon => write!(f, ":"),
            Token::Paren(paren) => write!(f, "{}", paren),
            Token::Integral(int) => write!(f, "{}", int),
            Token::Float(float) => write!(f, "{}", float),
            Token::Double(double) => write!(f, "{}", double),
            Token::Word(word) => write!(f, "{}", word),
            Token::Operation(op) => write!(f, "{}", op),
            Token::OpAssign(op) => write!(f, "{}=", op),
            Token::LogicalOperation(op) => write!(f, "{0}=", op),
            Token::ShiftOperation(op) => write!(f, "{0}{0}", op),
            Token::Unknown(unknown) => write!(f, "{}", unknown),
            Token::LineComment => write!(f, "//"),
            Token::MultiLineCommentOpen => write!(f, "/*"),
            Token::MultiLineCommentClose => write!(f, "*/"),
            Token::Preprocessor => write!(f, "#"),
            Token::End => write!(f, ""),
            Token::Selection => write!(f, "?"),
            Token::Sufix(op) => write!(f, "{0}{0}", op),
            Token::TokenPasting => write!(f, "##"),
        }
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    UnexpectedToken {
        expected: Vec<Token>,
        got: TokenMetadata,
    },
    ExpectedEOL {
        got: TokenMetadata,
    },
    UnknownPragma {
        pragma: String,
    },
    ExtensionNotSupported {
        extension: String,
    },
    AllExtensionsEnabled,
    ExtensionUnknownBehavior {
        behavior: String,
    },
    UnsupportedVersion {
        version: usize,
    },
    UnsupportedProfile {
        profile: String,
    },
    UnknownProfile {
        profile: String,
    },
    UnknownPreprocessorDirective {
        directive: String,
    },
    ReservedMacro,
    EOL,
    EOF,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::UnexpectedToken { expected, got } => write!(
                f,
                "Unexpected token:\nexpected: {}\ngot: {}",
                expected
                    .iter()
                    .map(|token| {
                        let mut type_string = token.type_to_string();
                        type_string.push_str(" |");
                        type_string
                    })
                    .collect::<String>(),
                got.token.to_string()
            ),
            ErrorKind::ExpectedEOL { got } => {
                write!(f, "Expected end of line:\ngot: {}", got.token.to_string())
            }
            ErrorKind::UnknownPragma { pragma } => write!(f, "Unknown pragma: {}", pragma),
            ErrorKind::ExtensionNotSupported { extension } => {
                write!(f, "The extension \"{}\" is not supported", extension)
            }
            ErrorKind::AllExtensionsEnabled => {
                write!(f, "All extensions can't be require or enable")
            }
            ErrorKind::ExtensionUnknownBehavior { behavior } => write!(
                f,
                "The extension behavior must be one of require|enable|warn|disable got: {}",
                behavior
            ),
            ErrorKind::UnsupportedVersion { version } => write!(
                f,
                "The version {} isn't supported use either 450 or 460",
                version
            ),
            ErrorKind::UnsupportedProfile { profile } => {
                write!(f, "The profile {} isn't supported use core", profile)
            }
            ErrorKind::UnknownProfile { profile } => {
                write!(f, "The profile {} isn't defined use core", profile)
            }
            ErrorKind::UnknownPreprocessorDirective { directive } => {
                write!(f, "The preprocessor directive {} isn't defined", directive)
            }
            ErrorKind::ReservedMacro => write!(f, "Macro can't begin with GL_"),
            ErrorKind::EOL => write!(f, "End of line"),
            ErrorKind::EOF => write!(f, "End of file"),
        }
    }
}

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

pub fn preprocess(input: &str) -> Result<String, Error> {
    let lexer = lex::Lexer::new(input);

    let stripped_tokens = parse_comments(lexer)?;
    let tokens = parse_preprocessor(stripped_tokens)?;

    let mut line = 0;
    let mut start = 0;

    Ok(tokens.into_iter().fold(String::new(), |mut acc, token| {
        if token.line - line != 0 {
            acc.push_str(&"\n".repeat(token.line - line));
            start = 0;
            line = token.line;
        }

        acc.push_str(&" ".repeat(token.chars.start - start));

        acc.push_str(&token.token.to_string());

        start = token.chars.end;

        acc
    }))
}

fn parse_comments(mut lexer: lex::Lexer) -> Result<Vec<TokenMetadata>, Error> {
    let mut tokens = Vec::new();

    loop {
        let token = lexer.next();

        match token.token {
            Token::MultiLineCommentOpen => {
                let mut token = lexer.next();
                while Token::MultiLineCommentClose != token.token {
                    match token.token {
                        Token::End => {
                            return Err(Error {
                                kind: ErrorKind::EOF,
                            })
                        }
                        _ => {}
                    }

                    token = lexer.next();
                }
            }
            Token::LineComment => {
                while token.line != lexer.peek().line || Token::End != lexer.peek().token {
                    let _ = lexer.next();
                }
            }
            Token::End => {
                tokens.push(token);
                break;
            }
            _ => tokens.push(token),
        }
    }

    Ok(tokens)
}

fn parse_preprocessor(stripped_tokens: Vec<TokenMetadata>) -> Result<Vec<TokenMetadata>, Error> {
    let mut lexer = stripped_tokens.into_iter().peekable();

    let mut tokens = Vec::new();
    let mut macros: FastHashMap<String, Vec<TokenMetadata>> = FastHashMap::default();
    let mut line_offset = 0i32;

    let mut offset = (0, 0);

    macros.insert(
        String::from("GL_SPIRV"),
        vec![TokenMetadata {
            token: Token::Integral(100),
            line: 0,
            chars: 0..1,
        }],
    );
    macros.insert(
        String::from("VULKAN"),
        vec![TokenMetadata {
            token: Token::Integral(100),
            line: 0,
            chars: 0..1,
        }],
    );

    macro_rules! get_macro {
        ($name:expr, $token:expr) => {
            match $name.as_str() {
                "__LINE__" => Some(vec![TokenMetadata {
                    token: Token::Integral(($token.line as i32 + line_offset + 1) as usize),
                    line: 0,
                    chars: 0..1,
                }]),
                "__FILE__" => Some(vec![TokenMetadata {
                    token: Token::Integral(0),
                    line: 0,
                    chars: 0..1,
                }]),
                "__VERSION__" => Some(vec![TokenMetadata {
                    token: Token::Integral(460),
                    line: 0,
                    chars: 0..1,
                }]), /* TODO */
                other => macros.get(other).cloned().map(|mut tokens| {
                    let mut start = tokens[0].chars.start;
                    let mut offset = 0;

                    for token in tokens.iter_mut() {
                        token.line = $token.line;

                        let length = token.chars.end - token.chars.start;

                        offset += token.chars.start - start;
                        start = token.chars.start;

                        token.chars.start = $token.chars.start + offset;

                        token.chars.end = length + $token.chars.start + offset;
                    }
                    tokens
                }),
            }
        };
    }

    loop {
        let token = lexer.next().ok_or(Error {
            kind: ErrorKind::EOF,
        })?;

        match token.token {
            Token::Preprocessor => {
                let preprocessor_op_token = if token.line
                    == lexer
                        .peek()
                        .ok_or(Error {
                            kind: ErrorKind::EOF,
                        })?
                        .line
                {
                    lexer.next().ok_or(Error {
                        kind: ErrorKind::EOF,
                    })?
                } else {
                    continue;
                };

                let preprocessor_op = if let Token::Word(name) = preprocessor_op_token.token {
                    name
                } else {
                    return Err(Error {
                        kind: ErrorKind::UnexpectedToken {
                            expected: vec![Token::Word(String::new())],
                            got: preprocessor_op_token,
                        },
                    });
                };

                match preprocessor_op.as_str() {
                    "define" => {
                        let macro_name_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        let macro_name = if let Token::Word(name) = macro_name_token.token {
                            name
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Word(String::new())],
                                    got: macro_name_token,
                                },
                            });
                        };

                        if macro_name.starts_with("GL_") {
                            return Err(Error {
                                kind: ErrorKind::ReservedMacro,
                            });
                        }

                        let mut macro_tokens = Vec::new();

                        while token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            let macro_token = lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?;

                            match macro_token.token {
                                Token::Word(ref word) => match get_macro!(word, &token) {
                                    Some(stream) => macro_tokens.append(&mut stream.clone()),
                                    None => macro_tokens.push(macro_token),
                                },
                                _ => macro_tokens.push(macro_token),
                            }
                        }

                        macros.insert(macro_name, macro_tokens);
                    }
                    "undef" => {
                        let macro_name_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        let macro_name = if let Token::Word(name) = macro_name_token.token {
                            name
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Word(String::new())],
                                    got: macro_name_token,
                                },
                            });
                        };

                        macros.remove(&macro_name);
                    }
                    "if" => unimplemented!(),
                    "ifdef" => unimplemented!(),
                    "ifndef" => unimplemented!(),
                    "else" => unimplemented!(),
                    "elif" => unimplemented!(),
                    "endif" => unimplemented!(),
                    "error" => {
                        let mut error_token = lexer.next();

                        let first_byte = error_token
                            .as_ref()
                            .ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                            .chars
                            .start;

                        let mut error_message = String::new();

                        while error_token.as_ref().map(|t| t.line) == Some(token.line) {
                            let error_msg_token = error_token.as_ref().unwrap();

                            let spacing = error_msg_token.chars.start
                                - first_byte
                                - error_message.chars().count();

                            error_message.push_str(&" ".repeat(spacing));
                            error_message.push_str(error_msg_token.token.to_string().as_str());

                            error_token = lexer.next()
                        }

                        panic!(error_message)
                    }
                    "pragma" => {
                        let pragma_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        let pragma = if let Token::Word(name) = pragma_token.token {
                            name
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Word(String::new())],
                                    got: pragma_token,
                                },
                            });
                        };

                        match pragma.as_str() {
                            "optimize" => {
                                let open_paren_token = if token.line
                                    == lexer
                                        .peek()
                                        .ok_or(Error {
                                            kind: ErrorKind::EOF,
                                        })?
                                        .line
                                {
                                    lexer.next().ok_or(Error {
                                        kind: ErrorKind::EOF,
                                    })?
                                } else {
                                    return Err(Error {
                                        kind: ErrorKind::EOL,
                                    });
                                };

                                if Token::Paren('(') != open_paren_token.token {
                                    return Err(Error {
                                        kind: ErrorKind::UnexpectedToken {
                                            expected: vec![Token::Paren('(')],
                                            got: open_paren_token,
                                        },
                                    });
                                };

                                let status_token = if token.line
                                    == lexer
                                        .peek()
                                        .ok_or(Error {
                                            kind: ErrorKind::EOF,
                                        })?
                                        .line
                                {
                                    lexer.next().ok_or(Error {
                                        kind: ErrorKind::EOF,
                                    })?
                                } else {
                                    return Err(Error {
                                        kind: ErrorKind::EOL,
                                    });
                                };

                                let _ = if let Token::Word(name) = status_token.token {
                                    name
                                } else {
                                    return Err(Error {
                                        kind: ErrorKind::UnexpectedToken {
                                            expected: vec![Token::Word(String::new())],
                                            got: status_token,
                                        },
                                    });
                                };

                                let close_paren_token = if token.line
                                    == lexer
                                        .peek()
                                        .ok_or(Error {
                                            kind: ErrorKind::EOF,
                                        })?
                                        .line
                                {
                                    lexer.next().ok_or(Error {
                                        kind: ErrorKind::EOF,
                                    })?
                                } else {
                                    return Err(Error {
                                        kind: ErrorKind::EOL,
                                    });
                                };

                                if Token::Paren(')') != close_paren_token.token {
                                    return Err(Error {
                                        kind: ErrorKind::UnexpectedToken {
                                            expected: vec![Token::Paren(')')],
                                            got: close_paren_token,
                                        },
                                    });
                                };
                            }
                            "debug" => {
                                let open_paren_token = if token.line
                                    == lexer
                                        .peek()
                                        .ok_or(Error {
                                            kind: ErrorKind::EOF,
                                        })?
                                        .line
                                {
                                    lexer.next().ok_or(Error {
                                        kind: ErrorKind::EOF,
                                    })?
                                } else {
                                    return Err(Error {
                                        kind: ErrorKind::EOL,
                                    });
                                };

                                if Token::Paren('(') != open_paren_token.token {
                                    return Err(Error {
                                        kind: ErrorKind::UnexpectedToken {
                                            expected: vec![Token::Paren('(')],
                                            got: open_paren_token,
                                        },
                                    });
                                };

                                let status_token = if token.line
                                    == lexer
                                        .peek()
                                        .ok_or(Error {
                                            kind: ErrorKind::EOF,
                                        })?
                                        .line
                                {
                                    lexer.next().ok_or(Error {
                                        kind: ErrorKind::EOF,
                                    })?
                                } else {
                                    return Err(Error {
                                        kind: ErrorKind::EOL,
                                    });
                                };

                                let _ = if let Token::Word(name) = status_token.token {
                                    name
                                } else {
                                    return Err(Error {
                                        kind: ErrorKind::UnexpectedToken {
                                            expected: vec![Token::Word(String::new())],
                                            got: status_token,
                                        },
                                    });
                                };

                                let close_paren_token = if token.line
                                    == lexer
                                        .peek()
                                        .ok_or(Error {
                                            kind: ErrorKind::EOF,
                                        })?
                                        .line
                                {
                                    lexer.next().ok_or(Error {
                                        kind: ErrorKind::EOF,
                                    })?
                                } else {
                                    return Err(Error {
                                        kind: ErrorKind::EOL,
                                    });
                                };

                                if Token::Paren(')') != close_paren_token.token {
                                    return Err(Error {
                                        kind: ErrorKind::UnexpectedToken {
                                            expected: vec![Token::Paren(')')],
                                            got: close_paren_token,
                                        },
                                    });
                                };
                            }
                            _ => {
                                return Err(Error {
                                    kind: ErrorKind::UnknownPragma { pragma },
                                })
                            }
                        }
                    }
                    "extension" => {
                        let extension_name_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        let extension_name = if let Token::Word(word) = extension_name_token.token {
                            word
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Word(String::new())],
                                    got: extension_name_token,
                                },
                            });
                        };

                        let separator_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        if separator_token.token != Token::DoubleColon {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::DoubleColon],
                                    got: separator_token,
                                },
                            });
                        }

                        let behavior_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        let behavior = if let Token::Word(word) = behavior_token.token {
                            word
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Word(String::new())],
                                    got: behavior_token,
                                },
                            });
                        };

                        match extension_name.as_str() {
                            "all" => match behavior.as_str() {
                                "require" | "enable" => {
                                    return Err(Error {
                                        kind: ErrorKind::AllExtensionsEnabled,
                                    })
                                }
                                "warn" | "disable" => {}
                                _ => {
                                    return Err(Error {
                                        kind: ErrorKind::ExtensionUnknownBehavior { behavior },
                                    })
                                }
                            },
                            _ => match behavior.as_str() {
                                "require" => {
                                    return Err(Error {
                                        kind: ErrorKind::ExtensionNotSupported {
                                            extension: extension_name,
                                        },
                                    })
                                }
                                "enable" | "warn" | "disable" => log::warn!(
                                    "Unsupported extensions was enabled: {}",
                                    extension_name
                                ),
                                _ => {
                                    return Err(Error {
                                        kind: ErrorKind::ExtensionUnknownBehavior { behavior },
                                    })
                                }
                            },
                        }
                    }
                    "version" => {
                        let version_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        let version = if let Token::Integral(int) = version_token.token {
                            int
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Integral(0)],
                                    got: version_token,
                                },
                            });
                        };

                        match version {
                            450 | 460 => {}
                            _ => {
                                return Err(Error {
                                    kind: ErrorKind::UnsupportedVersion { version },
                                })
                            }
                        };

                        let profile_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        let profile = if let Token::Word(word) = profile_token.token {
                            word
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Word(String::new())],
                                    got: profile_token,
                                },
                            });
                        };

                        match profile.as_str() {
                            "core" => macros.insert(
                                String::from("GL_core_profile"),
                                vec![TokenMetadata {
                                    token: Token::Integral(1),
                                    line: 0,
                                    chars: 0..1,
                                }],
                            ),
                            "compatibility" | "es" => {
                                return Err(Error {
                                    kind: ErrorKind::UnsupportedProfile { profile },
                                })
                            }
                            _ => {
                                return Err(Error {
                                    kind: ErrorKind::UnknownProfile { profile },
                                })
                            }
                        };
                    }
                    "line" => {
                        let line_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        let line = if let Token::Integral(int) = line_token.token {
                            int
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Integral(0)],
                                    got: line_token,
                                },
                            });
                        };

                        let source_string_token = if token.line
                            == lexer
                                .peek()
                                .ok_or(Error {
                                    kind: ErrorKind::EOF,
                                })?
                                .line
                        {
                            lexer.next().ok_or(Error {
                                kind: ErrorKind::EOF,
                            })?
                        } else {
                            return Err(Error {
                                kind: ErrorKind::EOL,
                            });
                        };

                        if let Token::Word(_) = source_string_token.token {
                        } else {
                            return Err(Error {
                                kind: ErrorKind::UnexpectedToken {
                                    expected: vec![Token::Word(String::new())],
                                    got: source_string_token,
                                },
                            });
                        }

                        line_offset = line as i32 - token.line as i32;
                    }
                    _ => {
                        return Err(Error {
                            kind: ErrorKind::UnknownPreprocessorDirective {
                                directive: preprocessor_op,
                            },
                        })
                    }
                }

                if lexer.peek().map(|t| t.line) == Some(token.line) {
                    return Err(Error {
                        kind: ErrorKind::ExpectedEOL {
                            got: lexer.next().unwrap(),
                        },
                    });
                }
            }
            Token::End => {
                let mut token = token;

                if offset.0 == token.line {
                    token.chars.start = (token.chars.start as isize + offset.1) as usize;
                    token.chars.end = (token.chars.end as isize + offset.1) as usize;
                }

                tokens.push(token);
                break;
            }
            Token::Word(ref word) => match get_macro!(word, &token) {
                Some(mut stream) => {
                    for macro_token in stream.iter_mut() {
                        if offset.0 == token.line {
                            macro_token.chars.start =
                                (macro_token.chars.start as isize + offset.1) as usize;
                            macro_token.chars.end =
                                (macro_token.chars.end as isize + offset.1) as usize;
                        }
                    }

                    offset.0 = stream.last().unwrap().line;
                    offset.1 = stream.last().unwrap().chars.end as isize - token.chars.end as isize;

                    tokens.append(&mut stream)
                }
                None => {
                    let mut token = token;

                    if offset.0 == token.line {
                        token.chars.start = (token.chars.start as isize + offset.1) as usize;
                        token.chars.end = (token.chars.end as isize + offset.1) as usize;
                    }

                    tokens.push(token)
                }
            },
            _ => {
                let mut token = token;

                if offset.0 == token.line {
                    token.chars.start = (token.chars.start as isize + offset.1) as usize;
                    token.chars.end = (token.chars.end as isize + offset.1) as usize;
                }

                tokens.push(token)
            }
        }
    }

    Ok(tokens)
}
