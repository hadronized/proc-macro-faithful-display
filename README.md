# A more faithful [`Display`] implementation for [proc-macro] types

This crate provides a *faithful* implementation of display regarding the input token stream. That
is, the display formatted output will contain the same spaces and newlines as the input flow of
Rust tokens.

Feel free to browse the [documentation] for further details.

[`Display`]: https://doc.rust-lang.org/std/fmt/trait.Display.html
[proc-macro]: https://doc.rust-lang.org/stable/proc_macro
[documentation]: https://docs.rs/proc-macro-faithful-display
