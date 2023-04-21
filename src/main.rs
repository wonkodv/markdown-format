// allow all in debug, deny in release
#![allow(unreachable_code, unused_variables)]

use anyhow::Result;
use markdown::Span::*;
use markdown::*;
use std::{env, fs, path::Path, process::ExitCode};

struct Formatter {
    output: String,
    prefixes: Vec<&'static str>,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
            prefixes: Vec::new(),
        }
    }

    fn into_output(self) -> String {
        self.output
    }

    fn write(&mut self, string: &str) {
        assert!(string.chars().all(|c| c != '\n'));
        self.output.push_str(string);
    }

    fn write_all(&mut self, strings: &[&str]) {
        for s in strings {
            self.write(s);
        }
    }

    fn line_feed(&mut self) {
        for p in &self.prefixes {
            self.output.push_str(p);
        }
        self.output.push('\n');
    }

    fn write_lines(&mut self, string: &str) {
        for line in string.split('\n') {
            self.write(line);
        }
    }

    fn push(&mut self, prefix: &'static str) {
        self.prefixes.push(prefix);
        self.write(prefix);
    }

    fn pop(&mut self) {
        self.prefixes.pop().unwrap();
    }

    fn format_spans(&mut self, spans: &[Span], can_break: bool) {
        for span in spans {
            match span {
                Break => {
                    assert!(can_break);
                    self.line_feed()
                }
                Text(text) => {
                    self.write(text);
                }
                Code(text) => {
                    let b = can_break && text.len() > 20;
                    if b {
                        self.line_feed();
                    }
                    self.write_all(&["`", text, "`"]);
                    if b {
                        self.line_feed();
                    }
                }
                Link(text, url, None) => {
                    if can_break {
                        self.line_feed();
                    }
                    self.write_all(&["[", text, "](", url, ")"]);
                    if can_break {
                        self.line_feed();
                    }
                }
                Link(text, url, Some(title)) => {
                    if can_break {
                        self.line_feed();
                    }
                    self.write_all(&["[", text, "](", url, " \"", title, "\")"]);
                    if can_break {
                        self.line_feed();
                    }
                }
                Image(text, url, None) => {
                    if can_break {
                        self.line_feed();
                    }
                    self.write_all(&["![", text, "](", url, ")"]);
                    if can_break {
                        self.line_feed();
                    }
                }
                Image(ref text, ref url, Some(ref title)) => {
                    if can_break {
                        self.line_feed();
                    }
                    self.write_all(&["![", text, "](", url, " \"", title, "\")"]);
                    if can_break {
                        self.line_feed();
                    }
                }
                Emphasis(ref content) => {
                    self.write("*");
                    self.format_spans(content, can_break);
                    self.write("*");
                }
                Strong(ref content) => {
                    self.write("__");
                    self.format_spans(content, can_break);
                    self.write("__");
                }
            };
        }
    }

    fn format_header(&mut self, spans: &[Span], level: usize) {
        self.line_feed();
        self.line_feed();
        match level {
            1 | 2 => {
                let len = self.output.len();
                self.format_spans(spans, false);
                let len = self.output.len() - len;
                self.line_feed();
                let bar;
                if level == 1 {
                    bar = "=".repeat(len);
                } else {
                    bar = "-".repeat(len);
                }
                self.write(&bar);
            }
            level if level > 2 => {
                self.write(&"#".repeat(level));
                self.format_spans(spans, false);
            }
            _ => unreachable!(),
        };
        self.line_feed();
        self.line_feed();
    }

    fn format_block(&mut self, block: &Block) {
        match block {
            Block::Header(spans, level) => self.format_header(&spans, *level),
            Block::Paragraph(spans) => {
                self.format_spans(&spans, true);
                self.line_feed();
            }
            Block::Blockquote(blocks) => {
                self.push("> ");
                self.format_blocks(&blocks);
                self.pop();
            }
            Block::CodeBlock(None, code) => {
                self.push("    ");
                self.write_lines(code);
                self.pop();
            }
            Block::CodeBlock(Some(options), code) => {
                self.line_feed();
                self.write("```");
                self.write(options);
                self.line_feed();
                self.write_lines(code);
                self.line_feed();
                self.write("```");
            }
            Block::OrderedList(items, typ) => {
                let mut counter = if let Ok(index) = typ.0.parse::<usize>() {
                    index
                } else {
                    todo!("list type {}", typ.0);
                    1
                };
                for (index, item) in items.iter().enumerate() {
                    self.write(&format!("{:<4}", format!("{counter}.")));
                    match item {
                        ListItem::Simple(spans) => self.format_spans(spans, false),
                        ListItem::Paragraph(blocks) => {
                            self.prefixes.push("    ");
                            self.format_blocks(blocks);
                            self.pop();
                        }
                    }
                    self.line_feed();
                    counter += 1;
                }
            }
            Block::UnorderedList(items) => {
                for item in items {
                    self.write("-   ");
                    match item {
                        ListItem::Simple(spans) => self.format_spans(spans, false),
                        ListItem::Paragraph(blocks) => {
                            self.prefixes.push("    ");
                            self.format_blocks(blocks);
                            self.pop();
                        }
                    }
                    self.line_feed();
                }
            }
            Block::Raw(_) => todo!(),
            Block::Hr => {
                let len: usize = self.prefixes.iter().map(|s| s.len()).sum();
                assert!(len < 70);
                let bar = "-".repeat(80 - len);
                self.line_feed();
                self.write(&bar);
                self.line_feed();
            }
        }
        self.line_feed();
    }

    fn format_blocks(&mut self, blocks: &[Block]) {
        for b in blocks {
            self.format_block(b);
        }
    }
}
fn format(input: &str) -> Result<String> {
    let md = markdown::tokenize(&input);
    eprintln!("{md:?}");
    let mut formatter = Formatter::new();
    formatter.format_blocks(&md);
    Ok(formatter.into_output())
}

fn process_file(path: &Path) -> Result<()> {
    println!("Processing {}", path.display());

    let s = fs::read_to_string(path)?;

    let s = format(&s)?;

    let mut pb = path.to_path_buf();
    pb.set_extension("formatted-md");
    fs::write(&pb, s)?;

    Ok(())
}

fn walk(path: &Path) -> bool {
    let mut ok = true;
    if path.is_dir() {
        let rd = path.read_dir();
        if let Ok(rd) = rd {
            for c in rd {
                if let Ok(c) = c {
                    walk(&c.path());
                }
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
