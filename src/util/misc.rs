use std::borrow::Cow;

use tree_sitter_lint::{
    tree_sitter::Node, tree_sitter_grep::SupportedLanguage, NodeExt, QueryMatchContext,
};
use tree_sitter_lint_plugin_eslint_builtin::{
    assert_kind,
    kind::{
        ComputedPropertyName, Identifier, MethodDefinition, PrivatePropertyIdentifier,
        PropertyIdentifier,
    },
    utils::ast_utils::get_static_string_value,
};

use crate::kind::MethodSignature;
use crate::type_utils::requires_quoting;

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
    assert_kind!(
        member,
        MethodDefinition | MethodSignature /*TODO: others*/
    );
    let key = member.field("name");
    get_name_from_member_key(key, context)
}

fn get_name_from_member_key<'a>(
    key: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> MemberName<'a> {
    match key.kind() {
        Identifier | PropertyIdentifier => MemberName {
            type_: MemberNameType::Normal,
            name: key.text(context),
        },
        PrivatePropertyIdentifier => MemberName {
            type_: MemberNameType::Private,
            name: key.text(context),
        },
        ComputedPropertyName => get_name_from_member_key(
            key.first_non_comment_named_child(SupportedLanguage::Javascript),
            context,
        ),
        tree_sitter_lint_plugin_eslint_builtin::kind::String => {
            let name = get_static_string_value(key, context).unwrap();
            if requires_quoting(&name) {
              return MemberName {
                type_: MemberNameType::Quoted,
                name: format!("\"{name}\"").into(),
              };
            }
            MemberName {
                type_: MemberNameType::Normal,
                name,
            }
        }
        _ => MemberName {
            type_: MemberNameType::Expression,
            name: key.text(context),
        }
    }
}
