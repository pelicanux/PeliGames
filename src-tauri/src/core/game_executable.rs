//! Finds the folder of a game's real executable, where DLSSNR must go.
//!
//! Every `.exe` below the game folder gets points instead of trusting folder
//! names alone: installers, redistributables, crash reporters, anti-cheat and
//! pre-launchers are left out; the largest executables, the ones named like
//! the game, Unreal's `-Shipping` build and those linked to a graphics API
//! (from their PE imports) score higher. When a game ships DX11 and DX12
//! builds side by side (The Witcher 3 Next-Gen: `bin/x64` and
//! `bin/x64_dx12`), the DX12 one wins: that is where DLSS and FSR run.
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

const MAX_ENTRIES: usize = 20_000;
const MAX_INSPECTED: usize = 48;
const MIB: u64 = 1024 * 1024;

/// Never the game: installers, runtimes, reporters, anti-cheat services.
const IGNORED_NAMES: [&str; 22] = [
    "unins", "uninstall", "setup", "installer", "vc_redist", "vcredist", "dxsetup", "dotnet", "ue4prereq", "ueprereq",
    "crashreport", "crashhandler", "crashpad", "unitycrashhandler", "easyanticheat", "start_protected_game", "beservice",
    "battleye", "eac_launcher", "qtwebengineprocess", "cefsharp", "zfgamebrowser",
];
/// Usually a helper next to the game rather than the game itself.
const HELPER_NAMES: [&str; 17] = [
    "launcher", "prelauncher", "updater", "patcher", "config", "settings", "crash", "report", "helper",
    "server", "editor", "benchmark", "tool", "redkit", "exporter", "converter", "importer",
];
/// Folders of modding kits and developer tools shipped with some games.
const TOOL_FOLDERS: [&str; 6] = ["tools", "tool", "sdk", "devtools", "modkit", "redkit"];
/// Folders that hold redistributables or third-party tools, never the game.
const IGNORED_FOLDERS: [&str; 12] = [
    "_commonredist", "redist", "redistributables", "directx", "support", "easyanticheat", "battleye",
    "__installer", "installer", "prereqs", "thirdparty", "extras",
];

struct Candidate {
    path: PathBuf,
    size: u64,
    depth: usize,
}

fn normalized(text: &str) -> String {
    let text = text.to_lowercase();
    let text = text.strip_prefix("the ").unwrap_or(&text);
    text.chars().filter(char::is_ascii_alphanumeric).collect()
}

fn candidates(root: &Path, max_depth: usize) -> Vec<Candidate> {
    let mut found = Vec::new();
    let mut pending = VecDeque::from([(root.to_path_buf(), 0usize)]);
    let mut visited = 0;
    while let Some((folder, depth)) = pending.pop_front() {
        let Ok(entries) = fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_ENTRIES {
                return found;
            }
            let Ok(kind) = entry.file_type() else { continue };
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            // Folder links are not followed: they can point anywhere.
            if kind.is_dir() {
                if depth + 1 < max_depth && !IGNORED_FOLDERS.contains(&name.as_str()) {
                    pending.push_back((path, depth + 1));
                }
            } else if name.ends_with(".exe") && !IGNORED_NAMES.iter().any(|ignored| name.contains(ignored)) {
                if let Some(meta) = fs::metadata(&path).ok().filter(|meta| meta.is_file()) {
                    found.push(Candidate { path, size: meta.len(), depth });
                }
            }
        }
    }
    found
}

fn score(candidate: &Candidate, root: &Path, game: &str, largest: u64, imports: Option<&[String]>) -> i64 {
    let stem = candidate.path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let relative = candidate.path.strip_prefix(root).unwrap_or(&candidate.path).to_string_lossy().to_lowercase();
    let folders: Vec<&str> = relative.split('/').rev().skip(1).collect();
    let mut points = 0i64;
    // The game binary is the largest one, give or take a sibling build.
    if largest > 0 && candidate.size * 100 >= largest * 85 {
        points += 30;
    } else if largest > 0 && candidate.size * 2 >= largest {
        points += 10;
    }
    if candidate.size < MIB && largest > 8 * MIB {
        points -= 25; // A small starter beside a much larger game binary.
    }
    if HELPER_NAMES.iter().any(|helper| stem.contains(helper)) {
        points -= 30;
    }
    // Final builds: Unreal's "-Shipping", other engines' "Retail".
    if stem.contains("-shipping") {
        points += 25;
    } else if stem.contains("retail") {
        points += 15;
    }
    if folders.iter().any(|folder| TOOL_FOLDERS.contains(folder)) {
        points -= 40;
    }
    let name = normalized(&stem.replace("-win64-shipping", "").replace("-wingdk-shipping", "").replace("-shipping", ""));
    if name.len() >= 3 && !game.is_empty() && (game.contains(&name) || name.contains(game)) {
        points += 15;
    }
    if folders.windows(2).any(|pair| pair[0] == "win64" && pair[1] == "binaries") {
        points += 8;
    }
    points += 2 * folders.iter().filter(|folder| matches!(**folder, "win64" | "x64" | "bin" | "binaries")).count() as i64;
    if folders.first().is_some_and(|folder| folder.contains("dx12")) {
        points += 6;
    }
    if let Some(dlls) = imports {
        let has = |dll: &str| dlls.iter().any(|name| name == dll);
        let graphics = [("d3d12.dll", 12), ("d3d11.dll", 6), ("vulkan-1.dll", 6), ("dxgi.dll", 3), ("d3d9.dll", 2)];
        let api: i64 = graphics.iter().filter(|(dll, _)| has(dll)).map(|(_, value)| value).sum();
        // Games also load their renderer at run time; only a binary with no
        // graphics import at all and no other strong sign is a likely helper.
        points += if api == 0 && candidate.size < 16 * MIB { -10 } else { api };
    }
    points
}

/// The folder of the game's executable below `root`, if any.
pub fn best_executable_dir(root: &Path, max_depth: usize) -> Option<PathBuf> {
    let mut found = candidates(root, max_depth);
    if found.is_empty() {
        return None;
    }
    let game = root.file_name().map(|name| normalized(&name.to_string_lossy())).unwrap_or_default();
    let largest = found.iter().map(|candidate| candidate.size).max().unwrap_or(0);
    // Reading imports costs a few kilobytes per file: only the larger ones.
    found.sort_by(|a, b| b.size.cmp(&a.size));
    let ranked = found.iter().enumerate().map(|(index, candidate)| {
        let imports = (index < MAX_INSPECTED).then(|| super::pe_imports::imports(&candidate.path)).flatten();
        (score(candidate, root, &game, largest, imports.as_deref()), candidate)
    });
    // Ties: the larger binary, then the one closer to the game folder.
    let (_, best) = ranked.max_by(|(a, x), (b, y)| a.cmp(b).then(x.size.cmp(&y.size)).then(y.depth.cmp(&x.depth)))?;
    best.path.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pe_imports::tests::fake_pe;

    struct Game(PathBuf);
    impl Game {
        fn new(name: &str) -> Self {
            Game(std::env::temp_dir().join(format!("peligames-exe-{}", uuid::Uuid::new_v4())).join(name))
        }
        fn exe(&self, relative: &str, imports: &[&str], size_mib: usize) -> &Self {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, fake_pe(imports, &[], size_mib * MIB as usize + 4096)).unwrap();
            self
        }
        fn best(&self) -> String {
            best_executable_dir(&self.0, 5).unwrap().strip_prefix(&self.0).unwrap().to_string_lossy().into_owned()
        }
    }
    impl Drop for Game {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    /// Read-only: the folder chosen for every installed Steam game, with the
    /// previous keyword rule beside it. Run by hand to review real libraries.
    #[test]
    #[ignore = "reads the real Steam libraries; run by hand"]
    fn compare_with_installed_games() {
        fn keyword_rule(dir: &Path, depth: u8) -> Option<PathBuf> {
            let (mut best, mut best_score) = (None, -1);
            let mut pending = vec![(dir.to_path_buf(), depth)];
            while let Some((current, depth)) = pending.pop() {
                let Ok(entries) = fs::read_dir(&current) else { continue };
                let (mut exe, mut shipping) = (false, false);
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
                        exe = true;
                        shipping |= path.file_stem().is_some_and(|n| n.to_string_lossy().to_lowercase().contains("shipping"));
                    } else if path.is_dir() && depth > 1 {
                        pending.push((path, depth - 1));
                    }
                }
                if exe {
                    let text = current.to_string_lossy().to_lowercase();
                    let score = ["win64", "x64", "binaries", "bin"].iter().filter(|k| text.contains(*k)).count() as i32 + if shipping { 2 } else { 0 };
                    if score > best_score { best_score = score; best = Some(current.clone()); }
                }
            }
            best
        }
        let home = dirs::home_dir().unwrap();
        let mut libraries = vec![home.join(".local/share/Steam/steamapps/common")];
        for mount in fs::read_dir("/mnt").into_iter().flatten().flatten() {
            libraries.push(mount.path().join("SteamLibrary/steamapps/common"));
        }
        for library in libraries.iter().filter(|l| l.is_dir()) {
            for game in fs::read_dir(library).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
                let show = |p: Option<PathBuf>| p.map(|p| p.strip_prefix(&game).map(|r| format!("/{}", r.display())).unwrap_or_default()).unwrap_or_else(|| "—".into());
                let (old, new) = (show(keyword_rule(&game, 4)), show(best_executable_dir(&game, 5)));
                println!("{}\t{}\t{}\t{}", if old == new { "=" } else { "≠" }, game.file_name().unwrap().to_string_lossy(), old, new);
            }
        }
    }

    #[test]
    fn witcher_next_gen_prefers_the_dx12_build() {
        let game = Game::new("The Witcher 3");
        game.exe("REDprelauncher.exe", &["kernel32.dll", "user32.dll"], 3)
            .exe("bin/x64/witcher3.exe", &["kernel32.dll", "d3d11.dll", "dxgi.dll"], 70)
            .exe("bin/x64_dx12/witcher3.exe", &["kernel32.dll", "d3d12.dll", "dxgi.dll"], 74)
            .exe("bin/x64/CrashReporter.exe", &["kernel32.dll"], 2)
            .exe("_CommonRedist/vcredist/vc_redist.x64.exe", &["kernel32.dll"], 24);
        assert_eq!(game.best(), "bin/x64_dx12");
    }

    #[test]
    fn dx12_build_wins_even_when_slightly_smaller() {
        let game = Game::new("Some Game");
        game.exe("bin/x64/game.exe", &["d3d11.dll"], 80).exe("bin/x64_dx12/game.exe", &["d3d12.dll"], 72);
        assert_eq!(game.best(), "bin/x64_dx12");
    }

    #[test]
    fn unreal_uses_the_shipping_binary_not_the_root_starter() {
        let game = Game::new("Clair Obscur Expedition 33");
        game.exe("Expedition33_Steam.exe", &["kernel32.dll"], 0)
            .exe("Sandfall/Binaries/Win64/SandFall-Win64-Shipping.exe", &["kernel32.dll"], 120)
            .exe("Engine/Binaries/ThirdParty/CEF3/Win64/UnrealCEFSubProcess.exe", &["kernel32.dll"], 1)
            .exe("Engine/Extras/Redist/en-us/UEPrereqSetup_x64.exe", &["kernel32.dll"], 40)
            .exe("EasyAntiCheat/EasyAntiCheat_EOS_Setup.exe", &["kernel32.dll"], 5);
        assert_eq!(game.best(), "Sandfall/Binaries/Win64");
    }

    #[test]
    fn unity_game_beside_its_crash_handler() {
        let game = Game::new("Lethal Company");
        game.exe("Lethal Company.exe", &["kernel32.dll"], 0).exe("UnityCrashHandler64.exe", &["kernel32.dll"], 1);
        assert_eq!(game.best(), "");
    }

    #[test]
    fn launcher_in_root_loses_to_the_named_game_binary() {
        let game = Game::new("Cyberpunk 2077");
        game.exe("REDprelauncher.exe", &["kernel32.dll"], 3)
            .exe("bin/x64/Cyberpunk2077.exe", &["kernel32.dll", "d3d12.dll"], 90)
            .exe("bin/x64/REDEngineErrorReporter.exe", &["kernel32.dll"], 4);
        assert_eq!(game.best(), "bin/x64");
    }

    #[test]
    fn modding_tools_shipped_with_the_game_are_not_chosen() {
        let game = Game::new("RoadCraft");
        game.exe("root/bin/pc/EpicOnlineServices/EpicOnlineServicesInstaller.exe", &["kernel32.dll"], 128)
            .exe("root/tools/bin/TerraformingConverter/terraforming_exporter.exe", &["kernel32.dll", "d3d11.dll"], 123)
            .exe("root/bin/pc/Roadcraft ModEditor.exe", &["kernel32.dll", "d3d12.dll"], 120)
            .exe("root/bin/pc/Roadcraft - Retail.exe", &["kernel32.dll", "d3d12.dll"], 86)
            .exe("tools/bin/ResourceManager/LightProbeConverterExe.exe", &["kernel32.dll"], 39);
        assert_eq!(game.best(), "root/bin/pc");
    }

    #[test]
    fn single_executable_in_the_root_and_empty_folders() {
        let game = Game::new("Old Game");
        game.exe("oldgame.exe", &["d3d9.dll"], 6);
        assert_eq!(game.best(), "");
        let empty = Game::new("Nothing");
        fs::create_dir_all(&empty.0).unwrap();
        assert!(best_executable_dir(&empty.0, 5).is_none());
    }
}
