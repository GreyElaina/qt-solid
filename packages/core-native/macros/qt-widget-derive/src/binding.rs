use syn::{Expr, Path};

fn parse_label_path(path: Path) -> syn::Result<String> {
    if path.segments.is_empty() {
        return Err(syn::Error::new_spanned(path, "expected notify label path"));
    }

    Ok(path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::"))
}

pub(crate) fn parse_label_expr(expr: Expr) -> syn::Result<String> {
    match expr {
        Expr::Path(path) => parse_label_path(path.path),
        other => Err(syn::Error::new_spanned(other, "notify label expects path")),
    }
}
