use std::path::PathBuf;

use qt_solid_core_widgets::core_widgets_library;
use widget_build::{
    NativeTsExportStyle, NativeTsOptions, WidgetBuild, WidgetTsBindings, render_library_host_ts,
    render_library_intrinsics_ts, render_library_native_dts, render_library_native_ts,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let native_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let build = WidgetBuild::discover(&native_dir)?;
    let core_widgets_library = core_widgets_library();

    build.emit_ts_bindings(
        "packages/core-widgets/src",
        WidgetTsBindings {
            intrinsics_ts: &render_library_intrinsics_ts(core_widgets_library),
            host_ts: &render_library_host_ts(core_widgets_library, "@qt-solid/core-widgets/native"),
            native_ts: &render_library_native_ts(
                core_widgets_library,
                NativeTsOptions {
                    package_native_specifier: "@qt-solid/core/native",
                    resolve_relative_to_package: ".",
                    resolve_relative_to_source: "../../core/native",
                    export_style: NativeTsExportStyle::DirectEntities,
                },
            ),
            native_d_ts: &render_library_native_dts(core_widgets_library),
        },
    )?;
    build.remove_file("packages/core-widgets/src/qt-host.internal.ts")?;
    build.remove_ts_bindings("packages/core-widgets/packages/core-widgets/src")?;
    build.remove_dir_if_empty("packages/core-widgets/packages/core-widgets")?;
    build.remove_dir_if_empty("packages/core-widgets/packages")?;
    build.remove_file("src/generated/qt-intrinsics.ts")?;
    build.remove_file("src/generated/qt-host.ts")?;
    build.remove_dir_if_empty("src/generated")?;
    build.remove_dir_if_empty("src")?;

    Ok(())
}
