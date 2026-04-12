#[path = "../../../agent-browser/cli/src/chat.rs"]
mod chat;
#[path = "../../../agent-browser/cli/src/color.rs"]
mod color;
#[path = "../../../agent-browser/cli/src/commands.rs"]
mod commands;
#[path = "../../../agent-browser/cli/src/connection.rs"]
mod connection;
#[path = "../../../agent-browser/cli/src/flags.rs"]
mod flags;
#[path = "../../../agent-browser/cli/src/install.rs"]
mod install;
mod native;
#[path = "../../../agent-browser/cli/src/output.rs"]
mod output;
#[cfg(test)]
#[path = "../../../agent-browser/cli/src/test_utils.rs"]
mod test_utils;
#[path = "../../../agent-browser/cli/src/upgrade.rs"]
mod upgrade;
#[path = "../../../agent-browser/cli/src/validation.rs"]
mod validation;

include!(concat!(env!("OUT_DIR"), "/main_body.rs"));
