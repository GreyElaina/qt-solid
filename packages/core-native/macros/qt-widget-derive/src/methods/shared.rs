use syn::{Expr, LitStr, ReturnType, Type, spanned::Spanned};

pub(super) fn qt_cpp_macro_body(block: &syn::Block) -> syn::Result<Option<LitStr>> {
    let Some(stmt) = block.stmts.first() else {
        return Ok(None);
    };
    if block.stmts.len() != 1 {
        return Ok(None);
    }

    let mac = match stmt {
        syn::Stmt::Expr(Expr::Macro(expr_macro), _) => &expr_macro.mac,
        syn::Stmt::Macro(stmt_macro) => &stmt_macro.mac,
        _ => return Ok(None),
    };

    let Some(last) = mac.path.segments.last() else {
        return Ok(None);
    };
    if last.ident != "cpp" {
        return Ok(None);
    }

    Ok(Some(LitStr::new(
        &mac.tokens.to_string(),
        mac.tokens.span(),
    )))
}

pub(super) fn method_return_type(output: &ReturnType) -> syn::Result<Option<&Type>> {
    let ty = match output {
        ReturnType::Default => return Ok(None),
        ReturnType::Type(_, ty) => ty.as_ref(),
    };

    let Some(inner) = unwrap_result_type(ty)? else {
        return Ok(Some(ty));
    };

    Ok(Some(inner))
}

pub(super) fn unwrap_result_type(ty: &Type) -> syn::Result<Option<&Type>> {
    let Type::Path(type_path) = ty else {
        return Ok(None);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Ok(None);
    };
    if segment.ident != "Result" {
        return Ok(None);
    }

    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            &segment.arguments,
            "Result return type requires type arguments",
        ));
    };
    let Some(first) = arguments.args.first() else {
        return Err(syn::Error::new_spanned(
            arguments,
            "Result return type requires an Ok value type",
        ));
    };
    let syn::GenericArgument::Type(inner) = first else {
        return Err(syn::Error::new_spanned(first, "unsupported Result Ok type"));
    };

    Ok(Some(inner))
}

pub(super) fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}
