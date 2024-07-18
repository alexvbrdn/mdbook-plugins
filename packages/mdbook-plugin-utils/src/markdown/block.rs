use std::ops::Range;

use anyhow::{bail, Result};
use log::debug;
use pulldown_cmark::{Event, Parser};

#[derive(Clone, Debug, PartialEq)]
pub struct Block<'a> {
    pub closed: bool,
    pub events: Vec<Event<'a>>,
    pub span: Range<usize>,
    pub inner_span: Range<usize>,
}

impl<'a> Block<'a> {
    pub fn new(first_event: Event<'a>, span: Range<usize>) -> Self {
        let inner_span = 0..0;
        Block {
            closed: false,
            events: vec![first_event],
            span,
            inner_span,
        }
    }
}

pub fn parse_blocks<IsStartFn, IsEndFn>(
    content: &str,
    is_start: IsStartFn,
    is_end: IsEndFn,
) -> Result<Vec<Block>>
where
    IsStartFn: Fn(&Event) -> bool,
    IsEndFn: Fn(&Event) -> bool,
{
    let mut blocks: Vec<Block> = vec![];

    for (event, span) in Parser::new(content).into_offset_iter() {
        debug!("{:?} {:?}", event, span);

        if is_start(&event) {
            if let Some(block) = blocks.last_mut() {
                if !block.closed {
                    bail!("Block is not closed. Nested blocks are not supported.");
                }
            }

            blocks.push(Block::new(event, span));
        } else if is_end(&event) {
            if let Some(block) = blocks.last_mut() {
                if !block.closed {
                    block.events.push(event);
                    block.closed = true;

                    if span.end > block.span.end {
                        block.span = block.span.start..span.end;
                    }
                }
            }
        } else if let Some(block) = blocks.last_mut() {
            if !block.closed {
                block.events.push(event);

                if span.end > block.span.end {
                    block.span = block.span.start..span.end;
                }

                block.inner_span = match block.inner_span == (0..0) {
                    true => span,
                    false => block.inner_span.start..span.end,
                };
            }
        }
    }

    Ok(blocks)
}

#[cfg(test)]
mod test {
    use pulldown_cmark::{CodeBlockKind, CowStr, Tag, TagEnd};
    use test_log::test;

    use super::*;

    #[test]
    fn test_parse_blocks() -> Result<()> {
        let content = "\
        ```toml\n\
        key1 = \"value1\"\n\
        key2 = \"value2\"\n\
        ```";
        let expected: Vec<Block> = vec![Block {
            closed: true,
            events: vec![
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(CowStr::from("toml")))),
                Event::Text(CowStr::from("key1 = \"value1\"\nkey2 = \"value2\"\n")),
                Event::End(TagEnd::CodeBlock),
            ],
            span: 0..43,
            inner_span: 8..40,
        }];

        let actual = parse_blocks(
            content,
            |event| matches!(event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(tag))) if tag == &CowStr::from("toml")),
            |event| matches!(event, Event::End(TagEnd::CodeBlock)),
        )?;

        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn test_parse_blocks_surrounded() -> Result<()> {
        let content = "\
        Some text before the code block.\n\
        \n\
        ```toml\n\
        key1 = \"value1\"\n\
        key2 = \"value2\"\n\
        ```\n\
        \n\
        Some text after the code block.";
        let expected: Vec<Block> = vec![Block {
            closed: true,
            events: vec![
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(CowStr::from("toml")))),
                Event::Text(CowStr::from("key1 = \"value1\"\nkey2 = \"value2\"\n")),
                Event::End(TagEnd::CodeBlock),
            ],
            span: 34..77,
            inner_span: 42..74,
        }];

        let actual = parse_blocks(
            content,
            |event| matches!(event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(tag))) if tag == &CowStr::from("toml")),
            |event| matches!(event, Event::End(TagEnd::CodeBlock)),
        )?;

        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn test_parse_blocks_multiple() -> Result<()> {
        let content = "\
        First TOML block:\n\
        ```toml\n\
        key1 = \"value1\"\n\
        key2 = \"value2\"\n\
        ```\n\
        First non-TOML block:\n\
        ```shell\n\
        echo test\n\
        ```\n\
        Second TOML block:\n\
        ```toml\n\
        key3 = \"value3\"\n\
        key4 = \"value4\"\n\
        ```";
        let expected: Vec<Block> = vec![
            Block {
                closed: true,
                events: vec![
                    Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(CowStr::from("toml")))),
                    Event::Text(CowStr::from("key1 = \"value1\"\nkey2 = \"value2\"\n")),
                    Event::End(TagEnd::CodeBlock),
                ],
                span: 18..61,
                inner_span: 26..58,
            },
            Block {
                closed: true,
                events: vec![
                    Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(CowStr::from("toml")))),
                    Event::Text(CowStr::from("key3 = \"value3\"\nkey4 = \"value4\"\n")),
                    Event::End(TagEnd::CodeBlock),
                ],
                span: 126..169,
                inner_span: 134..166,
            },
        ];

        let actual = parse_blocks(
            content,
            |event| matches!(event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(tag))) if tag == &CowStr::from("toml")),
            |event| matches!(event, Event::End(TagEnd::CodeBlock)),
        )?;

        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn test_parse_blocks_nested() -> Result<()> {
        let content = "*a **sentence** with **some** words*";

        let actual = parse_blocks(
            content,
            |event| {
                matches!(
                    event,
                    Event::Start(Tag::Emphasis) | Event::Start(Tag::Strong)
                )
            },
            |event| {
                matches!(
                    event,
                    Event::End(TagEnd::Emphasis) | Event::End(TagEnd::Strong)
                )
            },
        );

        assert_eq!(
            "Block is not closed. Nested blocks are not supported.",
            format!("{}", actual.unwrap_err().root_cause())
        );

        Ok(())
    }

    #[test]
    fn test_parse_blocks_text() -> Result<()> {
        let content = "\
        {{#tabs }}\n\
        Some content.\n\
        {{#endtabs }}\n\
        ";
        let expected: Vec<Block> = vec![Block {
            closed: true,
            events: vec![
                Event::Text(CowStr::from("{{#tabs }}")),
                Event::SoftBreak,
                Event::Text(CowStr::from("Some content.")),
                Event::SoftBreak,
                Event::Text(CowStr::from("{{#endtabs }}")),
            ],
            span: 0..38,
            inner_span: 10..25,
        }];

        let actual = parse_blocks(
            content,
            |event| matches!(event, Event::Text(text) if text.starts_with("{{#tabs ")),
            |event| matches!(event, Event::Text(text) if text.starts_with("{{#endtabs ")),
        )?;

        assert_eq!(expected, actual);

        Ok(())
    }
}
