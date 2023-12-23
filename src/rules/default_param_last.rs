use std::sync::Arc;

use itertools::Itertools;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt, Rule,
};
use tree_sitter_lint_plugin_eslint_builtin::kind::RestPattern;

use crate::kind::OptionalParameter;

fn is_plain_param(node: Node) -> bool {
    !(node.kind() == OptionalParameter
        || node.child_by_field_name("value").is_some()
        || node.field("pattern").kind() == RestPattern)
}

pub fn default_param_last_rule() -> Arc<dyn Rule> {
    rule! {
        name => "default-param-last",
        languages => [Typescript],
        messages => [
            should_be_last => "Default parameters should be last.",
        ],
        listeners => [
            r#"
              (function_declaration) @c
              (function) @c
              (generator_function_declaration) @c
              (generator_function) @c
              (method_definition) @c
              (arrow_function) @c
            "# => |node, context| {
                let mut has_seen_plain_param = false;

                for param in node.field("parameters").non_comment_named_children(SupportedLanguage::Javascript).collect_vec().into_iter().rev() {
                    if is_plain_param(param) {
                        has_seen_plain_param = true;
                        continue;
                    }

                    if has_seen_plain_param && (
                        param.kind() == OptionalParameter ||
                        param.child_by_field_name("value").is_some()
                    ) {
                        context.report(violation! {
                            node => param,
                            message_id => "should_be_last",
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

    #[test]
    fn test_default_param_last_rule() {
        RuleTester::run(
            default_param_last_rule(),
            rule_tests! {
                valid => [
                  "function foo() {}",
                  "function foo(a: number) {}",
                  "function foo(a = 1) {}",
                  "function foo(a?: number) {}",
                  "function foo(a: number, b: number) {}",
                  "function foo(a: number, b: number, c?: number) {}",
                  "function foo(a: number, b = 1) {}",
                  "function foo(a: number, b = 1, c = 1) {}",
                  "function foo(a: number, b = 1, c?: number) {}",
                  "function foo(a: number, b?: number, c = 1) {}",
                  "function foo(a: number, b = 1, ...c) {}",

                  "const foo = function () {};",
                  "const foo = function (a: number) {};",
                  "const foo = function (a = 1) {};",
                  "const foo = function (a?: number) {};",
                  "const foo = function (a: number, b: number) {};",
                  "const foo = function (a: number, b: number, c?: number) {};",
                  "const foo = function (a: number, b = 1) {};",
                  "const foo = function (a: number, b = 1, c = 1) {};",
                  "const foo = function (a: number, b = 1, c?: number) {};",
                  "const foo = function (a: number, b?: number, c = 1) {};",
                  "const foo = function (a: number, b = 1, ...c) {};",

                  "const foo = () => {};",
                  "const foo = (a: number) => {};",
                  "const foo = (a = 1) => {};",
                  "const foo = (a?: number) => {};",
                  "const foo = (a: number, b: number) => {};",
                  "const foo = (a: number, b: number, c?: number) => {};",
                  "const foo = (a: number, b = 1) => {};",
                  "const foo = (a: number, b = 1, c = 1) => {};",
                  "const foo = (a: number, b = 1, c?: number) => {};",
                  "const foo = (a: number, b?: number, c = 1) => {};",
                  "const foo = (a: number, b = 1, ...c) => {};",
                  r#"
              class Foo {
                constructor(a: number, b: number, c: number) {}
              }
                  "#,
                  r#"
              class Foo {
                constructor(a: number, b?: number, c = 1) {}
              }
                  "#,
                  r#"
              class Foo {
                constructor(a: number, b = 1, c?: number) {}
              }
                  "#,
                  r#"
              class Foo {
                constructor(
                  public a: number,
                  protected b: number,
                  private c: number,
                ) {}
              }
                  "#,
                  r#"
              class Foo {
                constructor(
                  public a: number,
                  protected b?: number,
                  private c = 10,
                ) {}
              }
                  "#,
                  r#"
              class Foo {
                constructor(
                  public a: number,
                  protected b = 10,
                  private c?: number,
                ) {}
              }
                  "#,
                  r#"
              class Foo {
                constructor(
                  a: number,
                  protected b?: number,
                  private c = 0,
                ) {}
              }
                  "#,
                  r#"
              class Foo {
                constructor(
                  a: number,
                  b?: number,
                  private c = 0,
                ) {}
              }
                  "#,
                  r#"
              class Foo {
                constructor(
                  a: number,
                  private b?: number,
                  c = 0,
                ) {}
              }
                  "#,
                ],
                invalid => [
                  {
                    code => "function foo(a = 1, b: number) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                    ],
                  },
                  {
                    code => "function foo(a = 1, b = 2, c: number) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 21,
                        end_column => 26,
                      },
                    ],
                  },
                  {
                    code => "function foo(a = 1, b: number, c = 2, d: number) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 32,
                        end_column => 37,
                      },
                    ],
                  },
                  {
                    code => "function foo(a = 1, b: number, c = 2) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                    ],
                  },
                  {
                    code => "function foo(a = 1, b: number, ...c) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                    ],
                  },
                  {
                    code => "function foo(a?: number, b: number) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 24,
                      },
                    ],
                  },
                  {
                    code => "function foo(a: number, b?: number, c: number) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 25,
                        end_column => 35,
                      },
                    ],
                  },
                  {
                    code => "function foo(a = 1, b?: number, c: number) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 21,
                        end_column => 31,
                      },
                    ],
                  },
                  {
                    code => "function foo(a = 1, { b }) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                    ],
                  },
                  {
                    code => "function foo({ a } = {}, b) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 24,
                      },
                    ],
                  },
                  {
                    code => "function foo({ a, b } = { a: 1, b: 2 }, c) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 39,
                      },
                    ],
                  },
                  {
                    code => "function foo([a] = [], b) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 22,
                      },
                    ],
                  },
                  {
                    code => "function foo([a, b] = [1, 2], c) {}",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 29,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a = 1, b: number) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 28,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a = 1, b = 2, c: number) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 28,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 30,
                        end_column => 35,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a = 1, b: number, c = 2, d: number) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 28,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 41,
                        end_column => 46,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a = 1, b: number, c = 2) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 28,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a = 1, b: number, ...c) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 28,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a?: number, b: number) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 33,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a: number, b?: number, c: number) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 34,
                        end_column => 44,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a = 1, b?: number, c: number) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 28,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 30,
                        end_column => 40,
                      },
                    ],
                  },
                  {
                    code => "const foo = function (a = 1, { b }) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 28,
                      },
                    ],
                  },
                  {
                    code => "const foo = function ({ a } = {}, b) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 33,
                      },
                    ],
                  },
                  {
                    code => "const foo = function ({ a, b } = { a: 1, b: 2 }, c) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 48,
                      },
                    ],
                  },
                  {
                    code => "const foo = function ([a] = [], b) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 31,
                      },
                    ],
                  },
                  {
                    code => "const foo = function ([a, b] = [1, 2], c) {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 23,
                        end_column => 38,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a = 1, b: number) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a = 1, b = 2, c: number) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 21,
                        end_column => 26,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a = 1, b: number, c = 2, d: number) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 32,
                        end_column => 37,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a = 1, b: number, c = 2) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a = 1, b: number, ...c) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a?: number, b: number) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 24,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a: number, b?: number, c: number) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 25,
                        end_column => 35,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a = 1, b?: number, c: number) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 21,
                        end_column => 31,
                      },
                    ],
                  },
                  {
                    code => "const foo = (a = 1, { b }) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 19,
                      },
                    ],
                  },
                  {
                    code => "const foo = ({ a } = {}, b) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 24,
                      },
                    ],
                  },
                  {
                    code => "const foo = ({ a, b } = { a: 1, b: 2 }, c) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 39,
                      },
                    ],
                  },
                  {
                    code => "const foo = ([a] = [], b) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 22,
                      },
                    ],
                  },
                  {
                    code => "const foo = ([a, b] = [1, 2], c) => {};",
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 1,
                        column => 14,
                        end_column => 29,
                      },
                    ],
                  },
                  {
                    code => r#"
class Foo {
  constructor(
    public a: number,
    protected b?: number,
    private c: number,
  ) {}
}
                    "#,
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 5,
                        column => 5,
                        end_column => 25,
                      },
                    ],
                  },
                  {
                    code => r#"
class Foo {
  constructor(
    public a: number,
    protected b = 0,
    private c: number,
  ) {}
}
                    "#,
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 5,
                        column => 5,
                        end_column => 20,
                      },
                    ],
                  },
                  {
                    code => r#"
class Foo {
  constructor(
    public a?: number,
    private b: number,
  ) {}
}
                    "#,
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 4,
                        column => 5,
                        end_column => 22,
                      },
                    ],
                  },
                  {
                    code => r#"
class Foo {
  constructor(
    public a = 0,
    private b: number,
  ) {}
}
                    "#,
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 4,
                        column => 5,
                        end_column => 17,
                      },
                    ],
                  },
                  {
                    code => r#"
class Foo {
  constructor(a = 0, b: number) {}
}
                    "#,
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 3,
                        column => 15,
                        end_column => 20,
                      },
                    ],
                  },
                  {
                    code => r#"
class Foo {
  constructor(a?: number, b: number) {}
}
                    "#,
                    errors => [
                      {
                        message_id => "should_be_last",
                        line => 3,
                        column => 15,
                        end_column => 25,
                      },
                    ],
                  },
                ],
            }
        );
    }
}
