use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{
    range_between_end_and_start, rule, tree_sitter::Node, violation, NodeExt, Rule,
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Options {
    #[default]
    Interface,
    Type,
}

pub fn consistent_type_definitions_rule() -> Arc<dyn Rule> {
    rule! {
        name => "consistent-type-definitions",
        languages => [Typescript],
        messages => [
            interface_over_type => "Use an `interface` instead of a `type`.",
            type_over_interface => "Use a `type` instead of an `interface`.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            option: Options = options,
        },
        listeners => [
            r#"
              (type_alias_declaration
                value: (object_type)
              ) @c
            "# => |node, context| {
                if self.option != Options::Interface {
                    return;
                }

                context.report(violation! {
                    node => node.field("name"),
                    message_id => "interface_over_type",
                    fix => |fixer| {
                        let type_node = node.child_by_field_name("type_parameters").unwrap_or_else(|| node.field("name"));

                        let first_token = context.maybe_get_token_before(node.field("name"), Option::<fn(Node) -> bool>::None);
                        if let Some(first_token) = first_token {
                            fixer.replace_text(first_token, "interface");
                            fixer.replace_text_range(
                                range_between_end_and_start(
                                    type_node.range(),
                                    node.field("value").range()
                                ),
                                " "
                            );
                        }

                        let after_token = context.maybe_get_token_after(node.field("value"), Option::<fn(Node) -> bool>::None);
                        if let Some(after_token) = after_token.filter(|after_token| {
                            after_token.kind() == ";"
                        }) {
                            fixer.remove(after_token);
                        }
                    }
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
    fn test_consistent_type_definitions_rule() {
        RuleTester::run(
            consistent_type_definitions_rule(),
            rule_tests! {
                valid => [
                  {
                    code => "var foo = {};",
                    options => "interface",
                  },
                  {
                    code => "interface A {}",
                    options => "interface",
                  },
                  {
                    code => r#"
              interface A extends B {
                x: number;
              }
                    "#,
                    options => "interface",
                  },
                  {
                    code => "type U = string;",
                    options => "interface",
                  },
                  {
                    code => "type V = { x: number } | { y: string };",
                    options => "interface",
                  },
                  {
                    code => r#"
              type Record<T, U> = {
                [K in T]: U;
              };
                    "#,
                    options => "interface",
                  },
                  {
                    code => "type T = { x: number };",
                    options => "type",
                  },
                  {
                    code => "type A = { x: number } & B & C;",
                    options => "type",
                  },
                  {
                    code => "type A = { x: number } & B<T1> & C<T2>;",
                    options => "type",
                  },
                  {
                    code => r#"
              export type W<T> = {
                x: T;
              };
                    "#,
                    options => "type",
                  },
                ],
                invalid => [
                  {
                    code => r#"type T = { x: number; };"#,
                    output => r#"interface T { x: number; }"#,
                    options => "interface",
                    errors => [
                      {
                        message_id => "interface_over_type",
                        line => 1,
                        column => 6,
                      },
                    ],
                  },
                  {
                    code => r#"type T={ x: number; };"#,
                    output => r#"interface T { x: number; }"#,
                    options => "interface",
                    errors => [
                      {
                        message_id => "interface_over_type",
                        line => 1,
                        column => 6,
                      },
                    ],
                  },
                  {
                    code => r#"type T=                         { x: number; };"#,
                    output => r#"interface T { x: number; }"#,
                    options => "interface",
                    errors => [
                      {
                        message_id => "interface_over_type",
                        line => 1,
                        column => 6,
                      },
                    ],
                  },
                  {
                    code => r#"
export type W<T> = {
  x: T;
};
                    "#,
                    output => r#"
export interface W<T> {
  x: T;
}
                    "#,
                    options => "interface",
                    errors => [
                      {
                        message_id => "interface_over_type",
                        line => 2,
                        column => 13,
                      },
                    ],
                  },
                  {
                    code => r#"interface T { x: number; }"#,
                    output => r#"type T = { x: number; }"#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 1,
                        column => 11,
                      },
                    ],
                  },
                  {
                    code => r#"interface T{ x: number; }"#,
                    output => r#"type T = { x: number; }"#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 1,
                        column => 11,
                      },
                    ],
                  },
                  {
                    code => r#"interface T                          { x: number; }"#,
                    output => r#"type T = { x: number; }"#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 1,
                        column => 11,
                      },
                    ],
                  },
                  {
                    code => r#"interface A extends B, C { x: number; };"#,
                    output => r#"type A = { x: number; } & B & C;"#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 1,
                        column => 11,
                      },
                    ],
                  },
                  {
                    code => r#"interface A extends B<T1>, C<T2> { x: number; };"#,
                    output => r#"type A = { x: number; } & B<T1> & C<T2>;"#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 1,
                        column => 11,
                      },
                    ],
                  },
                  {
                    code => r#"
export interface W<T> {
  x: T;
}
                    "#,
                    output => r#"
export type W<T> = {
  x: T;
}
                    "#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 2,
                        column => 18,
                      },
                    ],
                  },
                  {
                    code => r#"
namespace JSX {
  interface Array<T> {
    foo(x: (x: number) => T): T[];
  }
}
                    "#,
                    output => r#"
namespace JSX {
  type Array<T> = {
    foo(x: (x: number) => T): T[];
  }
}
                    "#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 3,
                        column => 13,
                      },
                    ],
                  },
                  {
                    code => r#"
global {
  interface Array<T> {
    foo(x: (x: number) => T): T[];
  }
}
                    "#,
                    output => r#"
global {
  type Array<T> = {
    foo(x: (x: number) => T): T[];
  }
}
                    "#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 3,
                        column => 13,
                      },
                    ],
                  },
                  {
                    code => r#"
declare global {
  interface Array<T> {
    foo(x: (x: number) => T): T[];
  }
}
                    "#,
                    output => None,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 3,
                        column => 13,
                      },
                    ],
                  },
                  {
                    code => r#"
declare global {
  namespace Foo {
    interface Bar {}
  }
}
                    "#,
                    output => None,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 4,
                        column => 15,
                      },
                    ],
                  },
                  {
                    // https://github.com/typescript-eslint/typescript-eslint/issues/3894
                    code => r#"
export default interface Test {
  bar(): string;
  foo(): number;
}
                    "#,
                    output => r#"
type Test = {
  bar(): string;
  foo(): number;
}
export default Test
                    "#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 2,
                        column => 26,
                      },
                    ],
                  },
                  {
                    // https://github.com/typescript-eslint/typescript-eslint/issues/4333
                    code => r#"
export declare type Test = {
  foo: string;
  bar: string;
};
                    "#,
                    output => r#"
export declare interface Test {
  foo: string;
  bar: string;
}
                    "#,
                    options => "interface",
                    errors => [
                      {
                        message_id => "interface_over_type",
                        line => 2,
                        column => 21,
                      },
                    ],
                  },
                  {
                    // https://github.com/typescript-eslint/typescript-eslint/issues/4333
                    code => r#"
export declare interface Test {
  foo: string;
  bar: string;
}
                    "#,
                    output => r#"
export declare type Test = {
  foo: string;
  bar: string;
}
                    "#,
                    options => "type",
                    errors => [
                      {
                        message_id => "type_over_interface",
                        line => 2,
                        column => 26,
                      },
                    ],
                  },
                ],
            },
        )
    }
}
