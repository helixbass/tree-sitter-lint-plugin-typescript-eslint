use std::sync::Arc;

use indexmap::IndexSet;
use serde::Deserialize;
use squalid::EverythingExt;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt, Rule,
};
use tree_sitter_lint_plugin_eslint_builtin::kind::{Identifier, NewExpression, VariableDeclarator};

use crate::kind::{
    GenericType, OptionalParameter, PublicFieldDefinition, RequiredParameter, TypeIdentifier,
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Options {
    #[default]
    Constructor,
    TypeAnnotation,
}

pub fn consistent_generic_constructors_rule() -> Arc<dyn Rule> {
    rule! {
        name => "consistent-generic-constructors",
        languages => [Typescript],
        messages => [
            prefer_type_annotation => "The generic type arguments should be specified as part of the type annotation.",
            prefer_constructor => "The generic type arguments should be specified as part of the constructor type arguments.",
        ],
        fixable => true,
        concatenate_adjacent_insert_fixes => true,
        options_type => Options,
        state => {
            [per-config]
            mode: Options = options,
        },
        listeners => [
            r#"
              (variable_declarator) @c
              (public_field_definition) @c
              (required_parameter
                value: (_)
              ) @c
              (optional_parameter
                value: (_)
              ) @c
            "# => |node, context| {
                let (lhs_name, lhs, rhs) = match node.kind() {
                    VariableDeclarator | PublicFieldDefinition => (
                        node.field("name"),
                        node.child_by_field_name("type").map(|type_| type_.first_non_comment_named_child(SupportedLanguage::Javascript)),
                        node.child_by_field_name("value")
                    ),
                    RequiredParameter | OptionalParameter => (
                        node.field("pattern"),
                        node.child_by_field_name("type").map(|type_| type_.first_non_comment_named_child(SupportedLanguage::Javascript)),
                        node.child_by_field_name("value")
                    ),
                    _ => unreachable!()
                };
                let Some(rhs) = rhs.filter(|&rhs| {
                    rhs.kind() == NewExpression &&
                        rhs.field("constructor").kind() == Identifier
                }) else {
                    return;
                };

                match self.mode {
                    Options::TypeAnnotation => {
                        if lhs.is_some() {
                            return;
                        }
                        let Some(type_arguments) = rhs.child_by_field_name("type_arguments") else {
                            return;
                        };
                        let callee = rhs.field("constructor");
                        let type_annotation = format!(
                            "{}{}",
                            callee.text(context),
                            type_arguments.text(context),
                        );
                        context.report(violation! {
                            node => node,
                            message_id => "prefer_type_annotation",
                            fix => |fixer| {
                                let id_to_attach_annotation = match node.kind() {
                                    PublicFieldDefinition => node.field("name"),
                                    _ => lhs_name
                                };
                                fixer.remove(type_arguments);
                                fixer.insert_text_after(
                                    id_to_attach_annotation,
                                    format!(": {type_annotation}")
                                );
                            }
                        });
                    }
                    Options::Constructor => {
                        if rhs.child_by_field_name("type_arguments").is_some() {
                            return;
                        }
                        let Some(lhs_type_arguments) = lhs.filter(|&lhs| {
                            lhs.kind() == GenericType &&
                                lhs.field("name").thrush(|lhs_name| {
                                    lhs_name.kind() == TypeIdentifier &&
                                        lhs_name.text(context) == rhs.field("constructor").text(context)
                                })
                        }).map(|lhs| lhs.field("type_arguments")) else {
                            return;
                        };
                        let lhs = lhs.unwrap();
                        let has_parens = rhs.child_by_field_name("arguments").is_some();
                        let mut extra_comments: IndexSet<Node<'a>> = context
                            .get_comments_inside(lhs.parent().unwrap())
                            .collect();
                        context.get_comments_inside(lhs_type_arguments).for_each(|c| {
                            extra_comments.remove(&c);
                        });
                        context.report(violation! {
                            node => node,
                            message_id => "prefer_constructor",
                            fix => |fixer| {
                                fixer.remove(lhs.parent().unwrap());
                                for &comment in &extra_comments {
                                    fixer.insert_text_after(
                                        rhs.field("constructor"),
                                        comment.text(context)
                                    );
                                }
                                fixer.insert_text_after(
                                    rhs.field("constructor"),
                                    lhs_type_arguments.text(context),
                                );
                                if !has_parens {
                                    fixer.insert_text_after(
                                        rhs.field("constructor"),
                                        "()"
                                    );
                                }
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

    #[test]
    fn test_consistent_generic_constructors_rule() {
        RuleTester::run(
            consistent_generic_constructors_rule(),
            rule_tests! {
                valid => [
                  // default: constructor
                  "const a = new Foo();",
                  "const a = new Foo<string>();",
                  "const a: Foo<string> = new Foo<string>();",
                  "const a: Foo = new Foo();",
                  "const a: Bar<string> = new Foo();",
                  "const a: Foo = new Foo<string>();",
                  "const a: Bar = new Foo<string>();",
                  "const a: Bar<string> = new Foo<string>();",
                  "const a: Foo<string> = Foo<string>();",
                  "const a: Foo<string> = Foo();",
                  "const a: Foo = Foo<string>();",
                  r#"
              class Foo {
                a = new Foo<string>();
              }
                  "#,
                  r#"
              function foo(a: Foo = new Foo<string>()) {}
                  "#,
                  r#"
              function foo({ a }: Foo = new Foo<string>()) {}
                  "#,
                  r#"
              function foo([a]: Foo = new Foo<string>()) {}
                  "#,
                  r#"
              class A {
                constructor(a: Foo = new Foo<string>()) {}
              }
                  "#,
                  r#"
              const a = function (a: Foo = new Foo<string>()) {};
                  "#,
                  // type-annotation
                  {
                    code => "const a = new Foo();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a: Foo<string> = new Foo();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a: Foo<string> = new Foo<string>();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a: Foo = new Foo();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a: Bar = new Foo<string>();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a: Bar<string> = new Foo<string>();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a: Foo<string> = Foo<string>();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a: Foo<string> = Foo();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a: Foo = Foo<string>();",
                    options => "type-annotation",
                  },
                  {
                    code => "const a = new (class C<T> {})<string>();",
                    options => "type-annotation",
                  },
                  {
                    code => r#"
              class Foo {
                a: Foo<string> = new Foo();
              }
                    "#,
                    options => "type-annotation",
                  },
                  {
                    code => r#"
              function foo(a: Foo<string> = new Foo()) {}
                    "#,
                    options => "type-annotation",
                  },
                  {
                    code => r#"
              function foo({ a }: Foo<string> = new Foo()) {}
                    "#,
                    options => "type-annotation",
                  },
                  {
                    code => r#"
              function foo([a]: Foo<string> = new Foo()) {}
                    "#,
                    options => "type-annotation",
                  },
                  {
                    code => r#"
              class A {
                constructor(a: Foo<string> = new Foo()) {}
              }
                    "#,
                    options => "type-annotation",
                  },
                  {
                    code => r#"
              const a = function (a: Foo<string> = new Foo()) {};
                    "#,
                    options => "type-annotation",
                  },
                  {
                    code => r#"
              const [a = new Foo<string>()] = [];
                    "#,
                    options => "type-annotation",
                  },
                  {
                    code => r#"
              function a([a = new Foo<string>()]) {}
                    "#,
                    options => "type-annotation",
                  },
                ],
                invalid => [
                  {
                    code => "const a: Foo<string> = new Foo();",
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => "const a = new Foo<string>();",
                  },
                  {
                    code => "const a: Map<string, number> = new Map();",
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => "const a = new Map<string, number>();",
                  },
                  {
                    code => r#"const a: Map <string, number> = new Map();"#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"const a = new Map<string, number>();"#,
                  },
                  {
                    code => r#"const a: Map< string, number > = new Map();"#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"const a = new Map< string, number >();"#,
                  },
                  {
                    code => r#"const a: Map<string, number> = new Map ();"#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"const a = new Map<string, number> ();"#,
                  },
                  {
                    code => r#"const a: Foo<number> = new Foo;"#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"const a = new Foo<number>();"#,
                  },
                  {
                    code => "const a: /* comment */ Foo/* another */ <string> = new Foo();",
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"const a = new Foo/* comment *//* another */<string>();"#,
                  },
                  {
                    code => "const a: Foo/* comment */ <string> = new Foo /* another */();",
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"const a = new Foo/* comment */<string> /* another */();"#,
                  },
                  {
                    code => "const a: Foo<string> = new \n Foo \n ();",
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => "const a = new \n Foo<string> \n ();",
                  },
                  {
                    code => r#"
              class Foo {
                a: Foo<string> = new Foo();
              }
                    "#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"
              class Foo {
                a = new Foo<string>();
              }
                    "#,
                  },
                  {
                    code => r#"
              class Foo {
                [a]: Foo<string> = new Foo();
              }
                    "#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"
              class Foo {
                [a] = new Foo<string>();
              }
                    "#,
                  },
                  {
                    code => r#"
              function foo(a: Foo<string> = new Foo()) {}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"
              function foo(a = new Foo<string>()) {}
                    "#,
                  },
                  {
                    code => r#"
              function foo({ a }: Foo<string> = new Foo()) {}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"
              function foo({ a } = new Foo<string>()) {}
                    "#,
                  },
                  {
                    code => r#"
              function foo([a]: Foo<string> = new Foo()) {}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"
              function foo([a] = new Foo<string>()) {}
                    "#,
                  },
                  {
                    code => r#"
              class A {
                constructor(a: Foo<string> = new Foo()) {}
              }
                    "#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"
              class A {
                constructor(a = new Foo<string>()) {}
              }
                    "#,
                  },
                  {
                    code => r#"
              const a = function (a: Foo<string> = new Foo()) {};
                    "#,
                    errors => [
                      {
                        message_id => "prefer_constructor",
                      },
                    ],
                    output => r#"
              const a = function (a = new Foo<string>()) {};
                    "#,
                  },
                  {
                    code => "const a = new Foo<string>();",
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => "const a: Foo<string> = new Foo();",
                  },
                  {
                    code => "const a = new Map<string, number>();",
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => "const a: Map<string, number> = new Map();",
                  },
                  {
                    code => r#"const a = new Map <string, number> ();"#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"const a: Map<string, number> = new Map  ();"#,
                  },
                  {
                    code => r#"const a = new Map< string, number >();"#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"const a: Map< string, number > = new Map();"#,
                  },
                  {
                    code => r#"const a = new \n Foo<string> \n ();"#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"const a: Foo<string> = new \n Foo \n ();"#,
                  },
                  {
                    code => "const a = new Foo/* comment */ <string> /* another */();",
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"const a: Foo<string> = new Foo/* comment */  /* another */();"#,
                  },
                  {
                    code => "const a = new Foo</* comment */ string, /* another */ number>();",
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"const a: Foo</* comment */ string, /* another */ number> = new Foo();"#,
                  },
                  {
                    code => r#"
              class Foo {
                a = new Foo<string>();
              }
                    "#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"
              class Foo {
                a: Foo<string> = new Foo();
              }
                    "#,
                  },
                  {
                    code => r#"
              class Foo {
                [a] = new Foo<string>();
              }
                    "#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"
              class Foo {
                [a]: Foo<string> = new Foo();
              }
                    "#,
                  },
                  {
                    code => r#"
              class Foo {
                [a + b] = new Foo<string>();
              }
                    "#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"
              class Foo {
                [a + b]: Foo<string> = new Foo();
              }
                    "#,
                  },
                  {
                    code => r#"
              function foo(a = new Foo<string>()) {}
                    "#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"
              function foo(a: Foo<string> = new Foo()) {}
                    "#,
                  },
                  {
                    code => r#"
              function foo({ a } = new Foo<string>()) {}
                    "#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"
              function foo({ a }: Foo<string> = new Foo()) {}
                    "#,
                  },
                  {
                    code => r#"
              function foo([a] = new Foo<string>()) {}
                    "#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"
              function foo([a]: Foo<string> = new Foo()) {}
                    "#,
                  },
                  {
                    code => r#"
              class A {
                constructor(a = new Foo<string>()) {}
              }
                    "#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"
              class A {
                constructor(a: Foo<string> = new Foo()) {}
              }
                    "#,
                  },
                  {
                    code => r#"
              const a = function (a = new Foo<string>()) {};
                    "#,
                    options => "type-annotation",
                    errors => [
                      {
                        message_id => "prefer_type_annotation",
                      },
                    ],
                    output => r#"
              const a = function (a: Foo<string> = new Foo()) {};
                    "#,
                  },
                ],
            },
        )
    }
}
