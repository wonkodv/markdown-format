use anyhow::Result;
use markdown::Span::*;
use markdown::*;
use std::{
    env,
    fmt::{self, Write},
    fs,
    io::{self},
    path::Path,
    process::ExitCode,
};

fn format_spans(s: &mut String, elements: &[Span]) -> Result<()> {
    for element in elements.iter() {
        match *element {
            Break => {
                write!(s, "\n\n")?;
            }
            Text(ref text) => {
                write!(s, "{}", text)?;
            }
            Code(ref text) => {
                write!(s, "```{}```", text)?;
            }
            Link(ref text, ref url, None) => {
                write!(s, "[{}]({})", text, url)?;
            }
            Link(ref text, ref url, Some(ref title)) => {
                write!(s, "[{}]({} \"{}\")", text, url, title)?;
            }
            Image(ref text, ref url, None) => {
                write!(s, "![{}]({})", text, url)?;
            }
            Image(ref text, ref url, Some(ref title)) => {
                write!(s, "![{}]({} \"{}\")", text, url, title)?;
            }
            Emphasis(ref content) => {
                write!(s, "*")?;
                format_spans(s, content)?;
                write!(s, "*")?;
            }
            Strong(ref content) => {
                write!(s, "__")?;
                format_spans(s, content)?;
                write!(s, "__")?;
            }
        };
    }

    Ok(())
}

fn format_header(s: &mut String, span: &[Span], level: usize) -> Result<()> {
    match level {
        1 | 2 => {
            let len = s.len();
            format_spans(s, span);
            let len = s.len() - len;
            let bar;
            if level == 1 {
                bar = "=".repeat(len);
            } else {
                bar = "-".repeat(len);
            }
            s.push_str(&bar);
            s.push('\n');
        }
        mut level => {
            assert!(level > 0);
            while level > 0 {
                s.push('#');
                level -= 1;
            }
            format_spans(s, span)?;
        }
    };

    Ok(())
}

fn format_block(s: &mut String, block: Block) -> Result<()> {
    match block {
        Block::Header(_, _) => todo!(),
        Block::Paragraph(_) => todo!(),
        Block::Blockquote(_) => todo!(),
        Block::CodeBlock(_, _) => todo!(),
        Block::OrderedList(_, _) => todo!(),
        Block::UnorderedList(_) => todo!(),
        Block::Raw(_) => todo!(),
        Block::Hr => todo!(),
    }
}

fn format(input: &str) -> Result<String> {
    let md = markdown::tokenize(&input);
    let mut output = String::new();
    let mut iter: std::slice::Iter<Block> = md.iter();
    for b in md {
        format_block(&mut output, b);
    }

    Ok(output)
}

fn process_file(path: &Path) -> Result<()> {
    println!("Processing {}", path.display());

    let s = fs::read_to_string(path)?;

    let s = format(&s)?;

    let mut pb = path.to_path_buf();
    pb.set_extension("md.txt");
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
