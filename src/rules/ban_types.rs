use std::{borrow::Cow, collections::HashMap, sync::Arc};

use derive_builder::Builder;
use once_cell::sync::Lazy;
use serde::Deserialize;
use squalid::{regex, CowExt, NonEmpty};
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule,
};

use crate::ast_helpers::{get_is_type_literal, get_is_type_reference};

#[derive(Builder, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[builder(default, setter(strip_option, into))]
struct BanConfigObject {
    message: Option<String>,
    fix_with: Option<String>,
    suggest: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum BanConfig {
    None,
    Bool(bool),
    String(String),
    Object(BanConfigObject),
}

type Types = HashMap<String, BanConfig>;

static DEFAULT_TYPES: Lazy<Types> = Lazy::new(|| {
    [
        (
            "String".to_owned(),
            BanConfig::Object(
                BanConfigObjectBuilder::default()
                    .message("Use string instead")
                    .fix_with("string")
                    .build()
                    .unwrap(),
            ),
        ),
        (
            "Boolean".to_owned(),
            BanConfig::Object(
                BanConfigObjectBuilder::default()
                    .message("Use boolean instead")
                    .fix_with("boolean")
                    .build()
                    .unwrap(),
            ),
        ),
        (
            "Number".to_owned(),
            BanConfig::Object(
                BanConfigObjectBuilder::default()
                    .message("Use number instead")
                    .fix_with("number")
                    .build()
                    .unwrap(),
            ),
        ),
        (
            "Symbol".to_owned(),
            BanConfig::Object(
                BanConfigObjectBuilder::default()
                    .message("Use symbol instead")
                    .fix_with("symbol")
                    .build()
                    .unwrap(),
            ),
        ),
        (
            "BigInt".to_owned(),
            BanConfig::Object(
                BanConfigObjectBuilder::default()
                    .message("Use bigint instead")
                    .fix_with("bigint")
                    .build()
                    .unwrap(),
            ),
        ),
        (
            "Function".to_owned(),
            BanConfig::Object(
                BanConfigObjectBuilder::default()
                    .message([
                        "The `Function` type accepts any function-like value.",
                        "It provides no type safety when calling the function, which can be a common source of bugs.",
                        "It also accepts things like class declarations, which will throw at runtime as they will not be called with `new`.",
                        "If you are expecting the function to accept certain arguments, you should explicitly define the function shape.",
                    ].join("\n"))
                    .build()
                    .unwrap(),
            ),
        ),
        (
            "Object".to_owned(),
            BanConfig::Object(
                BanConfigObjectBuilder::default()
                    .message([
                        "The `Object` type actually means \"any non-nullish value\", so it is marginally better than `unknown`.",
                        "- If you want a type meaning \"any object\", you probably want `object` instead.",
                        "- If you want a type meaning \"any value\", you probably want `unknown` instead.",
                        "- If you really want a type meaning \"any non-nullish value\", you probably want `NonNullable<unknown>` instead.",
                    ].join("\n"))
                    // TODO: suggestions?
                    .build()
                    .unwrap(),
            ),
        ),
        (
            "{}".to_owned(),
            BanConfig::Object(
                BanConfigObjectBuilder::default()
                    .message([
                        "`{}` actually means \"any non-nullish value\".",
                        "- If you want a type meaning \"any object\", you probably want `object` instead.",
                        "- If you want a type meaning \"any value\", you probably want `unknown` instead.",
                        "- If you want a type meaning \"empty object\", you probably want `Record<string, never>` instead.",
                        "- If you really want a type meaning \"any non-nullish value\", you probably want `NonNullable<unknown>` instead.",
                    ].join("\n"))
                    .build()
                    .unwrap(),
            ),
        ),
    ]
    .into()
});

fn remove_spaces(str_: &str) -> Cow<'_, str> {
    regex!(r#"\s"#).replace_all(str_, "")
}

fn stringify_node<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> Cow<'a, str> {
    node.text(context).map_cow(remove_spaces)
}

fn get_custom_message(banned_type: &BanConfig) -> String {
    match banned_type {
        BanConfig::String(banned_type) => format!(" {banned_type}"),
        BanConfig::Object(banned_type) if banned_type.message.as_ref().is_non_empty() => {
            format!(" {}", banned_type.message.as_ref().unwrap())
        }
        _ => "".to_owned(),
    }
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
struct Options {
    types: Option<Types>,
    extend_defaults: Option<bool>,
}

impl Options {
    fn extend_defaults(&self) -> bool {
        self.extend_defaults.unwrap_or(true)
    }
}

pub fn ban_types_rule() -> Arc<dyn Rule> {
    rule! {
        name => "ban-types",
        languages => [Typescript],
        messages => [
            banned_type_message => "Don't use `{{name}}` as a type.{{custom_message}}",
            banned_type_replacement => "Replace `{{name}}` with `{{replacement}}`",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            banned_types: Types = {
                let mut types = if options.extend_defaults() {
                    DEFAULT_TYPES.clone()
                } else {
                    Default::default()
                };
                if let Some(options_types) = options.types.as_ref() {
                    types.extend(options_types.into_iter().map(|(type_, data)| (remove_spaces(type_).into_owned(), data.clone())));
                }
                types
            },
        },
        methods => {
            fn check_banned_types(&self, type_node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
                let name = stringify_node(type_node, context);
                let Some(banned_type) = self.banned_types.get(&*name).filter(|&banned_type| {
                    *banned_type != BanConfig::Bool(false)
                }) else {
                    return;
                };

                let custom_message = get_custom_message(banned_type);
                let fix_with = match banned_type {
                    BanConfig::Object(banned_type) => banned_type.fix_with.as_ref(),
                    _ => None
                };

                context.report(violation! {
                    node => type_node,
                    message_id => "banned_type_message",
                    data => {
                        name => name,
                        custom_message => custom_message,
                    },
                    fix => |fixer| {
                        let Some(fix_with) = fix_with else {
                            return;
                        };
                        fixer.replace_text(
                            type_node,
                            fix_with
                        );
                    },
                });
            }
        },
        listeners => [
            r#"
              (type_identifier) @c
            "# => |node, context| {
                if !get_is_type_reference(node) {
                    return;
                }

                self.check_banned_types(node, context);
            },
            r#"
              (predefined_type) @c
              (literal_type
                (undefined) @c
              )
              (literal_type
                (null) @c
              )
              (generic_type) @c
              (nested_type_identifier) @c
            "# => |node, context| {
                self.check_banned_types(node, context);
            },
            r#"
              (tuple_type) @c
            "# => |node, context| {
                if node.non_comment_named_children(SupportedLanguage::Javascript).next().is_some() {
                    return;
                }

                self.check_banned_types(node, context);
            },
            r#"
              (object_type) @c
            "# => |node, context| {
                if !get_is_type_literal(node) {
                    return;
                }

                if node.non_comment_named_children(SupportedLanguage::Javascript).next().is_some() {
                    return;
                }

                self.check_banned_types(node, context);
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{
        rule_tests, serde_json::json, RuleTestExpectedErrorBuilder, RuleTestInvalidBuilder,
        RuleTester,
    };

    use super::*;

    #[test]
    fn test_ban_types_rule() {
        let options = json!({
            "types": {
              "String": {
                "message": "Use string instead.",
                "fix_with": "string",
              },
              "Object": "Use '{}' instead.",
              "Array": null,
              "F": null,
              "NS.Bad": {
                "message": "Use NS.Good instead.",
                "fix_with": "NS.Good",
              },
            },
            "extend_defaults": false,
        });
        RuleTester::run(
            ban_types_rule(),
            rule_tests! {
                valid => [
                  "let f = Object();", // Should not fail if there is no options set
                  "let f: { x: number; y: number } = { x: 1, y: 1 };",
                  {
                    code => "let f = Object();",
                    options => options,
                  },
                  {
                    code => "let g = Object.create(null);",
                    options => options,
                  },
                  {
                    code => "let h = String(false);",
                    options => options,
                  },
                  {
                    code => "let e: foo.String;",
                    options => options,
                  },
                  {
                    code => "let a: _.NS.Bad;",
                    options => options,
                  },
                  {
                    code => "let a: NS.Bad._;",
                    options => options,
                  },
                  // Replace default options instead of merging with extend_defaults => false
                  {
                    code => "let a: String;",
                    options =>
                      {
                        types => {
                          Number => {
                            message => "Use number instead.",
                            fix_with => "number",
                          },
                        },
                        extend_defaults => false,
                      },
                  },
                  {
                    code => "let a: undefined;",
                    options =>
                      {
                        types => {
                          null => {
                            message => "Use undefined instead.",
                            fix_with => "undefined",
                          },
                        },
                      },
                  },
                  {
                    code => "let a: null;",
                    options =>
                      {
                        types => {
                          undefined => null,
                        },
                        extend_defaults => false,
                      },
                  },
                  {
                    code => "type Props = {};",
                    options =>
                      {
                        types => {
                          "{}" => false,
                        },
                        extend_defaults => true,
                      },
                  },
                  "let a: [];",
                ],
                invalid => [
                  {
                    code => "let a: String;",
                    output => "let a: string;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        line => 1,
                        column => 8,
                      },
                    ],
                  },
                  {
                    code => "let a: Object;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "Object",
                          custom_message => " Use '{}' instead.",
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options => options,
                  },
                  {
                    code => "let a: Object;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "Object",
                          custom_message => [
                            " The `Object` type actually means \"any non-nullish value\", so it is marginally better than `unknown`.",
                            "- If you want a type meaning \"any object\", you probably want `object` instead.",
                            "- If you want a type meaning \"any value\", you probably want `unknown` instead.",
                            "- If you really want a type meaning \"any non-nullish value\", you probably want `NonNullable<unknown>` instead.",
                          ].join("\n"),
                        },
                        line => 1,
                        column => 8,
                        // suggestions: [
                        //   {
                        //     message_id => "bannedTypeReplacement",
                        //     data => { name => "Object", replacement => "object" },
                        //     output => "let a: object;",
                        //   },
                        //   {
                        //     message_id => "bannedTypeReplacement",
                        //     data => { name => "Object", replacement => "unknown" },
                        //     output => "let a: unknown;",
                        //   },
                        //   {
                        //     message_id => "bannedTypeReplacement",
                        //     data => { name => "Object", replacement => "NonNullable<unknown>" },
                        //     output => "let a: NonNullable<unknown>;",
                        //   },
                        // ],
                      },
                    ],
                    options => {},
                  },
                  {
                    code => "let aa: Foo;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "Foo",
                          custom_message => "",
                        },
                      },
                    ],
                    options =>
                      {
                        types => {
                          Foo => { message => "" },
                        },
                      },
                  },
                  {
                    code => "let b: { c: String };",
                    output => "let b: { c: string };",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 1,
                        column => 13,
                      },
                    ],
                    options => options,
                  },
                  {
                    code => "function foo(a: String) {}",
                    output => "function foo(a: string) {}",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 1,
                        column => 17,
                      },
                    ],
                    options => options,
                  },
                  {
                    code => "'a' as String;",
                    output => "'a' as string;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options => options,
                  },
                  {
                    code => "let c: F;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => { name => "F", custom_message => "" },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options => options,
                  },
                  {
                    code => r#"
class Foo<F = String> extends Bar<String> implements Baz<Object> {
  constructor(foo: String | Object) {}

  exit(): Array<String> {
    const foo: String = 1 as String;
  }
}
                    "#,
                    output => r#"
class Foo<F = string> extends Bar<string> implements Baz<Object> {
  constructor(foo: string | Object) {}

  exit(): Array<string> {
    const foo: string = 1 as string;
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 2,
                        column => 15,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 2,
                        column => 35,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "Object",
                          custom_message => " Use '{}' instead.",
                        },
                        line => 2,
                        column => 58,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 3,
                        column => 20,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "Object",
                          custom_message => " Use '{}' instead.",
                        },
                        line => 3,
                        column => 29,
                      },
                      {
                        message_id => "banned_type_message",
                        data => { name => "Array", custom_message => "" },
                        line => 5,
                        column => 11,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 5,
                        column => 17,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 6,
                        column => 16,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "String",
                          custom_message => " Use string instead.",
                        },
                        line => 6,
                        column => 30,
                      },
                    ],
                    options => options,
                  },
                  {
                    code => "let a: NS.Bad;",
                    output => "let a: NS.Good;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "NS.Bad",
                          custom_message => " Use NS.Good instead.",
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options => options,
                  },
                  {
                    code => r#"
let a: NS.Bad<Foo>;
let b: Foo<NS.Bad>;
                    "#,
                    output => r#"
let a: NS.Good<Foo>;
let b: Foo<NS.Good>;
                    "#,
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "NS.Bad",
                          custom_message => " Use NS.Good instead.",
                        },
                        line => 2,
                        column => 8,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "NS.Bad",
                          custom_message => " Use NS.Good instead.",
                        },
                        line => 3,
                        column => 12,
                      },
                    ],
                    options => options,
                  },
                  {
                    code => "let foo: {} = {};",
                    output => "let foo: object = {};",
                    options =>
                      {
                        types => {
                          "{}" => {
                            message => "Use object instead.",
                            fix_with => "object",
                          },
                        },
                      },
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "{}",
                          custom_message => " Use object instead.",
                        },
                        line => 1,
                        column => 10,
                      },
                    ],
                  },
                  {
                    code => r#"
let foo: {} = {};
let bar: {     } = {};
let baz: {
} = {};
                    "#,
                    output => r#"
let foo: object = {};
let bar: object = {};
let baz: object = {};
                    "#,
                    options =>
                      {
                        types => {
                          "{   }" => {
                            message => "Use object instead.",
                            fix_with => "object",
                          },
                        },
                      },
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "{}",
                          custom_message => " Use object instead.",
                        },
                        line => 2,
                        column => 10,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "{}",
                          custom_message => " Use object instead.",
                        },
                        line => 3,
                        column => 10,
                      },
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "{}",
                          custom_message => " Use object instead.",
                        },
                        line => 4,
                        column => 10,
                      },
                    ],
                  },
                  {
                    code => "let a: NS.Bad;",
                    output => "let a: NS.Good;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "NS.Bad",
                          custom_message => " Use NS.Good instead.",
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options =>
                      {
                        types => {
                          "  NS.Bad  " => {
                            message => "Use NS.Good instead.",
                            fix_with => "NS.Good",
                          },
                        },
                      },
                  },
                  {
                    code => r#"let a: Foo<   F   >;"#,
                    output => r#"let a: Foo<   T   >;"#,
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "F",
                          custom_message => " Use T instead.",
                        },
                        line => 1,
                        column => 15,
                      },
                    ],
                    options =>
                      {
                        types => {
                          "       F      " => {
                            message => "Use T instead.",
                            fix_with => "T",
                          },
                        },
                      },
                  },
                  {
                    code => "type Foo = Bar<any>;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "Bar<any>",
                          custom_message => " Don't use `any` as a type parameter to `Bar`",
                        },
                        line => 1,
                        column => 12,
                      },
                    ],
                    options =>
                      {
                        types => {
                          "Bar<any>" => "Don't use `any` as a type parameter to `Bar`",
                        },
                      },
                  },
                  {
                    code => r#"type Foo = Bar<A,B>;"#,
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "Bar<A,B>",
                          custom_message => " Don't pass `A, B` as parameters to `Bar`",
                        },
                        line => 1,
                        column => 12,
                      },
                    ],
                    options =>
                      {
                        types => {
                          "Bar<A, B>" => "Don't pass `A, B` as parameters to `Bar`",
                        },
                      },
                  },
                  {
                    code => "let a: [];",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "[]",
                          custom_message => " `[]` does only allow empty arrays.",
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options =>
                      {
                        types => {
                          "[]" => "`[]` does only allow empty arrays.",
                        },
                      },
                  },
                  {
                    code => r#"let a:  [ ] ;"#,
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "[]",
                          custom_message => " `[]` does only allow empty arrays.",
                        },
                        line => 1,
                        column => 9,
                      },
                    ],
                    options =>
                      {
                        types => {
                          "[]" => "`[]` does only allow empty arrays.",
                        },
                      },
                  },
                  {
                    code => "let a: [];",
                    output => "let a: any[];",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "[]",
                          custom_message => " `[]` does only allow empty arrays.",
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options =>
                      {
                        types => {
                          "[]" => {
                            message => "`[]` does only allow empty arrays.",
                            fix_with => "any[]",
                          },
                        },
                      },
                  },
                  {
                    code => "let a: [[]];",
                    errors => [
                      {
                        message_id => "banned_type_message",
                        data => {
                          name => "[]",
                          custom_message => " `[]` does only allow empty arrays.",
                        },
                        line => 1,
                        column => 9,
                      },
                    ],
                    options =>
                      {
                        types => {
                          "[]" => "`[]` does only allow empty arrays.",
                        },
                      },
                  },
                  {
                    code => "type Baz = 1 & Foo;",
                    errors => [
                      {
                        message_id => "banned_type_message",
                      },
                    ],
                    options =>
                      {
                        types => {
                          Foo => { message => "" },
                        },
                      },
                  },
                  {
                    code => "interface Foo extends Bar {}",
                    errors => [
                      {
                        message_id => "banned_type_message",
                      },
                    ],
                    options =>
                      {
                        types => {
                          Bar => { message => "" },
                        },
                      },
                  },
                  {
                    code => "interface Foo extends Bar, Baz {}",
                    errors => [
                      {
                        message_id => "banned_type_message",
                      },
                    ],
                    options =>
                      {
                        types => {
                          Bar => { message => "" },
                        },
                      },
                  },
                  {
                    code => "class Foo implements Bar {}",
                    errors => [
                      {
                        message_id => "banned_type_message",
                      },
                    ],
                    options =>
                      {
                        types => {
                          Bar => { message => "" },
                        },
                      },
                  },
                  {
                    code => "class Foo implements Bar, Baz {}",
                    errors => [
                      {
                        message_id => "banned_type_message",
                      },
                    ],
                    options =>
                      {
                        types => {
                          Bar => { message => "Bla" },
                        },
                      },
                  },
                  ...[
                      "bigint",
                      "boolean",
                      "never",
                      "null",
                      "number",
                      "object",
                      "string",
                      "symbol",
                      "undefined",
                      "unknown",
                      "void",
                  ].into_iter()
                      .map(|key| {
                          RuleTestInvalidBuilder::default()
                              .code(format!("function foo(x: {key}) {{}}"))
                              .errors(vec![
                                  RuleTestExpectedErrorBuilder::default()
                                    .message_id("banned_type_message")
                                    .data([
                                        (
                                            "name".to_owned(),
                                            key.to_owned(),
                                        ),
                                        (
                                            "custom_message".to_owned(),
                                            "".to_owned(),
                                        ),
                                    ])
                                    .line(1)
                                    .column(17)
                                    .build()
                                    .unwrap()
                              ])
                              .options(json!({
                                  "extend_defaults": false,
                                  "types": {
                                    key: null,
                                  },
                              }))
                              .build()
                              .unwrap()
                      }),
                ],
            },
        )
    }
}
