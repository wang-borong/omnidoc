use std::collections::HashSet;
use rustyline::hint::{Hint, Hinter};
use rustyline::history::DefaultHistory;
use rustyline::Context;
use rustyline::{Completer, Helper, Highlighter, Hinter, Validator};
use std::borrow::Cow::{self, Borrowed, Owned};
use rustyline::highlight::Highlighter;
use rustyline::{
    Cmd, ConditionalEventHandler, Editor, Event, EventContext, EventHandler, KeyEvent, RepeatCount,
    Result
};

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

fn diy_hints() -> HashSet<CommandHint> {
    let mut set = HashSet::new();
    set.insert(CommandHint::new("ls", "ls"));
    set.insert(CommandHint::new("list", "list"));
    set.insert(CommandHint::new("ebook-md", "ebook-md"));
    set.insert(CommandHint::new("enote-md", "enote-md"));
    set.insert(CommandHint::new("ebook-tex", "ebook-tex"));
    set.insert(CommandHint::new("enote-tex", "enote-tex"));
    set.insert(CommandHint::new("myart-tex", "myart-tex"));
    set.insert(CommandHint::new("myrep-tex", "myrep-tex"));
    set.insert(CommandHint::new("mybook-tex", "mybook-tex"));
    set.insert(CommandHint::new("resume-ng-tex", "resume-ng-tex"));
    set
}

#[derive(Completer, Helper, Hinter, Validator)]
struct MyHelper(#[rustyline(Hinter)] DocTypeHinter);

impl Highlighter for MyHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Owned(format!("\x1b[1;32m{prompt}\x1b[m"))
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned(format!("\x1b[1m{hint}\x1b[m"))
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

struct TabEventHandler;
impl ConditionalEventHandler for TabEventHandler {
    fn handle(&self, _: &Event, _: RepeatCount, _: bool, ctx: &EventContext) -> Option<Cmd> {
        if !ctx.has_hint() {
            return None; // default
        }
        Some(Cmd::CompleteHint)
    }
}

pub struct DTRL {
    rl: Editor<MyHelper, DefaultHistory>,
}

impl DTRL {
    pub fn new() -> Result<Self> {
        let h = DocTypeHinter { hints: diy_hints() };
        let mut rl: Editor<MyHelper, DefaultHistory> = Editor::new()?;
        rl.set_helper(Some(MyHelper(h)));

        let ceh = Box::new(CompleteHintHandler);
        rl.bind_sequence(KeyEvent::ctrl('E'), EventHandler::Conditional(ceh.clone()));
        rl.bind_sequence(KeyEvent::alt('f'), EventHandler::Conditional(ceh));
        rl.bind_sequence(
            KeyEvent::from('\t'),
            EventHandler::Conditional(Box::new(TabEventHandler)),
        );
        //rl.bind_sequence(
        //    Event::KeySeq(vec![KeyEvent::ctrl('X'), KeyEvent::ctrl('E')]),
        //    EventHandler::Simple(Cmd::Suspend), // TODO external editor
        //);

        Ok(Self {
            rl
        })
    }

    pub fn readline(&mut self) -> Result<String> {

        let readline = &self.rl.readline("doctype> ")?;

        Ok(readline.to_string())
    }
}

