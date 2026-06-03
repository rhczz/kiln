use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write};

use comrak::adapters::{HeadingAdapter, HeadingMeta};
use comrak::arena_tree::Node;
use comrak::nodes::{Ast, AstNode, LineColumn, NodeHtmlBlock, NodeValue, Sourcepos};
use comrak::{
    format_html_with_plugins, parse_document, Arena, ComrakOptions, ComrakPlugins,
    ComrakRenderPlugins,
};

pub fn markdown_to_html(md: &str) -> String {
    let options = markdown_options();
    let arena = Arena::new();
    let root = parse_document(&arena, md, &options);
    transform_ast(&arena, root);

    let heading_adapter = AnchorHeadingAdapter::default();
    let mut render_plugins = ComrakRenderPlugins::default();
    render_plugins.heading_adapter = Some(&heading_adapter);

    let mut plugins = ComrakPlugins::default();
    plugins.render = render_plugins;

    let mut html = Vec::new();
    format_html_with_plugins(root, &options, &mut html, &plugins)
        .expect("rendering markdown HTML should not fail");
    String::from_utf8(html).expect("comrak should render valid UTF-8")
}

fn markdown_options() -> ComrakOptions {
    let mut options = ComrakOptions::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.superscript = true;
    options.extension.footnotes = true;
    options.extension.description_lists = true;
    options.render.unsafe_ = true;
    options
}

fn transform_ast<'a>(arena: &'a Arena<AstNode<'a>>, root: &'a AstNode<'a>) {
    let nodes: Vec<&AstNode<'_>> = root.descendants().collect();
    for node in nodes {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::Table(_) => wrap_table(arena, node),
            NodeValue::TaskItem(symbol) => rewrite_task_item(arena, node, symbol),
            _ => {}
        }
    }
}

fn wrap_table<'a>(arena: &'a Arena<AstNode<'a>>, table: &'a AstNode<'a>) {
    table.insert_before(html_block(arena, r#"<div class="table-scroll">"#));
    table.insert_after(html_block(arena, "</div>"));
}

fn rewrite_task_item<'a>(
    arena: &'a Arena<AstNode<'a>>,
    item: &'a AstNode<'a>,
    symbol: Option<char>,
) {
    let list_meta = item
        .parent()
        .and_then(|parent| match parent.data.borrow().value {
            NodeValue::List(meta) => Some(meta),
            _ => None,
        })
        .unwrap_or_default();

    item.data.borrow_mut().value = NodeValue::Item(list_meta);

    if let Some(first_child) = item.first_child() {
        if matches!(first_child.data.borrow().value, NodeValue::Paragraph) {
            first_child.prepend(html_inline(arena, &task_checkbox(symbol)));
            return;
        }
    }

    let paragraph = ast_node(arena, NodeValue::Paragraph);
    paragraph.append(html_inline(arena, &task_checkbox(symbol)));
    item.prepend(paragraph);
}

fn task_checkbox(symbol: Option<char>) -> String {
    if symbol.is_some() {
        r#"<input class="task-list-item-checkbox" type="checkbox" disabled checked> "#.to_string()
    } else {
        r#"<input class="task-list-item-checkbox" type="checkbox" disabled> "#.to_string()
    }
}

fn html_block<'a>(arena: &'a Arena<AstNode<'a>>, literal: &str) -> &'a AstNode<'a> {
    ast_node(
        arena,
        NodeValue::HtmlBlock(NodeHtmlBlock {
            block_type: 6,
            literal: literal.to_string(),
        }),
    )
}

fn html_inline<'a>(arena: &'a Arena<AstNode<'a>>, literal: &str) -> &'a AstNode<'a> {
    ast_node(arena, NodeValue::HtmlInline(literal.to_string()))
}

fn ast_node<'a>(arena: &'a Arena<AstNode<'a>>, value: NodeValue) -> &'a AstNode<'a> {
    arena.alloc(Node::new(RefCell::new(Ast::new(
        value,
        LineColumn { line: 0, column: 0 },
    ))))
}

#[derive(Default)]
struct AnchorHeadingAdapter {
    used: std::sync::Mutex<HashMap<String, usize>>,
}

impl HeadingAdapter for AnchorHeadingAdapter {
    fn enter(
        &self,
        output: &mut dyn Write,
        heading: &HeadingMeta,
        _sourcepos: Option<Sourcepos>,
    ) -> io::Result<()> {
        let id = self.unique_slug(&heading.content);
        write!(output, r#"<h{} id="{}" tabindex="-1">"#, heading.level, id)
    }

    fn exit(&self, output: &mut dyn Write, heading: &HeadingMeta) -> io::Result<()> {
        let id = self.current_slug(&heading.content);
        writeln!(
            output,
            r##" <a class="heading-anchor" href="#{}" aria-hidden="true">#</a></h{}>"##,
            id, heading.level
        )
    }
}

impl AnchorHeadingAdapter {
    fn unique_slug(&self, content: &str) -> String {
        let base = slugify_heading(content);
        let mut used = self.used.lock().expect("heading slug lock poisoned");
        let count = used.entry(base.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            base
        } else {
            format!("{}-{}", base, count)
        }
    }

    fn current_slug(&self, content: &str) -> String {
        let base = slugify_heading(content);
        let used = self.used.lock().expect("heading slug lock poisoned");
        match used.get(&base).copied().unwrap_or(1) {
            0 | 1 => base,
            count => format!("{}-{}", base, count),
        }
    }
}

fn slugify_heading(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_dash = false;

    for ch in value.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            slug.push(ch);
            previous_was_dash = false;
        } else if !previous_was_dash && !slug.is_empty() {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::markdown_to_html;

    #[test]
    fn renders_heading_anchor_from_ast() {
        let html = markdown_to_html("## Service 层从哪来");
        assert!(html.contains(
            r##"<h2 id="service-层从哪来" tabindex="-1">Service 层从哪来 <a class="heading-anchor" href="#service-层从哪来" aria-hidden="true">#</a></h2>"##
        ));
        assert!(!html.contains(r#"class="anchor""#));
    }

    #[test]
    fn wraps_tables_without_touching_code_blocks() {
        let html =
            markdown_to_html("| A | B |\n|---|---|\n| 1 | 2 |\n\n```html\n<table></table>\n```");
        assert!(html.contains(r#"<div class="table-scroll">"#));
        assert!(html.contains("<table>"));
        assert!(html.contains("&lt;table&gt;&lt;/table&gt;"));
    }

    #[test]
    fn renders_task_checkbox_class() {
        let html = markdown_to_html("- [x] done\n- [ ] todo");
        assert!(html.contains(
            r#"<input class="task-list-item-checkbox" type="checkbox" disabled checked> done"#
        ));
        assert!(html
            .contains(r#"<input class="task-list-item-checkbox" type="checkbox" disabled> todo"#));
    }
}
