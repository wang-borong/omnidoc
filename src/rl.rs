use crate::doctype::{DocumentType, DocumentTypeRegistry};
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hint, Hinter};
use rustyline::history::DefaultHistory;
use rustyline::Context;
use rustyline::{
    Cmd, CompletionType, ConditionalEventHandler, Config, Editor, Event, EventContext,
    EventHandler, KeyEvent, RepeatCount, Result,
};
use rustyline::{Completer, Helper, Highlighter, Hinter, Validator};
use std::borrow::Cow::{self, Borrowed, Owned};
use std::collections::HashSet;

#[derive(Completer, Helper, Validator, Highlighter)]
struct DocTypeHinter {
    // It's simple example of rustyline, for more efficient, please use ** radix trie **
    hints: HashSet<CommandHint>,
}

#[derive(Hash, Debug, PartialEq, Eq)]
struct CommandHint {
    display: String,
    complete_up_to: usize,
}

impl Hint for CommandHint {
    fn display(&self) -> &str {
        &self.display
    }

    fn completion(&self) -> Option<&str> {
        if self.complete_up_to > 0 {
            Some(&self.display[..self.complete_up_to])
        } else {
            None
        }
    }
}

impl CommandHint {
    fn new(text: &str, complete_up_to: &str) -> Self {
        assert!(text.starts_with(complete_up_to));
        Self {
            display: text.into(),
            complete_up_to: complete_up_to.len(),
        }
    }

    fn suffix(&self, strip_chars: usize) -> Self {
        Self {
            display: self.display[strip_chars..].to_owned(),
            complete_up_to: self.complete_up_to.saturating_sub(strip_chars),
        }
    }
}

impl Hinter for DocTypeHinter {
    type Hint = CommandHint;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<CommandHint> {
        if line.is_empty() || pos < line.len() {
            return None;
        }

        self.hints
            .iter()
            .filter_map(|hint| {
                // expect hint after word complete, like redis cli, add condition:
                // line.ends_with(" ")
                if hint.display.starts_with(line) {
                    Some(hint.suffix(pos))
                } else {
                    None
                }
            })
            .next()
    }
}

fn doctype_hints() -> HashSet<CommandHint> {
    let mut set = HashSet::new();
    set.insert(CommandHint::new("ls", "ls"));
    set.insert(CommandHint::new("list", "list"));

    // Use DocumentTypeRegistry to get all document types dynamically
    for doctype in DocumentTypeRegistry::all() {
        set.insert(CommandHint::new(doctype.as_str(), doctype.as_str()));
    }

    set
}

const fn default_break_chars(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t'
            | '\n'
            | '"'
            | '\\'
            | '\''
            | '`'
            | '@'
            | '$'
            | '>'
            | '<'
            | '='
            | ';'
            | '|'
            | '&'
            | '{'
            | '('
            | '\0'
    )
}

use rustyline::completion::{extract_word, Candidate, Completer, Pair};

struct DocTypeCompleter {
    doctypes: Vec<String>,
}

fn doctype_match(dti: &str, doctypes: Vec<String>) -> Vec<Pair> {
    let mut entries: Vec<Pair> = vec![];

    for dt in doctypes {
        if dt.contains(dti) {
            entries.push(Pair {
                display: dt,
                replacement: String::from(dti),
            })
        }
    }

    entries
}

impl DocTypeCompleter {
    /// Constructor
    #[must_use]
    pub fn new() -> Self {
        // Use DocumentTypeRegistry to get all document types dynamically
        let doctypes: Vec<String> = DocumentTypeRegistry::all()
            .into_iter()
            .map(|dt| dt.as_str().to_string())
            .collect();

        Self { doctypes }
    }

    pub fn complete(&self, line: &str, pos: usize) -> Result<(usize, Vec<Pair>)> {
        let (start, mut matches) = self.complete_unsorted(line, pos)?;
        #[allow(clippy::unnecessary_sort_by)]
        matches.sort_by(|a, b| a.display().cmp(b.display()));
        Ok((start, matches))
    }

    pub fn complete_unsorted(&self, line: &str, pos: usize) -> Result<(usize, Vec<Pair>)> {
        let (start, doctype) = extract_word(line, pos, None, default_break_chars);

        let matches = doctype_match(&doctype, self.doctypes.clone());

        Ok((start, matches))
    }
}

//impl Default for DocTypeCompleter {
//    fn default() -> Self {
//        Self::new()
//    }
//}

impl Completer for DocTypeCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>)> {
        self.complete(line, pos)
    }
}

#[derive(Completer, Helper, Hinter, Validator)]
struct MyHelper {
    #[rustyline(Hinter)]
    hinter: DocTypeHinter,

    #[rustyline(Completer)]
    completer: DocTypeCompleter,
}

impl Highlighter for MyHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Owned(format!("\x1b[32m{prompt}\x1b[m"))
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned(format!("\x1b[37m{hint}\x1b[m"))
    }
}

#[derive(Clone)]
struct CompleteHintHandler;
impl ConditionalEventHandler for CompleteHintHandler {
    fn handle(&self, evt: &Event, _: RepeatCount, _: bool, ctx: &EventContext) -> Option<Cmd> {
        if !ctx.has_hint() {
            return None; // default
        }
        if let Some(k) = evt.get(0) {
            #[allow(clippy::if_same_then_else)]
            if *k == KeyEvent::ctrl('E') {
                Some(Cmd::CompleteHint)
            } else if *k == KeyEvent::alt('f') && ctx.line().len() == ctx.pos() {
                let text = ctx.hint_text()?;
                let mut start = 0;
                if let Some(first) = text.chars().next() {
                    if !first.is_alphanumeric() {
                        start = text.find(|c: char| c.is_alphanumeric()).unwrap_or_default();
                    }
                }

                let text = text
                    .chars()
                    .enumerate()
                    .take_while(|(i, c)| *i <= start || c.is_alphanumeric())
                    .map(|(_, c)| c)
                    .collect::<String>();

                Some(Cmd::Insert(1, text))
            } else {
                None
            }
        } else {
            unreachable!()
        }
    }
}

pub struct DTRL {
    rl: Editor<MyHelper, DefaultHistory>,
}

impl DTRL {
    pub fn new() -> Result<Self> {
        let config = Config::builder()
            .completion_type(CompletionType::List)
            .build();
        let hinter = DocTypeHinter {
            hints: doctype_hints(),
        };
        let mut rl: Editor<MyHelper, DefaultHistory> = Editor::with_config(config)?;

        let h = MyHelper {
            completer: DocTypeCompleter::new(),
            hinter,
        };
        rl.set_helper(Some(h));

        let ceh = Box::new(CompleteHintHandler);
        rl.bind_sequence(KeyEvent::ctrl('E'), EventHandler::Conditional(ceh.clone()));
        rl.bind_sequence(KeyEvent::alt('f'), EventHandler::Conditional(ceh));

        Ok(Self { rl })
    }

    pub fn readline(&mut self) -> Result<String> {
        let readline = &self.rl.readline("doctype> ")?;

        Ok(readline.to_string())
    }
}
