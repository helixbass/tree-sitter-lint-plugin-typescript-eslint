use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn ban_tslint_comment_rule() -> Arc<dyn Rule> {
    rule! {
        name => "ban-tslint-comment",
        languages => [Typescript],
        messages => [
            comment_detected => "tslint comment detected: \"{{ text }}\"",
        ],
        fixable => true,
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_ban_tslint_comment_rule() {
        RuleTester::run(
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
        )
    }
}
