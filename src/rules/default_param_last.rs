use std::sync::Arc;

use itertools::Itertools;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt, Rule,
};
use tree_sitter_lint_plugin_eslint_builtin::kind::RestPattern;

use crate::kind::OptionalParameter;

fn is_plain_param(node: Node) -> bool {
    !(node.kind() == OptionalParameter
        || node.child_by_field_name("value").is_some()
        || node.field("pattern").kind() == RestPattern)
}

pub fn default_param_last_rule() -> Arc<dyn Rule> {
    rule! {
        name => "default-param-last",
        languages => [Typescript],
        messages => [
            should_be_last => "Default parameters should be last.",
        ],
        listeners => [
            r#"
              (function_declaration) @c
              (function) @c
              (generator_function_declaration) @c
              (generator_function) @c
              (method_definition) @c
              (arrow_function) @c
            "# => |node, context| {
                let mut has_seen_plain_param = false;

                for param in node.field("parameters").non_comment_named_children(SupportedLanguage::Javascript).collect_vec().into_iter().rev() {
                    if is_plain_param(param) {
                        has_seen_plain_param = true;
                        continue;
                    }

                    if has_seen_plain_param && (
                        param.kind() == OptionalParameter ||
                        param.child_by_field_name("value").is_some()
                    ) {
                        context.report(violation! {
                            node => param,
                            message_id => "should_be_last",
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
    fn test_default_param_last_rule() {
        unimplemented!()
    }
}
