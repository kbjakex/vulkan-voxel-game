/*use bevy_ecs::{prelude::World, schedule::{Schedule, IntoSystemDescriptor, StageLabel, SystemStage, SystemSet}, system::Resource};

use crate::CoreStage;


pub struct GameBuilder {
    pub world: World,
    runtime_schedule: Schedule,
    resize_listeners: SystemStage, // ran when window resizes
    cleanup_systems: SystemStage,
    runner: Box<dyn FnOnce(GameBuilder)>,
}

impl GameBuilder {
    pub fn new_with_runner(runner: impl FnOnce(GameBuilder) + 'static) -> Self {
        Self {
            world: World::new(),
            runtime_schedule: Schedule::default(),
            resize_listeners: SystemStage::single_threaded(),
            cleanup_systems: SystemStage::single_threaded(),
            runner: Box::new(runner),
        }
    }

    pub fn run(mut self) {
        let runner = std::mem::replace(&mut self.runner, Box::new(trap));
        (runner)(self);
    }

    pub fn run_cleanup(&mut self) {
        bevy_ecs::schedule::Stage::run(&mut self.cleanup_systems, &mut self.world);
    }

    pub fn window_resized(&mut self) {
        bevy_ecs::schedule::Stage::run(&mut self.resize_listeners, &mut self.world);
    }

    pub fn update(&mut self) {
        bevy_ecs::schedule::Stage::run(&mut self.runtime_schedule, &mut self.world);
    }

    pub fn insert_resource<T>(&mut self, resource: T) -> &mut Self
    where
        T: Resource,
    {
        self.world.insert_resource(resource);
        self
    }

    pub fn add_system<Params>(&mut self, system: impl IntoSystemDescriptor<Params>) -> &mut Self {
        self.add_system_to_stage(CoreStage::GameTick, system)
    }

    pub fn add_stage<S: bevy_ecs::schedule::Stage>(&mut self, label: impl StageLabel, stage: S) -> &mut Self {
        self.runtime_schedule.add_stage(label, stage);
        self
    }

    pub fn add_system_to_stage<Params>(
        &mut self,
        stage_label: impl StageLabel,
        system: impl IntoSystemDescriptor<Params>,
    ) -> &mut Self {
        self.runtime_schedule.add_system_to_stage(stage_label, system);
        self
    }

    pub fn add_system_set_to_stage(
        &mut self,
        stage_label: impl StageLabel,
        system_set: SystemSet,
    ) -> &mut Self {
        self.runtime_schedule
            .add_system_set_to_stage(stage_label, system_set);
        self
    }

    pub fn add_cleanup_system<Params>(&mut self, system: impl IntoSystemDescriptor<Params>) -> &mut Self {
        self.cleanup_systems.add_system(system);
        self
    }

    pub fn add_resize_listener_system<Params>(&mut self, system: impl IntoSystemDescriptor<Params>) -> &mut Self {
        self.resize_listeners.add_system(system);
        self
    }
}

fn trap(_: GameBuilder) {
    panic!("App run() called twice");
}*/