use crate::format_element::tag::{Condition, Tag};
use crate::prelude::tag::{DedentMode, GroupMode, LabelId};
use crate::prelude::*;
use crate::{Argument, Arguments, GroupId, TextRange, TextSize, format_element, write};
use crate::{Buffer, VecBuffer};
use Tag::*;
use biome_rowan::{Language, SyntaxNode, SyntaxToken, TextLen, TokenText};
use std::borrow::Cow;
use std::cell::Cell;
use std::marker::PhantomData;
use std::num::NonZeroU8;

/// A line break that only gets printed if the enclosing `Group` doesn't fit on a single line.
/// It's omitted if the enclosing `Group` fits on a single line.
/// A soft line break is identical to a hard line break when not enclosed inside of a `Group`.
///
/// # Examples
///
/// Soft line breaks are omitted if the enclosing `Group` fits on a single line
///
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![text("a,"), soft_line_break(), text("b")])
/// ])?;
///
/// assert_eq!(
///     "a,b",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
/// See [soft_line_break_or_space] if you want to insert a space between the elements if the enclosing
/// `Group` fits on a single line.
///
/// Soft line breaks are emitted if the enclosing `Group` doesn't fit on a single line
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(10).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("a long word,"),
///         soft_line_break(),
///         text("so that the group doesn't fit on a single line"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "a long word,\nso that the group doesn't fit on a single line",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub const fn soft_line_break() -> Line {
    Line::new(LineMode::Soft)
}

/// A forced line break that are always printed. A hard line break forces any enclosing `Group`
/// to be printed over multiple lines.
///
/// # Examples
///
/// It forces a line break, even if the enclosing `Group` would otherwise fit on a single line.
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("a,"),
///         hard_line_break(),
///         text("b"),
///         hard_line_break()
///     ])
/// ])?;
///
/// assert_eq!(
///     "a,\nb\n",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub const fn hard_line_break() -> Line {
    Line::new(LineMode::Hard)
}

/// A forced empty line. An empty line inserts enough line breaks in the output for
/// the previous and next element to be separated by an empty line.
///
/// # Examples
///
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// fn main() -> FormatResult<()> {
/// let elements = format!(
///     SimpleFormatContext::default(), [
///     group(&format_args![
///         text("a,"),
///         empty_line(),
///         text("b"),
///         empty_line()
///     ])
/// ])?;
///
/// assert_eq!(
///     "a,\n\nb\n\n",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub const fn empty_line() -> Line {
    Line::new(LineMode::Empty)
}

/// A line break if the enclosing `Group` doesn't fit on a single line, a space otherwise.
///
/// # Examples
///
/// The line breaks are emitted as spaces if the enclosing `Group` fits on a a single line:
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("a,"),
///         soft_line_break_or_space(),
///         text("b"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "a, b",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// The printer breaks the lines if the enclosing `Group` doesn't fit on a single line:
/// ```
/// use biome_formatter::{format_args, format, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(10).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("a long word,"),
///         soft_line_break_or_space(),
///         text("so that the group doesn't fit on a single line"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "a long word,\nso that the group doesn't fit on a single line",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub const fn soft_line_break_or_space() -> Line {
    Line::new(LineMode::SoftOrSpace)
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Line {
    mode: LineMode,
}

impl Line {
    const fn new(mode: LineMode) -> Self {
        Self { mode }
    }
}

impl<Context> Format<Context> for Line {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Line(self.mode))
    }
}

impl std::fmt::Debug for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Line").field(&self.mode).finish()
    }
}

/// Creates a token that gets written as is to the output. Make sure to properly escape the text if
/// it's user generated (e.g. a string and not a language keyword).
///
/// # Line feeds
/// Tokens may contain line breaks but they must use the line feeds (`\n`).
/// The [crate::Printer] converts the line feed characters to the character specified in the [crate::PrinterOptions].
///
/// # Examples
///
/// ```
/// use biome_formatter::format;
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [text("Hello World")])?;
///
/// assert_eq!(
///     "Hello World",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Printing a string literal as a literal requires that the string literal is properly escaped and
/// enclosed in quotes (depending on the target language).
///
/// ```
/// use biome_formatter::format;
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// // the tab must be encoded as \\t to not literally print a tab character ("Hello{tab}World" vs "Hello\tWorld")
/// let elements = format!(SimpleFormatContext::default(), [text("\"Hello\\tWorld\"")])?;
///
/// assert_eq!(r#""Hello\tWorld""#, elements.print()?.as_code());
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn text(text: &'static str) -> StaticText {
    debug_assert_no_newlines(text);

    StaticText { text }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct StaticText {
    text: &'static str,
}

impl<Context> Format<Context> for StaticText {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::StaticText { text: self.text })
    }
}

impl std::fmt::Debug for StaticText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::write!(f, "StaticToken({})", self.text)
    }
}

/// Creates a text from a dynamic string and a range of the input source
pub fn dynamic_text(text: &str, position: TextSize) -> DynamicText {
    debug_assert_no_newlines(text);

    DynamicText { text, position }
}

#[derive(Eq, PartialEq)]
pub struct DynamicText<'a> {
    text: &'a str,
    position: TextSize,
}

impl<Context> Format<Context> for DynamicText<'_> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::DynamicText {
            text: self.text.to_string().into_boxed_str(),
            source_position: self.position,
        })
    }
}

impl std::fmt::Debug for DynamicText<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::write!(f, "DynamicToken({})", self.text)
    }
}

/// String that is the same as in the input source text if `text` is [`Cow::Borrowed`] or
/// some replaced content if `text` is [`Cow::Owned`].
pub fn syntax_token_cow_slice<'a, L: Language>(
    text: Cow<'a, str>,
    token: &'a SyntaxToken<L>,
    start: TextSize,
) -> SyntaxTokenCowSlice<'a, L> {
    debug_assert_no_newlines(&text);

    SyntaxTokenCowSlice { text, token, start }
}

pub struct SyntaxTokenCowSlice<'a, L: Language> {
    text: Cow<'a, str>,
    token: &'a SyntaxToken<L>,
    start: TextSize,
}

impl<L: Language, Context> Format<Context> for SyntaxTokenCowSlice<'_, L> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        match &self.text {
            Cow::Borrowed(text) => {
                let range = TextRange::at(self.start, text.text_len());
                debug_assert_eq!(
                    *text,
                    &self.token.text()[range - self.token.text_range().start()],
                    "The borrowed string doesn't match the specified token substring. Does the borrowed string belong to this token and range?"
                );

                let relative_range = range - self.token.text_range().start();
                let slice = self.token.token_text().slice(relative_range);

                f.write_element(FormatElement::LocatedTokenText {
                    slice,
                    source_position: self.start,
                })
            }
            Cow::Owned(text) => f.write_element(FormatElement::DynamicText {
                text: text.to_string().into_boxed_str(),
                source_position: self.start,
            }),
        }
    }
}

impl<L: Language> std::fmt::Debug for SyntaxTokenCowSlice<'_, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::write!(f, "SyntaxTokenCowSlice({})", self.text)
    }
}

/// Copies a source text 1:1 into the output text.
pub fn located_token_text<L: Language>(
    token: &SyntaxToken<L>,
    range: TextRange,
) -> LocatedTokenText {
    let relative_range = range - token.text_range().start();
    let slice = token.token_text().slice(relative_range);

    debug_assert_no_newlines(&slice);

    LocatedTokenText {
        text: slice,
        source_position: range.start(),
    }
}

pub struct LocatedTokenText {
    text: TokenText,
    source_position: TextSize,
}

impl<Context> Format<Context> for LocatedTokenText {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::LocatedTokenText {
            slice: self.text.clone(),
            source_position: self.source_position,
        })
    }
}

impl std::fmt::Debug for LocatedTokenText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::write!(f, "LocatedTokenText({})", self.text)
    }
}

fn debug_assert_no_newlines(text: &str) {
    debug_assert!(
        !text.contains('\r'),
        "The content '{text}' contains an unsupported '\\r' line terminator character but text must only use line feeds '\\n' as line separator. Use '\\n' instead of '\\r' and '\\r\\n' to insert a line break in strings."
    );
}

/// Pushes some content to the end of the current line
///
/// ## Examples
///
/// ```
/// use biome_formatter::{format};
/// use biome_formatter::prelude::*;
///
/// fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     text("a"),
///     line_suffix(&text("c")),
///     text("b")
/// ])?;
///
/// assert_eq!(
///     "abc",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn line_suffix<Content, Context>(inner: &Content) -> LineSuffix<Context>
where
    Content: Format<Context>,
{
    LineSuffix {
        content: Argument::new(inner),
    }
}

#[derive(Copy, Clone)]
pub struct LineSuffix<'a, Context> {
    content: Argument<'a, Context>,
}

impl<Context> Format<Context> for LineSuffix<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Tag(StartLineSuffix))?;
        Arguments::from(&self.content).fmt(f)?;
        f.write_element(FormatElement::Tag(EndLineSuffix))
    }
}

impl<Context> std::fmt::Debug for LineSuffix<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("LineSuffix").field(&"{{content}}").finish()
    }
}

/// Inserts a boundary for line suffixes that forces the printer to print all pending line suffixes.
/// Helpful if a line sufix shouldn't pass a certain point.
///
/// ## Examples
///
/// Forces the line suffix "c" to be printed before the token `d`.
/// ```
/// use biome_formatter::format;
/// use biome_formatter::prelude::*;
///
/// # fn  main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     text("a"),
///     line_suffix(&text("c")),
///     text("b"),
///     line_suffix_boundary(),
///     text("d")
/// ])?;
///
/// assert_eq!(
///     "abc\nd",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
pub const fn line_suffix_boundary() -> LineSuffixBoundary {
    LineSuffixBoundary
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LineSuffixBoundary;

impl<Context> Format<Context> for LineSuffixBoundary {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::LineSuffixBoundary)
    }
}

/// Marks some content with a label.
///
/// This does not directly influence how this content will be printed, but some
/// parts of the formatter may inspect the [labelled element](Tag::StartLabelled)
/// using [FormatElements::has_label].
///
/// ## Examples
///
/// ```rust
/// # use biome_formatter::prelude::*;
/// # use biome_formatter::{format, write, LineWidth};
///
/// #[derive(Debug, Copy, Clone)]
/// enum MyLabels {
///     Main
/// }
///
/// impl tag::Label for MyLabels {
///     fn id(&self) -> u64 {
///         *self as u64
///     }
///
///     fn debug_name(&self) -> &'static str {
///         match self {
///             Self::Main => "Main"
///         }
///     }
/// }
///
/// # fn main() -> FormatResult<()> {
/// let formatted = format!(
///     SimpleFormatContext::default(),
///     [format_with(|f| {
///         let mut recording = f.start_recording();
///         write!(recording, [
///             labelled(
///                 LabelId::of(MyLabels::Main),
///                 &text("'I have a label'")
///             )
///         ])?;
///
///         let recorded = recording.stop();
///
///         let is_labelled = recorded.first().is_some_and(|element| element.has_label(LabelId::of(MyLabels::Main)));
///
///         if is_labelled {
///             write!(f, [text(" has label `Main`")])
///         } else {
///             write!(f, [text(" doesn't have label `Main`")])
///         }
///     })]
/// )?;
///
/// assert_eq!("'I have a label' has label `Main`", formatted.print()?.as_code());
/// # Ok(())
/// # }
/// ```
///
/// ## Alternatives
///
/// Use `Memoized.inspect(f)?.has_label(LabelId::of(MyLabels::Main)` if you need to know if some content breaks that should
/// only be written later.
#[inline]
pub fn labelled<Content, Context>(label_id: LabelId, content: &Content) -> FormatLabelled<Context>
where
    Content: Format<Context>,
{
    FormatLabelled {
        label_id,
        content: Argument::new(content),
    }
}

#[derive(Copy, Clone)]
pub struct FormatLabelled<'a, Context> {
    label_id: LabelId,
    content: Argument<'a, Context>,
}

impl<Context> Format<Context> for FormatLabelled<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Tag(StartLabelled(self.label_id)))?;
        Arguments::from(&self.content).fmt(f)?;
        f.write_element(FormatElement::Tag(EndLabelled))
    }
}

impl<Context> std::fmt::Debug for FormatLabelled<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Label")
            .field(&self.label_id)
            .field(&"{{content}}")
            .finish()
    }
}

/// Inserts a single space. Allows to separate different tokens.
///
/// # Examples
///
/// ```
/// use biome_formatter::format;
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// // the tab must be encoded as \\t to not literally print a tab character ("Hello{tab}World" vs "Hello\tWorld")
/// let elements = format!(SimpleFormatContext::default(), [text("a"), space(), text("b")])?;
///
/// assert_eq!("a b", elements.print()?.as_code());
/// # Ok(())
/// # }
/// ```
#[inline]
pub const fn space() -> Space {
    Space
}

/// Inserts a single space.
/// The main difference with space is that
/// it always adds a space even when it's the last element of a group.
///
/// # Examples
///
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(20).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("nineteen_characters"),
///         soft_line_break(),
///         text("1"),
///         hard_space(),
///     ])
/// ])?;
/// assert_eq!(
///     "nineteen_characters\n1",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
/// # Examples
///
/// Without HardSpace
///
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(20).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("nineteen_characters"),
///         soft_line_break(),
///         text("1"),
///         space(),
///     ])
/// ])?;
/// assert_eq!(
///     "nineteen_characters1",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub const fn hard_space() -> HardSpace {
    HardSpace
}

/// Optionally inserts a single space if the given condition is true.
///
/// # Examples
///
/// ```
/// use biome_formatter::format;
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [text("a"), maybe_space(true), text("b")])?;
/// let nospace = format!(SimpleFormatContext::default(), [text("a"), maybe_space(false), text("b")])?;
///
/// assert_eq!("a b", elements.print()?.as_code());
/// assert_eq!("ab", nospace.print()?.as_code());
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn maybe_space(should_insert: bool) -> Option<Space> {
    if should_insert { Some(Space) } else { None }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Space;

impl<Context> Format<Context> for Space {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Space)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct HardSpace;

impl<Context> Format<Context> for HardSpace {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::HardSpace)
    }
}
/// It adds a level of indentation to the given content
///
/// It doesn't add any line breaks at the edges of the content, meaning that
/// the line breaks have to be manually added.
///
/// This helper should be used only in rare cases, instead you should rely more on
/// [block_indent] and [soft_block_indent]
///
/// # Examples
///
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let block = format!(SimpleFormatContext::default(), [
///     text("switch {"),
///     block_indent(&format_args![
///         text("default:"),
///         indent(&format_args![
///             // this is where we want to use a
///             hard_line_break(),
///             text("break;"),
///         ])
///     ]),
///     text("}"),
/// ])?;
///
/// assert_eq!(
///     "switch {\n\tdefault:\n\t\tbreak;\n}",
///     block.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// When the indent_style is tab, [indent] convert the preceding alignments to indents
/// ```
/// use biome_formatter::prelude::*;
/// use biome_formatter::{format, format_args};
///
/// # fn main() -> FormatResult<()> {
/// let block = format!(
///     SimpleFormatContext::default(),
///     [
///         text("root"),
///         indent(&format_args![align(
///             2,
///             &format_args![indent(&format_args![
///                 hard_line_break(),
///                 text("shoud be 3 tabs"),
///             ])]
///         )])
///     ]
/// )?;
///
/// assert_eq!(
///     "root\n\t\t\tshoud be 3 tabs",
///     block.print()?.as_code()
/// );
/// #    Ok(())
/// # }
/// ```
#[inline]
pub fn indent<Content, Context>(content: &Content) -> Indent<Context>
where
    Content: Format<Context>,
{
    Indent {
        content: Argument::new(content),
    }
}

#[derive(Copy, Clone)]
pub struct Indent<'a, Context> {
    content: Argument<'a, Context>,
}

impl<Context> Format<Context> for Indent<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Tag(StartIndent))?;
        Arguments::from(&self.content).fmt(f)?;
        f.write_element(FormatElement::Tag(EndIndent))
    }
}

impl<Context> std::fmt::Debug for Indent<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Indent").field(&"{{content}}").finish()
    }
}

/// It reduces the indention for the given content depending on the closest [indent] or [align] parent element.
/// * [align] Undoes the spaces added by [align]
/// * [indent] Reduces the indention level by one
///
/// This is a No-op if the indention level is zero.
///
/// # Examples
///
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let block = format!(SimpleFormatContext::default(), [
///     text("root"),
///     align(2, &format_args![
///         hard_line_break(),
///         text("aligned"),
///         dedent(&format_args![
///             hard_line_break(),
///             text("not aligned"),
///         ]),
///         dedent(&indent(&format_args![
///             hard_line_break(),
///             text("Indented, not aligned")
///         ]))
///     ]),
///     dedent(&format_args![
///         hard_line_break(),
///         text("Dedent on root level is a no-op.")
///     ])
/// ])?;
///
/// assert_eq!(
///     "root\n  aligned\nnot aligned\n\tIndented, not aligned\nDedent on root level is a no-op.",
///     block.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// ```
/// use biome_formatter::prelude::*;
/// use biome_formatter::{format, format_args, IndentStyle, IndentWidth, SimpleFormatOptions};
///
/// # fn main() -> FormatResult<()> {
///     let context = SimpleFormatContext::new(SimpleFormatOptions {
///         indent_width: IndentWidth::try_from(8).unwrap(),
///         indent_style: IndentStyle::Space,
///         ..SimpleFormatOptions::default()
///     });
///     let elements = format!(
///         context,
///         [
///             text("root"),
///             indent(&format_args![
///                 hard_line_break(),
///                 text("Indented"),
///                 align(
///                     2,
///                     &format_args![
///                         hard_line_break(),
///                         text("Indented and aligned"),
///                         dedent(&format_args![
///                             hard_line_break(),
///                             text("Indented, not aligned"),
///                         ]),
///                     ]
///                 ),
///             ]),
///             align(
///                 2,
///                 &format_args![
///                     hard_line_break(),
///                     text("Aligned"),
///                     indent(&format_args![
///                         hard_line_break(),
///                         text("Aligned, and indented"),
///                         dedent(&format_args![
///                             hard_line_break(),
///                             text("aligned, not Intended"),
///                         ]),
///                     ])
///                 ]
///             ),
///             dedent(&format_args![hard_line_break(), text("root level")])
///         ]
///     )?;
///     assert_eq!(
///      "root\n        Indented\n          Indented and aligned\n        Indented, not aligned\n  Aligned\n          Aligned, and indented\n  aligned, not Intended\nroot level",
///      elements.print()?.as_code()
///  );
/// #    Ok(())
/// # }
/// ```
///
/// ```
/// use biome_formatter::prelude::*;
/// use biome_formatter::{format, format_args};
///
/// # fn main() -> FormatResult<()> {
/// let block = format!(
///     SimpleFormatContext::default(),
///     [
///         text("root"),
///         indent(&format_args![align(
///             2,
///             &format_args![align(
///                 2,
///                 &format_args![indent(&format_args![
///                     hard_line_break(),
///                     text("should be 4 tabs"),
///                     dedent(&format_args![
///                         hard_line_break(),
///                         text("should be 1 tab and 4 spaces"),
///                     ]),
///                 ])]
///             ),]
///         )])
///     ]
/// )?;
/// assert_eq!(
///     "root\n\t\t\t\tshould be 4 tabs\n\t    should be 1 tab and 4 spaces",
///     block.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn dedent<Content, Context>(content: &Content) -> Dedent<Context>
where
    Content: Format<Context>,
{
    Dedent {
        content: Argument::new(content),
        mode: DedentMode::Level,
    }
}

#[derive(Copy, Clone)]
pub struct Dedent<'a, Context> {
    content: Argument<'a, Context>,
    mode: DedentMode,
}

impl<Context> Format<Context> for Dedent<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Tag(StartDedent(self.mode)))?;
        Arguments::from(&self.content).fmt(f)?;
        f.write_element(FormatElement::Tag(EndDedent(self.mode)))
    }
}

impl<Context> std::fmt::Debug for Dedent<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Dedent").field(&"{{content}}").finish()
    }
}

/// It resets the indent document so that the content will be printed at the start of the line.
///
/// # Examples
///
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let block = format!(SimpleFormatContext::default(), [
///     text("root"),
///     indent(&format_args![
///         hard_line_break(),
///         text("indent level 1"),
///         indent(&format_args![
///             hard_line_break(),
///             text("indent level 2"),
///             align(2, &format_args![
///                 hard_line_break(),
///                 text("two space align"),
///                 dedent_to_root(&format_args![
///                     hard_line_break(),
///                     text("starts at the beginning of the line")
///                 ]),
///             ]),
///             hard_line_break(),
///             text("end indent level 2"),
///         ])
///  ]),
/// ])?;
///
/// assert_eq!(
///     "root\n\tindent level 1\n\t\tindent level 2\n\t\t  two space align\nstarts at the beginning of the line\n\t\tend indent level 2",
///     block.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// ## Prettier
///
/// This resembles the behaviour of Prettier's `align(Number.NEGATIVE_INFINITY, content)` IR element.
#[inline]
pub fn dedent_to_root<Content, Context>(content: &Content) -> Dedent<Context>
where
    Content: Format<Context>,
{
    Dedent {
        content: Argument::new(content),
        mode: DedentMode::Root,
    }
}

/// Aligns its content by indenting the content by `count` spaces.
///
/// [align] is a variant of `[indent]` that indents its content by a specified number of spaces rather than
/// using the configured indent character (tab or a specified number of spaces).
///
/// You should use [align] when you want to indent a content by a specific number of spaces.
/// Using [indent] is preferred in all other situations as it respects the users preferred indent character.
///
/// # Examples
///
/// ## Tab indention
///
/// ```
/// use std::num::NonZeroU8;
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let block = format!(SimpleFormatContext::default(), [
///     text("a"),
///     hard_line_break(),
///     text("?"),
///     space(),
///     align(2, &format_args![
///         text("function () {"),
///         hard_line_break(),
///         text("}"),
///     ]),
///     hard_line_break(),
///     text(":"),
///     space(),
///     align(2, &format_args![
///         text("function () {"),
///         block_indent(&text("console.log('test');")),
///         text("}"),
///     ]),
///     text(";")
/// ])?;
///
/// assert_eq!(
///     "a\n? function () {\n  }\n: function () {\n\t\tconsole.log('test');\n  };",
///     block.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// You can see that:
///
/// * the printer indents the function's `}` by two spaces because it is inside of an `align`.
/// * the block `console.log` gets indented by two tabs.
///   This is because `align` increases the indention level by one (same as `indent`)
///   if you nest an `indent` inside an `align`.
///   Meaning that, `align > ... > indent` results in the same indention as `indent > ... > indent`.
///
/// ## Spaces indention
///
/// ```
/// use std::num::NonZeroU8;
/// use biome_formatter::{format, format_args, IndentStyle, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     indent_style: IndentStyle::Space,
///     indent_width: 4.try_into().unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let block = format!(context, [
///     text("a"),
///     hard_line_break(),
///     text("?"),
///     space(),
///     align(2, &format_args![
///         text("function () {"),
///         hard_line_break(),
///         text("}"),
///     ]),
///     hard_line_break(),
///     text(":"),
///     space(),
///     align(2, &format_args![
///         text("function () {"),
///         block_indent(&text("console.log('test');")),
///         text("}"),
///     ]),
///     text(";")
/// ])?;
///
/// assert_eq!(
///     "a\n? function () {\n  }\n: function () {\n      console.log('test');\n  };",
///     block.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// The printing of `align` differs if using spaces as indention sequence *and* it contains an `indent`.
/// You can see the difference when comparing the indention of the `console.log(...)` expression to the previous example:
///
/// * tab indention: Printer indents the expression with two tabs because the `align` increases the indention level.
/// * space indention: Printer indents the expression by 4 spaces (one indention level) **and** 2 spaces for the align.
pub fn align<Content, Context>(count: u8, content: &Content) -> Align<Context>
where
    Content: Format<Context>,
{
    Align {
        count: NonZeroU8::new(count).expect("Alignment count must be a non-zero number."),
        content: Argument::new(content),
    }
}

#[derive(Copy, Clone)]
pub struct Align<'a, Context> {
    count: NonZeroU8,
    content: Argument<'a, Context>,
}

impl<Context> Format<Context> for Align<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Tag(StartAlign(tag::Align(self.count))))?;
        Arguments::from(&self.content).fmt(f)?;
        f.write_element(FormatElement::Tag(EndAlign))
    }
}

impl<Context> std::fmt::Debug for Align<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Align")
            .field("count", &self.count)
            .field("content", &"{{content}}")
            .finish()
    }
}

/// Inserts a hard line break before and after the content and increases the indention level for the content by one.
///
/// Block indents indent a block of code, such as in a function body, and therefore insert a line
/// break before and after the content.
///
/// Doesn't create an indention if the passed in content is [FormatElement.is_empty].
///
/// # Examples
///
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let block = format![
///     SimpleFormatContext::default(),
///     [
///         text("{"),
///         block_indent(&format_args![
///             text("let a = 10;"),
///             hard_line_break(),
///             text("let c = a + 5;"),
///         ]),
///         text("}"),
///     ]
/// ]?;
///
/// assert_eq!(
///     "{\n\tlet a = 10;\n\tlet c = a + 5;\n}",
///     block.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn block_indent<Context>(content: &impl Format<Context>) -> BlockIndent<Context> {
    BlockIndent {
        content: Argument::new(content),
        mode: IndentMode::Block,
    }
}

/// Indents the content by inserting a line break before and after the content and increasing
/// the indention level for the content by one if the enclosing group doesn't fit on a single line.
/// Doesn't change the formatting if the enclosing group fits on a single line.
///
/// # Examples
///
/// Indents the content by one level and puts in new lines if the enclosing `Group` doesn't fit on a single line
///
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(10).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("'First string',"),
///             soft_line_break_or_space(),
///             text("'second string',"),
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[\n\t'First string',\n\t'second string',\n]",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Doesn't change the formatting if the enclosing `Group` fits on a single line
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("5,"),
///             soft_line_break_or_space(),
///             text("10"),
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[5, 10]",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn soft_block_indent<Context>(content: &impl Format<Context>) -> BlockIndent<Context> {
    BlockIndent {
        content: Argument::new(content),
        mode: IndentMode::Soft,
    }
}

/// Conditionally adds spaces around the content if its enclosing group fits
/// within a single line and the caller suggests that the space should be added.
/// Otherwise indents the content and separates it by line breaks.
///
/// # Examples
///
/// Adds line breaks and indents the content if the enclosing group doesn't fit on the line.
///
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(10).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("{"),
///         soft_block_indent_with_maybe_space(&format_args![
///             text("aPropertyThatExceeds"),
///             text(":"),
///             space(),
///             text("'line width'"),
///         ], false),
///         text("}")
///     ])
/// ])?;
///
/// assert_eq!(
///     "{\n\taPropertyThatExceeds: 'line width'\n}",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Adds spaces around the content if the caller requests it and the group fits on the line.
///
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("{"),
///         soft_block_indent_with_maybe_space(&format_args![
///             text("a"),
///             text(":"),
///             space(),
///             text("5"),
///         ], true),
///         text("}")
///     ])
/// ])?;
///
/// assert_eq!(
///     "{ a: 5 }",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Does not add spaces around the content if the caller denies it and the group fits on the line.
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("{"),
///         soft_block_indent_with_maybe_space(&format_args![
///             text("a"),
///             text(":"),
///             space(),
///             text("5"),
///         ], false),
///         text("}")
///     ])
/// ])?;
///
/// assert_eq!(
///     "{a: 5}",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
pub fn soft_block_indent_with_maybe_space<Context>(
    content: &impl Format<Context>,
    should_add_space: bool,
) -> BlockIndent<Context> {
    if should_add_space {
        soft_space_or_block_indent(content)
    } else {
        soft_block_indent(content)
    }
}

/// If the enclosing `Group` doesn't fit on a single line, inserts a line break and indent.
/// Otherwise, just inserts a space.
///
/// Line indents are used to break a single line of code, and therefore only insert a line
/// break before the content and not after the content.
///
/// # Examples
///
/// Indents the content by one level and puts in new lines if the enclosing `Group` doesn't
/// fit on a single line. Otherwise, just inserts a space.
///
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(10).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("name"),
///         space(),
///         text("="),
///         soft_line_indent_or_space(&format_args![
///             text("firstName"),
///             space(),
///             text("+"),
///             space(),
///             text("lastName"),
///         ]),
///     ])
/// ])?;
///
/// assert_eq!(
///     "name =\n\tfirstName + lastName",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Only adds a space if the enclosing `Group` fits on a single line
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("a"),
///         space(),
///         text("="),
///         soft_line_indent_or_space(&text("10")),
///     ])
/// ])?;
///
/// assert_eq!(
///     "a = 10",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn soft_line_indent_or_space<Context>(content: &impl Format<Context>) -> BlockIndent<Context> {
    BlockIndent {
        content: Argument::new(content),
        mode: IndentMode::SoftLineOrSpace,
    }
}

/// It functions similarly to soft_line_indent_or_space, but instead of a regular space, it inserts a hard space.
///
/// # Examples
///
/// Indents the content by one level and puts in new lines if the enclosing `Group` doesn't
/// fit on a single line. Otherwise, just inserts a space.
///
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(10).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("name"),
///         space(),
///         text("="),
///         soft_line_indent_or_hard_space(&format_args![
///             text("firstName"),
///             space(),
///             text("+"),
///             space(),
///             text("lastName"),
///         ]),
///     ])
/// ])?;
///
/// assert_eq!(
///     "name =\n\tfirstName + lastName",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Only adds a space if the enclosing `Group` fits on a single line
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("a"),
///         space(),
///         text("="),
///         soft_line_indent_or_hard_space(&text("10")),
///     ])
/// ])?;
///
/// assert_eq!(
///     "a = 10",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// It enforces a space after the "=" assignment operators
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(8).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("value"),
///         soft_line_break_or_space(),
///         text("="),
///         soft_line_indent_or_hard_space(&format_args![
///             text("10"),
///         ]),
///     ])
/// ])?;
///
/// assert_eq!(
///     "value\n=\n\t10",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn soft_line_indent_or_hard_space<Context>(
    content: &impl Format<Context>,
) -> BlockIndent<Context> {
    BlockIndent {
        content: Argument::new(content),
        mode: IndentMode::HardSpace,
    }
}

#[derive(Copy, Clone)]
pub struct BlockIndent<'a, Context> {
    content: Argument<'a, Context>,
    mode: IndentMode,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum IndentMode {
    Soft,
    Block,
    SoftSpace,
    HardSpace,
    SoftLineOrSpace,
}

impl<Context> Format<Context> for BlockIndent<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        let snapshot = f.snapshot();

        f.write_element(FormatElement::Tag(StartIndent))?;

        match self.mode {
            IndentMode::Soft => write!(f, [soft_line_break()])?,
            IndentMode::Block => write!(f, [hard_line_break()])?,
            IndentMode::SoftLineOrSpace | IndentMode::SoftSpace => {
                write!(f, [soft_line_break_or_space()])?
            }
            IndentMode::HardSpace => write!(f, [hard_space(), soft_line_break()])?,
        }

        let is_empty = {
            let mut recording = f.start_recording();
            recording.write_fmt(Arguments::from(&self.content))?;
            recording.stop().is_empty()
        };

        if is_empty {
            f.restore_snapshot(snapshot);
            return Ok(());
        }

        f.write_element(FormatElement::Tag(EndIndent))?;

        match self.mode {
            IndentMode::Soft => write!(f, [soft_line_break()]),
            IndentMode::Block => write!(f, [hard_line_break()]),
            IndentMode::SoftSpace => write!(f, [soft_line_break_or_space()]),
            IndentMode::SoftLineOrSpace | IndentMode::HardSpace => Ok(()),
        }
    }
}

impl<Context> std::fmt::Debug for BlockIndent<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.mode {
            IndentMode::Soft => "SoftBlockIndent",
            IndentMode::Block => "HardBlockIndent",
            IndentMode::SoftLineOrSpace => "SoftLineIndentOrSpace",
            IndentMode::SoftSpace => "SoftSpaceBlockIndent",
            IndentMode::HardSpace => "HardSpaceBlockIndent",
        };

        f.debug_tuple(name).field(&"{{content}}").finish()
    }
}

/// Adds spaces around the content if its enclosing group fits on a line, otherwise indents the content and separates it by line breaks.
///
/// # Examples
///
/// Adds line breaks and indents the content if the enclosing group doesn't fit on the line.
///
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(10).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("{"),
///         soft_space_or_block_indent(&format_args![
///             text("aPropertyThatExceeds"),
///             text(":"),
///             space(),
///             text("'line width'"),
///         ]),
///         text("}")
///     ])
/// ])?;
///
/// assert_eq!(
///     "{\n\taPropertyThatExceeds: 'line width'\n}",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Adds spaces around the content if the group fits on the line
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("{"),
///         soft_space_or_block_indent(&format_args![
///             text("a"),
///             text(":"),
///             space(),
///             text("5"),
///         ]),
///         text("}")
///     ])
/// ])?;
///
/// assert_eq!(
///     "{ a: 5 }",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
pub fn soft_space_or_block_indent<Context>(content: &impl Format<Context>) -> BlockIndent<Context> {
    BlockIndent {
        content: Argument::new(content),
        mode: IndentMode::SoftSpace,
    }
}

/// Creates a logical `Group` around the content that should either consistently be printed on a single line
/// or broken across multiple lines.
///
/// The printer will try to print the content of the `Group` on a single line, ignoring all soft line breaks and
/// emitting spaces for soft line breaks or spaces. The printer tracks back if it isn't successful either
/// because it encountered a hard line break, or because printing the `Group` on a single line exceeds
/// the configured line width, and thus it must print all its content on multiple lines,
/// emitting line breaks for all line break kinds.
///
/// # Examples
///
/// `Group` that fits on a single line
///
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("1,"),
///             soft_line_break_or_space(),
///             text("2,"),
///             soft_line_break_or_space(),
///             text("3"),
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[1, 2, 3]",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// The printer breaks the `Group` over multiple lines if its content doesn't fit on a single line
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(20).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("'Good morning! How are you today?',"),
///             soft_line_break_or_space(),
///             text("2,"),
///             soft_line_break_or_space(),
///             text("3"),
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[\n\t'Good morning! How are you today?',\n\t2,\n\t3\n]",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn group<Context>(content: &impl Format<Context>) -> Group<Context> {
    Group {
        content: Argument::new(content),
        group_id: None,
        should_expand: false,
    }
}

#[derive(Copy, Clone)]
pub struct Group<'a, Context> {
    content: Argument<'a, Context>,
    group_id: Option<GroupId>,
    should_expand: bool,
}

impl<Context> Group<'_, Context> {
    pub fn with_group_id(mut self, group_id: Option<GroupId>) -> Self {
        self.group_id = group_id;
        self
    }

    /// Changes the [PrintMode] of the group from [`Flat`](PrintMode::Flat) to [`Expanded`](PrintMode::Expanded).
    /// The result is that any soft-line break gets printed as a regular line break.
    ///
    /// This is useful for content rendered inside of a [FormatElement::BestFitting] that prints each variant
    /// in [PrintMode::Flat] to change some content to be printed in [`Expanded`](PrintMode::Expanded) regardless.
    /// See the documentation of the [`best_fitting`] macro for an example.
    pub fn should_expand(mut self, should_expand: bool) -> Self {
        self.should_expand = should_expand;
        self
    }
}

impl<Context> Format<Context> for Group<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        let mode = match self.should_expand {
            true => GroupMode::Expand,
            false => GroupMode::Flat,
        };

        f.write_element(FormatElement::Tag(StartGroup(
            tag::Group::new().with_id(self.group_id).with_mode(mode),
        )))?;

        Arguments::from(&self.content).fmt(f)?;

        f.write_element(FormatElement::Tag(EndGroup))
    }
}

impl<Context> std::fmt::Debug for Group<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroupElements")
            .field("group_id", &self.group_id)
            .field("should_expand", &self.should_expand)
            .field("content", &"{{content}}")
            .finish()
    }
}

/// IR element that forces the parent group to print in expanded mode.
///
/// Has no effect if used outside of a group or element that introduce implicit groups (fill element).
///
/// ## Examples
///
/// ```
/// use biome_formatter::{format, format_args, LineWidth};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("'Good morning! How are you today?',"),
///             soft_line_break_or_space(),
///             text("2,"),
///             expand_parent(), // Forces the parent to expand
///             soft_line_break_or_space(),
///             text("3"),
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[\n\t'Good morning! How are you today?',\n\t2,\n\t3\n]",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// # Prettier
/// Equivalent to Prettier's `break_parent` IR element
pub const fn expand_parent() -> ExpandParent {
    ExpandParent
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ExpandParent;

impl<Context> Format<Context> for ExpandParent {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::ExpandParent)
    }
}

/// Adds a conditional content that is emitted only if it isn't inside an enclosing `Group` that
/// is printed on a single line. The element allows, for example, to insert a trailing comma after the last
/// array element only if the array doesn't fit on a single line.
///
/// The element has no special meaning if used outside of a `Group`. In that case, the content is always emitted.
///
/// If you're looking for a way to only print something if the `Group` fits on a single line see [self::if_group_fits_on_line].
///
/// # Examples
///
/// Omits the trailing comma for the last array element if the `Group` fits on a single line
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let elements = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("1,"),
///             soft_line_break_or_space(),
///             text("2,"),
///             soft_line_break_or_space(),
///             text("3"),
///             if_group_breaks(&text(","))
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[1, 2, 3]",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Prints the trailing comma for the last array element if the `Group` doesn't fit on a single line
/// ```
/// use biome_formatter::{format_args, format, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
/// use biome_formatter::printer::PrintWidth;
///
/// fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(20).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let elements = format!(context, [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("'A somewhat longer string to force a line break',"),
///             soft_line_break_or_space(),
///             text("2,"),
///             soft_line_break_or_space(),
///             text("3"),
///             if_group_breaks(&text(","))
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[\n\t'A somewhat longer string to force a line break',\n\t2,\n\t3,\n]",
///     elements.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn if_group_breaks<Content, Context>(content: &Content) -> IfGroupBreaks<Context>
where
    Content: Format<Context>,
{
    IfGroupBreaks {
        content: Argument::new(content),
        group_id: None,
        mode: PrintMode::Expanded,
    }
}

/// Adds a conditional content specific for `Group`s that fit on a single line. The content isn't
/// emitted for `Group`s spanning multiple lines.
///
/// See [if_group_breaks] if you're looking for a way to print content only for groups spanning multiple lines.
///
/// # Examples
///
/// Adds the trailing comma for the last array element if the `Group` fits on a single line
/// ```
/// use biome_formatter::{format, format_args};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let formatted = format!(SimpleFormatContext::default(), [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("1,"),
///             soft_line_break_or_space(),
///             text("2,"),
///             soft_line_break_or_space(),
///             text("3"),
///             if_group_fits_on_line(&text(","))
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[1, 2, 3,]",
///     formatted.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// Omits the trailing comma for the last array element if the `Group` doesn't fit on a single line
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(20).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let formatted = format!(context, [
///     group(&format_args![
///         text("["),
///         soft_block_indent(&format_args![
///             text("'A somewhat longer string to force a line break',"),
///             soft_line_break_or_space(),
///             text("2,"),
///             soft_line_break_or_space(),
///             text("3"),
///             if_group_fits_on_line(&text(","))
///         ]),
///         text("]"),
///     ])
/// ])?;
///
/// assert_eq!(
///     "[\n\t'A somewhat longer string to force a line break',\n\t2,\n\t3\n]",
///     formatted.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn if_group_fits_on_line<Content, Context>(flat_content: &Content) -> IfGroupBreaks<Context>
where
    Content: Format<Context>,
{
    IfGroupBreaks {
        mode: PrintMode::Flat,
        group_id: None,
        content: Argument::new(flat_content),
    }
}

#[derive(Copy, Clone)]
pub struct IfGroupBreaks<'a, Context> {
    content: Argument<'a, Context>,
    group_id: Option<GroupId>,
    mode: PrintMode,
}

impl<Context> IfGroupBreaks<'_, Context> {
    /// Inserts some content that the printer only prints if the group with the specified `group_id`
    /// is printed in multiline mode. The referred group must appear before this element in the document
    /// but doesn't have to one of its ancestors.
    ///
    /// # Examples
    ///
    /// Prints the trailing comma if the array group doesn't fit. The `group_id` is necessary
    /// because `fill` creates an implicit group around each item and tries to print the item in flat mode.
    /// The item `[4]` in this example fits on a single line but the trailing comma should still be printed
    ///
    /// ```
    /// use biome_formatter::{format, format_args, write, LineWidth, SimpleFormatOptions};
    /// use biome_formatter::prelude::*;
    ///
    /// # fn main() -> FormatResult<()> {
    /// let context = SimpleFormatContext::new(SimpleFormatOptions {
    ///     line_width: LineWidth::try_from(20).unwrap(),
    ///     ..SimpleFormatOptions::default()
    /// });
    ///
    /// let formatted = format!(context, [format_with(|f| {
    ///     let group_id = f.group_id("array");
    ///
    ///     write!(f, [
    ///         group(
    ///             &format_args![
    ///                 text("["),
    ///                 soft_block_indent(&format_with(|f| {
    ///                     f.fill()
    ///                         .entry(&soft_line_break_or_space(), &text("1,"))
    ///                         .entry(&soft_line_break_or_space(), &text("234568789,"))
    ///                         .entry(&soft_line_break_or_space(), &text("3456789,"))
    ///                         .entry(&soft_line_break_or_space(), &format_args!(
    ///                             text("["),
    ///                             soft_block_indent(&text("4")),
    ///                             text("]"),
    ///                             if_group_breaks(&text(",")).with_group_id(Some(group_id))
    ///                         ))
    ///                     .finish()
    ///                 })),
    ///                 text("]")
    ///             ],
    ///         ).with_group_id(Some(group_id))
    ///     ])
    /// })])?;
    ///
    /// assert_eq!(
    ///     "[\n\t1, 234568789,\n\t3456789, [4],\n]",
    ///     formatted.print()?.as_code()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_group_id(mut self, group_id: Option<GroupId>) -> Self {
        self.group_id = group_id;
        self
    }
}

impl<Context> Format<Context> for IfGroupBreaks<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Tag(StartConditionalContent(
            Condition::new(self.mode).with_group_id(self.group_id),
        )))?;
        Arguments::from(&self.content).fmt(f)?;
        f.write_element(FormatElement::Tag(EndConditionalContent))
    }
}

impl<Context> std::fmt::Debug for IfGroupBreaks<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.mode {
            PrintMode::Flat => "IfGroupFitsOnLine",
            PrintMode::Expanded => "IfGroupBreaks",
        };

        f.debug_struct(name)
            .field("group_id", &self.group_id)
            .field("content", &"{{content}}")
            .finish()
    }
}

/// Increases the indent level by one if the group with the specified id breaks.
///
/// This IR has the same semantics as using [if_group_breaks] and [if_group_fits_on_line] together.
///
/// ```
/// # use biome_formatter::prelude::*;
/// # use biome_formatter::write;
/// # let format = format_with(|f: &mut Formatter<SimpleFormatContext>| {
/// let id = f.group_id("head");
///
/// write!(f, [
///     group(&text("Head")).with_group_id(Some(id)),
///     if_group_breaks(&indent(&text("indented"))).with_group_id(Some(id)),
///     if_group_fits_on_line(&text("indented")).with_group_id(Some(id))
/// ])
///
/// # });
/// ```
///
/// If you want to indent some content if the enclosing group breaks, use [`indent`].
///
/// Use [if_group_breaks] or [if_group_fits_on_line] if the fitting and breaking content differs more than just the
/// indention level.
///
/// # Examples
///
/// Indent the body of an arrow function if the group wrapping the signature breaks:
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions, write};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let content = format_with(|f| {
///     let group_id = f.group_id("header");
///
///     write!(f, [
///         group(&text("(aLongHeaderThatBreaksForSomeReason) =>")).with_group_id(Some(group_id)),
///         indent_if_group_breaks(&format_args![hard_line_break(), text("a => b")], group_id)
///     ])
/// });
///
/// let context = SimpleFormatContext::new(SimpleFormatOptions {
///     line_width: LineWidth::try_from(20).unwrap(),
///     ..SimpleFormatOptions::default()
/// });
///
/// let formatted = format!(context, [content])?;
///
/// assert_eq!(
///     "(aLongHeaderThatBreaksForSomeReason) =>\n\ta => b",
///     formatted.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
///
/// It doesn't add an indent if the group wrapping the signature doesn't break:
/// ```
/// use biome_formatter::{format, format_args, LineWidth, SimpleFormatOptions, write};
/// use biome_formatter::prelude::*;
///
/// # fn main() -> FormatResult<()> {
/// let content = format_with(|f| {
///     let group_id = f.group_id("header");
///
///     write!(f, [
///         group(&text("(aLongHeaderThatBreaksForSomeReason) =>")).with_group_id(Some(group_id)),
///         indent_if_group_breaks(&format_args![hard_line_break(), text("a => b")], group_id)
///     ])
/// });
///
/// let formatted = format!(SimpleFormatContext::default(), [content])?;
///
/// assert_eq!(
///     "(aLongHeaderThatBreaksForSomeReason) =>\na => b",
///     formatted.print()?.as_code()
/// );
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn indent_if_group_breaks<Content, Context>(
    content: &Content,
    group_id: GroupId,
) -> IndentIfGroupBreaks<Context>
where
    Content: Format<Context>,
{
    IndentIfGroupBreaks {
        group_id,
        content: Argument::new(content),
    }
}

#[derive(Copy, Clone)]
pub struct IndentIfGroupBreaks<'a, Context> {
    content: Argument<'a, Context>,
    group_id: GroupId,
}

impl<Context> Format<Context> for IndentIfGroupBreaks<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        f.write_element(FormatElement::Tag(StartIndentIfGroupBreaks(self.group_id)))?;
        Arguments::from(&self.content).fmt(f)?;
        f.write_element(FormatElement::Tag(EndIndentIfGroupBreaks(self.group_id)))
    }
}

impl<Context> std::fmt::Debug for IndentIfGroupBreaks<'_, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndentIfGroupBreaks")
            .field("group_id", &self.group_id)
            .field("content", &"{{content}}")
            .finish()
    }
}

/// Utility for formatting some content with an inline lambda function.
#[derive(Copy, Clone)]
pub struct FormatWith<Context, T> {
    formatter: T,
    context: PhantomData<Context>,
}

impl<Context, T> Format<Context> for FormatWith<Context, T>
where
    T: Fn(&mut Formatter<Context>) -> FormatResult<()>,
{
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        (self.formatter)(f)
    }
}

impl<Context, T> std::fmt::Debug for FormatWith<Context, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FormatWith").field(&"{{formatter}}").finish()
    }
}

/// Creates an object implementing `Format` that calls the passed closure to perform the formatting.
///
/// # Examples
///
/// ```
/// use biome_formatter::prelude::*;
/// use biome_formatter::{SimpleFormatContext, format, write};
/// use biome_rowan::TextSize;
///
/// struct MyFormat {
///     items: Vec<&'static str>,
/// }
///
/// impl Format<SimpleFormatContext> for MyFormat {
///     fn fmt(&self, f: &mut Formatter<SimpleFormatContext>) -> FormatResult<()> {
///         write!(f, [
///             text("("),
///             block_indent(&format_with(|f| {
///                 let separator = space();
///                 let mut join = f.join_with(&separator);
///
///                 for item in &self.items {
///                     join.entry(&format_with(|f| write!(f, [dynamic_text(item, TextSize::default())])));
///                 }
///                 join.finish()
///             })),
///             text(")")
///         ])
///     }
/// }
///
/// # fn main() -> FormatResult<()> {
/// let formatted = format!(SimpleFormatContext::default(), [MyFormat { items: vec!["a", "b", "c"]}])?;
///
/// assert_eq!("(\n\ta b c\n)", formatted.print()?.as_code());
/// # Ok(())
/// # }
/// ```
pub const fn format_with<Context, T>(formatter: T) -> FormatWith<Context, T>
where
    T: Fn(&mut Formatter<Context>) -> FormatResult<()>,
{
    FormatWith {
        formatter,
        context: PhantomData,
    }
}

/// Creates an inline `Format` object that can only be formatted once.
///
/// This can be useful in situation where the borrow checker doesn't allow you to use [`format_with`]
/// because the code formatting the content consumes the value and cloning the value is too expensive.
/// An example of this is if you want to nest a `FormatElement` or non-cloneable `Iterator` inside of a
/// `block_indent` as shown can see in the examples section.
///
/// # Panics
///
/// Panics if the object gets formatted more than once.
///
/// # Example
///
/// ```
/// use biome_formatter::prelude::*;
/// use biome_formatter::{SimpleFormatContext, format, write, Buffer};
///
/// struct MyFormat;
///
/// fn generate_values() -> impl Iterator<Item=StaticText> {
///     vec![text("1"), text("2"), text("3"), text("4")].into_iter()
/// }
///
/// impl Format<SimpleFormatContext> for MyFormat {
///     fn fmt(&self, f: &mut Formatter<SimpleFormatContext>) -> FormatResult<()> {
///         let mut values = generate_values();
///
///         let first = values.next();
///
///         // Formats the first item outside of the block and all other items inside of the block,
///         // separated by line breaks
///         write!(f, [
///             first,
///             block_indent(&format_once(|f| {
///                 // Using format_with isn't possible here because the iterator gets consumed here
///                 f.join_with(&hard_line_break()).entries(values).finish()
///             })),
///         ])
///     }
/// }
///
/// # fn main() -> FormatResult<()> {
/// let formatted = format!(SimpleFormatContext::default(), [MyFormat])?;
///
/// assert_eq!("1\n\t2\n\t3\n\t4\n", formatted.print()?.as_code());
/// # Ok(())
/// # }
/// ```
///
/// Formatting the same value twice results in a panic.
///
/// ```panics
/// use biome_formatter::prelude::*;
/// use biome_formatter::{SimpleFormatContext, format, write, Buffer};
/// use biome_rowan::TextSize;
///
/// let mut count = 0;
///
/// let value = format_once(|f| {
///     write!(f, [dynamic_token(&std::format!("Formatted {count}."), TextSize::default())])
/// });
///
/// format!(SimpleFormatContext::default(), [value]).expect("Formatting once works fine");
///
/// // Formatting the value more than once panics
/// format!(SimpleFormatContext::default(), [value]);
/// ```
pub const fn format_once<T, Context>(formatter: T) -> FormatOnce<T, Context>
where
    T: FnOnce(&mut Formatter<Context>) -> FormatResult<()>,
{
    FormatOnce {
        formatter: Cell::new(Some(formatter)),
        context: PhantomData,
    }
}

pub struct FormatOnce<T, Context> {
    formatter: Cell<Option<T>>,
    context: PhantomData<Context>,
}

impl<T, Context> Format<Context> for FormatOnce<T, Context>
where
    T: FnOnce(&mut Formatter<Context>) -> FormatResult<()>,
{
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        let formatter = self.formatter.take().expect("Tried to format a `format_once` at least twice. This is not allowed. You may want to use `format_with` or `format.memoized` instead.");

        (formatter)(f)
    }
}

impl<T, Context> std::fmt::Debug for FormatOnce<T, Context> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FormatOnce").field(&"{{formatter}}").finish()
    }
}

/// Builder to join together a sequence of content.
/// See [Formatter::join]
#[must_use = "must eventually call `finish()` on Format builders"]
pub struct JoinBuilder<'fmt, 'buf, Separator, Context> {
    result: FormatResult<()>,
    fmt: &'fmt mut Formatter<'buf, Context>,
    with: Option<Separator>,
    has_elements: bool,
}

impl<'fmt, 'buf, Separator, Context> JoinBuilder<'fmt, 'buf, Separator, Context>
where
    Separator: Format<Context>,
{
    /// Creates a new instance that joins the elements without a separator
    pub(super) fn new(fmt: &'fmt mut Formatter<'buf, Context>) -> Self {
        Self {
            result: Ok(()),
            fmt,
            has_elements: false,
            with: None,
        }
    }

    /// Creates a new instance that prints the passed separator between every two entries.
    pub(super) fn with_separator(fmt: &'fmt mut Formatter<'buf, Context>, with: Separator) -> Self {
        Self {
            result: Ok(()),
            fmt,
            has_elements: false,
            with: Some(with),
        }
    }

    /// Adds a new entry to the join output.
    pub fn entry(&mut self, entry: &dyn Format<Context>) -> &mut Self {
        self.result = self.result.and_then(|_| {
            if let Some(with) = &self.with {
                if self.has_elements {
                    with.fmt(self.fmt)?;
                }
            }
            self.has_elements = true;

            entry.fmt(self.fmt)
        });

        self
    }

    /// Adds the contents of an iterator of entries to the join output.
    pub fn entries<F, I>(&mut self, entries: I) -> &mut Self
    where
        F: Format<Context>,
        I: IntoIterator<Item = F>,
    {
        for entry in entries {
            self.entry(&entry);
        }

        self
    }

    /// Finishes the output and returns any error encountered.
    pub fn finish(&mut self) -> FormatResult<()> {
        self.result
    }
}

/// Builder to join together nodes that ensures that nodes separated by empty lines continue
/// to be separated by empty lines in the formatted output.
#[must_use = "must eventually call `finish()` on Format builders"]
pub struct JoinNodesBuilder<'fmt, 'buf, Separator, Context> {
    result: FormatResult<()>,
    /// The separator to insert between nodes. Either a soft or hard line break
    separator: Separator,
    fmt: &'fmt mut Formatter<'buf, Context>,
    has_elements: bool,
}

impl<'fmt, 'buf, Separator, Context> JoinNodesBuilder<'fmt, 'buf, Separator, Context>
where
    Separator: Format<Context>,
{
    pub(super) fn new(separator: Separator, fmt: &'fmt mut Formatter<'buf, Context>) -> Self {
        Self {
            result: Ok(()),
            separator,
            fmt,
            has_elements: false,
        }
    }

    /// Adds a new node with the specified formatted content to the output, respecting any new lines
    /// that appear before the node in the input source.
    pub fn entry<L: Language>(&mut self, node: &SyntaxNode<L>, content: &dyn Format<Context>) {
        self.result = self.result.and_then(|_| {
            if self.has_elements {
                if get_lines_before(node) > 1 {
                    write!(self.fmt, [empty_line()])?;
                } else {
                    self.separator.fmt(self.fmt)?;
                }
            }

            self.has_elements = true;

            write!(self.fmt, [content])
        });
    }

    /// Writes an entry without adding a separating line break or empty line.
    pub fn entry_no_separator(&mut self, content: &dyn Format<Context>) {
        self.result = self.result.and_then(|_| {
            self.has_elements = true;

            write!(self.fmt, [content])
        })
    }

    /// Adds an iterator of entries to the output. Each entry is a `(node, content)` tuple.
    pub fn entries<L, F, I>(&mut self, entries: I) -> &mut Self
    where
        L: Language,
        F: Format<Context>,
        I: IntoIterator<Item = (SyntaxNode<L>, F)>,
    {
        for (node, content) in entries {
            self.entry(&node, &content)
        }

        self
    }

    pub fn finish(&mut self) -> FormatResult<()> {
        self.result
    }
}

/// Get the number of line breaks between two consecutive SyntaxNodes in the tree
pub fn get_lines_before<L: Language>(next_node: &SyntaxNode<L>) -> usize {
    // Count the newlines in the leading trivia of the next node
    if let Some(token) = next_node.first_token() {
        get_lines_before_token(&token)
    } else {
        0
    }
}

pub fn get_lines_before_token<L: Language>(token: &SyntaxToken<L>) -> usize {
    token
        .leading_trivia()
        .pieces()
        .take_while(|piece| {
            // Stop at the first comment or skipped piece, the comment printer
            // will handle newlines between the comment and the node
            !(piece.is_comments() || piece.is_skipped())
        })
        .filter(|piece| piece.is_newline())
        .count()
}

/// Builder to fill as many elements as possible on a single line.
#[must_use = "must eventually call `finish()` on Format builders"]
pub struct FillBuilder<'fmt, 'buf, Context> {
    result: FormatResult<()>,
    fmt: &'fmt mut Formatter<'buf, Context>,
    empty: bool,
}

impl<'a, 'buf, Context> FillBuilder<'a, 'buf, Context> {
    pub(crate) fn new(fmt: &'a mut Formatter<'buf, Context>) -> Self {
        let result = fmt.write_element(FormatElement::Tag(StartFill));

        Self {
            result,
            fmt,
            empty: true,
        }
    }

    /// Adds an iterator of entries to the fill output. Uses the passed `separator` to separate any two items.
    pub fn entries<F, I>(&mut self, separator: &dyn Format<Context>, entries: I) -> &mut Self
    where
        F: Format<Context>,
        I: IntoIterator<Item = F>,
    {
        for entry in entries {
            self.entry(separator, &entry);
        }

        self
    }

    /// Adds a new entry to the fill output. The `separator` isn't written if this is the first element in the list.
    pub fn entry(
        &mut self,
        separator: &dyn Format<Context>,
        entry: &dyn Format<Context>,
    ) -> &mut Self {
        self.result = self.result.and_then(|_| {
            if self.empty {
                self.empty = false;
            } else {
                self.fmt.write_element(FormatElement::Tag(StartEntry))?;
                separator.fmt(self.fmt)?;
                self.fmt.write_element(FormatElement::Tag(EndEntry))?;
            }

            self.fmt.write_element(FormatElement::Tag(StartEntry))?;
            entry.fmt(self.fmt)?;
            self.fmt.write_element(FormatElement::Tag(EndEntry))
        });

        self
    }

    /// Finishes the output and returns any error encountered
    pub fn finish(&mut self) -> FormatResult<()> {
        self.result
            .and_then(|_| self.fmt.write_element(FormatElement::Tag(EndFill)))
    }
}

/// The first variant is the most flat, and the last is the most expanded variant.
/// See [`best_fitting!`] macro for a more in-detail documentation
#[derive(Copy, Clone)]
pub struct BestFitting<'a, Context> {
    variants: Arguments<'a, Context>,
}

impl<'a, Context> BestFitting<'a, Context> {
    /// Creates a new best fitting IR with the given variants. The method itself isn't unsafe
    /// but it is to discourage people from using it because the printer will panic if
    /// the slice doesn't contain at least the least and most expanded variants.
    ///
    /// You're looking for a way to create a `BestFitting` object, use the `best_fitting![least_expanded, most_expanded]` macro.
    ///
    /// This is intended to be only used in the `best_fitting!` macro. As we can't place tail
    /// expressions in a block for temporary lifetime extension since Rust 2024, we can't use an
    /// `unsafe` block in the macro. Thus, this function can't be marked as unsafe, but it shouldn't
    /// be used from outside.
    ///
    /// ## Safety
    /// The slice must contain at least two variants.
    #[doc(hidden)]
    pub fn from_arguments_unchecked(variants: Arguments<'a, Context>) -> Self {
        assert!(
            variants.0.len() >= 2,
            "Requires at least the least expanded and most expanded variants"
        );

        Self { variants }
    }
}

impl<Context> Format<Context> for BestFitting<'_, Context> {
    fn fmt(&self, f: &mut Formatter<Context>) -> FormatResult<()> {
        let mut buffer = VecBuffer::new(f.state_mut());
        let variants = self.variants.items();

        let mut formatted_variants = Vec::with_capacity(variants.len());

        for variant in variants {
            buffer.write_element(FormatElement::Tag(StartEntry))?;
            buffer.write_fmt(Arguments::from(variant))?;
            buffer.write_element(FormatElement::Tag(EndEntry))?;

            formatted_variants.push(buffer.take_vec().into_boxed_slice());
        }

        // SAFETY: The constructor guarantees that there are always at least two variants. It's, therefore,
        // safe to call into the unsafe `from_vec_unchecked` function
        let element = unsafe {
            FormatElement::BestFitting(format_element::BestFittingElement::from_vec_unchecked(
                formatted_variants,
            ))
        };

        f.write_element(element)
    }
}
