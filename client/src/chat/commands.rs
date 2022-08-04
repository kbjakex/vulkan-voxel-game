use hecs::World;

use crate::{
    renderer::text_renderer::TextColor, resources::game_state,
};

use super::Chat;

pub fn process_commands(world: &mut World, time_secs: f32, res: &mut game_state::Resources) {
    process_commands_inner(world, time_secs, &mut res.chat);
}

fn process_commands_inner(world: &mut World, time_secs: f32, chat: &mut Chat) {
    if chat.unprocessed_commands.is_empty() {
        return;
    }

    let chat = &mut *chat;

    let commands = chat.unprocessed_commands.drain(..).collect::<Vec<String>>();
    for command in &commands {
        process_command(world, chat, time_secs, &command[1..]);
    }
}

fn process_command(_world: &mut World, chat: &mut Chat, time: f32, cmd: &str) {
    let parts = cmd.split_ascii_whitespace().collect::<Vec<&str>>();
    let (cmd_name, _args) = match parts.split_first() {
        Some((name, args)) => (*name, args),
        None => return,
    };

    match cmd_name {
        /* "join" => process_cmd_join(world, chat, args, time),
        "disconnect" => process_cmd_disconnect(world, chat, args, time), */
        _ => unknown_command(chat, cmd_name, time),
    }
}

/* fn process_cmd_disconnect(chat: &mut Chat, args: &[&str], time: f32) {
    let connection = &mut *world.get_resource_mut::<Connection>().unwrap();
    if !connection.connected() {
        crate::chat!(RGBA(0xFF_22_22_FF), "Nothing to disconnect from!");
        return;
    }

    if !connection.disconnect() {
        crate::chat!(RGBA(0xFF_22_22_FF), "This feature appears to be broken..");
    } else {
        crate::chat!("Left the server.");
    }
} */

/* fn process_cmd_join(world: &mut World, chat: &mut Chat, args: &[&str], time: f32) {
    let (ip, username) = match args.len() {
        2 => (args[0], args[1]),
        _ => {
            chat.add_chat_entry(
                None,
                "Usage: /join <ip> <username>".to_owned(),
                TextColor::from_rgba32(0xFF_22_22_FF),
                time,
            );
            return;
        }
    };

    let address = match ip.parse::<SocketAddr>() {
        Ok(address) => address,
        Err(e) => {
            if ip == "." {
                "127.0.0.1:29477".parse().unwrap()
            } else {
                Chat::write(format!("Invalid address: Error: {}", e).to_shared_str(), TextColor::from_rgba32(0xFF_22_22_FF));
                return;
            }
        }
    };

    let username = username.to_owned();
    world.get_resource_mut::<Username>().unwrap().0 = username.clone();

    let connection = &mut *world.get_resource_mut::<Connection>().unwrap();
    if connection.connected() {
        crate::chat!("Disconnecting...");
        connection.disconnect();
    }

    connection.username = username;
    connection.connect(address);
} */

fn unknown_command(chat: &mut Chat, cmd_name: &str, time: f32) {
    chat.add_chat_entry(
        None,
        format!("Unknown command '{}'", cmd_name),
        TextColor::from_rgba32(0xFF_22_22_FF),
        time,
    );
}
