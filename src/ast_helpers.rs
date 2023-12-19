use tree_sitter_lint::{tree_sitter::Node, tree_sitter_grep::SupportedLanguage, NodeExt};
use tree_sitter_lint_plugin_eslint_builtin::{assert_kind, kind::MethodDefinition};

use crate::kind::MethodSignature;

pub fn get_is_member_static(node: Node) -> bool {
    assert_kind!(node, MethodDefinition | MethodSignature);
    node.non_comment_children_and_field_names(SupportedLanguage::Javascript)
        .take_while(|(_, field_name)| *field_name != Some("name"))
        .any(|(child, _)| child.kind() == "static")
}
