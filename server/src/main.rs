#![feature(let_else)]

pub mod game_builder;
pub mod networking;
pub mod server;
pub mod resources;
pub mod components;
pub mod net;

use std::{
    time::{Duration, Instant}, sync::atomic::{AtomicBool, Ordering},
};

use flexstr::SharedStr;
use shared::TICK_DURATION;

pub struct Username(pub SharedStr);

/* fn players_changed_listener(world: &mut World, resources: &mut Resources) {
    while let Some(message) = resources.net.handle.poll_joins() {
        match message {
            networking::PlayersChanged::NetworkIdRequest { channel } => {
                let allocator = &mut resources.net.id_manager;
                let id = allocator.allocate();
                if let Err(e) = channel.try_send(id.raw()) {
                    allocator.free(id);
                    println!("Arbitrary error #141825...");
                }
            }
            networking::PlayersChanged::Connected {
                username,
                network_id,
                chat_send,
                chat_recv,
                entity_state,
                player_state,
            } => {
                let network_id = NetworkId::from_raw(network_id);
                println!(
                    "{} joined the game! Network id: {}",
                    username, network_id.raw()
                );

                let entity = world
                    .spawn((
                        PlayerConnection {
                            chat_recv,
                            chat_send,
                            entity_state,
                            player_state
                        },
                        network_id,
                        Username(username.clone()),
                    ));

                resources.net.id_manager.add_mapping(network_id, entity);

                let broadcast = format!("{} joined", username);

                let mut query = world.query_mut::<(&mut PlayerConnection, &NetworkId)>();
                for (_, (conn, id)) in query {
                    println!("Broadcasting join to #{}", id.raw());
                    if conn.chat_send.send(broadcast.clone()).is_err() {
                        println!("Player with id {} is screwed", id.raw());
                    }
                };
            }
            networking::PlayersChanged::Disconnect { network_id } => {
                let entity = world
                    .get_resource_mut::<NetworkIdToEntityMap>()
                    .unwrap()
                    .mapping[network_id as usize];
                // TODO: "remove" the entity rather than leave a dangling ref

                let username = &world.get::<Username>(entity).unwrap().0;
                println!("Player '{}' disconnected!", username);

                let broadcast = format!("{} left the game", username);

                let mut query = world.query::<(&mut PlayerConnection, &NetworkId)>();
                query.for_each_mut(world, move |(conn, id)| {
                    println!("Broadcasting disconnect to #{}", id.0);
                    if conn.chat_send.send(broadcast.clone()).is_err() {
                        println!("Player with id {} is screwed", id.0);
                    }
                });

                let allocator = &mut resources.net.id_manager;
                allocator.free(NetworkId::from_raw(network_id));

                world.despawn(entity);
            }
        }
    }
}

fn chat_handler(world: &mut World) {
    let mut messages = SmallVec::<[String; 8]>::new();
    world
        .query::<&mut PlayerConnection>()
        .for_each_mut(world, |mut connection| {
            while let Ok(msg) = connection.chat_recv.try_recv() {
                messages.push(msg);
            }
        });
    
    world.query::<&PlayerConnection>().for_each(world, |conn| {
        for msg in &messages {
            let _ = conn.chat_send.send(msg.clone());
        }
    });
} */

pub fn main() {
    runner();
    println!("Server stopped.");
}

fn runner() {
    let mut state = server::init().unwrap();

    static SHOULD_STOP : AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| {
        println!();
        SHOULD_STOP.store(true, Ordering::Relaxed);
    }).unwrap();

    let mut last_sec = Instant::now();
    let mut updates = 0;

    let mut tick = 0u32;
    let server_start_time = Instant::now();
    while !SHOULD_STOP.load(Ordering::Relaxed) {
        server::tick(&mut state);
        tick += 1;

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

        let target = server_start_time + tick * TICK_DURATION;
        if time < target {
            std::thread::sleep(target - time);
        }
    }

    println!("Stopping server...");
    server::shutdown(state);
}

/* let mut app = GameBuilder::new_with_runner(runner);

networking::init(&mut app)?;

     app.add_stage(CoreStage::GameTick, SystemStage::single_threaded())
        .add_stage("Tick Loop", SystemStage::single_threaded())
        .add_system(players_changed_listener.exclusive_system())
        .add_system(chat_handler.exclusive_system())
        .insert_resource(NetworkIdAllocator::default())
        .insert_resource(NetworkIdToEntityMap::default());

    app.run();*/
