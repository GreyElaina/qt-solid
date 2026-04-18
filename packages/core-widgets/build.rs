#[path = "src/builtins.rs"]
mod builtins;

#[path = "src/prelude.rs"]
pub mod prelude;

#[path = "src/widgets/mod.rs"]
pub mod widgets;

use builtins::core_widgets_library;

use widget_build::{
    NativeTsExportStyle, NativeTsOptions, WidgetBuild, WidgetTsBindings, render_library_host_ts,
    render_library_intrinsics_ts, render_library_native_dts, render_library_native_ts,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/builtins.rs");
    println!("cargo:rerun-if-changed=src/widgets");

    let build = WidgetBuild::discover(env!("CARGO_MANIFEST_DIR"))?;
    let library = core_widgets_library();
    build.emit_ts_bindings(
        "packages/core-widgets/src",
        WidgetTsBindings {
            intrinsics_ts: &render_library_intrinsics_ts(library),
            host_ts: &render_library_host_ts(library, "@qt-solid/core-widgets/native"),
            native_ts: &render_library_native_ts(
                library,
                NativeTsOptions {
                    package_native_specifier: "@qt-solid/core/native",
                    resolve_relative_to_package: ".",
                    resolve_relative_to_source: "../../core/native",
                    export_style: NativeTsExportStyle::DirectEntities,
                },
            ),
            native_d_ts: &render_library_native_dts(library),
        },
    )?;
    build.remove_ts_bindings("packages/core-widgets/packages/core-widgets/src")?;
    build.remove_dir_if_empty("packages/core-widgets/packages/core-widgets")?;
    build.remove_dir_if_empty("packages/core-widgets/packages")?;

    Ok(())
}
