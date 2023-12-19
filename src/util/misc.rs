use std::borrow::Cow;

use tree_sitter_lint::{tree_sitter::Node, NodeExt, QueryMatchContext};
use tree_sitter_lint_plugin_eslint_builtin::{assert_kind, kind::Identifier};

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
    let name = member.field("name");
    match name.kind() {
        Identifier => MemberName {
            type_: MemberNameType::Normal,
            name: name.text(context),
        },
        _ => unimplemented!(),
    }
}
