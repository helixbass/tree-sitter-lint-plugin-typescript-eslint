use squalid::OptionExt;
use tree_sitter_lint::{tree_sitter::Node, tree_sitter_grep::SupportedLanguage, NodeExt};
use tree_sitter_lint_plugin_eslint_builtin::{
    assert_kind,
    ast_helpers::skip_nodes_of_type,
    kind::{Class, ClassDeclaration, ClassHeritage, MethodDefinition},
};

use crate::kind::{
    AbstractMethodSignature, AccessibilityModifier, AmbientDeclaration, ImplementsClause,
    IndexSignature, InterfaceDeclaration, MappedTypeClause, MethodSignature, NestedTypeIdentifier,
    ObjectType, OverrideModifier, ParenthesizedType, PropertySignature, PublicFieldDefinition,
    TypeIdentifier, TypeParameter,
};

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
    if node.kind() != ObjectType {
        return false;
    }

    if node
        .parent()
        .matches(|parent| parent.kind() == InterfaceDeclaration && parent.field("body") == node)
    {
        return false;
    }

    if is_mapped_type(node) {
        return false;
    }

    true
}

pub fn is_mapped_type(node: Node) -> bool {
    if node.kind() != ObjectType {
        return false;
    }

    node.non_comment_named_children(SupportedLanguage::Javascript)
        .next()
        .matches(|child| {
            child.kind() == IndexSignature
                && child
                    .non_comment_children(SupportedLanguage::Javascript)
                    .take_while(|child| child.kind() != "]")
                    .any(|child| child.kind() == MappedTypeClause)
        })
}

pub fn get_is_type_reference(node: Node) -> bool {
    assert_kind!(node, TypeIdentifier);
    if node
        .parent()
        .matches(|parent| matches!(parent.kind(), NestedTypeIdentifier | TypeParameter))
    {
        return false;
    }
    true
}

pub fn get_class_heritage(node: Node) -> Option<Node> {
    assert_kind!(node, Class | ClassDeclaration);

    node.non_comment_named_children_and_field_names(SupportedLanguage::Javascript)
        .take_while(|(_, field_name)| *field_name != Some("body"))
        .find_map(|(node, _)| (node.kind() == ClassHeritage).then_some(node))
}

pub fn get_class_has_implements_clause(node: Node) -> bool {
    assert_kind!(node, Class | ClassDeclaration);

    get_class_heritage(node)
        .matches(|class_heritage| class_heritage.has_child_of_kind(ImplementsClause))
}

pub fn get_has_override_modifier(node: Node) -> bool {
    assert_kind!(
        node,
        MethodDefinition | PropertySignature | PublicFieldDefinition | MethodSignature
    );

    node.non_comment_named_children_and_field_names(SupportedLanguage::Javascript)
        .take_while(|(_, field_name)| *field_name != Some("name"))
        .any(|(node, _)| node.kind() == OverrideModifier)
}

pub fn get_accessibility_modifier(node: Node) -> Option<Node> {
    assert_kind!(
        node,
        PublicFieldDefinition
            | MethodSignature
            | AbstractMethodSignature
            | MethodDefinition
            | PropertySignature
    );

    node.non_comment_named_children_and_field_names(SupportedLanguage::Javascript)
        .take_while(|(_, field_name)| *field_name != Some("name"))
        .find_map(|(node, _)| (node.kind() == AccessibilityModifier).then_some(node))
}

#[allow(dead_code)]
pub fn get_is_index_signature(node: Node) -> bool {
    if node.kind() != IndexSignature {
        return false;
    }

    !is_mapped_type(node)
}

pub fn get_is_global_ambient_declaration(node: Node) -> bool {
    node.kind() == AmbientDeclaration
        && node
            .non_comment_children(SupportedLanguage::Javascript)
            .nth(1)
            .unwrap()
            .kind()
            == "global"
}
