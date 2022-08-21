use std::collections::BinaryHeap;

use bevy_utils::HashSet;
use flexstr::SharedStr;
use glam::Vec3;
use hecs::Entity;
use shared::{protocol::{NetworkId, RawNetworkId}, bits_and_bytes::ByteWriter};
use tokio::sync::mpsc::UnboundedSender;

use anyhow::Result;

use crate::{
    components::{OldPosition, Position, HeadYawPitch, self, PlayerBundle, YawPitch, Username, PlayerId},
    networking::{NetHandle, PlayersChanged, LoginResponse, client_connection::entity_state::{EntityStateMsg, EntityStateOut}},
    resources::Resources,
};

struct Channels {
    chat: Vec<Option<UnboundedSender<SharedStr>>>,
}

struct EntityStateTracker {
    player_entity: Entity,
    entities: HashSet<Entity>,
    entity_state_channel: UnboundedSender<EntityStateOut>,

    last_player_input_tag: Option<u16>,
    packets_lost: u8,
}

// A main-thread controller for anything related to networking.
pub struct Network {
    // A handle to the network thread
    handle: NetHandle,
    entity_mapping: NidEntityMapping,
    network_id_allocator: IdAllocator,
    player_id_allocator: IdAllocator,

    channels: Channels,
    entity_trackers: Vec<Option<EntityStateTracker>>,

    entity_state_buf: Vec<(NetworkId, EntityStateMsg)>,
}

impl Network {
    pub fn network_thread_alive(&self) -> bool {
        !self.handle.closed()
    }

    pub fn track_entity_add(&mut self, new_entity: Entity, nid: NetworkId) -> anyhow::Result<()> {
        self.entity_mapping.add_mapping(nid, new_entity)
    }

    pub fn track_entity_remove(&mut self, nid: NetworkId) -> anyhow::Result<Entity> {
        self.entity_mapping.remove_mapping(nid)
    }

    pub fn broadcast_chat(&mut self, message: SharedStr) {
        for channel in self.channels.chat.iter_mut().flatten() {
            if let Err(e) = channel.send(message.clone()) {
                eprintln!("Failed to send chat message: {e}");
            }
        }
    }
}


pub fn tick(res: &mut Resources) -> anyhow::Result<()> {
    // Process any incoming login attempts and add new players to the server
    poll_joins(res)?;
    // Broadcast recent chat messages to everybody
    broadcast_chat_messages(res);
    // Process received player state messages (position, facing)
    // Should be before `update_entity_trackers` to immediately send back
    // the tag of the most recently processed input
    process_player_state(res);    
    // For each player: 
    // - detect entities the player can now see that it previously couldn't and send spawn message,
    // - detect entities the player can no longer see, send despawn message
    // - send entity data update message for each currently visible entity
    update_entity_trackers(res);

    Ok(())
}

fn process_player_state(res: &mut Resources) {
    let net = &mut res.net;
    let handle = &mut net.handle;
    while let Ok((nid, packet_loss, msg)) = handle.channels.player_state_recv.try_recv() {
        let Some(entity) = net.entity_mapping.get(nid) else {
            continue; // Fine: might have just disconnected
        };
        let entity = res.main_world.entity(entity).unwrap();

        if let Some(tracker) = net.entity_trackers[entity.get::<&PlayerId>().unwrap().raw() as usize].as_mut() {
            tracker.last_player_input_tag = Some(msg.tag);
            tracker.packets_lost = tracker.packets_lost.wrapping_add(packet_loss as u8);
        }

        if let Some(delta) = msg.delta_pos {
            let mut pos = entity.get::<&mut Position>().unwrap();
            pos.0 += delta;
            //println!("Pos @ {}: {:.8}, {:.8}, {:.8}", msg.tag, o.x, o.y, o.z);
            //println!("Delta for tick {}: {:.8}, {:.8}, {:.8}, pos {:.8}, {:.8}, {:.8}", msg.tick, delta.x, delta.y, delta.z, pos.0.x, pos.0.y, pos.0.z);
        }

        if let Some(delta) = msg.delta_yaw_pitch {
            let mut rot = entity.get::<&mut HeadYawPitch>().unwrap();
            rot.value += delta;
            rot.delta += delta;

            //println!("Rot delta for tick {}: {:.8}, {:.8}, rot: {:.8}, {:.8}", msg.tick, delta.x.to_degrees(), delta.y.to_degrees(), rot.0.x.to_degrees(), rot.0.y.to_degrees());
        }
    }
}

fn update_entity_trackers(res: &mut Resources) {
    const ADD_THRESHOLD_SQ : f32 = 144.0 * 144.0;
    const REMOVE_THRESHOLD_SQ : f32 = 160.0 * 160.0;

    // TODO: O(n²). This ought to change once chunks are a thing and tracking of adds/removes can be done
    // when an entity crosses a chunk boundary, after which it is enough to iterate over only seen entities.
    // At that point, consider replacing HashSet with a dense tree structure (such as binary heap modified to
    // remove duplicates)
    let buf = &mut res.net.entity_state_buf;
    
    for tracker in res.net.entity_trackers.iter_mut().flatten() {
        let player_pos = res.main_world.get::<&Position>(tracker.player_entity).unwrap().0;
        
        buf.clear();
        for (entity, (&Position(position), &OldPosition(old_position), &id, &head_rotation)) 
            in res.main_world.query_mut::<(&Position, &OldPosition, &NetworkId, &HeadYawPitch)>() {
            let d = player_pos.distance_squared(position);
            if d < ADD_THRESHOLD_SQ && tracker.entities.insert(entity) {
                // Newly tracked, send spawn packet
                buf.push((id, EntityStateMsg::EntityAdded {
                    position, 
                    head_rotation: head_rotation.value 
                }));
                println!("Adding entity {entity:?} to player {:?}'s tracker (d={d})", tracker.player_entity);
            } 
            else if d > REMOVE_THRESHOLD_SQ && tracker.entities.remove(&entity) {
                buf.push((id, EntityStateMsg::EntityRemoved));
                println!("Removing entity {entity:?} from player {:?}'s tracker (d={d})", tracker.player_entity);
            } 
            else if tracker.entities.contains(&entity) {
                buf.push((id, EntityStateMsg::EntityMoved { 
                    delta_pos: position - old_position, 
                    delta_head_rotation: head_rotation.delta 
                }));
            }
        }

        let msg = EntityStateOut {
            player_input_tag: tracker.last_player_input_tag,
            packets_lost: tracker.packets_lost,
            player_pos,
            player_head_rot: res.main_world.get::<&HeadYawPitch>(tracker.player_entity).unwrap().value,
            changes: buf.clone(), // Does not allocate if empty
        };
        
        if tracker.entity_state_channel.send(msg).is_err() {
            eprintln!("Failed to send entity state");
        }

        tracker.last_player_input_tag = None;
        tracker.packets_lost = 0;
    }
}

fn broadcast_chat_messages(res: &mut Resources) {
    while let Ok((_, message)) = res.net.handle.channels.chat_recv.try_recv() {
        res.net.broadcast_chat(message);
    }
}

fn poll_joins(res: &mut Resources) -> anyhow::Result<()> {
    let net = &mut res.net;
    while let Some(evt) = net.handle.poll_joins() {
        match evt {
            PlayersChanged::LoginRequest { channel, username: _ } => {
                let id = NetworkId::from_raw(net.network_id_allocator.allocate() as RawNetworkId);

                let mut response_buf = [0u8; 128];
                let mut writer = ByteWriter::new_for_message(&mut response_buf);
                writer.write_u16(id.raw() as u16);
                writer.write_f32(0.0); // X
                writer.write_f32(0.0); // Y
                writer.write_f32(0.0); // Z
                writer.write_f32(0.0); // Yaw
                writer.write_f32(0.0); // Pitch
                writer.write_u64(0); // World seed
                writer.write_message_len();

                if channel.send((id, LoginResponse::Success(writer.bytes().into()))).is_err() {
                    eprintln!("Failed to send network id to network thread!");
                }
            }
            PlayersChanged::Connected {
                username,
                network_id,
                channels,
            } => {
                println!("Player login finished! Username: {username}, network id: {network_id}");

                net.broadcast_chat(format!("{username} joined").into());

                let player_id = PlayerId::from_raw(net.player_id_allocator.allocate() as _);
                let entity = components::spawn_player(&mut res.main_world, PlayerBundle {
                    nid: network_id,
                    player_id,
                    username,
                    position: Vec3::ZERO,
                    head_rotation: YawPitch::ZERO,
                });
                net.track_entity_add(entity, network_id)?;
                place_at(&mut net.channels.chat, player_id.raw() as usize, Some(channels.chat_send));
                place_at(&mut net.entity_trackers, player_id.raw() as usize, Some(EntityStateTracker {
                    player_entity: entity,
                    entities: HashSet::new(),
                    entity_state_channel: channels.entity_state,
                    last_player_input_tag: None,
                    packets_lost: 0
                }));
            }
            PlayersChanged::Disconnect { network_id } => {
                let entity = net.track_entity_remove(network_id)?;
                net.network_id_allocator.free(network_id.raw() as u16);
                println!("Player with network id {network_id} disconnected");

                let player_id = *res.main_world.get::<&PlayerId>(entity).unwrap();
                let username = &res.main_world.remove_one::<Username>(entity).unwrap().0;

                net.broadcast_chat(format!("{username} disconnected").into());

                place_at(&mut net.channels.chat, player_id.raw() as usize, None);
                place_at(&mut net.entity_trackers, player_id.raw() as usize, None);
                if res.main_world.despawn(entity).is_err() {
                    eprintln!("disconnect: entity was already despawned");
                }
            }
        }
    }
    Ok(())
}

fn place_at<T>(vec: &mut Vec<T>, idx: usize, t: T) {
    debug_assert!(idx <= vec.len(), "idx = {idx}, vec.len() = {}", vec.len());
    if idx >= vec.len() {
        vec.push(t);
    } else {
        vec[idx] = t;
    }
}

pub struct NidEntityMapping {
    mapping: Vec<(NetworkId, Entity)>,
}

impl NidEntityMapping {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            mapping: Vec::with_capacity(capacity)
        }
    }

    pub fn add_mapping(&mut self, id: NetworkId, entity: Entity) -> anyhow::Result<()> {
        if self.mapping.len() <= id.raw() as usize {
            self.mapping.resize(id.raw() as usize + 1, (NetworkId::INVALID, Entity::DANGLING));
        }

        if self.mapping[id.raw() as usize].0 != NetworkId::INVALID {
            anyhow::bail!("Id {id} is already mapped to an entity!");
        }

        self.mapping[id.raw() as usize] = (id, entity);
        Ok(())
    }

    pub fn remove_mapping(&mut self, id: NetworkId) -> anyhow::Result<Entity> {
        let idx = id.raw() as usize;
        if idx >= self.mapping.len() || self.mapping[idx].0 != id {
            anyhow::bail!("remove_mapping(): was mapped to ({}, {:?}) instead of input ({})",
                self.mapping[idx].0, self.mapping[idx].1,
                id,
            );
        }

        Ok(std::mem::replace(&mut self.mapping[idx], (NetworkId::INVALID, Entity::DANGLING)).1)
    }

    pub fn get(&self, id: NetworkId) -> Option<Entity> {
        let (mapped_id, entity) = self.mapping[id.raw() as usize];
        if mapped_id != id {
            None
        } else {
            Some(entity)
        }
    }
}

pub struct IdAllocator {
    recycled_ids: BinaryHeap<i16>,

    // grows monotonically => always guaranteed to be unused
    // id 0 is never assigned to anything and is reserved as 'invalid'
    next_unused_id: u16,

    #[cfg(debug_assertions)]
    allocated_ids: HashSet<u16>,
}

impl IdAllocator {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            recycled_ids: BinaryHeap::with_capacity(capacity),
            next_unused_id: 1,

            #[cfg(debug_assertions)]
            allocated_ids: HashSet::new(),
        }
    }

    pub fn allocate(&mut self) -> u16 {
        let id = self.recycled_ids
            .pop()
            .map(|neg| (-neg) as u16)
            .unwrap_or_else(|| {
                self.next_unused_id += 1;
                self.next_unused_id - 1
            });

        if cfg!(debug_assertions) {
            debug_assert!(self.allocated_ids.insert(id), "Returned an already-allocated ID!");
        }

        id
    }

    pub fn free(&mut self, id: u16) {
        debug_assert!(id < self.next_unused_id);
        self.recycled_ids.push(-(id as i16)); // reverse order to make min heap

        if cfg!(debug_assertions) {
            debug_assert!(self.allocated_ids.remove(&id), "Tried to remove an ID ({id}) that was not allocated!");
        }
    }
}

#[derive(Debug)]
pub struct PlayerChannels {
    pub chat_send: UnboundedSender<SharedStr>,
    pub entity_state: UnboundedSender<EntityStateOut>,
}

pub fn init() -> Result<Network> {
    Ok(Network {
        handle: crate::networking::init()?,
        entity_mapping: NidEntityMapping::with_capacity(128),
        network_id_allocator: IdAllocator::with_capacity(128),
        player_id_allocator: IdAllocator::with_capacity(8),
        channels: Channels {
            chat: vec![None]
        },
        entity_trackers: vec![None],
        entity_state_buf: Vec::new(),
    })
}
