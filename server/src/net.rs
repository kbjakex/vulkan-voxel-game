use flexstr::{SharedStr, ToSharedStr};
use glam::Vec3;
use hecs::{Entity, World};
use shared::protocol::{NetworkId, RawNetworkId};
use tokio::sync::mpsc::UnboundedSender;

use anyhow::Result;

use crate::{
    components::Position,
    networking::{NetHandle, PlayersChanged},
    resources::Resources,
    Username,
};

// A main-thread controller for anything related to networking.
pub struct Network {
    // A handle to the network thread
    handle: NetHandle,
    pub id_manager: NetworkIdManager,
}

impl Network {
    pub fn network_thread_alive(&self) -> bool {
        !self.handle.closed()
    }
}

pub fn broadcast_chat(message: SharedStr, world: &mut World) {
    for (_, connection) in world.query_mut::<&mut PlayerConnection>() {
        if let Err(e) = connection.chat_send.send(message.clone()) {
            eprintln!("Failed to send chat message: {e}");
        }
    }
}

pub fn tick(res: &mut Resources) {
    poll_joins(res);

    let handle = &mut res.net.handle;
    while let Ok((_, message)) = handle.channels.chat_recv.try_recv() {
        broadcast_chat(message, &mut res.main_world);
    }

    while let Ok((id, msg)) = handle.channels.player_state_recv.try_recv() {
        let Some(entity) = res.net.id_manager.get_entity(id) else {
            eprintln!("ERROR: Received PlayerStateMsg from player with no entity mapping! Network id: {}", id.raw());
            continue;
        };

        if let Some(delta) = msg.delta_pos {
            let mut pos = res.main_world.get::<&mut Position>(entity).unwrap();
            pos.xyz += delta;
            println!("Position of player #{}: ({:.8}, {:.8}, {:.8})", id.raw(), pos.xyz.x, pos.xyz.y, pos.xyz.z);
        }

        /* for (_, connection) in res.main_world.query_mut::<&mut PlayerConnection>() {
            if let Err(e) = connection.pl.send(message.clone()) {
                eprintln!("Failed to send chat message: {e}");
            }
        } */
    }
}

fn poll_joins(res: &mut Resources) {
    let net = &mut res.net;
    while let Some(evt) = net.handle.poll_joins() {
        match evt {
            PlayersChanged::NetworkIdRequest { channel } => {
                let player_entity = res.main_world.spawn(());
                let id = net.id_manager.allocate_for(player_entity);

                if let Err(e) = channel.try_send(id) {
                    eprintln!("Failed to send network id to network thread: {e}");
                }
            }
            PlayersChanged::Connected {
                username,
                network_id,
                channels,
            } => {
                println!(
                    "Player login finished! Username: {username}, network id: {}",
                    network_id.raw()
                );

                let Some(entity) = net.id_manager.get_entity(network_id) else {
                    eprintln!("player login finished, but id -> entity mapping has been removed?!");
                    continue;
                };

                broadcast_chat(
                    format!("{username} joined").to_shared_str(),
                    &mut res.main_world,
                );

                if res
                    .main_world
                    .insert(entity, (network_id, Username(username), channels, Position{ xyz: Vec3::ZERO }))
                    .is_err()
                {
                    eprintln!("Entity was removed from world when player was connecting?!");
                }
            }
            PlayersChanged::Disconnect { network_id } => {
                let entity = net.id_manager.free(network_id);
                println!("Player with network id {network_id} disconnected");

                let username = &res
                    .main_world
                    .get::<&Username>(entity)
                    .unwrap()
                    .0
                    .clone();

                broadcast_chat(
                    format!("{username} disconnected").to_shared_str(),
                    &mut res.main_world,
                );

                if res.main_world.despawn(entity).is_err() {
                    eprintln!("ERR: disconnect: entity was already despawned");
                }
            }
        }
    }
}

// Manages allocating and freeing network ids, and provides
// a mapping from network id to the entity.
//
// Importantly, makes sure that
//  1. Network IDs are always unique.
//  2. Network IDs are always densely allocated.
//     If ID 533 is allocated, then every ID before that
//     should also be allocated for some entity currently
//     in the world.
//
// Low-level tool: one must not forget to deallocate the ID.
pub struct NetworkIdManager {
    recycled_ids: Vec<NetworkId>,

    // grows monotonically => always guaranteed to be unused
    // id 0 is never assigned to anything and is reserved as 'invalid'
    next_unused: RawNetworkId,

    // Mapping from NetworkId to Entity
    mapping: Vec<(NetworkId, Entity)>,
}

impl Default for NetworkIdManager {
    fn default() -> Self {
        Self {
            recycled_ids: Vec::with_capacity(128),
            next_unused: 1,
            mapping: vec![(NetworkId::from_raw(0), Entity::DANGLING); 128],
        }
    }
}

impl NetworkIdManager {
    // Allocates a unique network ID for the entity.
    pub fn allocate_for(&mut self, entity: Entity) -> NetworkId {
        let id = self.recycled_ids.pop().unwrap_or_else(|| {
            self.next_unused += 1;
            NetworkId::from_raw(self.next_unused - 1)

        });

        self.mapping[id.raw() as usize] = (id, entity);

        id
    }

    pub fn free(&mut self, id: NetworkId) -> Entity {
        debug_assert!(id.raw() < self.next_unused);
        debug_assert!(!self.recycled_ids.contains(&id));
        debug_assert!(self.mapping[id.raw() as usize].0 == id);

        let entity = self.mapping[id.raw() as usize].1;

        self.recycled_ids.push(id);
        self.mapping[id.raw() as usize] = (NetworkId::from_raw(0), Entity::DANGLING);
        entity
    }

    pub fn get_entity(&self, id: NetworkId) -> Option<Entity> {
        let (mapped_id, entity) = self.mapping[id.raw() as usize];
        if mapped_id != id {
            None
        } else {
            Some(entity)
        }
    }
}

#[derive(Debug)]
pub struct PlayerConnection {
    pub chat_send: UnboundedSender<SharedStr>,
    pub entity_state: UnboundedSender<Vec<u8>>,
}

pub fn init() -> Result<Network> {
    Ok(Network {
        handle: crate::networking::init()?,
        id_manager: NetworkIdManager::default(),
    })
}
