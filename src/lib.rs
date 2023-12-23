#![allow(non_upper_case_globals, clippy::into_iter_on_ref)]

use tree_sitter_lint::{
    instance_provider_factory, FromFileRunContextInstanceProviderFactory, Plugin,
};
use tree_sitter_lint_plugin_eslint_builtin::AllComments;

mod ast_helpers;
mod kind;
mod rules;
mod type_utils;
mod util;

use rules::{
    adjacent_overload_signatures_rule, array_type_rule, ban_ts_comment_rule,
    ban_tslint_comment_rule, ban_types_rule, class_literal_property_style_rule,
    class_methods_use_this_rule, consistent_generic_constructors_rule,
    consistent_type_definitions_rule, default_param_last_rule,
};

pub type ProvidedTypes<'a> = ();

pub fn instantiate() -> Plugin {
    Plugin {
        name: "typescript-eslint".to_owned(),
        rules: vec![
            adjacent_overload_signatures_rule(),
            array_type_rule(),
            ban_ts_comment_rule(),
            ban_tslint_comment_rule(),
            ban_types_rule(),
            class_literal_property_style_rule(),
            class_methods_use_this_rule(),
            consistent_generic_constructors_rule(),
            consistent_type_definitions_rule(),
            default_param_last_rule(),
        ],
    }
}

pub fn get_instance_provider_factory() -> Box<dyn FromFileRunContextInstanceProviderFactory> {
    type ProvidedTypesForRuleTests<'a> = (AllComments<'a>,);

    Box::new(instance_provider_factory!(ProvidedTypesForRuleTests))
}
