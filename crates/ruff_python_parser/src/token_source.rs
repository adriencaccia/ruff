use ruff_python_trivia::CommentRanges;
use ruff_text_size::{TextRange, TextSize};

use crate::lexer::{Lexer, LexerCheckpoint, LexicalError, Token, TokenFlags, TokenValue};
use crate::{Mode, TokenKind};

/// Token source for the parser that skips over any trivia tokens.
#[derive(Debug)]
pub(crate) struct TokenSource<'src> {
    /// The underlying source for the tokens.
    lexer: Lexer<'src>,

    /// A vector containing all the tokens emitted by the lexer. This is returned when the parser
    /// is finished consuming all the tokens. Note that unlike the emitted tokens, this vector
    /// holds both the trivia and non-trivia tokens.
    tokens: Vec<Token>,

    /// A vector containing the range of all the comment tokens emitted by the lexer.
    comments: Vec<TextRange>,
}

impl<'src> TokenSource<'src> {
    /// Create a new token source for the given lexer.
    pub(crate) fn new(lexer: Lexer<'src>) -> Self {
        // TODO(dhruvmanila): Use `allocate_tokens_vec`
        TokenSource {
            lexer,
            tokens: vec![],
            comments: vec![],
        }
    }

    /// Create a new token source from the given source code which starts at the given offset.
    pub(crate) fn from_source(source: &'src str, mode: Mode, start_offset: TextSize) -> Self {
        let lexer = Lexer::new(source, mode, start_offset);
        let mut source = TokenSource::new(lexer);

        // Initialize the token source so that the current token is set correctly.
        source.do_bump();
        source
    }

    /// Returns the kind of the current token.
    pub(crate) fn current_kind(&self) -> TokenKind {
        self.lexer.current_kind()
    }

    /// Returns the range of the current token.
    pub(crate) fn current_range(&self) -> TextRange {
        self.lexer.current_range()
    }

    /// Returns the flags for the current token.
    pub(crate) fn current_flags(&self) -> TokenFlags {
        self.lexer.current_flags()
    }

    /// Calls the underlying [`take_value`] method on the lexer. Refer to its documentation
    /// for more info.
    ///
    /// [`take_value`]: Lexer::take_value
    pub(crate) fn take_value(&mut self) -> TokenValue {
        self.lexer.take_value()
    }

    /// Returns the next non-trivia token without consuming it.
    ///
    /// Use [`peek2`] to get the next two tokens.
    ///
    /// [`peek2`]: TokenSource::peek2
    pub(crate) fn peek(&mut self) -> TokenKind {
        let checkpoint = self.lexer.checkpoint();
        let next = self.next_non_trivia_token();
        self.lexer.rewind(checkpoint);
        next
    }

    /// Returns the next two non-trivia tokens without consuming it.
    ///
    /// Use [`peek`] to only get the next token.
    ///
    /// [`peek`]: TokenSource::peek
    pub(crate) fn peek2(&mut self) -> (TokenKind, TokenKind) {
        let checkpoint = self.lexer.checkpoint();
        let first = self.next_non_trivia_token();
        let second = self.next_non_trivia_token();
        self.lexer.rewind(checkpoint);
        (first, second)
    }

    /// Bumps the token source to the next non-trivia token.
    ///
    /// It pushes the given kind to the token vector with the current token range.
    pub(crate) fn bump(&mut self, kind: TokenKind) {
        self.tokens
            .push(Token::new(kind, self.current_range(), self.current_flags()));
        self.do_bump();
    }

    /// Bumps the token source to the next non-trivia token without adding the current token to the
    /// token vector. It does add the trivia tokens to the token vector.
    fn do_bump(&mut self) {
        loop {
            let kind = self.lexer.next_token();
            if is_trivia(kind) {
                if kind == TokenKind::Comment {
                    self.comments.push(self.current_range());
                }
                self.tokens
                    .push(Token::new(kind, self.current_range(), self.current_flags()));
                continue;
            }
            break;
        }
    }

    /// Returns the next non-trivia token without adding it to the token vector.
    fn next_non_trivia_token(&mut self) -> TokenKind {
        loop {
            let kind = self.lexer.next_token();
            if is_trivia(kind) {
                continue;
            }
            break kind;
        }
    }

    /// Creates a checkpoint to which the token source can later return to using [`Self::rewind`].
    pub(crate) fn checkpoint(&self) -> TokenSourceCheckpoint<'src> {
        TokenSourceCheckpoint {
            lexer_checkpoint: self.lexer.checkpoint(),
            tokens_position: self.tokens.len(),
            comments_position: self.comments.len(),
        }
    }

    /// Restore the token source to the given checkpoint.
    pub(crate) fn rewind(&mut self, checkpoint: TokenSourceCheckpoint<'src>) {
        let TokenSourceCheckpoint {
            lexer_checkpoint,
            tokens_position,
            comments_position,
        } = checkpoint;

        self.lexer.rewind(lexer_checkpoint);
        self.tokens.truncate(tokens_position);
        self.comments.truncate(comments_position);
    }

    /// Consumes the token source, returning the collected tokens, comment ranges, and any errors
    /// encountered during lexing. The token collection includes both the trivia and non-trivia
    /// tokens.
    pub(crate) fn finish(mut self) -> (Vec<Token>, CommentRanges, Vec<LexicalError>) {
        assert_eq!(
            self.current_kind(),
            TokenKind::EndOfFile,
            "TokenSource was not fully consumed"
        );

        // The `EndOfFile` token shouldn't be included in the token stream, it's mainly to signal
        // the parser to stop. This isn't in `do_bump` because it only needs to be done once.
        if let Some(last) = self.tokens.pop() {
            assert_eq!(last.kind(), TokenKind::EndOfFile);
        }

        let comment_ranges = CommentRanges::new(self.comments);
        (self.tokens, comment_ranges, self.lexer.finish())
    }
}

pub(crate) struct TokenSourceCheckpoint<'src> {
    lexer_checkpoint: LexerCheckpoint<'src>,
    tokens_position: usize,
    comments_position: usize,
}

/// Allocates a [`Vec`] with an approximated capacity to fit all tokens
/// of `contents`.
///
/// See [#9546](https://github.com/astral-sh/ruff/pull/9546) for a more detailed explanation.
#[allow(dead_code)]
fn allocate_tokens_vec(contents: &str) -> Vec<Token> {
    let lower_bound = contents.len().saturating_mul(15) / 100;
    Vec::with_capacity(lower_bound)
}

fn is_trivia(token: TokenKind) -> bool {
    matches!(token, TokenKind::Comment | TokenKind::NonLogicalNewline)
}
