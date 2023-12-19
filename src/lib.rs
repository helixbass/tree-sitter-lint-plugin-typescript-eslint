#![allow(non_upper_case_globals, clippy::into_iter_on_ref)]

use tree_sitter_lint::{
    instance_provider_factory, FromFileRunContextInstanceProviderFactory, Plugin,
};

mod ast_helpers;
mod kind;
mod rules;
mod type_utils;
mod util;

use rules::adjacent_overload_signatures_rule;

pub type ProvidedTypes<'a> = ();

pub fn instantiate() -> Plugin {
    Plugin {
        name: "typescript-eslint".to_owned(),
        rules: vec![adjacent_overload_signatures_rule()],
    }
}

pub fn get_instance_provider_factory() -> Box<dyn FromFileRunContextInstanceProviderFactory> {
    Box::new(instance_provider_factory!(ProvidedTypes))
}
