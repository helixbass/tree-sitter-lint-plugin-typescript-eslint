#![allow(dead_code)]

pub type Kind = &'static str;

pub const AmbientDeclaration: &str = "ambient_declaration";
pub const CallSignature: &str = "call_signature";
pub const ConstructSignature: &str = "construct_signature";
pub const FunctionSignature: &str = "function_signature";
pub const InterfaceDeclaration: &str = "interface_declaration";
pub const MethodSignature: &str = "method_signature";
pub const ObjectType: &str = "object_type";
pub const TypeAliasDeclaration: &str = "type_alias_declaration";
