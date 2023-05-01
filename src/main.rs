#![cfg_attr(
    debug_assertions,
    allow(dead_code, unused_imports, unreachable_code, unused_variables)
)]

use anyhow::Result;
use markdown::Span::*;
use markdown::*;
use std::collections::VecDeque;
use std::{env, fs, path::Path, process::ExitCode};

use lazy_static::lazy_static;
use regex::Captures;
use regex::Regex;
lazy_static! {
    static ref RE_SPLIT: Regex = Regex::new(r",|\?|!|:|;|\.$|\w{4,}\.").unwrap();
}

const CODE_WRAP_LENGTH: usize = 20;

#[derive(Debug)]
enum Lowered<'input> {
    /// Good Place to wrap line
    MaybeBreak,

    /// Text should continue in next line. Join with any following breaks
    Break,

    /// Insert 1 empty line here (depending on what came before, 1 or 2 `'\n'`
    EmptyLine,

    /// All lines after this get a prefix
    Prefix(&'static str),

    /// This line gets .0 as prefix, all lower lines get .1
    Prefix2(String, &'static str),

    /// Remove the latest prefix
    Pop,

    /// A String
    String(String),

    /// also a String
    Str(&'input str),

    /// a horizontal ruler
    Hr,
}

impl<'i> PartialEq for Lowered<'i> {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Lowered::MaybeBreak => {
                if let Lowered::MaybeBreak = other {
                    true
                } else {
                    false
                }
            }
            Lowered::Break => {
                if let Lowered::Break = other {
                    true
                } else {
                    false
                }
            }
            Lowered::EmptyLine => {
                if let Lowered::EmptyLine = other {
                    true
                } else {
                    false
                }
            }
            Lowered::Pop => {
                if let Lowered::Pop = other {
                    true
                } else {
                    false
                }
            }
            Lowered::Hr => {
                if let Lowered::Hr = other {
                    true
                } else {
                    false
                }
            }
            Lowered::Prefix(s) => {
                if let Lowered::Prefix(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Lowered::Prefix2(s, s2) => {
                if let Lowered::Prefix2(o, o2) = other {
                    s == o && s2 == o2
                } else {
                    false
                }
            }
            Lowered::String(s) => {
                if let Lowered::String(o) = other {
                    s == o
                } else {
                    if let Lowered::Str(o) = other {
                        s == o
                    } else {
                        false
                    }
                }
            }
            Lowered::Str(s) => {
                if let Lowered::String(o) = other {
                    s == o
                } else {
                    if let Lowered::Str(o) = other {
                        s == o
                    } else {
                        false
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct LoweredBuffer<'input> {
    buffer: Vec<Lowered<'input>>,
}

impl<'input> LoweredBuffer<'input> {
    fn new() -> Self {
        Self { buffer: vec![] }
    }

    fn maybe_break_line(&mut self) {
        self.buffer.push(Lowered::MaybeBreak);
    }

    fn break_line(&mut self) {
        self.buffer.push(Lowered::Break);
    }

    fn empty_line(&mut self) {
        self.buffer.push(Lowered::EmptyLine);
    }

    fn write(&mut self, string: &'input str) {
        if !string.is_empty() {
            assert!(string.chars().all(|c| c != '\n'));

            self.buffer.push(Lowered::Str(string.trim()));
        }
    }
    fn write_string(&mut self, string: String) {
        if !string.is_empty() {
            assert!(string.chars().all(|c| c != '\n'));

            let string = string.trim().to_owned();
            self.buffer.push(Lowered::String(string));
        }
    }

    /// Write prefix, but clear `text_inserted` so maybe_line_feed will not linefeed.
    fn prefix2(&mut self, this_prefix: String, next_prefix: &'static str) {
        self.buffer.push(Lowered::Prefix2(this_prefix, next_prefix));
    }

    fn prefix(&mut self, prefix: &'static str) {
        self.buffer.push(Lowered::Prefix(prefix));
    }

    fn pop(&mut self) {
        self.buffer.push(Lowered::Pop);
    }

    fn hr(&mut self) {
        self.buffer.push(Lowered::Hr);
    }

    fn lower_spans(&mut self, spans: &'input [Span]) {
        for span in spans {
            match span {
                Break => {
                    self.write("\\");
                    self.break_line();
                }
                Text(text) => {
                    // TODO: cooler regex
                    let mut split = text
                        .split_inclusive(&[';', ':', ',', '!', '?', '.'])
                        .peekable();
                    loop {
                        let Some(part) = split.next()  else {break;};
                        self.write(part);
                        if split.peek().is_some() {
                            self.break_line();
                        }
                    }
                }
                Code(text) => {
                    if text.len() > CODE_WRAP_LENGTH {
                        self.break_line()
                    } else {
                        self.maybe_break_line()
                    }
                    self.write("`");
                    if text.contains("`") {
                        self.write_string(text.replace("\\", "\\\\").replace("`", "\\`"));
                    } else {
                        self.write(text);
                    }
                    self.write("`");
                    if text.len() > CODE_WRAP_LENGTH {
                        self.break_line()
                    } else {
                        self.maybe_break_line()
                    }
                }
                Link(text, url, title) => {
                    self.break_line();
                    self.write("[");
                    self.write(text);
                    self.write("](");
                    self.write(url);
                    if let Some(title) = title.as_ref() {
                        self.write(" \"");
                        self.write(title);
                        self.write("\"");
                    }
                    self.write(")");
                    self.break_line();
                }
                Image(text, url, title) => {
                    self.break_line();
                    self.write("![");
                    self.write(text);
                    self.write("](");
                    self.write(url);
                    if let Some(title) = title.as_ref() {
                        self.write(" \"");
                        self.write(title);
                        self.write("\"");
                    }
                    self.write(")");
                    self.break_line();
                }
                Emphasis(ref content) => {
                    self.write("*");
                    self.lower_spans(content);
                    self.write("*");
                }
                Strong(ref content) => {
                    self.write("__");
                    self.lower_spans(content);
                    self.write("__");
                }
            };
        }
    }

    fn lower_header(&mut self, spans: &[Span], level: usize) {
        let mut buffer = LoweredBuffer::new();
        buffer.lower_spans(spans);
        let text: String = lowered_to_one_line(&buffer.buffer);
        match level {
            1 | 2 => {
                let bar = if level == 1 {
                    "=".repeat(text.len())
                } else {
                    "-".repeat(text.len())
                };

                self.empty_line();
                self.write_string(text);
                self.break_line();
                self.write_string(bar);
                self.empty_line();
            }
            level if level > 2 => {
                let hashes = "#".repeat(level);
                self.write_string(format!("{hashes} {text}"));
                self.empty_line();
            }
            _ => unreachable!(),
        };
    }

    fn lower_blocks(&mut self, blocks: &'input [Block]) {
        for block in blocks {
            match block {
                Block::Header(spans, level) => self.lower_header(spans, *level),
                Block::Paragraph(spans) => {
                    self.lower_spans(spans);
                }
                Block::Blockquote(blocks) => {
                    self.prefix("> ");
                    self.lower_blocks(blocks);
                    self.pop();
                }
                Block::CodeBlock(None, code) => {
                    self.prefix("    ");
                    for line in code.lines() {
                        self.write(line);
                        self.break_line();
                    }
                    self.pop();
                }
                Block::CodeBlock(Some(options), code) => {
                    self.write("```");
                    self.write(options);
                    self.break_line();
                    for line in code.lines() {
                        self.write(line);
                        self.break_line();
                    }
                    self.write("```");
                }
                Block::OrderedList(items, typ) => {
                    let mut counter = if let Ok(index) = typ.0.parse::<usize>() {
                        index
                    } else {
                        todo!("list type {}", typ.0);
                        1
                    };
                    for item in items.iter() {
                        self.prefix2(format!("{:<4}", format!("{counter}.")), "    ");

                        match item {
                            ListItem::Simple(spans) => self.lower_spans(spans),
                            ListItem::Paragraph(blocks) => self.lower_blocks(blocks),
                        }
                        self.pop();
                        self.break_line();
                        counter += 1;
                    }
                }
                Block::UnorderedList(items) => {
                    for item in items {
                        self.prefix2("*   ".to_owned(), "    ");
                        match item {
                            ListItem::Simple(spans) => self.lower_spans(spans),
                            ListItem::Paragraph(blocks) => self.lower_blocks(blocks),
                        }
                        self.pop();
                        self.break_line();
                    }
                }
                Block::Raw(_) => todo!(),
                Block::Hr => {
                    self.hr();
                }
            }
            self.empty_line();
        }
    }
}

fn lower<'input>(markdown: &'input [Block]) -> Vec<Lowered<'input>> {
    let mut buffer = LoweredBuffer::new();
    buffer.lower_blocks(markdown);
    buffer.buffer
}

fn fix_line_breaks<'i>(input: Vec<Lowered<'i>>) -> Vec<Lowered<'i>> {
    let mut input = VecDeque::from(input);

    let mut result = Vec::with_capacity(input.len());
    let mut line_length = 0;

    // remove all breaks from the front
    loop {
        match input.get(0) {
            Some(Lowered::EmptyLine) | Some(Lowered::Break) | Some(Lowered::MaybeBreak) => {
                input.pop_front();
            }
            _ => break,
        }
    }
    // remove all breaks from the back
    loop {
        match input.get(input.len() - 1) {
            Some(Lowered::EmptyLine) | Some(Lowered::Break) | Some(Lowered::MaybeBreak) => {
                input.pop_back();
            }
            _ => break,
        }
    }
    // add 1 newline, so the file behaves like a good unix file
    input.push_back(Lowered::Break);

    loop {
        let Some(element) = input.pop_front() else {break};
        match element {
            Lowered::MaybeBreak => {
                if line_length > 80 {
                    result.push(Lowered::Break);
                    line_length = 0;
                } else {
                    // count unbreakable length in following elements
                    let mut next_length = 0;
                    for j in &input {
                        match j {
                            Lowered::MaybeBreak
                            | Lowered::Break
                            | Lowered::Hr
                            | Lowered::EmptyLine => {
                                break;
                            }
                            Lowered::String(s) => next_length += s.len(),
                            Lowered::Str(s) => next_length += s.len(),
                            _ => {}
                        }
                    }
                    if line_length + next_length > 80 {
                        result.push(Lowered::Break);
                        line_length = 0;
                    } else {
                        result.push(Lowered::MaybeBreak); // HACK: this is now a space
                    }
                }
            }
            Lowered::String(ref s) => {
                line_length += s.len();
                result.push(element);
            }
            Lowered::Str(s) => {
                line_length += s.len();
                result.push(Lowered::Str(s));
            }
            element => result.push(element),
        }
    }

    result
}

struct Formatter {
    buffer: String,
    prefixes: Vec<&'static str>,
    newlines: usize,
}

impl Formatter {
    fn lf(&mut self) {
        self.buffer.push('\n');
        self.newlines += 1;
    }
    fn write(&mut self, s: &str) {
        if self.newlines > 0 {
            for p in &self.prefixes {
                self.buffer.push_str(&p);
            }
        }
        self.buffer.push_str(s);
        self.newlines = 0;
    }
    fn format(&mut self, element: &Lowered) {
        match element {
            Lowered::MaybeBreak => self.write(" "),
            Lowered::Break => match self.newlines {
                0 => self.lf(),
                _ => {}
            },
            Lowered::EmptyLine => match self.newlines {
                0 => {
                    self.lf();
                    self.lf();
                }
                1 => self.lf(),
                2 => {}
                _ => unreachable!(),
            },
            Lowered::Prefix(p) => {
                self.prefixes.push(p);
            }
            Lowered::Prefix2(this, following) => {
                self.write(this);
                self.prefixes.push(following);
            }
            Lowered::Pop => {
                self.prefixes.pop().unwrap();
            }
            Lowered::String(s) => self.write(&s),
            Lowered::Str(s) => self.write(s),
            Lowered::Hr => {
                match self.newlines {
                    0 => {
                        self.lf();
                        self.lf();
                    }
                    1 => self.lf(),
                    2 => {}
                    _ => unreachable!(),
                }
                let prefix_len: usize = self.prefixes.iter().map(|s| s.len()).sum();
                let l = if prefix_len > 70 { 10 } else { 80 - prefix_len };
                self.write(&"-".repeat(l));
                self.lf();
                self.lf();
            }
        }
    }
}

fn lowered_to_text(elements: &[Lowered<'_>]) -> String {
    let mut f = Formatter {
        buffer: String::new(),
        prefixes: Vec::new(),
        newlines: 0,
    };

    for e in elements {
        f.format(e);
    }

    f.buffer
}

fn lowered_to_one_line(elements: &[Lowered<'_>]) -> String {
    let mut result = String::new();
    let mut iter = elements.iter().peekable();
    loop {
        let Some(element) = iter.next() else {break;};
        match element {
            Lowered::EmptyLine => {}
            Lowered::MaybeBreak | Lowered::Break => {
                if iter.peek().is_some() {
                    result.push(' ');
                }
            }
            Lowered::Prefix(_) => unreachable!("Prefix in 1liner"),
            Lowered::Prefix2(_, _) => unreachable!("Prefix2 in 1liner"),
            Lowered::Pop => unreachable!("Pop in 1liner"),
            Lowered::String(s) => result.push_str(&s),
            Lowered::Str(s) => result.push_str(s),
            Lowered::Hr => unreachable!("HR in 1liner"),
        }
    }
    result
}

fn process_file(path: &Path) -> Result<()> {
    println!("Processing {}", path.display());

    let input = fs::read_to_string(path)?;
    let s = format(&input);

    let mut pb = path.to_path_buf();
    pb.set_extension("formatted-md");
    fs::write(&pb, s)?;

    Ok(())
}

fn format(input: &str) -> String {
    let md = markdown::tokenize(&input);
    let s = lowered_to_text(&fix_line_breaks(lower(&md)));
    s
}

fn walk(path: &Path) -> bool {
    let mut ok = true;
    if path.is_dir() {
        let rd = path.read_dir();
        if let Ok(rd) = rd {
            for c in rd.flatten() {
                walk(&c.path());
            }
        }
    } else if path.is_file() {
        let r = process_file(path);
        if let Err(e) = r {
            println!("Error processing {}: {:?}", path.display(), e);
            ok = false;
        }
    }

    ok
}

fn main() -> ExitCode {
    let mut ok = true;
    for a in env::args().skip(1) {
        let p = Path::new(a.as_str());
        ok = ok && walk(p);
    }

    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;
    use Lowered::*;

    #[test]
    fn test_files() {
        let temp = std::env::temp_dir();
        let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests"));
        let rd = path.read_dir();
        if let Ok(rd) = rd {
            for c in rd.flatten() {
                let file_name = c.file_name().into_string().unwrap();
                if file_name.ends_with(".in.md") {
                    let test_name = &file_name[..file_name.len() - 6];
                    let p_in = c.path();
                    let p_out = path.join(format!("{}.out.md", test_name));
                    let input = fs::read_to_string(p_in).unwrap();
                    let expected_output = fs::read_to_string(p_out).unwrap();

                    eprintln!("{}/{}.in.md", path.display(), test_name);
                    eprintln!("{}/{}.out.md", path.display(), test_name);
                    let md = markdown::tokenize(&input);
                    fs::write(
                        temp.join(format!("{}.phase1", test_name)),
                        format!("{md:#?}"),
                    )
                    .unwrap();
                    eprintln!("{}/{}.phase1", temp.display(), test_name);
                    let lowered = lower(&md);
                    fs::write(
                        temp.join(format!("{}.phase2", test_name)),
                        format!("{lowered:#?}"),
                    )
                    .unwrap();
                    eprintln!("{}/{}.phase2", temp.display(), test_name);
                    let broken = &fix_line_breaks(lowered);
                    fs::write(
                        temp.join(format!("{}.phase3", test_name)),
                        format!("{broken:#?}"),
                    )
                    .unwrap();
                    eprintln!("{}/{}.phase3", temp.display(), test_name);
                    let actual_output = lowered_to_text(broken);
                    fs::write(
                        temp.join(format!("{}.actual.md", test_name)),
                        format!("{actual_output}"),
                    )
                    .unwrap();
                    eprintln!("{}/{}.actual.md", temp.display(), test_name);
                    fs::write(
                        temp.join(format!("{}.actual.raw", test_name)),
                        format!("{actual_output:#?}"),
                    )
                    .unwrap();
                    eprintln!("{}/{}.actual.raw", temp.display(), test_name);
                    fs::write(
                        temp.join(format!("{}.out.raw", test_name)),
                        format!("{expected_output:#?}"),
                    )
                    .unwrap();
                    eprintln!("{}/{}.out.raw", temp.display(), test_name);

                    assert_eq!(expected_output, actual_output);
                }
            }
        }
    }

    // fn pass1(md: &str, expected: &[Lowered]) {
    //     let input = md.replace("\n            ", "\n");
    //     let md = markdown::tokenize(&input);
    //     let mut buffer = LoweredBuffer::new();
    //     lower_blocks(&mut buffer, &md);
    //     if buffer.buffer != expected {
    //         for (i, (actual, expected)) in buffer.buffer.iter().zip(expected).enumerate() {
    //             if actual != expected {
    //                 panic!("Formatted not as expected in line {i}\nexpected: {expected:?}\nactual:   {actual:?}\n");
    //             }
    //         }
    //     }
    // }

    // macro_rules! mdtest {
    //     ($input:literal, $output:literal) => {
    //         let input = $input;
    //         let output = $output;
    //
    //         let input: &str = &input;
    //         let md = markdown::tokenize(&input);
    //         eprintln!("@@@@@ Markdown\n{md:?}");
    //         let lowered = lower(&md);
    //         eprintln!("@@@@@ Lowered\n{lowered:?}");
    //         let broken = &fix_line_breaks(lowered);
    //         eprintln!("@@@@@ Broken\n{broken:?}");
    //         let s = lowered_to_text(broken);
    //         let formatted = s;
    //
    //         eprintln!("@@@@@ expected text\n{output:?}");
    //         eprintln!("@@@@@ actual text\n{formatted:?}");
    //
    //         if formatted != output {
    //             let mut formatted_lines = formatted.lines();
    //             let mut output_lines = output.lines();
    //
    //             for i in 0.. {
    //                 let expected = output_lines.next();
    //                 let actual = formatted_lines.next();
    //
    //                 if expected.is_none() {
    //                     if actual.is_none() {
    //                         break; // lines are equal, do char comparison next
    //                     }
    //                 }
    //                 if actual != expected {
    //                     eprintln!(
    //                         "Difference in line {i}:\nexpected: {expected:?}\nactual  : {actual:?}"
    //                     );
    //                     panic!("line {i} differs");
    //                 }
    //             }
    //             let mut formatted_chars = formatted.chars();
    //             let mut output_chars = output.chars();
    //             for i in 0.. {
    //                 let expected = output_chars.next();
    //                 let actual = formatted_chars.next();
    //                 if expected.is_none() {
    //                     if actual.is_none() {
    //                         unreachable!("strings are different but all chars are equal");
    //                     }
    //                 }
    //                 if actual != expected {
    //                     panic!("char {i} differs: {actual:?} != {expected:?}");
    //                 }
    //             }
    //         }
    //     };
    // }
}
