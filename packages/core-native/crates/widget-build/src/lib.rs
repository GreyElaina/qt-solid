use std::{
    fs, io,
    path::{Path, PathBuf},
};

mod ts_codegen;

pub use ts_codegen::{
    NativeTsExportStyle, NativeTsOptions, render_library_host_ts, render_library_intrinsics_ts,
    render_library_native_dts, render_library_native_ts,
};

#[derive(Debug, Clone, Copy)]
pub struct WidgetTsBindings<'a> {
    pub intrinsics_ts: &'a str,
    pub host_ts: &'a str,
    pub native_ts: &'a str,
    pub native_d_ts: &'a str,
}

#[derive(Debug, Clone)]
pub struct WidgetBuild {
    workspace_root: PathBuf,
}

impl WidgetBuild {
    pub fn discover(crate_dir: impl AsRef<Path>) -> io::Result<Self> {
        let crate_dir = crate_dir.as_ref();
        let workspace_root = find_workspace_root(crate_dir).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("workspace root not found from {}", crate_dir.display()),
            )
        })?;

        Ok(Self { workspace_root })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn emit_ts_bindings(
        &self,
        library_src_dir: impl AsRef<Path>,
        bindings: WidgetTsBindings<'_>,
    ) -> io::Result<()> {
        let library_src_dir = self.workspace_root.join(library_src_dir.as_ref());
        fs::create_dir_all(&library_src_dir)?;

        write_if_changed(
            &library_src_dir.join("qt-intrinsics.ts"),
            bindings.intrinsics_ts,
        )?;
        write_if_changed(&library_src_dir.join("qt-host.ts"), bindings.host_ts)?;
        write_if_changed(&library_src_dir.join("native.ts"), bindings.native_ts)?;
        write_if_changed(&library_src_dir.join("native.d.ts"), bindings.native_d_ts)?;

        Ok(())
    }

    pub fn remove_file(&self, relative_path: impl AsRef<Path>) -> io::Result<()> {
        remove_if_exists(&self.workspace_root.join(relative_path))
    }

    pub fn remove_dir_if_empty(&self, relative_path: impl AsRef<Path>) -> io::Result<()> {
        remove_dir_if_empty(&self.workspace_root.join(relative_path))
    }

    pub fn remove_ts_bindings(&self, library_src_dir: impl AsRef<Path>) -> io::Result<()> {
        let library_src_dir = self.workspace_root.join(library_src_dir.as_ref());
        remove_if_exists(&library_src_dir.join("qt-intrinsics.ts"))?;
        remove_if_exists(&library_src_dir.join("qt-host.ts"))?;
        remove_if_exists(&library_src_dir.join("native.ts"))?;
        remove_if_exists(&library_src_dir.join("native.d.ts"))?;
        remove_dir_if_empty(&library_src_dir)
    }
}

pub fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .filter(|dir| dir.join("package.json").is_file() && dir.join("Cargo.toml").is_file())
        .last()
        .map(Path::to_path_buf)
}

fn write_if_changed(path: &Path, content: &str) -> io::Result<()> {
    match fs::read_to_string(path) {
        Ok(existing) if existing == content => Ok(()),
        _ => fs::write(path, content),
    }
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_dir_if_empty(path: &Path) -> io::Result<()> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{WidgetBuild, WidgetTsBindings, find_workspace_root};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let path = env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn discovers_workspace_root_from_nested_crate() {
        let temp = TempDir::new("widget-build-workspace");
        let workspace_root = temp.path().join("workspace");
        let crate_dir = workspace_root.join("packages/example/crate");

        fs::create_dir_all(&crate_dir).expect("create crate dir");
        fs::write(workspace_root.join("Cargo.toml"), "[workspace]\n").expect("write cargo");
        fs::write(workspace_root.join("package.json"), "{}\n").expect("write package");

        let resolved = find_workspace_root(&crate_dir).expect("workspace root");
        assert_eq!(resolved, workspace_root);
    }

    #[test]
    fn prefers_topmost_workspace_root_when_package_is_nested_workspace() {
        let temp = TempDir::new("widget-build-monorepo");
        let repo_root = temp.path().join("repo");
        let package_root = repo_root.join("packages/example");
        let crate_dir = package_root.join("crates/schema");

        fs::create_dir_all(&crate_dir).expect("create crate dir");
        fs::write(repo_root.join("Cargo.toml"), "[workspace]\n").expect("write repo cargo");
        fs::write(repo_root.join("package.json"), "{}\n").expect("write repo package");
        fs::write(
            package_root.join("Cargo.toml"),
            "[package]\nname = \"example\"\nversion = \"0.0.0\"\n",
        )
        .expect("write package cargo");
        fs::write(package_root.join("package.json"), "{}\n").expect("write package package");

        let resolved = find_workspace_root(&crate_dir).expect("workspace root");
        assert_eq!(resolved, repo_root);
    }

    #[test]
    fn emits_widget_ts_bindings_into_library_src_dir() {
        let temp = TempDir::new("widget-build-emit");
        let workspace_root = temp.path().join("workspace");
        let crate_dir = workspace_root.join("packages/core-native");

        fs::create_dir_all(&crate_dir).expect("create crate dir");
        fs::write(workspace_root.join("Cargo.toml"), "[workspace]\n").expect("write cargo");
        fs::write(workspace_root.join("package.json"), "{}\n").expect("write package");

        let build = WidgetBuild::discover(&crate_dir).expect("discover build");
        build
            .emit_ts_bindings(
                "packages/example/src",
                WidgetTsBindings {
                    intrinsics_ts: "intrinsics\n",
                    host_ts: "host\n",
                    native_ts: "native ts\n",
                    native_d_ts: "native\n",
                },
            )
            .expect("emit ts bindings");

        assert_eq!(
            fs::read_to_string(workspace_root.join("packages/example/src/qt-intrinsics.ts"))
                .expect("read intrinsics"),
            "intrinsics\n"
        );
        assert_eq!(
            fs::read_to_string(workspace_root.join("packages/example/src/qt-host.ts"))
                .expect("read host"),
            "host\n"
        );
        assert_eq!(
            fs::read_to_string(workspace_root.join("packages/example/src/native.ts"))
                .expect("read native ts"),
            "native ts\n"
        );
        assert_eq!(
            fs::read_to_string(workspace_root.join("packages/example/src/native.d.ts"))
                .expect("read native dts"),
            "native\n"
        );
    }
}
