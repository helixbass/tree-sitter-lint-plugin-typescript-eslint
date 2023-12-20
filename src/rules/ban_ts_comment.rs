use std::{collections::HashMap, sync::Arc};

use regex::Regex;
use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
enum AllowWithDescription {
    AllowWithDescription,
}

#[derive(Clone, Deserialize)]
struct DescriptionFormat {
    description_format: String,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum DirectiveConfig {
    Bool(bool),
    AllowWithDescription(AllowWithDescription),
    DescriptionFormat(DescriptionFormat),
}

impl From<bool> for DirectiveConfig {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<AllowWithDescription> for DirectiveConfig {
    fn from(value: AllowWithDescription) -> Self {
        Self::AllowWithDescription(value)
    }
}

impl From<DescriptionFormat> for DirectiveConfig {
    fn from(value: DescriptionFormat) -> Self {
        Self::DescriptionFormat(value)
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    #[serde(rename = "kebab-case")]
    ts_expect_error: Option<DirectiveConfig>,
    #[serde(rename = "kebab-case")]
    ts_ignore: Option<DirectiveConfig>,
    #[serde(rename = "kebab-case")]
    ts_nocheck: Option<DirectiveConfig>,
    #[serde(rename = "kebab-case")]
    ts_check: Option<DirectiveConfig>,
    minimum_description_length: Option<usize>,
}

impl Options {
    fn ts_expect_error(&self) -> DirectiveConfig {
        self.ts_expect_error
            .clone()
            .unwrap_or(AllowWithDescription::AllowWithDescription.into())
    }

    fn ts_ignore(&self) -> DirectiveConfig {
        self.ts_ignore.clone().unwrap_or(true.into())
    }

    fn ts_nocheck(&self) -> DirectiveConfig {
        self.ts_nocheck.clone().unwrap_or(true.into())
    }

    fn ts_check(&self) -> DirectiveConfig {
        self.ts_check.clone().unwrap_or(false.into())
    }

    fn minimum_description_length(&self) -> usize {
        self.minimum_description_length.unwrap_or(3)
    }
}

fn populate_description_format(
    description_formats: &mut HashMap<&'static str, Regex>,
    option: DirectiveConfig,
    directive: &'static str,
) {
    if let DirectiveConfig::DescriptionFormat(DescriptionFormat { description_format }) = option {
        description_formats.insert(directive, Regex::new(&description_format).unwrap());
    }
}

pub fn ban_ts_comment_rule() -> Arc<dyn Rule> {
    rule! {
        name => "ban-ts-comment",
        languages => [Typescript],
        messages => [
            ts_directive_comment => "Do not use \"@ts-{{directive}}\" because it alters compilation errors.",
            ts_ignore_instead_of_expect_error => "Use \"@ts-expect-error\" instead of \"@ts-ignore\", as \"@ts-ignore\" will do nothing if the following line is error-free.",
            ts_directive_comment_requires_description => "Include a description after the \"@ts-{{directive}}\" directive to explain why the @ts-{{directive}} is necessary. The description must be {{minimum_description_length}} characters or longer.",
            ts_directive_comment_description_not_match_pattern => "The description for the \"@ts-{{directive}}\" directive must match the {{format}} format.",
            replace_ts_ignore_with_ts_expect_error => "Replace \"@ts-ignore\" with \"@ts-expect-error\".",
        ],
        options_type => Options,
        state => {
            [per-config]
            description_formats: HashMap<&'static str, Regex> = {
                let mut description_formats: HashMap<&'static str, Regex> = Default::default();
                populate_description_format(&mut description_formats, options.ts_expect_error(), "ts-expect-error");
                populate_description_format(&mut description_formats, options.ts_ignore(), "ts-ignore-error");
                populate_description_format(&mut description_formats, options.ts_nocheck(), "ts-nocheck");
                populate_description_format(&mut description_formats, options.ts_check(), "ts-check");
                description_formats
            },
            ts_expect_error: DirectiveConfig = options.ts_expect_error(),
            ts_ignore: DirectiveConfig = options.ts_ignore(),
            ts_nocheck: DirectiveConfig = options.ts_nocheck(),
            ts_check: DirectiveConfig = options.ts_check(),
            minimum_description_length: usize = options.minimum_description_length(),
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
    fn test_ban_ts_comment_rule() {
        RuleTester::run(
            ban_ts_comment_rule(),
            rule_tests! {
                valid => [
                    "// just a comment containing @ts-expect-error somewhere",
                    r#"
                        /*
                         @ts-expect-error running with long description in a block
                        */
                    "#,
                    {
                        code => "// @ts-expect-error",
                        options => { "ts-expect-error" => false },
                    },
                    {
                        code => "// @ts-expect-error here is why the error is expected",
                        options => {
                          "ts-expect-error" => "allow-with-description",
                        },
                    },
                    {
                        code => "// @ts-expect-error exactly 21 characters",
                        options => {
                            "ts-expect-error" => "allow-with-description",
                            minimum_description_length => 21,
                        },
                    },
                    {
                        code => "// @ts-expect-error: TS1234 because xyz",
                        options => {
                            "ts-expect-error" => {
                                description_format => "^: TS\\d+ because .+$",
                            },
                            minimum_description_length => 10,
                        },
                    },
                    {
                      code => r#"// @ts-expect-error 👨‍👩‍👧‍👦👨‍👩‍👧‍👦👨‍👩‍👧‍👦"#,
                      options => {
                          "ts-expect-error" => "allow-with-description",
                      },
                    },
                    "// just a comment containing @ts-ignore somewhere",
                    {
                      code => "// @ts-ignore",
                      options => { "ts-ignore" => false },
                    },
                    {
                      code => "// @ts-ignore I think that I am exempted from any need to follow the rules!",
                      options => { "ts-ignore" => "allow-with-description" },
                    },
                    {
                      code => r#"
                /*
                 @ts-ignore running with long description in a block
                */
                      "#,
                      options => {
                        "ts-ignore" => "allow-with-description",
                        minimum_description_length => 21,
                      },
                    },
                    {
                      code => "// @ts-ignore: TS1234 because xyz",
                      options => {
                        "ts-ignore" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                        minimum_description_length => 10,
                      },
                    },
                    {
                      code => r#"// @ts-ignore 👨‍👩‍👧‍👦👨‍👩‍👧‍👦👨‍👩‍👧‍👦"#,
                      options => {
                        "ts-ignore" => "allow-with-description",
                      },
                    },
                    "// just a comment containing @ts-nocheck somewhere",
                    {
                      code => "// @ts-nocheck",
                      options => { "ts-nocheck" => false },
                    },
                    {
                      code => "// @ts-nocheck no doubt, people will put nonsense here from time to time just to get the rule to stop reporting, perhaps even long messages with other nonsense in them like other // @ts-nocheck or // @ts-ignore things",
                      options => { "ts-nocheck" => "allow-with-description" },
                    },
                    {
                      code => r#"
                /*
                 @ts-nocheck running with long description in a block
                */
                      "#,
                      options => {
                        "ts-nocheck" => "allow-with-description",
                        minimum_description_length => 21,
                      },
                    },
                    {
                      code => "// @ts-nocheck: TS1234 because xyz",
                      options => {
                        "ts-nocheck" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                        minimum_description_length => 10,
                      },
                    },
                    {
                      code => r#"// @ts-nocheck 👨‍👩‍👧‍👦👨‍👩‍👧‍👦👨‍👩‍👧‍👦"#,
                      options => {
                        "ts-nocheck" => "allow-with-description",
                      },
                    },
                    "// just a comment containing @ts-check somewhere",
                    r#"
                /*
                 @ts-check running with long description in a block
                */
                    "#,
                    {
                      code => "// @ts-check",
                      options => { "ts-check" => false },
                    },
                    {
                      code => "// @ts-check with a description and also with a no-op // @ts-ignore",
                      options => { "ts-check" => "allow-with-description", minimum_description_length => 3 },
                    },
                    {
                      code => "// @ts-check: TS1234 because xyz",
                      options => {
                        "ts-check" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                        minimum_description_length => 10,
                      },
                    },
                    {
                      code => r#"// @ts-check 👨‍👩‍👧‍👦👨‍👩‍👧‍👦👨‍👩‍👧‍👦"#,
                      options => {
                        "ts-check" => "allow-with-description",
                      },
                    },
                ],
                invalid => [
                  {
                    code => "// @ts-expect-error",
                    options => { "ts-expect-error" => true },
                    errors => [
                      {
                        data => { directive => "expect-error" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/* @ts-expect-error */",
                    options => { "ts-expect-error" => true },
                    errors => [
                      {
                        data => { directive => "expect-error" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"
/*
@ts-expect-error
*/
                    "#,
                    options => { "ts-expect-error" => true },
                    errors => [
                      {
                        data => { directive => "expect-error" },
                        message_id => "ts_directive_comment",
                        line => 2,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/** @ts-expect-error */",
                    options => { "ts-expect-error" => true },
                    errors => [
                      {
                        data => { directive => "expect-error" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-expect-error: Suppress next line",
                    options => { "ts-expect-error" => true },
                    errors => [
                      {
                        data => { directive => "expect-error" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/////@ts-expect-error: Suppress next line",
                    options => { "ts-expect-error" => true },
                    errors => [
                      {
                        data => { directive => "expect-error" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"
if (false) {
  // @ts-expect-error: Unreachable code error
  console.log('hello');
}
                    "#,
                    options => { "ts-expect-error" => true },
                    errors => [
                      {
                        data => { directive => "expect-error" },
                        message_id => "ts_directive_comment",
                        line => 3,
                        column => 3,
                      },
                    ],
                  },
                  {
                    code => "// @ts-expect-error",
                    options => {
                      "ts-expect-error" => "allow-with-description",
                    },
                    errors => [
                      {
                        data => { directive => "expect-error", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-expect-error: TODO",
                    options => {
                      "ts-expect-error" => "allow-with-description",
                      minimum_description_length => 10,
                    },
                    errors => [
                      {
                        data => { directive => "expect-error", minimum_description_length => 10 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-expect-error: TS1234 because xyz",
                    options => {
                      "ts-expect-error" => {
                        description_format => "^: TS\\d+ because .+$",
                      },
                      minimum_description_length => 25,
                    },
                    errors => [
                      {
                        data => { directive => "expect-error", minimum_description_length => 25 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-expect-error: TS1234",
                    options => {
                      "ts-expect-error" => {
                        description_format => "^: TS\\d+ because .+$",
                      },
                    },
                    errors => [
                      {
                        data => { directive => "expect-error", format => "^: TS\\d+ because .+$" },
                        message_id => "ts_directive_comment_description_not_match_pattern",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-expect-error    : TS1234 because xyz"#,
                    options =>
                      {
                        "ts-expect-error" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                      },
                    errors => [
                      {
                        data => { directive => "expect-error", format => "^: TS\\d+ because .+$" },
                        message_id => "ts_directive_comment_description_not_match_pattern",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-expect-error 👨‍👩‍👧‍👦"#,
                    options => 
                      {
                        "ts-expect-error" => "allow-with-description",
                      },
                    errors => [
                      {
                        data => { directive => "expect-error", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-ignore",
                    options => { "ts-ignore" => true, "ts-expect-error" => true },
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 1,
                        column => 1,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: "// @ts-expect-error",
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => "// @ts-ignore",
                    options => 
                      { "ts-ignore" => true, "ts-expect-error" => "allow-with-description" },
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 1,
                        column => 1,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: "// @ts-expect-error",
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => "// @ts-ignore",
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 1,
                        column => 1,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: "// @ts-expect-error",
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => "/* @ts-ignore */",
                    options => { "ts-ignore" => true },
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 1,
                        column => 1,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: "/* @ts-expect-error */",
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
/*
 @ts-ignore
*/
                    "#,
                    options => { "ts-ignore" => true },
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 2,
                        column => 1,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: r#"
              // /*
               // @ts-expect-error
              // */
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => "/** @ts-ignore */",
                    options => { "ts-ignore" => true, "ts-expect-error" => false },
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 1,
                        column => 1,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: "/** @ts-expect-error */",
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => "// @ts-ignore: Suppress next line",
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 1,
                        column => 1,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: "// @ts-expect-error: Suppress next line",
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => "/////@ts-ignore: Suppress next line",
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 1,
                        column => 1,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: "/////@ts-expect-error: Suppress next line",
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => r#"
if (false) {
  // @ts-ignore: Unreachable code error
  console.log('hello');
}
                    "#,
                    errors => [
                      {
                        message_id => "tsIgnoreInsteadOfExpectError",
                        line => 3,
                        column => 3,
                        // suggestions: [
                        //   {
                        //     message_id => "replaceTsIgnoreWithTsExpectError",
                        //     output: r#"
              // if (false) {
                // // @ts-expect-error: Unreachable code error
                // console.log('hello');
              // }
                    // "#,
                        //   },
                        // ],
                      },
                    ],
                  },
                  {
                    code => "// @ts-ignore",
                    options => { "ts-ignore" => "allow-with-description" },
                    errors => [
                      {
                        data => { directive => "ignore", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-ignore         "#,
                    options => { "ts-ignore" => "allow-with-description" },
                    errors => [
                      {
                        data => { directive => "ignore", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-ignore    .",
                    options => { "ts-ignore" => "allow-with-description" },
                    errors => [
                      {
                        data => { directive => "ignore", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-ignore: TS1234 because xyz",
                    options => 
                      {
                        "ts-ignore" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                        minimum_description_length => 25,
                      },
                    errors => [
                      {
                        data => { directive => "ignore", minimum_description_length => 25 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-ignore: TS1234",
                    options => 
                      {
                        "ts-ignore" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                      },
                    errors => [
                      {
                        data => { directive => "ignore", format => "^: TS\\d+ because .+$" },
                        message_id => "ts_directive_comment_description_not_match_pattern",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-ignore    : TS1234 because xyz"#,
                    options => 
                      {
                        "ts-ignore" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                      },
                    errors => [
                      {
                        data => { directive => "ignore", format => "^: TS\\d+ because .+$" },
                        message_id => "ts_directive_comment_description_not_match_pattern",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-ignore 👨‍👩‍👧‍👦"#,
                    options => 
                      {
                        "ts-ignore" => "allow-with-description",
                      },
                    errors => [
                      {
                        data => { directive => "ignore", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-nocheck",
                    options => { "ts-nocheck" => true },
                    errors => [
                      {
                        data => { directive => "nocheck" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-nocheck",
                    errors => [
                      {
                        data => { directive => "nocheck" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/* @ts-nocheck */",
                    options => { "ts-nocheck" => true },
                    errors => [
                      {
                        data => { directive => "nocheck" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"
/*
 @ts-nocheck
*/
                    "#,
                    options => { "ts-nocheck" => true },
                    errors => [
                      {
                        data => { directive => "nocheck" },
                        message_id => "ts_directive_comment",
                        line => 2,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/** @ts-nocheck */",
                    options => { "ts-nocheck" => true },
                    errors => [
                      {
                        data => { directive => "nocheck" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-nocheck: Suppress next line",
                    errors => [
                      {
                        data => { directive => "nocheck" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/////@ts-nocheck: Suppress next line",
                    errors => [
                      {
                        data => { directive => "nocheck" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"
if (false) {
  // @ts-nocheck: Unreachable code error
  console.log('hello');
}
                    "#,
                    errors => [
                      {
                        data => { directive => "nocheck" },
                        message_id => "ts_directive_comment",
                        line => 3,
                        column => 3,
                      },
                    ],
                  },
                  {
                    code => "// @ts-nocheck",
                    options => { "ts-nocheck" => "allow-with-description" },
                    errors => [
                      {
                        data => { directive => "nocheck", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-nocheck: TS1234 because xyz",
                    options => 
                      {
                        "ts-nocheck" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                        minimum_description_length => 25,
                      },
                    errors => [
                      {
                        data => { directive => "nocheck", minimum_description_length => 25 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-nocheck: TS1234",
                    options => 
                      {
                        "ts-nocheck" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                      },
                    errors => [
                      {
                        data => { directive => "nocheck", format => "^: TS\\d+ because .+$" },
                        message_id => "ts_directive_comment_description_not_match_pattern",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-nocheck    : TS1234 because xyz"#,
                    options => 
                      {
                        "ts-nocheck" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                      },
                    errors => [
                      {
                        data => { directive => "nocheck", format => "^: TS\\d+ because .+$" },
                        message_id => "ts_directive_comment_description_not_match_pattern",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-nocheck 👨‍👩‍👧‍👦"#,
                    options => 
                      {
                        "ts-nocheck" => "allow-with-description",
                      },
                    errors => [
                      {
                        data => { directive => "nocheck", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-check",
                    options => { "ts-check" => true },
                    errors => [
                      {
                        data => { directive => "check" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/* @ts-check */",
                    options => { "ts-check" => true },
                    errors => [
                      {
                        data => { directive => "check" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"
/*
 @ts-check
*/
                    "#,
                    options => { "ts-check" => true },
                    errors => [
                      {
                        data => { directive => "check" },
                        message_id => "ts_directive_comment",
                        line => 2,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/** @ts-check */",
                    options => { "ts-check" => true },
                    errors => [
                      {
                        data => { directive => "check" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-check: Suppress next line",
                    options => { "ts-check" => true },
                    errors => [
                      {
                        data => { directive => "check" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "/////@ts-check: Suppress next line",
                    options => { "ts-check" => true },

                    errors => [
                      {
                        data => { directive => "check" },
                        message_id => "ts_directive_comment",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"
if (false) {
  // @ts-check: Unreachable code error
  console.log('hello');
}
                    "#,
                    options => { "ts-check" => true },
                    errors => [
                      {
                        data => { directive => "check" },
                        message_id => "ts_directive_comment",
                        line => 3,
                        column => 3,
                      },
                    ],
                  },
                  {
                    code => "// @ts-check",
                    options => { "ts-check" => "allow-with-description" },
                    errors => [
                      {
                        data => { directive => "check", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-check: TS1234 because xyz",
                    options => 
                      {
                        "ts-check" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                        minimum_description_length => 25,
                      },
                    errors => [
                      {
                        data => { directive => "check", minimum_description_length => 25 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => "// @ts-check: TS1234",
                    options => 
                      {
                        "ts-check" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                      },
                    errors => [
                      {
                        data => { directive => "check", format => "^: TS\\d+ because .+$" },
                        message_id => "ts_directive_comment_description_not_match_pattern",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-check    : TS1234 because xyz"#,
                    options => 
                      {
                        "ts-check" => {
                          description_format => "^: TS\\d+ because .+$",
                        },
                      },
                    errors => [
                      {
                        data => { directive => "check", format => "^: TS\\d+ because .+$" },
                        message_id => "ts_directive_comment_description_not_match_pattern",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                  {
                    code => r#"// @ts-check 👨‍👩‍👧‍👦"#,
                    options => 
                      {
                        "ts-check" => "allow-with-description",
                      },
                    errors => [
                      {
                        data => { directive => "check", minimum_description_length => 3 },
                        message_id => "ts_directive_comment_requires_description",
                        line => 1,
                        column => 1,
                      },
                    ],
                  },
                ],
            },
        )
    }
}
