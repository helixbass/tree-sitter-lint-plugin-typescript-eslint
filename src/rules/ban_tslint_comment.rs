use std::sync::Arc;

use squalid::regex;
use tree_sitter_lint::{
    rule,
    tree_sitter::{Point, Range},
    violation, Rule,
};
use tree_sitter_lint_plugin_eslint_builtin::{
    ast_helpers::{get_comment_contents, get_comment_type, CommentType},
    AllComments,
};

fn to_text(text: &str, type_: CommentType) -> String {
    match type_ {
        CommentType::Line => ["//", text.trim()].join(" "),
        CommentType::Block => ["/*", text.trim(), "*/"].join(" "),
    }
}

pub fn ban_tslint_comment_rule() -> Arc<dyn Rule> {
    rule! {
        name => "ban-tslint-comment",
        languages => [Typescript],
        messages => [
            comment_detected => "tslint comment detected: \"{{ text }}\"",
        ],
        fixable => true,
        listeners => [
            r#"
              (program) @c
            "# => |node, context| {
                for &c in context.retrieve::<AllComments<'a>>().iter() {
                    let comment_contents = get_comment_contents(c, context);
                    if regex!(r#"^\s*tslint:(enable|disable)(?:-(line|next-line))?(:|\s|$)"#).is_match(&comment_contents) {
                        context.report(violation! {
                            data => {
                                text => to_text(&comment_contents, get_comment_type(c, context)),
                            },
                            node => c,
                            message_id => "comment_detected",
                            fix => |fixer| {
                                let should_remove_byte_before_comment_start = c.start_position().column > 0;
                                let should_remove_byte_after_comment_end = c.end_byte() < context.file_run_context.tree.root_node().end_byte();
                                fixer.remove_range(Range {
                                    start_byte: if should_remove_byte_before_comment_start {
                                        c.start_byte() - 1
                                    } else {
                                        c.start_byte()
                                    },
                                    end_byte: if should_remove_byte_after_comment_end {
                                        c.end_byte() + 1
                                    } else {
                                        c.end_byte()
                                    },
                                    start_point: Point {
                                        row: c.start_position().row,
                                        column: if should_remove_byte_before_comment_start {
                                            c.start_position().column - 1
                                        } else {
                                            c.start_position().column
                                        },
                                    },
                                    end_point: Point {
                                        row: c.end_position().row,
                                        column: if should_remove_byte_after_comment_end {
                                            c.end_position().column + 1
                                        } else {
                                            c.end_position().column
                                        },
                                    },
                                });
                            }
                        });
                    }
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::get_instance_provider_factory;

    #[test]
    fn test_ban_tslint_comment_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            ban_tslint_comment_rule(),
            rule_tests! {
                valid => [
                    {
                      code => "let a: readonly any[] = [];",
                    },
                    {
                      code => "let a = new Array();",
                    },
                    {
                      code => "// some other comment",
                    },
                    {
                      code => "// TODO: this is a comment that mentions tslint",
                    },
                    {
                      code => "/* another comment that mentions tslint */",
                    },
                ],
                invalid => [
                  {
                      code => "/* tslint:disable */",
                      output => "",
                      errors => [
                        {
                          column => 1,
                          line => 1,
                          data => { text => "/* tslint:disable */" },
                          message_id => "comment_detected",
                        },
                      ],
                  }, // Disable all rules for the rest of the file
                  {
                      code => "/* tslint:enable */",
                      output => "",
                      errors => [
                        {
                          column => 1,
                          line => 1,
                          data => { text => "/* tslint:enable */" },
                          message_id => "comment_detected",
                        },
                      ],
                  }, // Enable all rules for the rest of the file
                  {
                    code => "/* tslint:disable:rule1 rule2 rule3... */",
                    output => "",
                      errors => [
                        {
                          column => 1,
                          line => 1,
                          data => { text => "/* tslint:disable:rule1 rule2 rule3... */" },
                          message_id => "comment_detected",
                        },
                      ],
                  }, // Disable the listed rules for the rest of the file
                  {
                    code => "/* tslint:enable:rule1 rule2 rule3... */",
                    output => "",
                      errors => [
                        {
                          column => 1,
                          line => 1,
                          data => { text => "/* tslint:enable:rule1 rule2 rule3... */" },
                          message_id => "comment_detected",
                        },
                      ],
                  }, // Enable the listed rules for the rest of the file
                  {
                      code => "// tslint:disable-next-line",
                     output => "",
                      errors => [
                        {
                          column => 1,
                          line => 1,
                          data => { text => "// tslint:disable-next-line" },
                          message_id => "comment_detected",
                        },
                      ],
                  }, // Disables all rules for the following line
                  {
                    code => "someCode(); // tslint:disable-line",
                    output => "someCode();",
                      errors => [
                        {
                          column => 13,
                          line => 1,
                          data => { text => "// tslint:disable-line" },
                          message_id => "comment_detected",
                        },
                      ],
                  }, // Disables all rules for the current line
                  {
                    code => "// tslint:disable-next-line =>rule1 rule2 rule3...",
                   output => "",
                      errors => [
                        {
                          column => 1,
                          line => 1,
                          data => { text => "// tslint:disable-next-line =>rule1 rule2 rule3..." },
                          message_id => "comment_detected",
                        },
                      ],
                  }, // Disables the listed rules for the next line

                  {
                    code => r#"const woah = doSomeStuff();
// tslint:disable-line
console.log(woah);
                "#,
                    output => r#"const woah = doSomeStuff();
console.log(woah);
                "#,
                      errors => [
                        {
                          column => 1,
                          line => 2,
                          data => { text => "// tslint:disable-line" },
                          message_id => "comment_detected",
                        },
                      ],
                  },
                ],
            },
            get_instance_provider_factory(),
        )
    }
}
