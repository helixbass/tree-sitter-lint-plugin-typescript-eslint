use std::{borrow::Cow, collections::HashMap, sync::Arc};

use derive_builder::Builder;
use once_cell::sync::Lazy;
use serde::Deserialize;
use squalid::regex;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Builder, Clone, Default, Deserialize)]
#[builder(default, setter(strip_option, into))]
struct BanConfigObject {
    message: Option<String>,
    fix_with: Option<String>,
    suggest: Option<Vec<String>>,
}

#[derive(Clone, Deserialize)]
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

#[derive(Default, Deserialize)]
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
            extend_defaults: bool = options.extend_defaults(),
            banned_types: Types = {
                let mut types = if options.extend_defaults() {
                    DEFAULT_TYPES.clone()
                } else {
                    Default::default()
                };
                if let Some(options_types) = options.types.as_ref() {
                    types.extend(options_types.clone());
                }
                types.into_iter().map(|(type_, data)| (remove_spaces(&type_).into_owned(), data)).collect()
            },
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
use tree_sitter_lint_plugin_eslint_builtin::kind::Object;
use tree_sitter_lint_plugin_eslint_builtin::kind::Array;

    #[test]
    fn test_ban_types_rule() {
        RuleTester::run(
            ban_types_rule(),
            rule_tests! {
                valid => [
                  'let f = Object();', // Should not fail if there is no options set
                  'let f: { x: number; y: number } = { x: 1, y: 1 };',
                  {
                    code => 'let f = Object();',
                    options,
                  },
                  {
                    code => 'let g = Object.create(null);',
                    options,
                  },
                  {
                    code => 'let h = String(false);',
                    options,
                  },
                  {
                    code => 'let e: foo.String;',
                    options,
                  },
                  {
                    code => 'let a: _.NS.Bad;',
                    options,
                  },
                  {
                    code => 'let a: NS.Bad._;',
                    options,
                  },
                  // Replace default options instead of merging with extendDefaults: false
                  {
                    code => 'let a: String;',
                    options => [
                      {
                        types: {
                          Number: {
                            message: 'Use number instead.',
                            fixWith: 'number',
                          },
                        },
                        extendDefaults: false,
                      },
                    ],
                  },
                  {
                    code => 'let a: undefined;',
                    options => [
                      {
                        types: {
                          null: {
                            message: 'Use undefined instead.',
                            fixWith: 'undefined',
                          },
                        },
                      },
                    ],
                  },
                  {
                    code => 'let a: null;',
                    options => [
                      {
                        types: {
                          undefined: null,
                        },
                        extendDefaults: false,
                      },
                    ],
                  },
                  {
                    code => 'type Props = {};',
                    options => [
                      {
                        types: {
                          '{}': false,
                        },
                        extendDefaults: true,
                      },
                    ],
                  },
                  'let a: [];',
                ],
                invalid: [
                  {
                    code => 'let a: String;',
                    output: 'let a: string;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        line => 1,
                        column => 8,
                      },
                    ],
                  },
                  {
                    code => 'let a: Object;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'Object',
                          customMessage: " Use '{}' instead.",
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options,
                  },
                  {
                    code => 'let a: Object;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'Object',
                          customMessage: [
                            ' The `Object` type actually means "any non-nullish value", so it is marginally better than `unknown`.',
                            '- If you want a type meaning "any object", you probably want `object` instead.',
                            '- If you want a type meaning "any value", you probably want `unknown` instead.',
                            '- If you really want a type meaning "any non-nullish value", you probably want `NonNullable<unknown>` instead.',
                          ].join('\n'),
                        },
                        line => 1,
                        column => 8,
                        suggestions: [
                          {
                            message_id => 'bannedTypeReplacement',
                            data => { name: 'Object', replacement: 'object' },
                            output: 'let a: object;',
                          },
                          {
                            message_id => 'bannedTypeReplacement',
                            data => { name: 'Object', replacement: 'unknown' },
                            output: 'let a: unknown;',
                          },
                          {
                            message_id => 'bannedTypeReplacement',
                            data => { name: 'Object', replacement: 'NonNullable<unknown>' },
                            output: 'let a: NonNullable<unknown>;',
                          },
                        ],
                      },
                    ],
                    options => [{}],
                  },
                  {
                    code => 'let aa: Foo;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'Foo',
                          customMessage: '',
                        },
                      },
                    ],
                    options => [
                      {
                        types: {
                          Foo: { message: '' },
                        },
                      },
                    ],
                  },
                  {
                    code => 'let b: { c: String };',
                    output: 'let b: { c: string };',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 1,
                        column => 13,
                      },
                    ],
                    options,
                  },
                  {
                    code => 'function foo(a: String) {}',
                    output: 'function foo(a: string) {}',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 1,
                        column => 17,
                      },
                    ],
                    options,
                  },
                  {
                    code => "'a' as String;",
                    output: "'a' as string;",
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options,
                  },
                  {
                    code => 'let c: F;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => { name: 'F', customMessage: '' },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options,
                  },
                  {
                    code => `
              class Foo<F = String> extends Bar<String> implements Baz<Object> {
                constructor(foo: String | Object) {}

                exit(): Array<String> {
                  const foo: String = 1 as String;
                }
              }
                    `,
                    output: `
              class Foo<F = string> extends Bar<string> implements Baz<Object> {
                constructor(foo: string | Object) {}

                exit(): Array<string> {
                  const foo: string = 1 as string;
                }
              }
                    `,
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 2,
                        column => 15,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 2,
                        column => 35,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'Object',
                          customMessage: " Use '{}' instead.",
                        },
                        line => 2,
                        column => 58,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 3,
                        column => 20,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'Object',
                          customMessage: " Use '{}' instead.",
                        },
                        line => 3,
                        column => 29,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => { name: 'Array', customMessage: '' },
                        line => 5,
                        column => 11,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 5,
                        column => 17,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 6,
                        column => 16,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'String',
                          customMessage: ' Use string instead.',
                        },
                        line => 6,
                        column => 30,
                      },
                    ],
                    options,
                  },
                  {
                    code => 'let a: NS.Bad;',
                    output: 'let a: NS.Good;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'NS.Bad',
                          customMessage: ' Use NS.Good instead.',
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options,
                  },
                  {
                    code => `
              let a: NS.Bad<Foo>;
              let b: Foo<NS.Bad>;
                    `,
                    output: `
              let a: NS.Good<Foo>;
              let b: Foo<NS.Good>;
                    `,
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'NS.Bad',
                          customMessage: ' Use NS.Good instead.',
                        },
                        line => 2,
                        column => 8,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'NS.Bad',
                          customMessage: ' Use NS.Good instead.',
                        },
                        line => 3,
                        column => 12,
                      },
                    ],
                    options,
                  },
                  {
                    code => 'let foo: {} = {};',
                    output: 'let foo: object = {};',
                    options => [
                      {
                        types: {
                          '{}': {
                            message: 'Use object instead.',
                            fixWith: 'object',
                          },
                        },
                      },
                    ],
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: '{}',
                          customMessage: ' Use object instead.',
                        },
                        line => 1,
                        column => 10,
                      },
                    ],
                  },
                  {
                    code => noFormat`
              let foo: {} = {};
              let bar: {     } = {};
              let baz: {
              } = {};
                    `,
                    output: `
              let foo: object = {};
              let bar: object = {};
              let baz: object = {};
                    `,
                    options => [
                      {
                        types: {
                          '{   }': {
                            message: 'Use object instead.',
                            fixWith: 'object',
                          },
                        },
                      },
                    ],
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: '{}',
                          customMessage: ' Use object instead.',
                        },
                        line => 2,
                        column => 10,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: '{}',
                          customMessage: ' Use object instead.',
                        },
                        line => 3,
                        column => 10,
                      },
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: '{}',
                          customMessage: ' Use object instead.',
                        },
                        line => 4,
                        column => 10,
                      },
                    ],
                  },
                  {
                    code => 'let a: NS.Bad;',
                    output: 'let a: NS.Good;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'NS.Bad',
                          customMessage: ' Use NS.Good instead.',
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options => [
                      {
                        types: {
                          '  NS.Bad  ': {
                            message: 'Use NS.Good instead.',
                            fixWith: 'NS.Good',
                          },
                        },
                      },
                    ],
                  },
                  {
                    code => noFormat`let a: Foo<   F   >;`,
                    output: `let a: Foo<   T   >;`,
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'F',
                          customMessage: ' Use T instead.',
                        },
                        line => 1,
                        column => 15,
                      },
                    ],
                    options => [
                      {
                        types: {
                          '       F      ': {
                            message: 'Use T instead.',
                            fixWith: 'T',
                          },
                        },
                      },
                    ],
                  },
                  {
                    code => 'type Foo = Bar<any>;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'Bar<any>',
                          customMessage: " Don't use `any` as a type parameter to `Bar`",
                        },
                        line => 1,
                        column => 12,
                      },
                    ],
                    options => [
                      {
                        types: {
                          'Bar<any>': "Don't use `any` as a type parameter to `Bar`",
                        },
                      },
                    ],
                  },
                  {
                    code => noFormat`type Foo = Bar<A,B>;`,
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: 'Bar<A,B>',
                          customMessage: " Don't pass `A, B` as parameters to `Bar`",
                        },
                        line => 1,
                        column => 12,
                      },
                    ],
                    options => [
                      {
                        types: {
                          'Bar<A, B>': "Don't pass `A, B` as parameters to `Bar`",
                        },
                      },
                    ],
                  },
                  {
                    code => 'let a: [];',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: '[]',
                          customMessage: ' `[]` does only allow empty arrays.',
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options => [
                      {
                        types: {
                          '[]': '`[]` does only allow empty arrays.',
                        },
                      },
                    ],
                  },
                  {
                    code => noFormat`let a:  [ ] ;`,
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: '[]',
                          customMessage: ' `[]` does only allow empty arrays.',
                        },
                        line => 1,
                        column => 9,
                      },
                    ],
                    options => [
                      {
                        types: {
                          '[]': '`[]` does only allow empty arrays.',
                        },
                      },
                    ],
                  },
                  {
                    code => 'let a: [];',
                    output: 'let a: any[];',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: '[]',
                          customMessage: ' `[]` does only allow empty arrays.',
                        },
                        line => 1,
                        column => 8,
                      },
                    ],
                    options => [
                      {
                        types: {
                          '[]': {
                            message: '`[]` does only allow empty arrays.',
                            fixWith: 'any[]',
                          },
                        },
                      },
                    ],
                  },
                  {
                    code => 'let a: [[]];',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                        data => {
                          name: '[]',
                          customMessage: ' `[]` does only allow empty arrays.',
                        },
                        line => 1,
                        column => 9,
                      },
                    ],
                    options => [
                      {
                        types: {
                          '[]': '`[]` does only allow empty arrays.',
                        },
                      },
                    ],
                  },
                  {
                    code => 'type Baz = 1 & Foo;',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                      },
                    ],
                    options => [
                      {
                        types: {
                          Foo: { message: '' },
                        },
                      },
                    ],
                  },
                  {
                    code => 'interface Foo extends Bar {}',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                      },
                    ],
                    options => [
                      {
                        types: {
                          Bar: { message: '' },
                        },
                      },
                    ],
                  },
                  {
                    code => 'interface Foo extends Bar, Baz {}',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                      },
                    ],
                    options => [
                      {
                        types: {
                          Bar: { message: '' },
                        },
                      },
                    ],
                  },
                  {
                    code => 'class Foo implements Bar {}',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                      },
                    ],
                    options => [
                      {
                        types: {
                          Bar: { message: '' },
                        },
                      },
                    ],
                  },
                  {
                    code => 'class Foo implements Bar, Baz {}',
                    errors => [
                      {
                        message_id => 'bannedTypeMessage',
                      },
                    ],
                    options => [
                      {
                        types: {
                          Bar: { message: 'Bla' },
                        },
                      },
                    ],
                  },
                  ...objectReduceKey(
                    TYPE_KEYWORDS,
                    (acc: TSESLint.InvalidTestCase<MessageIds, Options>[], key) => {
                      acc.push({
                        code => `function foo(x: ${key}) {}`,
                        errors => [
                          {
                            message_id => 'bannedTypeMessage',
                            data => {
                              name: key,
                              customMessage: '',
                            },
                            line => 1,
                            column => 17,
                          },
                        ],
                        options => [
                          {
                            extendDefaults: false,
                            types: {
                              [key]: null,
                            },
                          },
                        ],
                      });
                      return acc;
                    },
                    [],
                  ),
                ],
            },
        )
    }
}
