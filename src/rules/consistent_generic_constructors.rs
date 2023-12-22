use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

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
        options_type => Options,
        state => {
            [per-config]
            mode: Options = options,
        },
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
                        message_id => "preferConstructor",
                      },
                    ],
                    output => "const a = new Foo<string>();",
                  },
                  {
                    code => "const a: Map<string, number> = new Map();",
                    errors => [
                      {
                        message_id => "preferConstructor",
                      },
                    ],
                    output => "const a = new Map<string, number>();",
                  },
                  {
                    code => r#"const a: Map <string, number> = new Map();"#,
                    errors => [
                      {
                        message_id => "preferConstructor",
                      },
                    ],
                    output => r#"const a = new Map<string, number>();"#,
                  },
                  {
                    code => r#"const a: Map< string, number > = new Map();"#,
                    errors => [
                      {
                        message_id => "preferConstructor",
                      },
                    ],
                    output => r#"const a = new Map< string, number >();"#,
                  },
                  {
                    code => r#"const a: Map<string, number> = new Map ();"#,
                    errors => [
                      {
                        message_id => "preferConstructor",
                      },
                    ],
                    output => r#"const a = new Map<string, number> ();"#,
                  },
                  {
                    code => r#"const a: Foo<number> = new Foo;"#,
                    errors => [
                      {
                        message_id => "preferConstructor",
                      },
                    ],
                    output => r#"const a = new Foo<number>();"#,
                  },
                  {
                    code => "const a: /* comment */ Foo/* another */ <string> = new Foo();",
                    errors => [
                      {
                        message_id => "preferConstructor",
                      },
                    ],
                    output => r#"const a = new Foo/* comment *//* another */<string>();"#,
                  },
                  {
                    code => "const a: Foo/* comment */ <string> = new Foo /* another */();",
                    errors => [
                      {
                        message_id => "preferConstructor",
                      },
                    ],
                    output => r#"const a = new Foo/* comment */<string> /* another */();"#,
                  },
                  {
                    code => r#"const a: Foo<string> = new \n Foo \n ();"#,
                    errors => [
                      {
                        message_id => "preferConstructor",
                      },
                    ],
                    output => r#"const a = new \n Foo<string> \n ();"#,
                  },
                  {
                    code => r#"
              class Foo {
                a: Foo<string> = new Foo();
              }
                    "#,
                    errors => [
                      {
                        message_id => "preferConstructor",
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
                        message_id => "preferConstructor",
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
                        message_id => "preferConstructor",
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
                        message_id => "preferConstructor",
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
                        message_id => "preferConstructor",
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
                        message_id => "preferConstructor",
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
                        message_id => "preferConstructor",
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
