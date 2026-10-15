//! Right-click menu ("Edit with Pixel Studio" / "Open with Pixel Studio") and "Open With"
//! registration for Windows Explorer (HKCU), Linux file managers (.desktop MIME types,
//! Nautilus scripts, Dolphin service menus, Nemo actions) and macOS Finder (Quick Actions).
//!
//! Registration never changes the user's default app: on Windows it adds verbs under
//! `SystemFileAssociations` and an `OpenWithProgids` entry; on Linux it only lists MIME
//! types in the .desktop file. Setting Pixel Studio as default is an opt-in installer task.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "Pixel Studio";
pub const PROG_ID: &str = "PixelStudio.Image";
pub const EXE_NAME: &str = "pixel-studio.exe";

pub const EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "gif", "ico", "webp", "tif", "tiff",
];

pub const MIME_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/bmp",
    "image/x-bmp",
    "image/gif",
    "image/x-icon",
    "image/vnd.microsoft.icon",
    "image/webp",
    "image/tiff",
];

/// (id, label, CLI flag)
pub const COMMANDS: &[(&str, &str, &str)] = &[
    ("edit", "Edit with Pixel Studio", "--edit"),
    ("open", "Open with Pixel Studio", "--view"),
];

fn zh_label(id: &str) -> &'static str {
    match id {
        "edit" => "用 Pixel Studio 編輯",
        _ => "用 Pixel Studio 開啟",
    }
}

// ------------------------------------------------------------------ Windows

#[derive(Debug, Clone, PartialEq)]
pub struct RegKey {
    pub path: String,
    pub values: Vec<(String, String)>,
}

/// `zh`: use Traditional Chinese verb labels.
pub fn windows_keys(exe: &str, base: &str, zh: bool) -> Vec<RegKey> {
    let mut keys = Vec::new();
    let icon = format!("\"{exe}\",0");
    for e in EXTENSIONS {
        for (id, label, flag) in COMMANDS {
            let root = format!(r"{base}\SystemFileAssociations\.{e}\shell\PixelStudio.{id}");
            let label = if zh {
                zh_label(id).to_string()
            } else {
                label.to_string()
            };
            keys.push(RegKey {
                path: root.clone(),
                values: vec![("MUIVerb".into(), label), ("Icon".into(), icon.clone())],
            });
            keys.push(RegKey {
                path: format!(r"{root}\command"),
                values: vec![(String::new(), format!("\"{exe}\" {flag} \"%1\""))],
            });
        }
        // "Open with" list entry, without touching the default handler.
        keys.push(RegKey {
            path: format!(r"{base}\.{e}\OpenWithProgids"),
            values: vec![(PROG_ID.into(), String::new())],
        });
    }
    keys.push(RegKey {
        path: format!(r"{base}\{PROG_ID}"),
        values: vec![
            (String::new(), "Pixel Studio Image".into()),
            ("FriendlyTypeName".into(), "Pixel Studio Image".into()),
        ],
    });
    keys.push(RegKey {
        path: format!(r"{base}\{PROG_ID}\DefaultIcon"),
        values: vec![(String::new(), icon.clone())],
    });
    keys.push(RegKey {
        path: format!(r"{base}\{PROG_ID}\shell\open\command"),
        values: vec![(String::new(), format!("\"{exe}\" \"%1\""))],
    });
    let app = format!(r"{base}\Applications\{EXE_NAME}");
    keys.push(RegKey {
        path: app.clone(),
        values: vec![("FriendlyAppName".into(), APP_NAME.into())],
    });
    keys.push(RegKey {
        path: format!(r"{app}\shell\open\command"),
        values: vec![(String::new(), format!("\"{exe}\" \"%1\""))],
    });
    keys.push(RegKey {
        path: format!(r"{app}\SupportedTypes"),
        values: EXTENSIONS
            .iter()
            .map(|e| (format!(".{e}"), String::new()))
            .collect(),
    });
    keys
}

/// Keys to delete on unregister (whole subtrees), plus OpenWithProgids values to remove.
pub fn windows_roots(base: &str) -> Vec<String> {
    let mut v = Vec::new();
    for e in EXTENSIONS {
        for (id, _, _) in COMMANDS {
            v.push(format!(
                r"{base}\SystemFileAssociations\.{e}\shell\PixelStudio.{id}"
            ));
        }
    }
    v.push(format!(r"{base}\{PROG_ID}"));
    v.push(format!(r"{base}\Applications\{EXE_NAME}"));
    v
}

#[cfg(windows)]
mod win {
    use super::*;
    use winreg::RegKey as WKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};

    pub fn apply(exe: &str, base: &str, zh: bool) -> std::io::Result<()> {
        let hkcu = WKey::predef(HKEY_CURRENT_USER);
        for k in windows_keys(exe, base, zh) {
            let (key, _) = hkcu.create_subkey(&k.path)?;
            for (n, v) in &k.values {
                key.set_value(n, v)?;
            }
        }
        Ok(())
    }

    pub fn remove(base: &str) -> std::io::Result<()> {
        let hkcu = WKey::predef(HKEY_CURRENT_USER);
        for r in windows_roots(base) {
            match hkcu.delete_subkey_all(&r) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        for e in EXTENSIONS {
            if let Ok(k) =
                hkcu.open_subkey_with_flags(format!(r"{base}\.{e}\OpenWithProgids"), KEY_ALL_ACCESS)
            {
                let _ = k.delete_value(PROG_ID);
            }
        }
        Ok(())
    }

    pub fn exists(base: &str) -> bool {
        WKey::predef(HKEY_CURRENT_USER)
            .open_subkey(format!(
                r"{base}\SystemFileAssociations\.png\shell\PixelStudio.edit"
            ))
            .is_ok()
    }

    pub fn read_default(path: &str) -> std::io::Result<String> {
        WKey::predef(HKEY_CURRENT_USER)
            .open_subkey(path)?
            .get_value("")
    }
}

// ------------------------------------------------------------------ Linux

/// A file to write: (path relative to $HOME, content, executable).
pub type GenFile = (PathBuf, String, bool);

fn desktop_quote(exe: &str) -> String {
    let mut s = String::from("\"");
    for c in exe.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('"');
    s
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn linux_files(exe: &str) -> Vec<GenFile> {
    let q = desktop_quote(exe);
    let mimes = format!("{};", MIME_TYPES.join(";"));
    let mut out: Vec<GenFile> = vec![(
        ".local/share/applications/pixel-studio.desktop".into(),
        format!(
            "[Desktop Entry]\nType=Application\nName={APP_NAME}\nName[zh_TW]=Pixel Studio 像素工作室\nGenericName=Image Editor\nGenericName[zh_TW]=圖片編輯器\n\
Comment=Paint, edit and convert images (PNG, JPEG, WebP, GIF, BMP, ICO)\nExec={q} %F\nIcon=applications-graphics\nTerminal=false\n\
Categories=Graphics;2DGraphics;RasterGraphics;Viewer;\nMimeType={mimes}\nActions=Edit;View;\n\n\
[Desktop Action Edit]\nName=Edit with Pixel Studio\nName[zh_TW]=用 Pixel Studio 編輯\nExec={q} --edit %F\n\n\
[Desktop Action View]\nName=Open with Pixel Studio\nName[zh_TW]=用 Pixel Studio 開啟\nExec={q} --view %F\n"
        ),
        false,
    )];
    for (_, label, flag) in COMMANDS {
        out.push((
            format!(".local/share/nautilus/scripts/{label}").into(),
            format!("#!/bin/sh\n# Pixel Studio Nautilus script\nfor f in \"$@\"; do {} {flag} \"$f\" & done\n", sh_quote(exe)),
            true,
        ));
    }
    let dolphin = format!(
        "[Desktop Entry]\nType=Service\nServiceTypes=KonqPopupMenu/Plugin\nMimeType={mimes}\nActions=edit;view;\nX-KDE-Priority=TopLevel\n\n\
[Desktop Action edit]\nName=Edit with Pixel Studio\nName[zh_TW]=用 Pixel Studio 編輯\nIcon=applications-graphics\nExec={q} --edit %f\n\n\
[Desktop Action view]\nName=Open with Pixel Studio\nName[zh_TW]=用 Pixel Studio 開啟\nIcon=applications-graphics\nExec={q} --view %f\n"
    );
    for dir in [
        ".local/share/kio/servicemenus",
        ".local/share/kservices5/ServiceMenus",
    ] {
        out.push((
            format!("{dir}/pixel-studio.desktop").into(),
            dolphin.clone(),
            true,
        ));
    }
    let exts = EXTENSIONS.join(";");
    for (id, label, flag) in COMMANDS {
        out.push((
            format!(".local/share/nemo/actions/pixel-studio-{id}.nemo_action").into(),
            format!("[Nemo Action]\nActive=true\nName={label}\nComment={label}\nExec={q} {flag} %F\nIcon-Name=applications-graphics\nSelection=notnone\nExtensions={exts};\n"),
            false,
        ));
    }
    out
}

fn linux_extra_dirs() -> Vec<PathBuf> {
    Vec::new()
}

// ------------------------------------------------------------------ macOS

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const IMAGE_UTIS: &[&str] = &[
    "public.png",
    "public.jpeg",
    "com.microsoft.bmp",
    "com.compuserve.gif",
    "com.microsoft.ico",
    "org.webmproject.webp",
    "public.tiff",
];

pub fn macos_workflow_name(label: &str) -> String {
    format!("{APP_NAME} - {}", label.trim_end_matches('…'))
}

/// Generate Quick Action bundles: files under ~/Library/Services/<name>.workflow/Contents/.
pub fn macos_files(exe: &str) -> Vec<GenFile> {
    let mut out = Vec::new();
    for (_, label, flag) in COMMANDS {
        let name = macos_workflow_name(label);
        let dir = PathBuf::from(format!("Library/Services/{name}.workflow/Contents"));
        let types: Vec<&str> = IMAGE_UTIS.to_vec();
        let types_xml: String = types
            .iter()
            .map(|t| format!("\t\t\t\t<string>{t}</string>\n"))
            .collect();
        let info = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{name}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
{types_xml}			</array>
		</dict>
	</array>
</dict>
</plist>
"#,
            name = xml_escape(&name)
        );
        let cmd = format!(
            "for f in \"$@\"; do {} {flag} \"$f\" &amp;&gt;/dev/null &amp; done",
            xml_escape(&sh_quote(exe))
        );
        let wflow = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>523</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{cmd}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/bash</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>6E4C5C38-0D4B-4B39-9A0B-1B5E0B6B0001</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
				</array>
				<key>OutputUUID</key>
				<string>6E4C5C38-0D4B-4B39-9A0B-1B5E0B6B0002</string>
				<key>UUID</key>
				<string>6E4C5C38-0D4B-4B39-9A0B-1B5E0B6B0003</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict/>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:253.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<integer>0</integer>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#
        );
        out.push((dir.join("Info.plist"), info, false));
        out.push((dir.join("document.wflow"), wflow, false));
    }
    out
}

fn macos_extra_dirs() -> Vec<PathBuf> {
    COMMANDS
        .iter()
        .map(|(_, l, _)| {
            PathBuf::from(format!(
                "Library/Services/{}.workflow",
                macos_workflow_name(l)
            ))
        })
        .collect()
}

// ------------------------------------------------------------------ file helpers

// ------------------------------------------------------------------ file install helpers

pub fn write_files(home: &Path, files: &[GenFile]) -> std::io::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for (rel, content, exec) in files {
        let p = home.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&p, content)?;
        #[cfg(unix)]
        if *exec {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))?;
        }
        #[cfg(not(unix))]
        let _ = exec;
        written.push(p);
    }
    Ok(written)
}

pub fn remove_files(home: &Path, files: &[GenFile], dirs: &[PathBuf]) -> std::io::Result<()> {
    for (rel, _, _) in files {
        let _ = std::fs::remove_file(home.join(rel));
    }
    for d in dirs {
        let p = home.join(d);
        if p.exists() {
            std::fs::remove_dir_all(p)?;
        }
    }
    Ok(())
}

fn home() -> std::io::Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| std::io::Error::other("HOME is not set"))
}

fn exe_path() -> std::io::Result<String> {
    let p = std::env::current_exe()?;
    let p = std::fs::canonicalize(&p).unwrap_or(p);
    let s = p.display().to_string();
    Ok(s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s))
}

#[allow(unused)]
fn run_quiet(cmd: &str, args: &[&str]) {
    let _ = std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

pub const WIN_BASE: &str = r"Software\Classes";

/// Install integration for the current user. `zh` selects Chinese menu labels on Windows.
pub fn register(zh: bool) -> std::io::Result<String> {
    let exe = exe_path()?;
    #[cfg(windows)]
    {
        win::apply(&exe, WIN_BASE, zh)?;
        Ok(format!(
            "Explorer context menu and Open With registered under HKCU\\{WIN_BASE} for {exe}"
        ))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = zh;
        let h = home()?;
        let w = write_files(&h, &macos_files(&exe))?;
        run_quiet("/System/Library/CoreServices/pbs", &["-update"]);
        Ok(format!(
            "Installed {} Finder Quick Action files in ~/Library/Services",
            w.len()
        ))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = zh;
        let h = home()?;
        let w = write_files(&h, &linux_files(&exe))?;
        run_quiet(
            "update-desktop-database",
            &[&h.join(".local/share/applications").display().to_string()],
        );
        Ok(format!(
            "Installed {} files (.desktop, Nautilus scripts, Dolphin service menus, Nemo actions)",
            w.len()
        ))
    }
}

pub fn unregister() -> std::io::Result<String> {
    #[cfg(windows)]
    {
        win::remove(WIN_BASE)?;
        Ok("Explorer context menu and Open With entries removed".into())
    }
    #[cfg(target_os = "macos")]
    {
        let h = home()?;
        remove_files(&h, &macos_files(""), &macos_extra_dirs())?;
        run_quiet("/System/Library/CoreServices/pbs", &["-update"]);
        Ok("Finder Quick Actions removed".into())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let h = home()?;
        remove_files(&h, &linux_files(""), &linux_extra_dirs())?;
        run_quiet(
            "update-desktop-database",
            &[&h.join(".local/share/applications").display().to_string()],
        );
        Ok("Linux file-manager integration removed".into())
    }
}

pub fn is_registered() -> bool {
    #[cfg(windows)]
    {
        win::exists(WIN_BASE)
    }
    #[cfg(target_os = "macos")]
    {
        home()
            .map(|h| h.join(&macos_extra_dirs()[0]).exists())
            .unwrap_or(false)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        home()
            .map(|h| {
                h.join(".local/share/applications/pixel-studio.desktop")
                    .exists()
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_registry_layout() {
        let keys = windows_keys(
            r"C:\Program Files\Pixel Studio\pixel-studio.exe",
            WIN_BASE,
            false,
        );
        let find = |p: &str| {
            keys.iter()
                .find(|k| k.path == p)
                .unwrap_or_else(|| panic!("missing {p}"))
        };
        let png = find(r"Software\Classes\SystemFileAssociations\.png\shell\PixelStudio.edit");
        assert!(
            png.values
                .contains(&("MUIVerb".into(), "Edit with Pixel Studio".into()))
        );
        let cmd =
            find(r"Software\Classes\SystemFileAssociations\.webp\shell\PixelStudio.open\command");
        assert_eq!(
            cmd.values[0].1,
            r#""C:\Program Files\Pixel Studio\pixel-studio.exe" --view "%1""#
        );
        let owp = find(r"Software\Classes\.png\OpenWithProgids");
        assert_eq!(owp.values[0].0, PROG_ID);
        // never touches the default handler of .png
        assert!(!keys.iter().any(|k| k.path == r"Software\Classes\.png"));
        find(r"Software\Classes\Applications\pixel-studio.exe\SupportedTypes");
        let zh = windows_keys("x", WIN_BASE, true);
        assert!(
            zh.iter()
                .any(|k| k.values.iter().any(|v| v.1 == "用 Pixel Studio 編輯"))
        );
        assert_eq!(windows_roots(WIN_BASE).len(), EXTENSIONS.len() * 2 + 2);
    }

    #[test]
    fn linux_files_roundtrip() {
        let files = linux_files("/opt/Pixel Studio/pixel-studio");
        let d = &files[0].1;
        assert!(d.contains("MimeType=image/png;") && d.contains("image/webp"));
        assert!(d.contains("Exec=\"/opt/Pixel Studio/pixel-studio\" --edit %F"));
        let t = std::env::temp_dir().join(format!("ps-shell-{}", std::process::id()));
        write_files(&t, &files).unwrap();
        assert!(
            t.join(".local/share/nautilus/scripts/Edit with Pixel Studio")
                .exists()
        );
        remove_files(&t, &files, &linux_extra_dirs()).unwrap();
        assert!(
            !t.join(".local/share/applications/pixel-studio.desktop")
                .exists()
        );
        let _ = std::fs::remove_dir_all(t);
    }

    #[test]
    fn macos_quick_actions() {
        let files = macos_files("/Applications/Pixel Studio.app/Contents/MacOS/pixel-studio");
        assert_eq!(files.len(), COMMANDS.len() * 2);
        assert!(files.iter().any(|f| f.1.contains("org.webmproject.webp")));
        assert!(files.iter().any(|f| f.1.contains("--edit")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_registry_roundtrip_in_test_hive() {
        let base = r"Software\PixelStudioTest\Classes";
        win::apply(r"C:\x\pixel-studio.exe", base, false).unwrap();
        assert!(win::exists(base));
        let v = win::read_default(&format!(
            r"{base}\SystemFileAssociations\.png\shell\PixelStudio.edit\command"
        ))
        .unwrap();
        assert_eq!(v, r#""C:\x\pixel-studio.exe" --edit "%1""#);
        win::remove(base).unwrap();
        assert!(!win::exists(base));
        let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .delete_subkey_all(r"Software\PixelStudioTest");
    }
}
