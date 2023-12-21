use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Style {
    #[default]
    Fields,
    Getters,
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
                        message_id => "preferFieldStyle",
                        column => 7,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferFieldStyle",
                        column => 7,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferFieldStyle",
                        column => 14,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferFieldStyle",
                        column => 21,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferFieldStyle",
                        column => 15,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferFieldStyle",
                        column => 15,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferGetterStyle",
                        column => 20,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferGetterStyleSuggestion",
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
                        message_id => "preferGetterStyle",
                        column => 12,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferGetterStyleSuggestion",
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
                        message_id => "preferGetterStyle",
                        column => 12,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferGetterStyleSuggestion",
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
                        message_id => "preferGetterStyle",
                        column => 19,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferGetterStyleSuggestion",
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
                        message_id => "preferFieldStyle",
                        column => 17,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferGetterStyle",
                        column => 22,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferGetterStyleSuggestion",
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
                        message_id => "preferFieldStyle",
                        column => 21,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferGetterStyle",
                        column => 26,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferGetterStyleSuggestion",
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
                        message_id => "preferFieldStyle",
                        column => 14,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferFieldStyleSuggestion",
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
                        message_id => "preferGetterStyle",
                        column => 19,
                        line => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "preferGetterStyleSuggestion",
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
