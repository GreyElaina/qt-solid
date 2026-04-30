use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;

    let variants: Vec<_> = match &input.data {
        syn::Data::Enum(data) => data.variants.iter().collect(),
        _ => unreachable!(),
    };

    // Each variant must be a single-field tuple variant wrapping a FragmentDecl struct.
    let mut variant_info = Vec::new();
    for variant in &variants {
        let var_ident = &variant.ident;
        match &variant.fields {
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let inner_ty = &fields.unnamed[0].ty;
                variant_info.push((var_ident, inner_ty));
            }
            syn::Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "Fragment enum variants must be single-field tuple variants, e.g., Group(GroupFragment)",
                ));
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "Fragment enum variants must be single-field tuple variants, e.g., Rect(RectFragment)",
                ));
            }
        }
    }

    // from_tag arms.
    let from_tag_arms: Vec<TokenStream> = variant_info.iter().map(|(var_ident, inner_ty)| {
        quote! {
            <#inner_ty as crate::fragment::decl::FragmentDecl>::TAG => {
                Some(Self::#var_ident(<#inner_ty as Default>::default()))
            }
        }
    }).collect();

    // apply_prop forwarding.
    let apply_prop_arms: Vec<TokenStream> = variant_info.iter().map(|(var_ident, _)| {
        quote! { Self::#var_ident(inner) => inner.apply_prop(key, value), }
    }).collect();

    // reset_prop forwarding.
    let reset_prop_arms: Vec<TokenStream> = variant_info.iter().map(|(var_ident, _)| {
        quote! { Self::#var_ident(inner) => inner.reset_prop(key), }
    }).collect();

    // local_bounds forwarding.
    let local_bounds_arms: Vec<TokenStream> = variant_info.iter().map(|(var_ident, _)| {
        quote! { Self::#var_ident(inner) => inner.local_bounds(), }
    }).collect();

    // encode forwarding.
    let encode_arms: Vec<TokenStream> = variant_info.iter().map(|(var_ident, _)| {
        quote! {
            Self::#var_ident(inner) => {
                crate::fragment::decl::FragmentEncode::encode(inner, scene, transform);
            }
        }
    }).collect();

    // tag forwarding.
    let tag_arms: Vec<TokenStream> = variant_info.iter().map(|(var_ident, inner_ty)| {
        quote! {
            Self::#var_ident(_) => <#inner_ty as crate::fragment::decl::FragmentDecl>::TAG,
        }
    }).collect();

    // all_schemas.
    let schema_entries: Vec<TokenStream> = variant_info.iter().map(|(_, inner_ty)| {
        quote! {
            crate::fragment::decl::FragmentSchemaEntry {
                tag: <#inner_ty as crate::fragment::decl::FragmentDecl>::TAG,
                props: <#inner_ty as crate::fragment::decl::FragmentDecl>::PROPS,
            }
        }
    }).collect();
    let schema_count = schema_entries.len();

    let expanded = quote! {
        impl #ident {
            pub fn from_tag(tag: &str) -> Option<Self> {
                use crate::fragment::decl::FragmentDecl;
                match tag {
                    #(#from_tag_arms)*
                    _ => None,
                }
            }

            pub fn apply_prop(
                &mut self,
                key: &str,
                value: crate::fragment::decl::FragmentValue,
            ) -> crate::fragment::decl::FragmentMutation {
                use crate::fragment::decl::FragmentDecl;
                match self {
                    #(#apply_prop_arms)*
                }
            }

            pub fn reset_prop(
                &mut self,
                key: &str,
            ) -> crate::fragment::decl::FragmentMutation {
                use crate::fragment::decl::FragmentDecl;
                match self {
                    #(#reset_prop_arms)*
                }
            }

            pub fn local_bounds(&self) -> Option<crate::vello::peniko::kurbo::Rect> {
                use crate::fragment::decl::FragmentDecl;
                match self {
                    #(#local_bounds_arms)*
                }
            }

            pub fn encode(
                &self,
                scene: &mut crate::vello::Scene,
                transform: crate::vello::peniko::kurbo::Affine,
            ) {
                use crate::fragment::decl::FragmentEncode;
                match self {
                    #(#encode_arms)*
                }
            }

            pub fn tag(&self) -> &'static str {
                use crate::fragment::decl::FragmentDecl;
                match self {
                    #(#tag_arms)*
                }
            }

            pub fn all_schemas() -> &'static [crate::fragment::decl::FragmentSchemaEntry] {
                static SCHEMAS: [crate::fragment::decl::FragmentSchemaEntry; #schema_count] = [
                    #(#schema_entries),*
                ];
                &SCHEMAS
            }
        }
    };

    Ok(expanded)
}
