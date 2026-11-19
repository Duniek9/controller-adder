use std::path::Path;

pub fn detect(game_path: &Path) -> Option<&'static str> {
    // Games may store their files directly in the
    // selected directory or inside a www folder.
    let roots = [game_path.to_path_buf(), game_path.join("www")];

    for root in roots {
        let js = root.join("js");
        let system = root.join("data").join("System.json");

        // Require the game's database as well as
        // the engine's core JavaScript file.
        if !system.is_file() {
            continue;
        }

        if js.join("rmmz_core.js").is_file() {
            return Some("RPG Maker MZ");
        }

        if js.join("rpg_core.js").is_file() {
            return Some("RPG Maker MV");
        }
    }

    None
}
