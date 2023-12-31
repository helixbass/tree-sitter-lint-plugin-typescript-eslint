use std::{borrow::Cow, sync::Arc};

use serde::Deserialize;
use squalid::{EverythingExt, OptionExt};
use tree_sitter_lint::{
    range_between_ends, range_between_starts, rule, tree_sitter::Node,
    tree_sitter_grep::SupportedLanguage, violation, NodeExt, QueryMatchContext, Rule,
};
use tree_sitter_lint_plugin_eslint_builtin::kind::{Identifier, Undefined};

use crate::{
    ast_helpers::NodeExtTypescript,
    kind::{
        ArrayType, ConstructorType, FunctionType, GenericType, InferType, IntersectionType,
        LiteralType, NestedTypeIdentifier, PredefinedType, ReadonlyType, ThisType, TypeIdentifier,
        UnionType,
    },
};

fn is_simple_type<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
    match node.kind() {
        Identifier | PredefinedType | ArrayType | ThisType | TypeIdentifier
        | NestedTypeIdentifier => true,
        LiteralType => {
            node.first_non_comment_named_child(SupportedLanguage::Javascript)
                .kind()
                == Undefined
        }
        GenericType => {
            node.field("name")
                .thrush(|name| name.kind() == TypeIdentifier && name.text(context) == "Array")
                && node
                    .field("type_arguments")
                    .non_comment_named_children(SupportedLanguage::Javascript)
                    .thrush(|mut type_arguments| {
                        let Some(first_type_argument) = type_arguments.next() else {
                            return true;
                        };
                        if type_arguments.next().is_some() {
                            return false;
                        }
                        is_simple_type(first_type_argument, context)
                    })
        }
        _ => false,
    }
}

fn type_needs_parentheses<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
    match node.kind() {
        GenericType => type_needs_parentheses(node.field("name"), context),
        UnionType | FunctionType | IntersectionType | InferType | ConstructorType => true,
        TypeIdentifier => node.text(context) == "ReadonlyArray",
        _ => false,
    }
}

fn get_message_type<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> Cow<'a, str> {
    if is_simple_type(node, context) {
        node.text(context)
    } else {
        "T".into()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ArrayOption {
    Array,
    Generic,
    ArraySimple,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    default: Option<ArrayOption>,
    readonly: Option<ArrayOption>,
}

impl Options {
    pub fn default(&self) -> ArrayOption {
        self.default.unwrap_or(ArrayOption::Array)
    }

    pub fn readonly(&self) -> ArrayOption {
        self.readonly.unwrap_or_else(|| self.default())
    }
}

pub fn array_type_rule() -> Arc<dyn Rule> {
    rule! {
        name => "array-type",
        languages => [Typescript],
        messages => [
            error_string_generic => "Array type using '{{readonly_prefix}}{{type}}[]' is forbidden. Use '{{class_name}}<{{type}}>' instead.",
            error_string_array => "Array type using '{{class_name}}<{{type}}>' is forbidden. Use '{{readonly_prefix}}{{type}}[]' instead.",
            error_string_array_simple => "Array type using '{{class_name}}<{{type}}>' is forbidden for simple types. Use '{{readonly_prefix}}{{type}}[]' instead.",
            error_string_generic_simple => "Array type using '{{readonly_prefix}}{{type}}[]' is forbidden for non-simple types. Use '{{class_name}}<{{type}}>' instead.",
        ],
        fixable => true,
        allow_self_conflicting_fixes => true,
        options_type => Options,
        state => {
            [per-config]
            default_option: ArrayOption = options.default(),
            readonly_option: ArrayOption = options.readonly(),
        },
        methods => {
            fn check_array_with_no_generic_params(&self, node_to_report: Node<'a>, inner_node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
                let is_readonly_array_type = inner_node.text(context) == "ReadonlyArray";
                let current_option = if is_readonly_array_type {
                    self.readonly_option
                } else {
                    self.default_option
                };

                if current_option == ArrayOption::Generic {
                    return;
                }

                let readonly_prefix =  if is_readonly_array_type {
                    "readonly "
                } else {
                    ""
                };
                let message_id = if current_option == ArrayOption::Array {
                    "error_string_array"
                } else {
                    "error_string_array_simple"
                };

                context.report(violation! {
                    node => node_to_report,
                    message_id => message_id,
                    data => {
                        class_name => if is_readonly_array_type {
                            "ReadonlyArray"
                        } else {
                            "Array"
                        },
                        readonly_prefix => readonly_prefix,
                        type_ => "any",
                    },
                    fix => |fixer| {
                        fixer.replace_text(node_to_report, format!("{readonly_prefix}any[]"));
                    }
                });
            }
        },
        listeners => [
            r#"
              (array_type) @c
            "# => |node, context| {
                let is_readonly = node.parent().matches(|parent| parent.kind() == ReadonlyType);
                let item_type_node = node.first_non_comment_named_child(SupportedLanguage::Javascript);

                let current_option = if is_readonly {
                    self.readonly_option
                } else {
                    self.default_option
                };

                if current_option == ArrayOption::Array ||
                    current_option == ArrayOption::ArraySimple &&
                        is_simple_type(item_type_node, context) {
                    return;
                }

                let message_id = if current_option == ArrayOption::Generic {
                    "error_string_generic"
                } else {
                    "error_string_generic_simple"
                };
                let error_node = if is_readonly {
                    node.parent().unwrap()
                } else {
                    node
                };

                context.report(violation! {
                    node => error_node,
                    message_id => message_id,
                    data => {
                        class_name => if is_readonly {
                            "ReadonlyArray"
                        } else {
                            "Array"
                        },
                        readonly_prefix => if is_readonly {
                            "readonly "
                        } else {
                            ""
                        },
                        type => get_message_type(item_type_node, context).into_owned(),
                    },
                    fix => |fixer| {
                        let type_node = item_type_node.skip_parenthesized_types();
                        let array_type = if is_readonly {
                            "ReadonlyArray"
                        } else {
                            "Array"
                        };

                        // TODO: should check/revisit whether these are
                        // guaranteed to both be applied (vs eg if only
                        // one doesn't conflict with fixes from other rules
                        // would it get applied) and if not then eg expose
                        // an API that "couples" them?
                        fixer.replace_text_range(
                            range_between_starts(error_node.range(), type_node.range()),
                            format!("{array_type}<"),
                        );
                        fixer.replace_text_range(
                            range_between_ends(type_node.range(), error_node.range()),
                            ">",
                        );
                    }
                });
            },
            r#"(
              (type_identifier) @c (#match? @c "^(?:Readonly)?Array$")
            )"# => |node, context| {
                if node.parent().matches(|parent| parent.kind() == GenericType) {
                    return;
                }

                self.check_array_with_no_generic_params(node, node, context);
            },
            r#"
              (generic_type
                name: (type_identifier) @inner (#match? @inner "^(?:Readonly)?Array$")
              ) @outer
            "# => |captures, context| {
                let node = captures["outer"];
                let num_type_arguments = node.field("type_arguments").num_non_comment_named_children(SupportedLanguage::Javascript);
                let inner_node = node.field("name");
                if num_type_arguments == 0 {
                    return self.check_array_with_no_generic_params(node, inner_node, context);
                }

                if num_type_arguments != 1 {
                    return;
                }
                let first_type_argument = node.field("type_arguments").non_comment_named_children(SupportedLanguage::Javascript).next().unwrap();

                let is_readonly_array_type = inner_node.text(context) == "ReadonlyArray";
                let current_option = if is_readonly_array_type {
                    self.readonly_option
                } else {
                    self.default_option
                };
                if current_option == ArrayOption::Generic {
                    return;
                }

                if current_option == ArrayOption::ArraySimple && !is_simple_type(first_type_argument, context) {
                    return;
                }

                let readonly_prefix =  if is_readonly_array_type {
                    "readonly "
                } else {
                    ""
                };
                let message_id = if current_option == ArrayOption::Array {
                    "error_string_array"
                } else {
                    "error_string_array_simple"
                };

                let type_ = first_type_argument.skip_parenthesized_types();
                let type_parens = type_needs_parentheses(type_, context);
                let parent_parens = !readonly_prefix.is_empty() &&
                    node.parent().matches(|parent| parent.kind() == ArrayType);


                context.report(violation! {
                    node => node,
                    message_id => message_id,
                    data => {
                        class_name => if is_readonly_array_type {
                            "ReadonlyArray"
                        } else {
                            "Array"
                        },
                        readonly_prefix => readonly_prefix,
                        type_ => get_message_type(type_, context),
                    },
                    fix => |fixer| {
                        let start = format!(
                            "{}{readonly_prefix}{}",
                            if parent_parens {
                                "("
                            } else {
                                ""
                            },
                            if type_parens {
                                "("
                            } else {
                                ""
                            },
                        );
                        let end = format!(
                            "{}[]{}",
                            if type_parens {
                                ")"
                            } else {
                                ""
                            },
                            if parent_parens {
                                ")"
                            } else {
                                ""
                            },
                        );

                        fixer.replace_text_range(
                            range_between_starts(node.range(), type_.range()),
                            start,
                        );
                        fixer.replace_text_range(
                            range_between_ends(type_.range(), node.range()),
                            end,
                        );
                    }
                });
            }
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_array_type_rule() {
        // TODO: there are other tests in the typescript-eslint version
        RuleTester::run(
            array_type_rule(),
            rule_tests! {
                valid => [
                    // Base cases from https://github.com/typescript-eslint/typescript-eslint/issues/2323#issuecomment-663977655
                    {
                      code => "let a: number[] = [];",
                      options => { default => "array" },
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      options => { default => "array" },
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      options => { default => "array" },
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      options => { default => "array" },
                    },
                    {
                      code => "let a: number[] = [];",
                      options => { default => "array", readonly => "array" },
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      options => { default => "array", readonly => "array" },
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      options => { default => "array", readonly => "array" },
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      options => { default => "array", readonly => "array" },
                    },
                    {
                      code => "let a: number[] = [];",
                      options => { default => "array", readonly => "array-simple" },
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      options => { default => "array", readonly => "array-simple" },
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      options => { default => "array", readonly => "array-simple" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array", readonly => "array-simple" },
                    },
                    {
                      code => "let a: number[] = [];",
                      options => { default => "array", readonly => "generic" },
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      options => { default => "array", readonly => "generic" },
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      options => { default => "array", readonly => "generic" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array", readonly => "generic" },
                    },
                    {
                      code => "let a: number[] = [];",
                      options => { default => "array-simple" },
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      options => { default => "array-simple" },
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      options => { default => "array-simple" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array-simple" },
                    },
                    {
                      code => "let a: number[] = [];",
                      options => { default => "array-simple", readonly => "array" },
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      options => { default => "array-simple", readonly => "array" },
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      options => { default => "array-simple", readonly => "array" },
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      options => { default => "array-simple", readonly => "array" },
                    },
                    {
                      code => "let a: number[] = [];",
                      options => { default => "array-simple", readonly => "array-simple" },
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      options => { default => "array-simple", readonly => "array-simple" },
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      options => { default => "array-simple", readonly => "array-simple" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array-simple", readonly => "array-simple" },
                    },
                    {
                      code => "let a: number[] = [];",
                      options => { default => "array-simple", readonly => "generic" },
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      options => { default => "array-simple", readonly => "generic" },
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      options => { default => "array-simple", readonly => "generic" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array-simple", readonly => "generic" },
                    },
                    {
                      code => "let a: Array<number> = [];",
                      options => { default => "generic" },
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      options => { default => "generic" },
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      options => { default => "generic" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "generic" },
                    },
                    {
                      code => "let a: Array<number> = [];",
                      options => { default => "generic", readonly => "generic" },
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      options => { default => "generic", readonly => "generic" },
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      options => { default => "generic", readonly => "generic" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "generic", readonly => "generic" },
                    },
                    {
                      code => "let a: Array<number> = [];",
                      options => { default => "generic", readonly => "array" },
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      options => { default => "generic", readonly => "array" },
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      options => { default => "generic", readonly => "array" },
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      options => { default => "generic", readonly => "array" },
                    },
                    {
                      code => "let a: Array<number> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      options => { default => "generic", readonly => "array-simple" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                    },
                    {
                      code => "let a: Array<bigint> = [];",
                      options => { default => "generic", readonly => "array" },
                    },
                    {
                      code => "let a: readonly bigint[] = [];",
                      options => { default => "generic", readonly => "array" },
                    },
                    {
                      code => "let a: readonly (string | bigint)[] = [];",
                      options => { default => "generic", readonly => "array" },
                    },
                    {
                      code => "let a: Array<bigint> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                    },
                    {
                      code => "let a: Array<string | bigint> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                    },
                    {
                      code => "let a: readonly bigint[] = [];",
                      options => { default => "generic", readonly => "array-simple" },
                    },
                    {
                      code => "let a: ReadonlyArray<string | bigint> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                    },

                    // End of base cases

                    {
                      code => "let a = new Array();",
                      options => { default => "array" },
                    },
                    {
                      code => "let a: { foo: Bar[] }[] = [];",
                      options => { default => "array" },
                    },
                    {
                      code => "function foo(a: Array<Bar>): Array<Bar> {}",
                      options => { default => "generic" },
                    },
                    {
                      code => "let yy: number[][] = [[4, 5], [6]];",
                      options => { default => "array-simple" },
                    },
                    {
                      code => r#"
                function fooFunction(foo: Array<ArrayClass<string>>) {
                  return foo.map(e => e.foo);
                }
                      "#,
                      options => { default => "array-simple" },
                    },
                    {
                      code => r#"
                function bazFunction(baz: Arr<ArrayClass<String>>) {
                  return baz.map(e => e.baz);
                }
                      "#,
                      options => { default => "array-simple" },
                    },
                    {
                      code => "let fooVar: Array<(c: number) => number>;",
                      options => { default => "array-simple" },
                    },
                    {
                      code => "type fooUnion = Array<string | number | boolean>;",
                      options => { default => "array-simple" },
                    },
                    {
                      code => "type fooIntersection = Array<string & number>;",
                      options => { default => "array-simple" },
                    },
                    {
                      code => r#"
                namespace fooName {
                  type BarType = { bar: string };
                  type BazType<T> = Arr<T>;
                }
                      "#,
                      options => { default => "array-simple" },
                    },
                    {
                      code => r#"
                interface FooInterface {
                  ".bar": { baz: string[] };
                }
                      "#,
                      options => { default => "array-simple" },
                    },
                    {
                      code => "let yy: number[][] = [[4, 5], [6]];",
                      options => { default => "array" },
                    },
                    {
                      code => "let ya = [[1, '2']] as [number, string][];",
                      options => { default => "array" },
                    },
                    {
                      code => r#"
                function barFunction(bar: ArrayClass<String>[]) {
                  return bar.map(e => e.bar);
                }
                      "#,
                      options => { default => "array" },
                    },
                    {
                      code => r#"
                function bazFunction(baz: Arr<ArrayClass<String>>) {
                  return baz.map(e => e.baz);
                }
                      "#,
                      options => { default => "array" },
                    },
                    {
                      code => "let barVar: ((c: number) => number)[];",
                      options => { default => "array" },
                    },
                    {
                      code => "type barUnion = (string | number | boolean)[];",
                      options => { default => "array" },
                    },
                    {
                      code => "type barIntersection = (string & number)[];",
                      options => { default => "array" },
                    },
                    {
                      code => r#"
                interface FooInterface {
                  '.bar': { baz: string[] };
                }
                      "#,
                      options => { default => "array" },
                    },
                    {
                      // https://github.com/typescript-eslint/typescript-eslint/issues/172
                      code => "type Unwrap<T> = T extends (infer E)[] ? E : T;",
                      options => { default => "array" },
                    },
                    {
                      code => "let xx: Array<Array<number>> = [[1, 2], [3]];",
                      options => { default => "generic" },
                    },
                    {
                      code => "type Arr<T> = Array<T>;",
                      options => { default => "generic" },
                    },
                    {
                      code => r#"
                function fooFunction(foo: Array<ArrayClass<string>>) {
                  return foo.map(e => e.foo);
                }
                      "#,
                      options => { default => "generic" },
                    },
                    {
                      code => r#"
                function bazFunction(baz: Arr<ArrayClass<String>>) {
                  return baz.map(e => e.baz);
                }
                      "#,
                      options => { default => "generic" },
                    },
                    {
                      code => "let fooVar: Array<(c: number) => number>;",
                      options => { default => "generic" },
                    },
                    {
                      code => "type fooUnion = Array<string | number | boolean>;",
                      options => { default => "generic" },
                    },
                    {
                      code => "type fooIntersection = Array<string & number>;",
                      options => { default => "generic" },
                    },
                    {
                      // https://github.com/typescript-eslint/typescript-eslint/issues/172
                      code => "type Unwrap<T> = T extends Array<infer E> ? E : T;",
                      options => { default => "generic" },
                    },

                    // nested readonly
                    {
                      code => "let a: ReadonlyArray<number[]> = [[]];",
                      options => { default => "array", readonly => "generic" },
                    },
                    {
                      code => "let a: readonly Array<number>[] = [[]];",
                      options => { default => "generic", readonly => "array" },
                    },
                  ],
                  invalid => [
                    // Base cases from https://github.com/typescript-eslint/typescript-eslint/issues/2323#issuecomment-663977655
                    {
                      code => "let a: Array<number> = [];",
                      output => "let a: number[] = [];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      output => "let a: (string | number)[] = [];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      output => "let a: readonly number[] = [];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      output => "let a: readonly (string | number)[] = [];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<number> = [];",
                      output => "let a: number[] = [];",
                      options => { default => "array", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      output => "let a: (string | number)[] = [];",
                      options => { default => "array", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      output => "let a: readonly number[] = [];",
                      options => { default => "array", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      output => "let a: readonly (string | number)[] = [];",
                      options => { default => "array", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<number> = [];",
                      output => "let a: number[] = [];",
                      options => { default => "array", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      output => "let a: (string | number)[] = [];",
                      options => { default => "array", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      output => "let a: readonly number[] = [];",
                      options => { default => "array", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      output => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<number> = [];",
                      output => "let a: number[] = [];",
                      options => { default => "array", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<string | number> = [];",
                      output => "let a: (string | number)[] = [];",
                      options => { default => "array", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      output => "let a: ReadonlyArray<number> = [];",
                      options => { default => "array", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      output => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<number> = [];",
                      output => "let a: number[] = [];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      output => "let a: Array<string | number> = [];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      output => "let a: readonly number[] = [];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      output => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<number> = [];",
                      output => "let a: number[] = [];",
                      options => { default => "array-simple", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      output => "let a: Array<string | number> = [];",
                      options => { default => "array-simple", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      output => "let a: readonly number[] = [];",
                      options => { default => "array-simple", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      output => "let a: readonly (string | number)[] = [];",
                      options => { default => "array-simple", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<number> = [];",
                      output => "let a: number[] = [];",
                      options => { default => "array-simple", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      output => "let a: Array<string | number> = [];",
                      options => { default => "array-simple", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      output => "let a: readonly number[] = [];",
                      options => { default => "array-simple", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      output => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array-simple", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<number> = [];",
                      output => "let a: number[] = [];",
                      options => { default => "array-simple", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      output => "let a: Array<string | number> = [];",
                      options => { default => "array-simple", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      output => "let a: ReadonlyArray<number> = [];",
                      options => { default => "array-simple", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      output => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "array-simple", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: number[] = [];",
                      output => "let a: Array<number> = [];",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      output => "let a: Array<string | number> = [];",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      output => "let a: ReadonlyArray<number> = [];",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      output => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: number[] = [];",
                      output => "let a: Array<number> = [];",
                      options => { default => "generic", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      output => "let a: Array<string | number> = [];",
                      options => { default => "generic", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      output => "let a: readonly number[] = [];",
                      options => { default => "generic", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<string | number> = [];",
                      output => "let a: readonly (string | number)[] = [];",
                      options => { default => "generic", readonly => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: number[] = [];",
                      output => "let a: Array<number> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      output => "let a: Array<string | number> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<number> = [];",
                      output => "let a: readonly number[] = [];",
                      options => { default => "generic", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      output => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: number[] = [];",
                      output => "let a: Array<number> = [];",
                      options => { default => "generic", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | number)[] = [];",
                      output => "let a: Array<string | number> = [];",
                      options => { default => "generic", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly number[] = [];",
                      output => "let a: ReadonlyArray<number> = [];",
                      options => { default => "generic", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "number",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | number)[] = [];",
                      output => "let a: ReadonlyArray<string | number> = [];",
                      options => { default => "generic", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: bigint[] = [];",
                      output => "let a: Array<bigint> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "bigint" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | bigint)[] = [];",
                      output => "let a: Array<string | bigint> = [];",
                      options => { default => "generic", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: ReadonlyArray<bigint> = [];",
                      output => "let a: readonly bigint[] = [];",
                      options => { default => "generic", readonly => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "bigint",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: (string | bigint)[] = [];",
                      output => "let a: Array<string | bigint> = [];",
                      options => { default => "generic", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly bigint[] = [];",
                      output => "let a: ReadonlyArray<bigint> = [];",
                      options => { default => "generic", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "bigint",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let a: readonly (string | bigint)[] = [];",
                      output => "let a: ReadonlyArray<string | bigint> = [];",
                      options => { default => "generic", readonly => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },

                    // End of base cases

                    {
                      code => "let a: { foo: Array<Bar> }[] = [];",
                      output => "let a: { foo: Bar[] }[] = [];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "Bar" },
                          line => 1,
                          column => 15,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<{ foo: Bar[] }> = [];",
                      output => "let a: Array<{ foo: Array<Bar> }> = [];",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "Bar" },
                          line => 1,
                          column => 21,
                        },
                      ],
                    },
                    {
                      code => "let a: Array<{ foo: Foo | Bar[] }> = [];",
                      output => "let a: Array<{ foo: Foo | Array<Bar> }> = [];",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "Bar" },
                          line => 1,
                          column => 27,
                        },
                      ],
                    },
                    {
                      code => "function foo(a: Array<Bar>): Array<Bar> {}",
                      output => "function foo(a: Bar[]): Bar[] {}",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "Bar" },
                          line => 1,
                          column => 17,
                        },
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "Bar" },
                          line => 1,
                          column => 30,
                        },
                      ],
                    },
                    {
                      code => "let x: Array<undefined> = [undefined] as undefined[];",
                      output => "let x: undefined[] = [undefined] as undefined[];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "undefined" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let y: string[] = <Array<string>>['2'];",
                      output => "let y: string[] = <string[]>['2'];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "string" },
                          line => 1,
                          column => 20,
                        },
                      ],
                      supported_language_languages => [Typescript],
                    },
                    {
                      code => "let z: Array = [3, '4'];",
                      output => "let z: any[] = [3, '4'];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "any" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let ya = [[1, '2']] as [number, string][];",
                      output => "let ya = [[1, '2']] as Array<[number, string]>;",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 24,
                        },
                      ],
                    },
                    {
                      code => "type Arr<T> = Array<T>;",
                      output => "type Arr<T> = T[];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 15,
                        },
                      ],
                    },
                    {
                      code => r#"
// Ignore user defined aliases
let yyyy: Arr<Array<Arr<string>>[]> = [[[['2']]]];
                      "#,
                      output => r#"
// Ignore user defined aliases
let yyyy: Arr<Array<Array<Arr<string>>>> = [[[['2']]]];
                      "#,
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 3,
                          column => 15,
                        },
                      ],
                    },
                    {
                      code => r#"
interface ArrayClass<T> {
  foo: Array<T>;
  bar: T[];
  baz: Arr<T>;
  xyz: this[];
}
                      "#,
                      output => r#"
interface ArrayClass<T> {
  foo: T[];
  bar: T[];
  baz: Arr<T>;
  xyz: this[];
}
                      "#,
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 3,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => r#"
function barFunction(bar: ArrayClass<String>[]) {
  return bar.map(e => e.bar);
}
                      "#,
                      output => r#"
function barFunction(bar: Array<ArrayClass<String>>) {
  return bar.map(e => e.bar);
}
                      "#,
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 2,
                          column => 27,
                        },
                      ],
                    },
                    {
                      code => "let barVar: ((c: number) => number)[];",
                      output => "let barVar: Array<(c: number) => number>;",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 13,
                        },
                      ],
                    },
                    {
                      code => "type barUnion = (string | number | boolean)[];",
                      output => "type barUnion = Array<string | number | boolean>;",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 17,
                        },
                      ],
                    },
                    {
                      code => "type barIntersection = (string & number)[];",
                      output => "type barIntersection = Array<string & number>;",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 24,
                        },
                      ],
                    },
                    {
                      code => "let v: Array<fooName.BarType> = [{ bar: 'bar' }];",
                      output => "let v: fooName.BarType[] = [{ bar: 'bar' }];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => {
                            class_name => "Array",
                            readonly_prefix => "",
                            type => "fooName.BarType",
                          },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let w: fooName.BazType<string>[] = [['baz']];",
                      output => "let w: Array<fooName.BazType<string>> = [['baz']];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_generic_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let x: Array<undefined> = [undefined] as undefined[];",
                      output => "let x: undefined[] = [undefined] as undefined[];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "undefined" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let y: string[] = <Array<string>>['2'];",
                      output => "let y: string[] = <string[]>['2'];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "string" },
                          line => 1,
                          column => 20,
                        },
                      ],
                      supported_language_languages => [Typescript],
                    },
                    {
                      code => "let z: Array = [3, '4'];",
                      output => "let z: any[] = [3, '4'];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "any" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "type Arr<T> = Array<T>;",
                      output => "type Arr<T> = T[];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 15,
                        },
                      ],
                    },
                    {
                      code => r#"
// Ignore user defined aliases
let yyyy: Arr<Array<Arr<string>>[]> = [[[['2']]]];
                      "#,
                      output => r#"
// Ignore user defined aliases
let yyyy: Arr<Arr<string>[][]> = [[[['2']]]];
                      "#,
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 3,
                          column => 15,
                        },
                      ],
                    },
                    {
                      code => r#"
interface ArrayClass<T> {
  foo: Array<T>;
  bar: T[];
  baz: Arr<T>;
}
                      "#,
                      output => r#"
interface ArrayClass<T> {
  foo: T[];
  bar: T[];
  baz: Arr<T>;
}
                      "#,
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 3,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => r#"
function fooFunction(foo: Array<ArrayClass<string>>) {
  return foo.map(e => e.foo);
}
                      "#,
                      output => r#"
function fooFunction(foo: ArrayClass<string>[]) {
  return foo.map(e => e.foo);
}
                      "#,
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 2,
                          column => 27,
                        },
                      ],
                    },
                    {
                      code => "let fooVar: Array<(c: number) => number>;",
                      output => "let fooVar: ((c: number) => number)[];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 13,
                        },
                      ],
                    },
                    {
                      code => "type fooUnion = Array<string | number | boolean>;",
                      output => "type fooUnion = (string | number | boolean)[];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 17,
                        },
                      ],
                    },
                    {
                      code => "type fooIntersection = Array<string & number>;",
                      output => "type fooIntersection = (string & number)[];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 24,
                        },
                      ],
                    },
                    {
                      code => "let x: Array;",
                      output => "let x: any[];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "any" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    // TODO: should support this? Looks like it's not
                    // syntactically valid according to Typescript.
                    // tree-sitter-typescript is parsing it as a single
                    // zero-width type_identifier (between the angle
                    // brackets)
                    // (see one other commented-out test case below)
                    // {
                    //   code => "let x: Array<>;",
                    //   output => "let x: any[];",
                    //   options => { default => "array" },
                    //   errors => [
                    //     {
                    //       message_id => "error_string_array",
                    //       data => { class_name => "Array", readonly_prefix => "", type => "any" },
                    //       line => 1,
                    //       column => 8,
                    //     },
                    //   ],
                    // },
                    {
                      code => "let x: Array;",
                      output => "let x: any[];",
                      options => { default => "array-simple" },
                      errors => [
                        {
                          message_id => "error_string_array_simple",
                          data => { class_name => "Array", readonly_prefix => "", type => "any" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    // {
                    //   code => "let x: Array<>;",
                    //   output => "let x: any[];",
                    //   options => { default => "array-simple" },
                    //   errors => [
                    //     {
                    //       message_id => "error_string_array_simple",
                    //       line => 1,
                    //       column => 8,
                    //     },
                    //   ],
                    // },
                    {
                      code => "let x: Array<number> = [1] as number[];",
                      output => "let x: Array<number> = [1] as Array<number>;",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "number" },
                          line => 1,
                          column => 31,
                        },
                      ],
                    },
                    {
                      code => "let y: string[] = <Array<string>>['2'];",
                      output => "let y: Array<string> = <Array<string>>['2'];",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "string" },
                          line => 1,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => "let ya = [[1, '2']] as [number, string][];",
                      output => "let ya = [[1, '2']] as Array<[number, string]>;",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 24,
                        },
                      ],
                    },
                    {
                      code => r#"
// Ignore user defined aliases
let yyyy: Arr<Array<Arr<string>>[]> = [[[['2']]]];
                      "#,
                      output => r#"
// Ignore user defined aliases
let yyyy: Arr<Array<Array<Arr<string>>>> = [[[['2']]]];
                      "#,
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 3,
                          column => 15,
                        },
                      ],
                    },
                    {
                      code => r#"
interface ArrayClass<T> {
  foo: Array<T>;
  bar: T[];
  baz: Arr<T>;
}
                      "#,
                      output => r#"
interface ArrayClass<T> {
  foo: Array<T>;
  bar: Array<T>;
  baz: Arr<T>;
}
                      "#,
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 4,
                          column => 8,
                        },
                      ],
                    },
                    {
                      code => r#"
function barFunction(bar: ArrayClass<String>[]) {
  return bar.map(e => e.bar);
}
                      "#,
                      output => r#"
function barFunction(bar: Array<ArrayClass<String>>) {
  return bar.map(e => e.bar);
}
                      "#,
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 2,
                          column => 27,
                        },
                      ],
                    },
                    {
                      code => "let barVar: ((c: number) => number)[];",
                      output => "let barVar: Array<(c: number) => number>;",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 13,
                        },
                      ],
                    },
                    {
                      code => "type barUnion = (string | number | boolean)[];",
                      output => "type barUnion = Array<string | number | boolean>;",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 17,
                        },
                      ],
                    },
                    {
                      code => "type barIntersection = (string & number)[];",
                      output => "type barIntersection = Array<string & number>;",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 24,
                        },
                      ],
                    },
                    {
                      code => r#"
interface FooInterface {
  '.bar': { baz: string[] };
}
                      "#,
                      output => r#"
interface FooInterface {
  '.bar': { baz: Array<string> };
}
                      "#,
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "string" },
                          line => 3,
                          column => 18,
                        },
                      ],
                    },
                    {
                      // https://github.com/typescript-eslint/typescript-eslint/issues/172
                      code => "type Unwrap<T> = T extends Array<infer E> ? E : T;",
                      output => "type Unwrap<T> = T extends (infer E)[] ? E : T;",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 28,
                        },
                      ],
                    },
                    {
                      // https://github.com/typescript-eslint/typescript-eslint/issues/172
                      code => "type Unwrap<T> = T extends (infer E)[] ? E : T;",
                      output => "type Unwrap<T> = T extends Array<infer E> ? E : T;",
                      options => { default => "generic" },
                      errors => [
                        {
                          message_id => "error_string_generic",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 28,
                        },
                      ],
                    },
                    {
                      code => "type Foo = ReadonlyArray<object>[];",
                      output => "type Foo = (readonly object[])[];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "object",
                          },
                          line => 1,
                          column => 12,
                        },
                      ],
                    },
                    {
                      code => "const foo: Array<new (...args: any[]) => void> = [];",
                      output => "const foo: (new (...args: any[]) => void)[] = [];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => { class_name => "Array", readonly_prefix => "", type => "T" },
                          line => 1,
                          column => 12,
                        },
                      ],
                    },
                    {
                      code => "const foo: ReadonlyArray<new (...args: any[]) => void> = [];",
                      output => "const foo: readonly (new (...args: any[]) => void)[] = [];",
                      options => { default => "array" },
                      errors => [
                        {
                          message_id => "error_string_array",
                          data => {
                            class_name => "ReadonlyArray",
                            readonly_prefix => "readonly ",
                            type => "T",
                          },
                          line => 1,
                          column => 12,
                        },
                      ],
                    },
                  ],
            },
        )
    }
}
