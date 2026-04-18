use qt_solid_example_widgets_schema::widgets::example_widgets_library;
use widget_build::{
    NativeTsExportStyle, NativeTsOptions, WidgetBuild, WidgetTsBindings, render_library_host_ts,
    render_library_intrinsics_ts, render_library_native_dts, render_library_native_ts,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    napi_build::setup();

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=crates/schema/src/lib.rs");
    println!("cargo:rerun-if-changed=crates/schema/src/widgets.rs");
    println!("cargo:rerun-if-changed=src/plugin_native.rs");

    let build = WidgetBuild::discover(env!("CARGO_MANIFEST_DIR"))?;
    let library = example_widgets_library();
    build.emit_ts_bindings(
        "packages/example-widgets/src",
        WidgetTsBindings {
            intrinsics_ts: &render_library_intrinsics_ts(library),
            host_ts: &render_library_host_ts(library, "@qt-solid/example-widgets/native"),
            native_ts: &render_library_native_ts(
                library,
                NativeTsOptions {
                    package_native_specifier: "@qt-solid/example-widgets/native",
                    resolve_relative_to_package: "../native",
                    resolve_relative_to_source: "../native",
                    export_style: NativeTsExportStyle::FunctionBridge,
                },
            ),
            native_d_ts: &render_library_native_dts(library),
        },
    )?;
    build.remove_ts_bindings("packages/example-widgets/packages/example-widgets/src")?;
    build.remove_dir_if_empty("packages/example-widgets/packages/example-widgets")?;
    build.remove_dir_if_empty("packages/example-widgets/packages")?;

    Ok(())
}
