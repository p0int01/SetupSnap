// Injected at the head of Game::native_commands (hook "commands", API 35).
// Horizon asks for the names the chat box colours and completes.
fn setupsnap_commands(commands: &mut Vec<&'static str>) {
    crate::game::setupsnap::push_command_names(commands);
}
