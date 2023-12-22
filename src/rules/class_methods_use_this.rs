use std::{collections::HashSet, sync::Arc};

use serde::Deserialize;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};
use tree_sitter_lint_plugin_eslint_builtin::{
    ast_helpers::{get_method_definition_kind, is_class_member_static, MethodDefinitionKind},
    kind::{
        is_literal_kind, ClassBody, ComputedPropertyName, MethodDefinition,
        PrivatePropertyIdentifier, PropertyIdentifier,
    },
    utils::ast_utils,
};

use crate::{
    ast_helpers::{
        get_accessibility_modifier, get_class_has_implements_clause, get_has_override_modifier,
    },
    kind::PublicFieldDefinition,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PublicFields {
    PublicFields,
}

#[derive(Copy, Clone, Deserialize)]
#[serde(untagged)]
enum IgnoreClassesThatImplementAnInterface {
    Bool(bool),
    PublicFields(PublicFields),
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    except_methods: Option<Vec<String>>,
    enforce_for_class_fields: Option<bool>,
    ignore_override_methods: Option<bool>,
    ignore_classes_that_implement_an_interface: Option<IgnoreClassesThatImplementAnInterface>,
}

impl Options {
    fn enforce_for_class_fields(&self) -> bool {
        self.enforce_for_class_fields.unwrap_or(true)
    }

    fn ignore_override_methods(&self) -> bool {
        self.ignore_override_methods.unwrap_or_default()
    }

    fn ignore_classes_that_implement_an_interface(&self) -> IgnoreClassesThatImplementAnInterface {
        self.ignore_classes_that_implement_an_interface
            .unwrap_or(IgnoreClassesThatImplementAnInterface::Bool(false))
    }
}

fn is_public_field(node: Node, context: &QueryMatchContext) -> bool {
    match get_accessibility_modifier(node) {
        None => true,
        Some(accessibility_modifier) => accessibility_modifier.text(context) == "public",
    }
}

#[derive(Debug)]
struct StackItem<'a> {
    member: Option<Node<'a>>,
    class: Option<Node<'a>>,
    uses_this: bool,
}

pub fn class_methods_use_this_rule() -> Arc<dyn Rule> {
    rule! {
        name => "class-methods-use-this",
        languages => [Typescript],
        messages => [
            missing_this => "Expected 'this' to be used by class {{name}}.",
        ],
        options_type => Options,
        state => {
            [per-config]
            enforce_for_class_fields: bool = options.enforce_for_class_fields(),
            except_methods: HashSet<String> = options.except_methods.clone().unwrap_or_default().into_iter().collect(),
            ignore_override_methods: bool = options.ignore_override_methods(),
            ignore_classes_that_implement_an_interface: IgnoreClassesThatImplementAnInterface = options.ignore_classes_that_implement_an_interface(),
            [per-file-run]
            stack: Vec<StackItem<'a>>,
        },
        methods => {
            fn push_context(&mut self, member: Option<Node<'a>>) {
                self.stack.push(match member.filter(|member| {
                    member.parent().unwrap().kind() == ClassBody
                }) {
                    Some(member) => StackItem {
                        member: Some(member),
                        class: Some(member.parent().unwrap().parent().unwrap()),
                        uses_this: false,
                    },
                    None => StackItem {
                        member: None,
                        class: None,
                        uses_this: false,
                    },
                });
            }

            fn enter_function(&mut self, node: Node<'a>) {
                if node.kind() == MethodDefinition {
                    self.push_context(Some(node));
                    return;
                }
                match node.parent().unwrap().kind() {
                    PublicFieldDefinition => self.push_context(Some(node.parent().unwrap())),
                    _ => self.push_context(None),
                }
            }

            fn pop_context(&mut self) -> StackItem<'a> {
                self.stack.pop().unwrap()
            }

            fn is_instance_method(&self, node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> Option<Node<'a>> {
                if is_class_member_static(node, context) {
                    return None;
                }
                if node.kind() == MethodDefinition &&
                    get_method_definition_kind(node, context) == MethodDefinitionKind::Constructor {
                    return None;
                }
                if node.kind() == PublicFieldDefinition && !self.enforce_for_class_fields {
                    return None;
                }

                Some(node.field("name"))
            }

            fn is_included_instance_method(&self, node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
                let Some(name_node) = self.is_instance_method(node, context) else {
                    return false;
                };
                if name_node.kind() == ComputedPropertyName || self.except_methods.is_empty() {
                    return true;
                }

                let name = if is_literal_kind(name_node.kind()) {
                    ast_utils::get_static_string_value(name_node, context).unwrap()
                } else {
                    match name_node.kind() {
                        PrivatePropertyIdentifier | PropertyIdentifier => name_node.text(context),
                        _ => "".into(),
                    }
                };

                !self.except_methods.contains(&*name)
            }

            fn exit_function(&mut self, node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
                let stack_context = self.pop_context();
                let Some(stack_context_member) = stack_context.member.filter(|&stack_context_member| {
                    !(stack_context.uses_this ||
                        self.ignore_override_methods && get_has_override_modifier(stack_context_member) ||
                        match self.ignore_classes_that_implement_an_interface {
                            IgnoreClassesThatImplementAnInterface::Bool(true) =>
                                get_class_has_implements_clause(stack_context.class.unwrap()),
                            IgnoreClassesThatImplementAnInterface::PublicFields(_) =>
                                get_class_has_implements_clause(stack_context.class.unwrap()) &&
                                    is_public_field(stack_context_member, context),
                            _ => false,
                        })
                }) else {
                    return;
                };

                if !self.is_included_instance_method(stack_context_member, context) {
                    return;
                }

                context.report(violation! {
                    node => node,
                    range => ast_utils::get_function_head_range(node),
                    message_id => "missing_this",
                    data => {
                        name => ast_utils::get_function_name_with_kind(node, context),
                    }
                });
            }
        },
        listeners => [
            r#"
                function,
                generator_function,
                method_definition
            "# => |node, context| {
                self.enter_function(node);
            },
            r#"
                function:exit,
                generator_function:exit,
                method_definition:exit
            "# => |node, context| {
                self.exit_function(node, context);
            },
            r#"
                (function_declaration) @c
                (generator_function_declaration) @c
                (public_field_definition
                  value: (_) @c
                )
                (class_static_block) @c
            "# => |node, context| {
                self.push_context(None);
            },
            r#"
                function_declaration:exit,
                generator_function_declaration:exit,
                public_field_definition:exit,
                class_static_block:exit
            "# => |node, context| {
                self.pop_context();
            },
            r#"
                (this) @c
                (super) @c
            "# => |node, context| {
                if let Some(last) = self.stack.last_mut() {
                    last.uses_this = true;
                }
            },
            r#"
                (public_field_definition
                  value: (arrow_function) @c
                )
            "# => |node, context| {
                if !self.enforce_for_class_fields {
                    return;
                }

                self.enter_function(node);
            },
            r#"arrow_function:exit"# => |node, context| {
                if !self.enforce_for_class_fields {
                    return;
                }
                if !node.parent().matches(|parent| {
                    parent.kind() == PublicFieldDefinition
                }) {
                    return;
                }

                self.exit_function(node, context);
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_class_methods_use_this_rule() {
        RuleTester::run(
            class_methods_use_this_rule(),
            rule_tests! {
                valid => [
                    { code => "class A { constructor() {} }", environment => { ecma_version => 6 } },
                    { code => "class A { foo() {this} }", environment => { ecma_version => 6 } },
                    { code => "class A { foo() {this.bar = 'bar';} }", environment => { ecma_version => 6 } },
                    { code => "class A { foo() {bar(this);} }", environment => { ecma_version => 6 } },
                    { code => "class A extends B { foo() {super.foo();} }", environment => { ecma_version => 6 } },
                    { code => "class A { foo() { if(true) { return this; } } }", environment => { ecma_version => 6 } },
                    { code => "class A { static foo() {} }", environment => { ecma_version => 6 } },
                    { code => "({ a(){} });", environment => { ecma_version => 6 } },
                    { code => "class A { foo() { () => this; } }", environment => { ecma_version => 6 } },
                    { code => "({ a: function () {} });", environment => { ecma_version => 6 } },
                    { code => "class A { foo() {this} bar() {} }", options => { except_methods => ["bar"] }, environment => { ecma_version => 6 } },
                    { code => "class A { \"foo\"() { } }", options => { except_methods => ["foo"] }, environment => { ecma_version => 6 } },
                    { code => "class A { 42() { } }", options => { except_methods => ["42"] }, environment => { ecma_version => 6 } },
                    { code => "class A { foo = function() {this} }", environment => { ecma_version => 2022 } },
                    { code => "class A { foo = () => {this} }", environment => { ecma_version => 2022 } },
                    { code => "class A { foo = () => {super.toString} }", environment => { ecma_version => 2022 } },
                    { code => "class A { static foo = function() {} }", environment => { ecma_version => 2022 } },
                    { code => "class A { static foo = () => {} }", environment => { ecma_version => 2022 } },
                    { code => "class A { #bar() {} }", options => { except_methods => ["#bar"] }, environment => { ecma_version => 2022 } },
                    { code => "class A { foo = function () {} }", options => { enforce_for_class_fields => false }, environment => { ecma_version => 2022 } },
                    { code => "class A { foo = () => {} }", options => { enforce_for_class_fields => false }, environment => { ecma_version => 2022 } },
                    { code => "class A { foo() { return class { [this.foo] = 1 }; } }", environment => { ecma_version => 2022 } },
                    { code => "class A { static {} }", environment => { ecma_version => 2022 } }
                ],
                invalid => [
                    {
                        code => "class A { foo() {} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ],
                    },
                    {
                        code => "class A { foo() {/**this**/} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {var a = function () {this};} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {var a = function () {var b = function(){this}};} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {window.this} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {that.this = 'this';} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() { () => undefined; } }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {} bar() {} }",
                        options => { except_methods => ["bar"] },
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {} hasOwnProperty() {} }",
                        options => { except_methods => ["foo"] },
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 20, message_id => "missing_this", data => { name => "method 'hasOwnProperty'" } }
                        ]
                    },
                    {
                        code => "class A { [foo]() {} }",
                        options => { except_methods => ["foo"] },
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 11, message_id => "missing_this", data => { name => "method" } }
                        ]
                    },
                    {
                        code => "class A { #foo() { } foo() {} #bar() {} }",
                        options => { except_methods => ["#foo"] },
                        environment => { ecma_version => 2022 },
                        errors => [
                            { type => MethodDefinition, line => 1, column => 22, message_id => "missing_this", data => { name => "method 'foo'" } },
                            { type => MethodDefinition, line => 1, column => 31, message_id => "missing_this", data => { name => "private method #bar" } }
                        ]
                    },
                    {
                        code => "class A { foo(){} 'bar'(){} 123(){} [`baz`](){} [a](){} [f(a)](){} get quux(){} set[a](b){} *quuux(){} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, type => MethodDefinition, column => 11 },
                            { message_id => "missing_this", data => { name => "method 'bar'" }, type => MethodDefinition, column => 19 },
                            { message_id => "missing_this", data => { name => "method '123'" }, type => MethodDefinition, column => 29 },
                            { message_id => "missing_this", data => { name => "method 'baz'" }, type => MethodDefinition, column => 37 },
                            { message_id => "missing_this", data => { name => "method" }, type => MethodDefinition, column => 49 },
                            { message_id => "missing_this", data => { name => "method" }, type => MethodDefinition, column => 57 },
                            { message_id => "missing_this", data => { name => "getter 'quux'" }, type => MethodDefinition, column => 68 },
                            { message_id => "missing_this", data => { name => "setter" }, type => MethodDefinition, column => 81 },
                            { message_id => "missing_this", data => { name => "generator method 'quuux'" }, type => MethodDefinition, column => 93 }
                        ]
                    },
                    {
                        code => "class A { foo = function() {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 25 }
                        ],
                    },
                    {
                        code => "class A { foo = () => {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 17 }
                        ]
                    },
                    {
                        code => "class A { #foo = function() {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private method #foo" }, column => 11, end_column => 26 }
                        ]
                    },
                    {
                        code => "class A { #foo = () => {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private method #foo" }, column => 11, end_column => 18 }
                        ]
                    },
                    {
                        code => "class A { #foo() {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private method #foo" }, column => 11, end_column => 15 }
                        ]
                    },
                    {
                        code => "class A { get #foo() {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private getter #foo" }, column => 11, end_column => 19 }
                        ]
                    },
                    {
                        code => "class A { set #foo(x) {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private setter #foo" }, column => 11, end_column => 19 }
                        ]
                    },
                    {
                        code => "class A { foo () { return class { foo = this }; } }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 15 }
                        ]
                    },
                    {
                        code => "class A { foo () { return function () { foo = this }; } }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 15 }
                        ]
                    },
                    {
                        code => "class A { foo () { return class { static { this; } } } }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 15 }
                        ]
                    }
                ]
            },
        )
    }

    #[test]
    fn test_class_methods_use_this_rule_typescript() {
        RuleTester::run(
            class_methods_use_this_rule(),
            rule_tests! {
                valid => [
                  {
                    code => r#"
              class Foo implements Bar {
                method() {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => true },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                get getter() {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => true },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                set setter() {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => true },
                  },
                  {
                    code => r#"
              class Foo {
                override method() {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                private override method() {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                protected override method() {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                override get getter(): number {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                private override get getter(): number {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                protected override get getter(): number {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                override set setter(v: number) {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                private override set setter(v: number) {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                protected override set setter(v: number) {}
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                override method() {}
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => true,
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                private override method() {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        // But overridden properties should be ignored.
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                protected override method() {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        // But overridden properties should be ignored.
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                override get getter(): number {}
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => true,
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                private override get getter(): number {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        // But overridden properties should be ignored.
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                protected override get getter(): number {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        // But overridden properties should be ignored.
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                override set setter(v: number) {}
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => true,
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                private override set setter(v: number) {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        // But overridden properties should be ignored.
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                protected override set setter(v: number) {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        // But overridden properties should be ignored.
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                property = () => {};
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => true },
                  },
                  {
                    code => r#"
              class Foo {
                override property = () => {};
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                private override property = () => {};
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo {
                protected override property = () => {};
              }
                    "#,
                    options => { ignore_override_methods => true },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                override property = () => {};
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => true,
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                property = () => {};
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => false,
                        enforce_for_class_fields => false,
                      },
                  },
                  {
                    code => r#"
              class Foo {
                override property = () => {};
              }
                    "#,
                    options =>
                      {
                        ignore_override_methods => false,
                        enforce_for_class_fields => false,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                private override property = () => {};
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should check only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        // But overridden properties should be ignored.
                        ignore_override_methods => true,
                      },
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                protected override property = () => {};
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should check only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        // But overridden properties should be ignored.
                        ignore_override_methods => true,
                      },
                  },
                ],
                invalid => [
                  {
                    code => r#"
              class Foo {
                method() {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                private method() {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                protected method() {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                #method() {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                get getter(): number {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                private get getter(): number {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                protected get getter(): number {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                get #getter(): number {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                set setter(b: number) {}
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => false,
                        ignore_override_methods => false,
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                private set setter(b: number) {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                protected set setter(b: number) {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                set #setter(b: number) {}
              }
                    "#,
                    options => {},
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                method() {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                #method() {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                private method() {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                protected method() {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                get getter(): number {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                get #getter(): number {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                private get getter(): number {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                protected get getter(): number {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                set setter(v: number) {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                set #setter(v: number) {}
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                private set setter(v: number) {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        ignore_override_methods => false,
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                protected set setter(v: number) {}
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                        ignore_override_methods => false,
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                override method() {}
              }
                    "#,
                    options => { ignore_override_methods => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                override get getter(): number {}
              }
                    "#,
                    options => { ignore_override_methods => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                override set setter(v: number) {}
              }
                    "#,
                    options => { ignore_override_methods => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                override method() {}
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => false,
                        ignore_override_methods => false,
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                override get getter(): number {}
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => false,
                        ignore_override_methods => false,
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                override set setter(v: number) {}
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => false,
                        ignore_override_methods => false,
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                property = () => {};
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                #property = () => {};
              }
                    "#,
                    options => { ignore_classes_that_implement_an_interface => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo {
                override property = () => {};
              }
                    "#,
                    options => { ignore_override_methods => false },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                override property = () => {};
              }
                    "#,
                    options =>
                      {
                        ignore_classes_that_implement_an_interface => false,
                        ignore_override_methods => false,
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                private property = () => {};
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                  {
                    code => r#"
              class Foo implements Bar {
                protected property = () => {};
              }
                    "#,
                    options =>
                      {
                        // _interface_ cannot have `private`/`protected` modifier on members.
                        // We should ignore only public members.
                        ignore_classes_that_implement_an_interface => "public-fields",
                      },
                    errors => [
                      {
                        message_id => "missing_this",
                      },
                    ],
                  },
                ],
            },
        )
    }
}
