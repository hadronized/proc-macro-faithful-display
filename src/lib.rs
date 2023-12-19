#![feature(proc_macro_span)]

//! An alternative [`Display`] impl for [proc_macro], respecting the input layout and formatting.
//!
//! The idea is that the impl of [`Display`] for [proc_macro] types doesn’t respect the input’s
//! layout. For most commun Rust use cases, this is okay, because the language doesn’t depend on
//! whitespaces and has its own grammar for floating point numbers, field access, etc. However, for
//! all other use cases, you will lose your formatting and indentation. Plus, some [EDSLs] might
//! require strict use of newlines or symbols with a leading colon, comma, etc. without whitespaces.
//!
//! This crate provides an implementation of [`Display`] that respects the input’s formatting, so
//! that one can display a [`TokenStream`] and parse it with a more esoteric parser than [syn].
//!
//! > Currently, this crate highly depends on *nightly* features. You cannot use it on the *stable*
//! > channel… just yet.
//!
//! You can get a faithful [`Display`] object by calling the [`faithful_display`] function on your
//! [`TokenStream`].
//!
//! > At the time of writing, traits don’t allow [existential `impl Trait`] to be used in methods.
//! > This is unfortunate, then the feature is accessed through a function instead of a method.
//!
//! [EDSLs]: https://wiki.haskell.org/Embedded_domain_specific_language
//! [syn]: https://crates.io/crates/syn
//! [existential `impl Trait`]: https://rust-lang-nursery.github.io/edition-guide/rust-2018/trait-system/impl-trait-for-returning-complex-types-with-ease.html#return-position

extern crate proc_macro;

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Span, TokenStream, TokenTree};
use std::fmt::{self, Display, Write};
use std::iter::FromIterator;

/// A more faithful [`Display`].
///
/// This trait works by accumulating a [`Span`] as it formats tokens. By recomputing on the
/// fly on the layout of each token, it’s possible to insert newlines and spaces to respect the
/// initial formatting.
pub trait FaithfulDisplay {
    /// Display a token in a faithful way.
    fn faithful_fmt(&self, f: &mut fmt::Formatter, prev_span: Span) -> Result<Span, fmt::Error>;
}

impl FaithfulDisplay for Ident {
    fn faithful_fmt(&self, f: &mut fmt::Formatter, prev_span: Span) -> Result<Span, fmt::Error> {
        let current_span = self.span();
        whitespace_adjust_span(f, prev_span, current_span.start())?;

        self.fmt(f).map(|_| current_span.end())
    }
}

impl FaithfulDisplay for Literal {
    fn faithful_fmt(&self, f: &mut fmt::Formatter, prev_span: Span) -> Result<Span, fmt::Error> {
        let current_span = self.span();
        whitespace_adjust_span(f, prev_span, current_span.start())?;

        self.fmt(f).map(|_| current_span.end())
    }
}

impl FaithfulDisplay for Punct {
    fn faithful_fmt(&self, f: &mut fmt::Formatter, prev_span: Span) -> Result<Span, fmt::Error> {
        let current_span = self.span();
        whitespace_adjust_span(f, prev_span, current_span.start())?;

        f.write_char(self.as_char()).map(|_| current_span.end())
    }
}

impl FaithfulDisplay for Group {
    fn faithful_fmt(&self, f: &mut fmt::Formatter, prev_span: Span) -> Result<Span, fmt::Error> {
        let current_span = self.span_open();
        whitespace_adjust_span(f, prev_span, current_span.start())?;

        match self.delimiter() {
            Delimiter::Parenthesis => {
                faithful_delimited(
                    f,
                    '(',
                    ')',
                    self.stream(),
                    current_span.end(),
                    self.span_close().start(),
                )?;
            }

            Delimiter::Brace => {
                faithful_delimited(
                    f,
                    '{',
                    '}',
                    self.stream(),
                    current_span.end(),
                    self.span_close().start(),
                )?;
            }

            Delimiter::Bracket => {
                faithful_delimited(
                    f,
                    '[',
                    ']',
                    self.stream(),
                    current_span.end(),
                    self.span_close().start(),
                )?;
            }

            Delimiter::None => {
                let line_col = self.stream().faithful_fmt(f, current_span.end())?;
                whitespace_adjust_span(f, prev_span, line_col)?;
            }
        }

        Ok(self.span_close().end())
    }
}

impl FaithfulDisplay for TokenStream {
    fn faithful_fmt(&self, f: &mut fmt::Formatter, prev_span: Span) -> Result<Span, fmt::Error> {
        let mut current_span = prev_span;

        for tree in self.clone() {
            current_span = tree.faithful_fmt(f, current_span)?;
        }

        Ok(current_span)
    }
}

impl FaithfulDisplay for TokenTree {
    fn faithful_fmt(&self, f: &mut fmt::Formatter, prev_span: Span) -> Result<Span, fmt::Error> {
        match self {
            TokenTree::Group(gr) => gr.faithful_fmt(f, prev_span),
            TokenTree::Ident(ident) => ident.faithful_fmt(f, prev_span),
            TokenTree::Punct(p) => p.faithful_fmt(f, prev_span),
            TokenTree::Literal(lit) => lit.faithful_fmt(f, prev_span),
        }
    }
}

/// Create a [`Display`] object out of a [`TokenStream`] that respects as closely as possible its
/// formatting.
///
/// > Disclaimer: because this function takes a reference and because [`TokenStream`] – at the time
/// > of writing – doesn’t support reference-based iteration, a complete deep clone of the token
/// > tree has to be performed prior to displaying it.
pub fn faithful_display(stream: &TokenStream) -> impl Display + '_ {
    struct D<'a>(&'a TokenStream);

    impl<'a> fmt::Display for D<'a> {
        fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            // get the first span, if any
            let mut iter = self.0.clone().into_iter();
            let first = iter.next();

            if let Some(tree) = first {
                let first_line_col = tree.span().start();
                let line_col = tree.faithful_fmt(f, first_line_col)?;

                TokenStream::from_iter(iter)
                    .faithful_fmt(f, line_col)
                    .map(|_| ())
            } else {
                Ok(())
            }
        }
    }

    D(stream)
}

/// Automatically adjust with whitespaces a formatter based on the current span and the previous
/// one.
///
/// This function is key to the overall implementation, has it enables to respect the input
/// indentation and general formatting.
fn whitespace_adjust_span(
    f: &mut fmt::Formatter,
    prev_span: Span,
    current_span: Span,
) -> Result<(), fmt::Error> {
    if current_span.line() == prev_span.line() {
        // we are on the same line, we just have to adjust the number of spaces
        let nb_spaces = current_span.column() - prev_span.column();
        f.write_str(" ".repeat(nb_spaces).as_str())
    } else {
        // we are on different lines; first add the newlines difference, then adjust with spaces
        let nb_newlines = current_span.line() - prev_span.line();
        let nb_spaces = current_span.column();
        f.write_str("\n".repeat(nb_newlines).as_str())?;
        f.write_str(" ".repeat(nb_spaces).as_str())
    }
}

/// Display a token stream that is surrounded by two matching characters.
fn faithful_delimited(
    f: &mut fmt::Formatter,
    del_first: char,
    del_end: char,
    stream: TokenStream,
    prev_span: Span,
    final_span: Span,
) -> Result<(), fmt::Error> {
    f.write_char(del_first)?;

    let current_span = stream.faithful_fmt(f, prev_span)?;

    whitespace_adjust_span(f, current_span, final_span)?;
    f.write_char(del_end)
}
