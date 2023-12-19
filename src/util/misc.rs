use std::borrow::Cow;

use tree_sitter_lint::{
    tree_sitter::Node, tree_sitter_grep::SupportedLanguage, NodeExt, QueryMatchContext,
};
use tree_sitter_lint_plugin_eslint_builtin::{
    assert_kind,
    kind::{ComputedPropertyName, Identifier, PropertyIdentifier},
    utils::ast_utils::get_static_string_value,
};

use crate::kind::MethodSignature;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MemberNameType {
    Private,
    Quoted,
    Normal,
    Expression,
}

pub struct MemberName<'a> {
    pub type_: MemberNameType,
    pub name: Cow<'a, str>,
}

pub fn get_name_from_member<'a>(
    member: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> MemberName<'a> {
    assert_kind!(member, MethodSignature /*TODO: others*/);
    let key = member.field("name");
    match key.kind() {
        Identifier | PropertyIdentifier => MemberName {
            type_: MemberNameType::Normal,
            name: key.text(context),
        },
        ComputedPropertyName => MemberName {
            type_: MemberNameType::Normal,
            name: key
                .first_non_comment_named_child(SupportedLanguage::Javascript)
                .text(context),
        },
        tree_sitter_lint_plugin_eslint_builtin::kind::String => {
            let name = get_static_string_value(key, context).unwrap();
            // if (requiresQuoting(name)) {
            //   return {
            //     type: MemberNameType.Quoted,
            //     name: `"${name}"`,
            //   };
            // }
            MemberName {
                type_: MemberNameType::Normal,
                name,
            }
        }
        _ => unimplemented!(),
    }
}
