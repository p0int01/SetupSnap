// Injected at the head of Game::native_command (hook "command", API 35).
// A command typed in chat reaches here without its slash, before the server
// or Horizon's own commands see it; setting `ran` keeps it from them.
fn setupsnap_command(
    game: &mut crate::game::Game<'_>,
    command: &str,
    ran: &mut bool,
) {
    // config/ sits beside options.txt, so its parent is the instance folder:
    // the same folder that holds the profiles the Fabric mod reads.
    let Ok(config_dir) = crate::game::Game::native_config_dir() else {
        return;
    };
    let Some(instance) = config_dir.parent() else {
        return;
    };

    let Some(lines) = crate::game::setupsnap::handle_command(instance, command) else {
        return;
    };

    *ran = true;
    for line in lines {
        game.native_chat(&line);
    }
}
