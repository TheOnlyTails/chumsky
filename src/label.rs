//! Items related to parser labelling.

use super::*;

/// A trait implemented by [`Error`]s that can originate from labelled parsers. See [`Parser::labelled`].
pub trait LabelError<'a, I: Input<'a>, L>: Error<'a, I> {
    /// Annotate the expected patterns within this parser with the given label.
    ///
    /// In practice, this usually removes all other labels and expected tokens in favor of a single label that
    /// represents the overall pattern.
    fn label_with(&mut self, label: L);

    /// Annotate this error, indicating that it occurred within the context denoted by the given label.
    ///
    /// A span that runs from the beginning of the context up until the error location is also provided.
    ///
    /// In practice, this usually means adding the context to a context 'stack', similar to a backtrace.
    fn in_context(&mut self, label: L, span: I::Span);
}

/// See [`Parser::labelled`].
#[derive(Copy, Clone)]
pub struct Labelled<A, L> {
    pub(crate) parser: A,
    pub(crate) label: L,
    pub(crate) is_context: bool,
}

impl<A, L> Labelled<A, L> {
    /// Specify that the label should be used as context when reporting errors.
    ///
    /// This allows error messages to use this label to add information to errors that occur *within* this parser.
    pub fn as_context(self) -> Self {
        Self {
            is_context: true,
            ..self
        }
    }
}

impl<'a, I, O, E, A, L> ParserSealed<'a, I, O, E> for Labelled<A, L>
where
    I: Input<'a>,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
    L: Clone,
    E::Error: LabelError<'a, I, L>,
{
    #[inline]
    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O> {
        let old_alt = inp.errors.alt.take();
        let before = inp.save();
        let res = self.parser.go::<M>(inp);

        // TODO: Label secondary errors too?
        let new_alt = inp.errors.alt.take();
        inp.errors.alt = old_alt;

        if let Some(mut new_alt) = new_alt {
            let before_loc = I::cursor_location(&before.cursor().inner);
            let new_alt_loc = I::cursor_location(&new_alt.pos);
            if new_alt_loc == before_loc {
                new_alt.err.label_with(self.label.clone());
            } else if self.is_context && new_alt_loc > before_loc {
                // SAFETY: cursors generated by previous call to `InputRef::next` (or similar).
                let span = unsafe { I::span(inp.cache, &before.cursor().inner..&new_alt.pos) };
                new_alt.err.in_context(self.label.clone(), span);
            }
            inp.add_alt_err(&new_alt.pos, new_alt.err);
        }

        if self.is_context {
            for err in inp.errors.secondary_errors_since(before.err_count) {
                // SAFETY: cursors generated by previous call to `InputRef::next` (or similar).
                let span = unsafe { I::span(inp.cache, &before.cursor().inner..&err.pos) };
                err.err.in_context(self.label.clone(), span);
            }
        }

        res
    }

    go_extra!(O);
}
