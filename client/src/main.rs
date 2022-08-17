#![feature(let_else)]

pub mod assets;
pub mod chat;
pub mod components;
pub mod entities;
pub mod game;
pub mod input;
pub mod networking;
pub mod player;
pub mod renderer;
pub mod resources;
pub mod states;
pub mod text_box;
pub mod world;

use game::Game;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use winit::event_loop::EventLoop;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
pub fn main() {
    /*     tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(tracing_tracy::TracyLayer::new()),
    ).expect("set up the subscriber"); */

    let event_loop = EventLoop::new();
    let mut game = Game::init(&event_loop).unwrap();
    event_loop.run(move |event, _, flow| game.on_event(event, flow));
}
