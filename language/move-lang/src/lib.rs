// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

// ༼ つ ◕_◕ ༽つ  #![forbid(unsafe_code)]
#[macro_use]
extern crate alloc;

#[macro_use(sp)]
extern crate move_ir_types;

pub mod cfgir;
pub mod compiled_unit;
pub mod errors;
pub mod expansion;
pub mod hlir;
pub mod interface_generator;
pub mod ir_translation;
pub mod name_pool;
pub mod naming;
pub mod parser;
pub mod shared;
pub mod to_bytecode;
pub mod typing;

use crate::name_pool::ConstPool;
use anyhow::anyhow;
use codespan::{ByteIndex, Span};
use compiled_unit::CompiledUnit;
use errors::*;
use move_ir_types::location::*;
use parser::syntax::parse_file_string;
use shared::Address;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    fs::File,
    io::{Read, Write},
    iter::Peekable,
    path::{Path, PathBuf},
    str::Chars,
};

pub const MOVE_EXTENSION: &str = "move";
pub const MOVE_COMPILED_EXTENSION: &str = "mv";
pub const MOVE_COMPILED_INTERFACES_DIR: &str = "mv_interfaces";
pub const SOURCE_MAP_EXTENSION: &str = "mvsm";

//**************************************************************************************************
// Entry
//**************************************************************************************************

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pass {
    Parser,
    Expansion,
    Naming,
    Typing,
    HLIR,
    CFGIR,
    Compilation,
}

pub enum PassResult {
    Parser(Option<Address>, parser::ast::Program),
    Expansion(expansion::ast::Program, Errors),
    Naming(naming::ast::Program, Errors),
    Typing(typing::ast::Program),
    HLIR(hlir::ast::Program, Errors),
    CFGIR(cfgir::ast::Program),
    Compilation(Vec<CompiledUnit>),
}


/// Runs the compiler from a previous result until a stopping point.
/// The stopping point is inclusive, meaning the pass specified by `until: Pass` will be run
pub fn move_continue_up_to(pass: PassResult, until: Pass) -> Result<PassResult, Errors> {
    run(pass, until)
}

//**************************************************************************************************
// Utils
//**************************************************************************************************

macro_rules! dir_path {
    ($($dir:expr),+) => {{
        let mut p = PathBuf::new();
        $(p.push($dir);)+
        p
    }};
}

macro_rules! file_path {
    ($dir:expr, $name:expr, $ext:expr) => {{
        let mut p = PathBuf::from($dir);
        p.push($name);
        p.set_extension($ext);
        p
    }};
}

fn has_compiled_module_magic_number(path: &str) -> bool {
    use move_vm::file_format_common::BinaryConstants;
    let mut file = match File::open(path) {
        Err(_) => return false,
        Ok(f) => f,
    };
    let mut magic = [0u8; BinaryConstants::DIEM_MAGIC_SIZE];
    let num_bytes_read = match file.read(&mut magic) {
        Err(_) => return false,
        Ok(n) => n,
    };
    num_bytes_read == BinaryConstants::DIEM_MAGIC_SIZE && magic == BinaryConstants::DIEM_MAGIC
}

pub fn path_to_string(path: &Path) -> anyhow::Result<String> {
    match path.to_str() {
        Some(p) => Ok(p.to_string()),
        None => Err(anyhow!("non-Unicode file name")),
    }
}

pub fn extension_equals(path: &Path, target_ext: &str) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(extension) => extension == target_ext,
        None => false,
    }
}

//**************************************************************************************************
// Translations
//**************************************************************************************************

impl PassResult {
    pub fn equivalent_pass(&self) -> Pass {
        match self {
            PassResult::Parser(_, _) => Pass::Parser,
            PassResult::Expansion(_, _) => Pass::Expansion,
            PassResult::Naming(_, _) => Pass::Naming,
            PassResult::Typing(_) => Pass::Typing,
            PassResult::HLIR(_, _) => Pass::HLIR,
            PassResult::CFGIR(_) => Pass::CFGIR,
            PassResult::Compilation(_) => Pass::Compilation,
        }
    }

    pub fn check_for_errors(self) -> Result<Self, Errors> {
        Ok(match self {
            result @ PassResult::Parser(_, _)
            | result @ PassResult::Typing(_)
            | result @ PassResult::CFGIR(_)
            | result @ PassResult::Compilation(_) => result,
            PassResult::Expansion(eprog, errors) => {
                check_errors(errors)?;
                PassResult::Expansion(eprog, Errors::new())
            }
            PassResult::Naming(nprog, errors) => {
                check_errors(errors)?;
                PassResult::Naming(nprog, Errors::new())
            }
            PassResult::HLIR(hprog, errors) => {
                check_errors(errors)?;
                PassResult::HLIR(hprog, Errors::new())
            }
        })
    }
}

fn run(cur: PassResult, until: Pass) -> Result<PassResult, Errors> {
    if cur.equivalent_pass() >= until {
        return Ok(cur);
    }

    match cur {
        PassResult::Parser(sender_opt, prog) => {
            let (eprog, errors) = expansion::translate::program(prog, sender_opt);
            run(PassResult::Expansion(eprog, errors), until)
        }
        PassResult::Expansion(eprog, errors) => {
            let (nprog, errors) = naming::translate::program(eprog, errors);
            run(PassResult::Naming(nprog, errors), until)
        }
        PassResult::Naming(nprog, errors) => {
            let (tprog, errors) = typing::translate::program(nprog, errors);
            check_errors(errors)?;
            run(PassResult::Typing(tprog), until)
        }
        PassResult::Typing(tprog) => {
            let (hprog, errors) = hlir::translate::program(tprog);
            run(PassResult::HLIR(hprog, errors), until)
        }
        PassResult::HLIR(hprog, errors) => {
            let (cprog, errors) = cfgir::translate::program(errors, hprog);
            check_errors(errors)?;
            run(PassResult::CFGIR(cprog), until)
        }
        PassResult::CFGIR(cprog) => {
            let compiled_units = to_bytecode::translate::program(cprog)?;
            assert!(until == Pass::Compilation);
            run(PassResult::Compilation(compiled_units), Pass::Compilation)
        }
        PassResult::Compilation(_) => unreachable!("ICE Pass::Compilation is >= all passes"),
    }
}

fn check_targets_deps_dont_intersect(
    targets: &[&'static str],
    deps: &[&'static str],
) -> anyhow::Result<()> {
    let target_set = targets.iter().collect::<BTreeSet<_>>();
    let dep_set = deps.iter().collect::<BTreeSet<_>>();
    let intersection = target_set.intersection(&dep_set).collect::<Vec<_>>();
    if intersection.is_empty() {
        return Ok(());
    }

    let all_files = intersection
        .into_iter()
        .map(|s| format!("    {}", s))
        .collect::<Vec<_>>()
        .join("\n");
    Err(anyhow!(
        "The following files were marked as both targets and dependencies:\n{}",
        all_files
    ))
}

//**************************************************************************************************
// Comments
//**************************************************************************************************

/// Determine if a character is an allowed eye-visible (printable) character.
///
/// The only allowed printable characters are the printable ascii characters (SPACE through ~) and
/// tabs. All other characters are invalid and we return false.
pub fn is_permitted_printable_char(c: char) -> bool {
    let x = c as u32;
    let is_above_space = x >= 0x20; // Don't allow meta characters
    let is_below_tilde = x <= 0x7E; // Don't allow DEL meta character
    let is_tab = x == 0x09; // Allow tabs
    (is_above_space && is_below_tilde) || is_tab
}

/// Determine if a character is a permitted newline character.
///
/// The only permitted newline character is \n. All others are invalid.
pub fn is_permitted_newline_char(c: char) -> bool {
    let x = c as u32;
    x == 0x0A
}

/// Determine if a character is permitted character.
///
/// A permitted character is either a permitted printable character, or a permitted
/// newline. Any other characters are disallowed from appearing in the file.
pub fn is_permitted_char(c: char) -> bool {
    is_permitted_printable_char(c) || is_permitted_newline_char(c)
}

fn verify_string(fname: &'static str, string: &str) -> Result<(), Errors> {
    match string
        .chars()
        .enumerate()
        .find(|(_, c)| !is_permitted_char(*c))
    {
        None => Ok(()),
        Some((idx, chr)) => {
            let span = Span::new(ByteIndex(idx as u32), ByteIndex(idx as u32));
            let loc = Loc::new(fname, span);
            let msg = format!(
                "Invalid character '{}' found when reading file. Only ASCII printable characters, \
                 tabs (\\t), and line endings (\\n) are permitted.",
                chr
            );
            Err(vec![vec![(loc, msg)]])
        }
    }
}

/// Types to represent comments.
pub type CommentMap = BTreeMap<&'static str, MatchedFileCommentMap>;
pub type MatchedFileCommentMap = BTreeMap<ByteIndex, String>;
pub type FileCommentMap = BTreeMap<Span, String>;

/// Strips line and block comments from input source, and collects documentation comments,
/// putting them into a map indexed by the span of the comment region. Comments in the original
/// source will be replaced by spaces, such that positions of source items stay unchanged.
/// Block comments can be nested.
///
/// Documentation comments are comments which start with
/// `///` or `/**`, but not `////` or `/***`. The actually comment delimiters
/// (`/// .. <newline>` and `/** .. */`) will be not included in extracted comment string. The
/// span in the returned map, however, covers the whole region of the comment, including the
/// delimiters.
fn strip_comments(fname: &'static str, input: &str) -> Result<(String, FileCommentMap), Errors> {
    const SLASH: char = '/';
    const SPACE: char = ' ';
    const STAR: char = '*';
    const QUOTE: char = '"';
    const BACKSLASH: char = '\\';

    enum State {
        Source,
        String,
        LineComment,
        BlockComment,
    }

    let mut source = String::with_capacity(input.len());
    let mut comment_map = FileCommentMap::new();

    let mut state = State::Source;
    let mut pos = 0;
    let mut comment_start_pos = 0;
    let mut comment = String::new();
    let mut block_nest = 0;

    let next_is =
        |peekable: &mut Peekable<Chars>, chr| peekable.peek().map(|c| *c == chr).unwrap_or(false);

    let mut commit_comment = |state, start_pos, end_pos, content: String| match state {
        State::BlockComment if !content.starts_with('*') || content.starts_with("**") => {}
        State::LineComment if !content.starts_with('/') || content.starts_with("//") => {}
        _ => {
            comment_map.insert(Span::new(start_pos, end_pos), content[1..].to_string());
        }
    };

    let mut char_iter = input.chars().peekable();
    while let Some(chr) = char_iter.next() {
        match state {
            // Strings
            State::Source if chr == QUOTE => {
                source.push(chr);
                pos += 1;
                state = State::String;
            }
            State::String => {
                source.push(chr);
                pos += 1;
                if chr == BACKSLASH {
                    // Skip over the escaped character (e.g., a quote or another backslash)
                    if let Some(next) = char_iter.next() {
                        source.push(next);
                        pos += 1;
                    }
                } else if chr == QUOTE {
                    state = State::Source;
                }
            }
            // Line comments
            State::Source if chr == SLASH && next_is(&mut char_iter, SLASH) => {
                // Starting line comment. We do not capture the `//` in the comment.
                char_iter.next();
                source.push(SPACE);
                source.push(SPACE);
                comment_start_pos = pos;
                pos += 2;
                state = State::LineComment;
            }
            State::LineComment if is_permitted_newline_char(chr) => {
                // Ending line comment. The newline will be added to the source.
                commit_comment(state, comment_start_pos, pos, std::mem::take(&mut comment));
                source.push(chr);
                pos += 1;
                state = State::Source;
            }
            State::LineComment => {
                // Continuing line comment.
                source.push(SPACE);
                comment.push(chr);
                pos += 1;
            }

            // Block comments.
            State::Source if chr == SLASH && next_is(&mut char_iter, STAR) => {
                // Starting block comment. We do not capture the `/*` in the comment.
                char_iter.next();
                source.push(SPACE);
                source.push(SPACE);
                comment_start_pos = pos;
                pos += 2;
                state = State::BlockComment;
            }
            State::BlockComment if chr == SLASH && next_is(&mut char_iter, STAR) => {
                // Starting nested block comment.
                char_iter.next();
                source.push(SPACE);
                comment.push(chr);
                pos += 1;
                block_nest += 1;
            }
            State::BlockComment
            if block_nest > 0 && chr == STAR && next_is(&mut char_iter, SLASH) =>
                {
                    // Ending nested block comment.
                    char_iter.next();
                    source.push(SPACE);
                    comment.push(chr);
                    pos -= 1;
                    block_nest -= 1;
                }
            State::BlockComment
            if block_nest == 0 && chr == STAR && next_is(&mut char_iter, SLASH) =>
                {
                    // Ending block comment. The `*/` will not be captured and also not part of the
                    // source.
                    char_iter.next();
                    source.push(SPACE);
                    source.push(SPACE);
                    pos += 2;
                    commit_comment(state, comment_start_pos, pos, std::mem::take(&mut comment));
                    state = State::Source;
                }
            State::BlockComment => {
                // Continuing block comment.
                source.push(SPACE);
                comment.push(chr);
                pos += 1;
            }
            State::Source => {
                // Continuing regular source.
                source.push(chr);
                pos += 1;
            }
        }
    }
    match state {
        State::LineComment => {
            // We allow the last line to have no line terminator
            commit_comment(state, comment_start_pos, pos, std::mem::take(&mut comment));
        }
        State::BlockComment => {
            if pos > 0 {
                // try to point to last real character
                pos -= 1;
            }
            return Err(vec![vec![
                (
                    Loc::new(fname, Span::new(pos, pos)),
                    "unclosed block comment".to_string(),
                ),
                (
                    Loc::new(fname, Span::new(comment_start_pos, comment_start_pos + 2)),
                    "begin of unclosed block comment".to_string(),
                ),
            ]]);
        }
        State::Source | State::String => {}
    }

    Ok((source, comment_map))
}

// We restrict strings to only ascii visual characters (0x20 <= c <= 0x7E) or a permitted newline
// character--\n--or a tab--\t.
pub fn strip_comments_and_verify(
    fname: &'static str,
    string: &str,
) -> Result<(String, FileCommentMap), Errors> {
    verify_string(fname, string)?;
    strip_comments(fname, string)
}
