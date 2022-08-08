pub mod assets;
pub mod camera;
pub mod chat;
pub mod entities;
pub mod game;
pub mod input;
pub mod networking;
pub mod renderer;
pub mod resources;
pub mod text_box;
pub mod world;
pub mod components;

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

/* fn runner() {
    let event_loop = EventLoop::new();



    //core_systems::init(&mut app, &event_loop);

    let mut has_focus = true;

    event_loop.run(move |event, _, flow| {
        if let Err(e) = handle_event(&mut app, event, flow, &mut has_focus) {
            println!("exec_event returned error: {}", e);
        }
    });
}

fn handle_event(app: &mut GameBuilder, event: Event<()>, flow: &mut ControlFlow, has_focus: &mut bool) -> Result<()> {
    match event {
        Event::WindowEvent {
            window_id: _,
            event,
        } => {
            handle_window_event(app, event, flow, has_focus)?;
        }
        Event::MainEventsCleared => {
            let keyboard = &mut *app.world.get_resource_mut::<Keyboard>().unwrap();
            if keyboard.pressed(Key::Escape) {
                *flow = ControlFlow::Exit;
            }

            //println!("FRAME START");
            let now = Instant::now();
            app.update();
            let end = Instant::now();
            //println!("FRAME TOOK {}ms, vs {}ms\n", (end-now).as_secs_f32() * 1000.0, (end - app.world.get_resource::<Time>().unwrap().now).as_secs_f32() * 1000.0);
        }
        Event::LoopDestroyed => {
            app.run_cleanup();
        }
        Event::DeviceEvent { device_id: _, event } if *has_focus => {
            handle_device_event(app, event)?;
        },
        _ => {}
    }
    Ok(())
}

fn handle_device_event(app: &mut GameBuilder, event: DeviceEvent) -> Result<()> {
    let keyboard = &mut *app.world.get_resource_mut::<Keyboard>().unwrap();
    if KeyboardUpdater::handle_key_event(&event, keyboard) {
        return Ok(());
    }

    let mouse = &mut *app.world.get_resource_mut::<Mouse>().unwrap();
    if MouseUpdater::handle_mouse_events(&event, mouse) {
        return Ok(());
    }

    Ok(())
}

fn handle_window_event(app: &mut Game, event: WindowEvent, flow: &mut ControlFlow, has_focus: &mut bool) -> Result<()> {
    let keyboard = &mut *app.world.get_resource_mut::<Keyboard>().unwrap();
    if KeyboardUpdater::handle_window_event(&event, keyboard) {
        return Ok(());
    }

    match event {
        WindowEvent::ScaleFactorChanged { .. } => {} ,
        WindowEvent::Resized(new_size) => {
            let vk = &*app.world.get_resource::<VkContext>().unwrap();
            let extent = vk.swapchain.surface.extent;
            if new_size.width == extent.width && new_size.height == extent.height {
                return Ok(());
            }

            app.window_resized();
        }

        WindowEvent::CloseRequested => {
            *flow = ControlFlow::Exit;
        }

        WindowEvent::Focused(focus_gained) => {
            *has_focus = focus_gained;
        }
        _ => {}
    }
    Ok(())
} */

/*// Based on the observations that:
// - Each triangle belongs in exactly 3 "rings" of the icosphere
// - Each ring is exactly arctan(1/2) radians wide
// - Each ring intersects every other ring
// - The intersection of any 3 rings contain exactly two triangles
// - There are 6 rings in total
// - The ray "belongs" in a ring if its dot product with the ring's normal
//   vector gives an angle less than `arcsin(1/sqrt(phi^2+2))` (approx 27.732 degrees)
//   Because: y=1/sqrt(phi^2 + 2), so angle = pi/2-arccos(y)
fn icosphere_tri_idx(ray: Vec3, face_normals: &[Vec3]) -> usize {
    const PHI : f32 = 1.618034; // golden ratio
    const S : f32 = 0.5257311; // scale: 1/sqrt(phi^2+1)
    const TRESH : f32 = 0.484019969; // arcsin(1/sqrt(phi^2+2))

    let mut candidates = 0xFFFFF; // all 20 faces
    candidates &= !0x07FE0 | (0xFFFFF * (ray.dot(Vec3::new(S, S*PHI, 0.0)).abs() > TRESH) as u32);
    candidates &= !0x73193 | (0xFFFFF * (ray.dot(Vec3::new(-S, S*PHI, 0.0)).abs() > TRESH) as u32);
    candidates &= !0x38C79 | (0xFFFFF * (ray.dot(Vec3::new(0.0, S, S*PHI)).abs() > TRESH) as u32);
    candidates &= !0xE4627 | (0xFFFFF * (ray.dot(Vec3::new(0.0, -S, S*PHI)).abs() > TRESH) as u32);
    candidates &= !0x9E31C | (0xFFFFF * (ray.dot(Vec3::new(S*PHI, 0.0, S)).abs() > TRESH) as u32);
    candidates &= !0xC98CE | (0xFFFFF * (ray.dot(Vec3::new(S*PHI, 0.0, -S)).abs() > TRESH) as u32);

    if candidates.count_ones() != 2 {
        panic!("Fail: {} bits set", candidates.count_ones());
    }

    // There should be exactly 2 bits set in candidates right now (unless floating-point accuracy screws this up)
    let candidate = candidates.trailing_zeros() as usize;
    if face_normals[candidate].dot(ray) > 0.0 {
        candidate
    } else {
        32 - candidates.leading_zeros() as usize
    }
}
 const PHI : f32 = 1.618034; // golden ratio
const S : f32 = 0.5257311; // scale: 1/sqrt(phi^2+1)
for v in &[
    Vec3::new(S, S*PHI, 0.0),
Vec3::new(-S, S*PHI, 0.0),
Vec3::new(0.0, S, S*PHI),
Vec3::new(0.0, -S, S*PHI),
Vec3::new(S*PHI, 0.0, S),
Vec3::new(S*PHI, 0.0, -S)
] {
    println!("{v}, {}", v.length());
}

let face_normals = [Vec3::ZERO; 20];
let mut rng = rand::thread_rng();
for i in 0..10000000 {
    println!("{i}");
    let ray = Vec3::new(
        rng.gen_range(-1.0..1.0),
        rng.gen_range(-1.0..1.0),
        rng.gen_range(-1.0..1.0),
    ).normalize();

    let _ = icosphere_tri_idx(ray, &face_normals);
} */
