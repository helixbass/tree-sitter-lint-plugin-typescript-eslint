use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt, Rule,
};
use tree_sitter_lint_plugin_eslint_builtin::{
    assert_kind,
    ast_helpers::{
        get_method_definition_kind, is_simple_template_literal, is_tagged_template_expression,
        MethodDefinitionKind,
    },
    kind::{is_literal_kind, CallExpression, ReturnStatement, TemplateString},
};

use crate::kind::PublicFieldDefinition;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Style {
    #[default]
    Fields,
    Getters,
}

fn is_supported_literal(node: Node) -> bool {
    match node.kind() {
        kind if is_literal_kind(kind) => true,
        CallExpression if is_tagged_template_expression(node) => {
            is_simple_template_literal(node.field("arguments"))
        }
        TemplateString => is_simple_template_literal(node),
        _ => false,
    }
}

fn is_readonly_and_not_declare(node: Node) -> bool {
    assert_kind!(node, PublicFieldDefinition);

    for (child, _) in node
        .non_comment_children_and_field_names(SupportedLanguage::Javascript)
        .take_while(|(_, field_name)| field_name != &Some("name"))
    {
        match child.kind() {
            "declare" => return false,
            "readonly" => return true,
            _ => (),
        }
    }
    false
}

pub fn class_literal_property_style_rule() -> Arc<dyn Rule> {
    rule! {
        name => "class-literal-property-style",
        languages => [Typescript],
        messages => [
            prefer_field_style => "Literals should be exposed using readonly fields.",
            prefer_field_style_suggestion => "Replace the literals with readonly fields.",
            prefer_getter_style => "Literals should be exposed using getters.",
            prefer_getter_style_suggestion => "Replace the literals with getters.",
        ],
        options_type => Option<Style>,
        state => {
            [per-config]
            style: Style = options.unwrap_or_default(),
        },
        listeners => [
            r#"
              (method_definition) @c
            "# => |node, context| {
                if self.style != Style::Fields {
                    return;
                }

                if get_method_definition_kind(node, context) != MethodDefinitionKind::Get {
                    return;
                }
                let Some(statement) = node.field("body").non_comment_named_children(SupportedLanguage::Javascript).next().filter(|statement| {
                    statement.kind() == ReturnStatement
                }) else {
                    return;
                };

                let Some(_argument) = statement.maybe_first_non_comment_named_child(SupportedLanguage::Javascript).filter(|&argument| {
                    is_supported_literal(argument)
                }) else {
                    return;
                };

                context.report(violation! {
                    node => node.field("name"),
                    message_id => "prefer_field_style",
                    // TODO: suggestions?
                });
            },
            r#"
              (public_field_definition) @c
            "# => |node, context| {
                if self.style != Style::Getters {
                    return;
                }

                if !is_readonly_and_not_declare(node) {
                    return;
                }

                let Some(_value) = node.child_by_field_name("value").filter(|&value| {
                    is_supported_literal(value)
                }) else {
                    return;
                };

                context.report(violation! {
                    node => node.field("name"),
                    message_id => "prefer_getter_style",
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
    fn test_class_literal_property_style_rule() {
        RuleTester::run(
            class_literal_property_style_rule(),
            rule_tests! {
                valid => [
                  r#"
              class Mx {
                declare readonly p1 = 1;
              }
                  "#,
                  r#"
              class Mx {
                readonly p1 = 'hello world';
              }
                  "#,
                  r#"
              class Mx {
                p1 = 'hello world';
              }
                  "#,
                  r#"
              class Mx {
                static p1 = 'hello world';
              }
                  "#,
                  r#"
              class Mx {
                p1: string;
              }
                  "#,
                  r#"
              class Mx {
                get p1();
              }
                  "#,
                  r#"
              class Mx {
                get p1() {}
              }
                  "#,
                  r#"
              abstract class Mx {
                abstract get p1(): string;
              }
                  "#,
                  r#"
                    class Mx {
                      get mySetting() {
                        if (this._aValue) {
                          return 'on';
                        }

                        return 'off';
                      }
                    }
                  "#,
                  r#"
                    class Mx {
                      get mySetting() {
                        return `build-\${process.env.build}`;
                      }
                    }
                  "#,
                  r#"
                    class Mx {
                      getMySetting() {
                        if (this._aValue) {
                          return 'on';
                        }

                        return 'off';
                      }
                    }
                  "#,
                  r#"
                    class Mx {
                      public readonly myButton = styled.button`
                        color: \${props => (props.primary ? 'hotpink' : 'turquoise')};
                      `;
                    }
                  "#,
                  {
                    code => r#"
                      class Mx {
                        public get myButton() {
                          return styled.button`
                            color: \${props => (props.primary ? 'hotpink' : 'turquoise')};
                          `;
                        }
                      }
                    "#,
                    options => "fields",
                  },
                  {
                    code => r#"
              class Mx {
                public declare readonly foo = 1;
              }
                    "#,
                    options => "getters",
                  },
                  {
                    code => r#"
              class Mx {
                get p1() {
                  return 'hello world';
                }
              }
                    "#,
                    options => "getters",
                  },
                  {
                    code => r#"
              class Mx {
                p1 = 'hello world';
              }
                    "#,
                    options => "getters",
                  },
                  {
                    code => r#"
              class Mx {
                p1: string;
              }
                    "#,
                    options => "getters",
                  },
                  {
                    code => r#"
              class Mx {
                readonly p1 = [1, 2, 3];
              }
                    "#,
                    options => "getters",
                  },
                  {
                    code => r#"
              class Mx {
                static p1: string;
              }
                    "#,
                    options => "getters",
                  },
                  {
                    code => r#"
              class Mx {
                static get p1() {
                  return 'hello world';
                }
              }
                    "#,
                    options => "getters",
                  },
                  {
                    code => r#"
                      class Mx {
                        public readonly myButton = styled.button`
                          color: \${props => (props.primary ? 'hotpink' : 'turquoise')};
                        `;
                      }
                    "#,
                    options => "getters",
                  },
                  {
                    code => r#"
                      class Mx {
                        public get myButton() {
                          return styled.button`
                            color: \${props => (props.primary ? 'hotpink' : 'turquoise')};
                          `;
                        }
                      }
                    "#,
                    options => "getters",
                  },
                ],
                invalid => [
                  {
                    code => r#"
class Mx {
  get p1() {
    return 'hello world';
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 7,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // readonly p1 = 'hello world';
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
class Mx {
  get p1() {
    return `hello world`;
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 7,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // readonly p1 = `hello world`;
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
class Mx {
  static get p1() {
    return 'hello world';
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 14,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // static readonly p1 = 'hello world';
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
class Mx {
  public static get foo() {
    return 1;
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 21,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // public static readonly foo = 1;
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
class Mx {
  public get [myValue]() {
    return 'a literal value';
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 15,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // public readonly [myValue] = 'a literal value';
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
class Mx {
  public get [myValue]() {
    return 12345n;
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 15,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // public readonly [myValue] = 12345n;
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
class Mx {
  public readonly [myValue] = 'a literal value';
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_getter_style",
                        column => 20,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_getter_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // public get [myValue]() { return 'a literal value'; }
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                    options => "getters",
                  },
                  {
                    code => r#"
class Mx {
  readonly p1 = 'hello world';
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_getter_style",
                        column => 12,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_getter_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // get p1() { return 'hello world'; }
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                    options => "getters",
                  },
                  {
                    code => r#"
class Mx {
  readonly p1 = `hello world`;
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_getter_style",
                        column => 12,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_getter_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // get p1() { return `hello world`; }
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                    options => "getters",
                  },
                  {
                    code => r#"
class Mx {
  static readonly p1 = 'hello world';
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_getter_style",
                        column => 19,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_getter_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // static get p1() { return 'hello world'; }
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                    options => "getters",
                  },
                  {
                    code => r#"
class Mx {
  protected get p1() {
    return 'hello world';
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 17,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // protected readonly p1 = 'hello world';
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                    options => "fields",
                  },
                  {
                    code => r#"
class Mx {
  protected readonly p1 = 'hello world';
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_getter_style",
                        column => 22,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_getter_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // protected get p1() { return 'hello world'; }
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                    options => "getters",
                  },
                  {
                    code => r#"
class Mx {
  public static get p1() {
    return 'hello world';
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 21,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // public static readonly p1 = 'hello world';
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
class Mx {
  public static readonly p1 = 'hello world';
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_getter_style",
                        column => 26,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_getter_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // public static get p1() { return 'hello world'; }
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                    options => "getters",
                  },
                  {
                    code => r#"
class Mx {
  public get myValue() {
    return gql`
      {
        user(id: 5) {
          firstName
          lastName
        }
      }
    `;
  }
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_field_style",
                        column => 14,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_field_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // public readonly myValue = gql`
                    // {
                      // user(id: 5) {
                        // firstName
                        // lastName
                      // }
                    // }
                  // `;
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
class Mx {
  public readonly myValue = gql`
    {
      user(id: 5) {
        firstName
        lastName
      }
    }
  `;
}
                    "#,
                    errors => [
                      {
                        message_id => "prefer_getter_style",
                        column => 19,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "prefer_getter_styleSuggestion",
                        //     output: r#"
              // class Mx {
                // public get myValue() { return gql`
                  // {
                    // user(id: 5) {
                      // firstName
                      // lastName
                    // }
                  // }
                // `; }
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                    options => "getters",
                  },
                ],
            },
        )
    }
}
