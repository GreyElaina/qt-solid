use crate::common::option_inner_type;
use quote::quote;
use syn::{Expr, Ident, LitStr, Type};

pub(crate) enum QtMethodImplKind {
    None,
    Pure,
    Mixed,
}

pub(crate) enum QtPropMethodConfig {
    Plain,
    Constructor,
    Prop(ManualPropMethod),
}

pub(crate) enum ManualPropKind {
    Setter,
    Getter,
}

pub(crate) struct ManualPropMethod {
    pub js_name: String,
    pub kind: ManualPropKind,
    pub init: bool,
    pub default: Option<Expr>,
}

pub(crate) struct ManualPropEntry {
    pub js_name: String,
    pub value_type_key: Option<String>,
    pub value_type: Option<proc_macro2::TokenStream>,
    pub default: Option<proc_macro2::TokenStream>,
    pub optional: bool,
    pub init_setter_slot: Option<u16>,
    pub setter_slot: Option<u16>,
    pub getter_slot: Option<u16>,
    pub init_setter_runtime: Option<SetterRuntimeMethod>,
    pub setter_runtime: Option<SetterRuntimeMethod>,
    pub getter_runtime: Option<GetterRuntimeMethod>,
}

impl ManualPropEntry {
    pub(crate) fn new(js_name: &str) -> Self {
        Self {
            js_name: js_name.to_owned(),
            value_type_key: None,
            value_type: None,
            default: None,
            optional: false,
            init_setter_slot: None,
            setter_slot: None,
            getter_slot: None,
            init_setter_runtime: None,
            setter_runtime: None,
            getter_runtime: None,
        }
    }

    pub(crate) fn record_value_type(&mut self, ty: &Type) -> syn::Result<()> {
        let normalized = quote!(#ty).to_string();
        if let Some(existing) = self.value_type_key.as_ref() {
            if existing != &normalized {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!(
                        "widget prop {} mixes incompatible Rust value types {} and {}",
                        self.js_name, existing, normalized
                    ),
                ));
            }
        } else {
            self.value_type_key = Some(normalized);
        }
        self.optional = option_inner_type(ty).is_some();
        Ok(())
    }

    pub(crate) fn spec_tokens(
        &self,
        schema: &proc_macro2::TokenStream,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let value_type = self.value_type.clone().ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "widget prop requires at least one typed setter/getter",
            )
        })?;
        let path = LitStr::new(&self.js_name, proc_macro2::Span::call_site());
        let default = self
            .default
            .clone()
            .unwrap_or_else(|| quote!(#schema::SpecPropDefaultValue::None));
        let init_setter_slot = super::slot_literal(self.init_setter_slot);
        let setter_slot = super::slot_literal(self.setter_slot);
        let getter_slot = super::slot_literal(self.getter_slot);

        Ok(quote! {
            #schema::SpecPropDecl {
                path: &[
                    #path,
                ],
                value_type: #value_type,
                default: #default,
                init_setter_slot: #init_setter_slot,
                setter_slot: #setter_slot,
                getter_slot: #getter_slot,
            }
        })
    }
}

#[derive(Clone)]
pub(crate) struct SetterRuntimeMethod {
    pub ident: Ident,
    pub value_type: Type,
    pub returns_result: bool,
}

#[derive(Clone)]
pub(crate) struct GetterRuntimeMethod {
    pub ident: Ident,
    pub value_type: Type,
    pub returns_result: bool,
}

pub(crate) struct ConstructorSpec {
    pub ident: Ident,
    pub returns_result: bool,
    pub params: Vec<ConstructorParamSpec>,
}

pub(crate) struct ConstructorParamSpec {
    pub arg_ident: Ident,
    pub js_name: String,
    pub arg_ty: Type,
    pub value_ty: Type,
    pub default: Option<Expr>,
    pub optional: bool,
}
