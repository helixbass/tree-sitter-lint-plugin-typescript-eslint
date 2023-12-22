use std::sync::Arc;

use itertools::Itertools;
use serde::Deserialize;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule,
};

use crate::{
    ast_helpers::get_is_type_literal,
    kind::{TypeAliasDeclaration, TypeAnnotation},
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Options {
    #[default]
    Record,
    IndexSignature,
}

fn find_parent_declaration(node: Node) -> Option<Node> {
    let node_parent = node
        .parent()
        .filter(|parent| parent.kind() != TypeAnnotation)?;
    if node_parent.kind() == TypeAliasDeclaration {
        return Some(node_parent);
    }
    find_parent_declaration(node_parent)
}

fn check_members<'a>(
    mut members: impl Iterator<Item = Node<'a>>,
    node: Node,
    parent_id: Option<Node>,
    prefix: &str,
    postfix: &str,
    safe_fix: Option<bool>,
    context: &QueryMatchContext,
) {
    let Some(member) = members.next() else {
        return;
    };
    if members.next().is_some() {
        return;
    }

    let key_type = member.field("index_type");

    let value_type = member.field("type");
    unimplemented!()
}

pub fn consistent_indexed_object_style_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-debugger",
        languages => [Typescript],
        messages => [
            prefer_record => "A record is preferred over an index signature.",
            prefer_index_signature => "An index signature is preferred over a record.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            mode: Options = options,
        },
        listeners => [
            r#"
              (generic_type
                name: (type_identifier) @record (#eq? @record "Record")
              ) @generic_type
            "# => |captures, context| {
                if self.mode != Options::IndexSignature {
                    return;
                }

                let node = captures["generic_type"];
                let type_arguments = node.field("type_arguments");
                if type_arguments.num_non_comment_named_children(SupportedLanguage::Javascript) != 2 {
                    return;
                }

                let params = type_arguments.non_comment_named_children(SupportedLanguage::Javascript).collect_vec();

                context.report(violation! {
                    node => node,
                    message_id => "prefer_index_signature",
                    fix => |fixer| {
                        let key = params[0].text(context);
                        let type_ = params[1].text(context);
                        fixer.replace_text(
                            node,
                            format!("{{ [key: {key}]: {type_} }}")
                        );
                    }
                });
            },
            r#"
              (object_type
                (index_signature
                  name: (identifier)
                  type: (type_annotation)
                )
              ) @c
            "# => |node, context| {
                if !get_is_type_literal(node) {
                    return;
                }

                let parent = find_parent_declaration(node);
                check_members(
                    node.non_comment_named_children(SupportedLanguage::Javascript),
                    node,
                    parent.map(|parent| parent.field("name")),
                    "",
                    "",
                    None,
                    context,
                );
            }
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_consistent_indexed_object_style_rule() {
        RuleTester::run(
            consistent_indexed_object_style_rule(),
            rule_tests! {
                valid => [
                  // 'record' (default)
                  // Record
                  "type Foo = Record<string, any>;",

                  // Interface
                  "interface Foo {}",
                  r#"
              interface Foo {
                bar: string;
              }
                  "#,
                  r#"
              interface Foo {
                bar: string;
                [key: string]: any;
              }
                  "#,
                  r#"
              interface Foo {
                [key: string]: any;
                bar: string;
              }
                  "#,
                  // circular
                  "type Foo = { [key: string]: string | Foo };",
                  "type Foo = { [key: string]: Foo };",
                  "type Foo = { [key: string]: Foo } | Foo;",
                  r#"
              interface Foo {
                [key: string]: Foo;
              }
                  "#,
                  r#"
              interface Foo<T> {
                [key: string]: Foo<T>;
              }
                  "#,
                  r#"
              interface Foo<T> {
                [key: string]: Foo<T> | string;
              }
                  "#,
                  // Type literal
                  "type Foo = {};",
                  r#"
              type Foo = {
                bar: string;
                [key: string]: any;
              };
                  "#,
                  r#"
              type Foo = {
                bar: string;
              };
                  "#,
                  r#"
              type Foo = {
                [key: string]: any;
                bar: string;
              };
                  "#,

                  // Generic
                  r#"
              type Foo = Generic<{
                [key: string]: any;
                bar: string;
              }>;
                  "#,

                  // Function types
                  "function foo(arg: { [key: string]: any; bar: string }) {}",
                  "function foo(): { [key: string]: any; bar: string } {}",

                  // Invalid syntax allowed by the parser
                  "type Foo = { [key: string] };",
                  "type Foo = { [] };",
                  r#"
              interface Foo {
                [key: string];
              }
                  "#,
                  r#"
              interface Foo {
                [];
              }
                  "#,
                  // 'index-signature'
                  // Unhandled type
                  {
                    code => "type Foo = Misc<string, unknown>;",
                    options => "index-signature",
                  },

                  // Invalid record
                  {
                    code => "type Foo = Record;",
                    options => "index-signature",
                  },
                  {
                    code => "type Foo = Record<string>;",
                    options => "index-signature",
                  },
                  {
                    code => "type Foo = Record<string, number, unknown>;",
                    options => "index-signature",
                  },

                  // Type literal
                  {
                    code => "type Foo = { [key: string]: any };",
                    options => "index-signature",
                  },

                  // Generic
                  {
                    code => "type Foo = Generic<{ [key: string]: any }>;",
                    options => "index-signature",
                  },

                  // Function types
                  {
                    code => "function foo(arg: { [key: string]: any }) {}",
                    options => "index-signature",
                  },
                  {
                    code => "function foo(): { [key: string]: any } {}",
                    options => "index-signature",
                  },

                  // Namespace
                  {
                    code => "type T = A.B;",
                    options => "index-signature",
                  },
                ],
                invalid => [
                  // Interface
                  {
                    code => r#"
              interface Foo {
                [key: string]: any;
              }
                    "#,
                    output => r#"
              type Foo = Record<string, any>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },

                  // Readonly interface
                  {
                    code => r#"
              interface Foo {
                readonly [key: string]: any;
              }
                    "#,
                    output => r#"
              type Foo = Readonly<Record<string, any>>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },

                  // Interface with generic parameter
                  {
                    code => r#"
              interface Foo<A> {
                [key: string]: A;
              }
                    "#,
                    output => r#"
              type Foo<A> = Record<string, A>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },

                  // Interface with generic parameter and default value
                  {
                    code => r#"
              interface Foo<A = any> {
                [key: string]: A;
              }
                    "#,
                    output => r#"
              type Foo<A = any> = Record<string, A>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },

                  // Interface with extends
                  {
                    code => r#"
              interface B extends A {
                [index: number]: unknown;
              }
                    "#,
                    output => None,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },
                  // Readonly interface with generic parameter
                  {
                    code => r#"
              interface Foo<A> {
                readonly [key: string]: A;
              }
                    "#,
                    output => r#"
              type Foo<A> = Readonly<Record<string, A>>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },

                  // Interface with multiple generic parameters
                  {
                    code => r#"
              interface Foo<A, B> {
                [key: A]: B;
              }
                    "#,
                    output => r#"
              type Foo<A, B> = Record<A, B>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },

                  // Readonly interface with multiple generic parameters
                  {
                    code => r#"
              interface Foo<A, B> {
                readonly [key: A]: B;
              }
                    "#,
                    output => r#"
              type Foo<A, B> = Readonly<Record<A, B>>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },

                  // Type literal
                  {
                    code => "type Foo = { [key: string]: any };",
                    output => "type Foo = Record<string, any>;",
                    errors => [{ message_id => "prefer_record", line => 1, column => 12 }],
                  },

                  // Readonly type literal
                  {
                    code => "type Foo = { readonly [key: string]: any };",
                    output => "type Foo = Readonly<Record<string, any>>;",
                    errors => [{ message_id => "prefer_record", line => 1, column => 12 }],
                  },

                  // Generic
                  {
                    code => "type Foo = Generic<{ [key: string]: any }>;",
                    output => "type Foo = Generic<Record<string, any>>;",
                    errors => [{ message_id => "prefer_record", line => 1, column => 20 }],
                  },

                  // Readonly Generic
                  {
                    code => "type Foo = Generic<{ readonly [key: string]: any }>;",
                    output => "type Foo = Generic<Readonly<Record<string, any>>>;",
                    errors => [{ message_id => "prefer_record", line => 1, column => 20 }],
                  },

                  // Function types
                  {
                    code => "function foo(arg: { [key: string]: any }) {}",
                    output => "function foo(arg: Record<string, any>) {}",
                    errors => [{ message_id => "prefer_record", line => 1, column => 19 }],
                  },
                  {
                    code => "function foo(): { [key: string]: any } {}",
                    output => "function foo(): Record<string, any> {}",
                    errors => [{ message_id => "prefer_record", line => 1, column => 17 }],
                  },

                  // Readonly function types
                  {
                    code => "function foo(arg: { readonly [key: string]: any }) {}",
                    output => "function foo(arg: Readonly<Record<string, any>>) {}",
                    errors => [{ message_id => "prefer_record", line => 1, column => 19 }],
                  },
                  {
                    code => "function foo(): { readonly [key: string]: any } {}",
                    output => "function foo(): Readonly<Record<string, any>> {}",
                    errors => [{ message_id => "prefer_record", line => 1, column => 17 }],
                  },

                  // Never
                  // Type literal
                  {
                    code => "type Foo = Record<string, any>;",
                    options => "index-signature",
                    output => "type Foo = { [key: string]: any };",
                    errors => [{ message_id => "prefer_index_signature", line => 1, column => 12 }],
                  },

                  // Type literal with generic parameter
                  {
                    code => "type Foo<T> = Record<string, T>;",
                    options => "index-signature",
                    output => "type Foo<T> = { [key: string]: T };",
                    errors => [{ message_id => "prefer_index_signature", line => 1, column => 15 }],
                  },

                  // Circular
                  {
                    code => "type Foo = { [k: string]: A.Foo };",
                    output => "type Foo = Record<string, A.Foo>;",
                    errors => [{ message_id => "prefer_record", line => 1, column => 12 }],
                  },
                  {
                    code => "type Foo = { [key: string]: AnotherFoo };",
                    output => "type Foo = Record<string, AnotherFoo>;",
                    errors => [{ message_id => "prefer_record", line => 1, column => 12 }],
                  },
                  {
                    code => "type Foo = { [key: string]: { [key: string]: Foo } };",
                    output => "type Foo = { [key: string]: Record<string, Foo> };",
                    errors => [{ message_id => "prefer_record", line => 1, column => 29 }],
                  },
                  {
                    code => "type Foo = { [key: string]: string } | Foo;",
                    output => "type Foo = Record<string, string> | Foo;",
                    errors => [{ message_id => "prefer_record", line => 1, column => 12 }],
                  },
                  {
                    code => r#"
              interface Foo<T> {
                [k: string]: T;
              }
                    "#,
                    output => r#"
              type Foo<T> = Record<string, T>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },
                  {
                    code => r#"
              interface Foo {
                [k: string]: A.Foo;
              }
                    "#,
                    output => r#"
              type Foo = Record<string, A.Foo>;
                    "#,
                    errors => [{ message_id => "prefer_record", line => 2, column => 1 }],
                  },
                  {
                    code => r#"
              interface Foo {
                [k: string]: { [key: string]: Foo };
              }
                    "#,
                    output => r#"
              interface Foo {
                [k: string]: Record<string, Foo>;
              }
                    "#,
                    errors => [{ message_id => "prefer_record", line => 3, column => 16 }],
                  },

                  // Generic
                  {
                    code => "type Foo = Generic<Record<string, any>>;",
                    options => "index-signature",
                    output => "type Foo = Generic<{ [key: string]: any }>;",
                    errors => [{ message_id => "prefer_index_signature", line => 1, column => 20 }],
                  },

                  // Function types
                  {
                    code => "function foo(arg: Record<string, any>) {}",
                    options => "index-signature",
                    output => "function foo(arg: { [key: string]: any }) {}",
                    errors => [{ message_id => "prefer_index_signature", line => 1, column => 19 }],
                  },
                  {
                    code => "function foo(): Record<string, any> {}",
                    options => "index-signature",
                    output => "function foo(): { [key: string]: any } {}",
                    errors => [{ message_id => "prefer_index_signature", line => 1, column => 17 }],
                  },
                ],
            },
        )
    }
}
