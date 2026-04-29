use proc_macro::TokenStream;
use syn::parse_macro_input;

mod fragment_struct;
mod fragment_enum;
mod parse;

/// Derive macro for fragment types.
///
/// On a **struct**: generates `impl FragmentDecl` with `apply_prop`, `reset_prop`,
/// `local_bounds`, `TAG`, and `PROPS`.
///
/// On an **enum**: generates forwarding dispatch (`from_tag`, `apply_prop`,
/// `reset_prop`, `local_bounds`, `all_schemas`).
#[proc_macro_derive(Fragment, attributes(fragment))]
pub fn derive_fragment(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match expand_fragment(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_fragment(input: syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    match &input.data {
        syn::Data::Struct(_) => fragment_struct::expand(input),
        syn::Data::Enum(_) => fragment_enum::expand(input),
        syn::Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "#[derive(Fragment)] is not supported on unions",
        )),
    }
}
