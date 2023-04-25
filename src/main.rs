// allow all in debug, deny in release
#![allow(unreachable_code, unused_variables)]

use anyhow::Result;
use markdown::Span::*;
use markdown::*;
use std::{env, fs, path::Path, process::ExitCode};

struct Formatter {
    output: String,
    prefixes: Vec<&'static str>,
    prefix_inserted: bool,
    text_inserted: bool,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
            prefixes: Vec::new(),
            prefix_inserted: false,
            text_inserted: false,
        }
    }

    fn into_output(self) -> String {
        self.output
    }

    fn write(&mut self, string: &str) {
        if !self.prefix_inserted {
            for p in &self.prefixes {
                self.output.push_str(p);
            }
            self.prefix_inserted = true;
        }

        if !string.is_empty() {
            assert!(string.chars().all(|c| c != '\n'));

            self.output.push_str(string);
            self.text_inserted = true;
        }
    }

    fn write_all(&mut self, strings: &[&str]) {
        for s in strings {
            self.write(s);
        }
    }

    fn line_feed(&mut self) {
        self.output.push('\n');
        self.prefix_inserted = false;
        self.text_inserted = false;
    }

    fn maybe_line_feed(&mut self) {
        if self.text_inserted {
            self.line_feed();
        }
    }

    fn write_lines(&mut self, string: &str) {
        for line in string.split('\n') {
            self.write(line);
        }
    }

    /// Write prefix, but clear `text_inserted` so maybe_line_feed will not linefeed.
    fn push2(&mut self, this_prefix: &str, next_prefix: &'static str) {
        assert!(!self.prefix_inserted);
        assert!(!self.text_inserted);
        self.write(this_prefix);
        self.text_inserted = false;
        self.push(next_prefix);
    }

    fn push(&mut self, prefix: &'static str) {
        self.prefixes.push(prefix);
    }

    fn pop(&mut self) {
        self.prefixes.pop().unwrap();
    }

    fn format_spans(&mut self, spans: &[Span], extra_breaks: bool) {
        for span in spans {
            match span {
                Break => {
                    assert!(extra_breaks);
                    self.write("\\");
                    self.line_feed();
                }
                Text(text) => {
                    self.write(text);
                }
                Code(text) => {
                    let b = extra_breaks && text.len() > 20;
                    if b {
                        self.maybe_line_feed();
                    }
                    self.write_all(&["`", text, "`"]);
                    if b {
                        self.line_feed();
                    }
                }
                Link(text, url, None) => {
                    if extra_breaks {
                        self.maybe_line_feed();
                    }
                    self.write_all(&["[", text, "](", url, ")"]);
                    if extra_breaks {
                        self.line_feed();
                    }
                }
                Link(text, url, Some(title)) => {
                    if extra_breaks {
                        self.maybe_line_feed();
                    }
                    self.write_all(&["[", text, "](", url, " \"", title, "\")"]);
                    if extra_breaks {
                        self.line_feed();
                    }
                }
                Image(text, url, None) => {
                    if extra_breaks {
                        self.maybe_line_feed();
                    }
                    self.write_all(&["![", text, "](", url, ")"]);
                    if extra_breaks {
                        self.line_feed();
                    }
                }
                Image(ref text, ref url, Some(ref title)) => {
                    if extra_breaks {
                        self.maybe_line_feed();
                    }
                    self.write_all(&["![", text, "](", url, " \"", title, "\")"]);
                    if extra_breaks {
                        self.line_feed();
                    }
                }
                Emphasis(ref content) => {
                    self.write("*");
                    self.format_spans(content, extra_breaks);
                    self.write("*");
                }
                Strong(ref content) => {
                    self.write("__");
                    self.format_spans(content, extra_breaks);
                    self.write("__");
                }
            };
        }
    }

    fn format_header(&mut self, spans: &[Span], level: usize) {
        match level {
            1 | 2 => {
                self.write(""); // ensure prefixes are written
                let len = self.output.len();
                self.format_spans(spans, false);
                let len = self.output.len() - len;
                self.line_feed();
                let bar = if level == 1 {
                    "=".repeat(len)
                } else {
                    "-".repeat(len)
                };
                self.write(&bar);
            }
            level if level > 2 => {
                self.write(&"#".repeat(level));
                self.write(" ");
                self.format_spans(spans, false);
            }
            _ => unreachable!(),
        };
        self.line_feed();
        self.line_feed();
    }

    fn format_block(&mut self, block: &Block) {
        match block {
            Block::Header(spans, level) => self.format_header(spans, *level),
            Block::Paragraph(spans) => {
                self.format_spans(spans, true);
                self.maybe_line_feed();
            }
            Block::Blockquote(blocks) => {
                self.push("> ");
                self.format_blocks(blocks);
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
                    self.push2(&format!("{:<4}", format!("{counter}.")), "    ");

                    match item {
                        ListItem::Simple(spans) => self.format_spans(spans, false),
                        ListItem::Paragraph(blocks) => {
                            self.format_blocks(blocks);
                        }
                    }
                    self.pop();
                    self.line_feed();
                    counter += 1;
                }
            }
            Block::UnorderedList(items) => {
                for item in items {
                    self.push2("*   ", "    ");
                    match item {
                        ListItem::Simple(spans) => self.format_spans(spans, false),
                        ListItem::Paragraph(blocks) => {
                            self.format_blocks(blocks);
                        }
                    }
                    self.pop();
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

fn format(input: &str) -> String {
    let md = markdown::tokenize(input);
    eprintln!("{md:#?}");
    let mut formatter = Formatter::new();
    formatter.format_blocks(&md);
    formatter.into_output()
}

fn process_file(path: &Path) -> Result<()> {
    println!("Processing {}", path.display());

    let s = fs::read_to_string(path)?;

    let s = format(&s);

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
    use super::*;

    fn mdtest(input: &str, output: &str) {
        let input = input.replace("\n            ", "\n");
        let output = output.replace("\n            ", "\n");

        let formatted = format(&input);

        eprintln!("@@@@@ expected text\n{output}");
        eprintln!("@@@@@ actual text\n{formatted}");

        if formatted != output {
            for (i, (actual, expected)) in formatted.split("\n").zip(output.split("\n")).enumerate()
            {
                if actual != expected {
                    panic!("Formatted not as expected in line {i}\nexpected: {expected}\nactual:   {actual}\n");
                }
            }
        }
    }

    #[test]
    fn headlines_underlines() {
        mdtest(
            "
            # H1

            text

            ## H2

            text

            ###     H3
            ",
            "H1
            ==

            text

            H2
            --

            text

            ### H3

            ",
        );
    }
}
