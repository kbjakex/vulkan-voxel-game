#![feature(let_else)]

pub mod game_builder;
pub mod networking;
pub mod server;
pub mod resources;
pub mod components;
pub mod net;

use std::{
    time::{Duration, Instant}, sync::atomic::{AtomicBool, Ordering}, net::SocketAddr,
};

pub fn main() {
    if let Some(address) = get_bind_address() {
        runner(address);
        println!("Server stopped.");
    }
}

fn get_bind_address() -> Option<SocketAddr> {
    if let Some(address) = std::env::args().skip(1).next() {
        match address.parse() {
            Ok(address) => Some(address),
            Err(e) => {
                println!("Invalid bind address '{address}': {e}");
                None
            }
        }
    } else {
        Some("0.0.0.0:29477".parse().unwrap())
    }
}

pub fn runner(address: SocketAddr) {
    let mut state = server::init(address).unwrap();

    println!("Server running @ {}Hz tick rate", shared::TICKS_PER_SECOND);

    static SHOULD_STOP : AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| {
        println!();
        SHOULD_STOP.store(true, Ordering::Relaxed);
    }).unwrap();

    let mut last_sec = Instant::now();
    let mut updates = 0;

    let server_start_time = Instant::now();
    while !SHOULD_STOP.load(Ordering::Relaxed) {
        if let Err(e) = server::tick(&mut state) {
            eprintln!("Error while ticking server: {e}");
        }

        state.current_tick += 1;

        if !state.net.network_thread_alive() {
            println!("Network thread crashed!");
            break;
        }

        updates += 1;

        let time = Instant::now();
        if time - last_sec >= Duration::from_secs(10) {
            /* println!("Updates per second {}", updates as f32 / 10.0); */
            last_sec = time;
            updates = 0;
        }

        let target = server_start_time + state.current_tick * shared::TICK_DURATION;
        if time < target {
            std::thread::sleep(target - time);
        }
    }

    println!("Stopping server...");
    server::shutdown(state);
}

