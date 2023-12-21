use squalid::OptionExt;
use tree_sitter_lint::{tree_sitter::Node, tree_sitter_grep::SupportedLanguage, NodeExt};
use tree_sitter_lint_plugin_eslint_builtin::{
    assert_kind, ast_helpers::skip_nodes_of_type, kind::MethodDefinition,
};

use crate::kind::{InterfaceDeclaration, MethodSignature, ObjectType, ParenthesizedType};

pub fn get_is_member_static(node: Node) -> bool {
    assert_kind!(node, MethodDefinition | MethodSignature);
    node.non_comment_children_and_field_names(SupportedLanguage::Javascript)
        .take_while(|(_, field_name)| *field_name != Some("name"))
        .any(|(child, _)| child.kind() == "static")
}

pub trait NodeExtTypescript<'a> {
    fn skip_parenthesized_types(&self) -> Node<'a>;
}

impl<'a> NodeExtTypescript<'a> for Node<'a> {
    fn skip_parenthesized_types(&self) -> Node<'a> {
        skip_nodes_of_type(*self, ParenthesizedType)
    }
}

pub fn get_is_type_literal(node: Node) -> bool {
    node.kind() == ObjectType
        && !node
            .parent()
            .matches(|parent| parent.kind() == InterfaceDeclaration && parent.field("body") == node)
}
