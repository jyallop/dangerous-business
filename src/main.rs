use std::io::BufReader;
use std::path::PathBuf;
use std::{fs, fs::File,
	  f32::consts::PI};
use bevy::{
    prelude::*,
};
use gpx::read;
//use gpx::Gpx;
//use time::{OffsetDateTime};
use circular_queue::CircularQueue;

const SPEED: f32 = 100000.0;
const SCALE_FACTOR: f32 = 10.0;
const CAM_HEIGHT: f32 = 100.0;
const PAN_SPEED: f32 = 10.0;
const IMAGE_TIME: f32 = 10000.0;
const CHUNK_SIZE: usize = 50;
const QUEUE_LENGTH: usize = 1000;
const FINAL_CHUNK: usize = 1000;

#[derive(Resource)]
struct Path {
    original_path: Vec<Vec3>,
    points: Vec<Vec3>,
    ids: CircularQueue<Entity>,
    curr: usize,
    last_update: f32,
    max_x: f32,
    max_y: f32,
    min_x: f32,
    min_y: f32,
    next_angle: fn(f32) -> f32,
}

#[derive(Resource)]
enum State {
    Run,
    Zoom(f32, f32, f32),
    Image,
}

#[derive(Component)]
struct Step;

fn spiral(x: f32) -> f32 {
    if x.abs() < 0.00001 {
	PI / 4.0
    } else {
	x - (x / PI)
    }
}

fn main() {
    App::new()
	.add_plugins(DefaultPlugins)
	.add_systems(Startup, setup)
	.add_systems(Update, (move_system, skip))
	.run();
}

fn skip(camera: Single<(&mut Transform, &mut Projection), With<Camera>>,
	mut commands: Commands,
	mut path: ResMut<Path>,
	step_query: Query<(Entity, &Step)>,
	window: Single<&mut Window>,
	mut materials: ResMut<Assets<ColorMaterial>>,
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut meshes: ResMut<Assets<Mesh>>,
	mut state: ResMut<State>) {
    let (mut transform, _projection) = camera.into_inner();
    if keyboard_input.pressed(KeyCode::Space) {
	let last = path.points[path.points.len() - 1];
	transform.translation = Vec3::new(last.x, last.y, CAM_HEIGHT);
	let mid_x = last.x / 2.0;
	let mid_y = last.y / 2.0;
	let diff_x = last.x;
	let diff_y = last.y;

	let mut z_diff = diff_y / window.height();
	if diff_x > diff_y {
	    z_diff = diff_x / window.width();
	}
	*state = State::Zoom(mid_x, mid_y, z_diff);
	for (entity, _) in step_query {
	    commands.entity(entity).despawn()
	}
	
	for i in 0..(path.points.len() / FINAL_CHUNK - 1) {
	    let Vec3{ x: sx, y: sy, z: _ } = path.points[i * FINAL_CHUNK];
	    let Vec3{ x: ex, y: ey, z: _ } = path.points[i * FINAL_CHUNK + FINAL_CHUNK];
	    commands.spawn((
		Mesh2d(meshes.add(Segment3d::new(
		    Vec3::new(sx, sy, 0.0),
		    Vec3::new(ex, ey, 0.0),
		))),
		MeshMaterial2d(materials.add(Color::WHITE)),
		Step));
	}
	
    }
    if keyboard_input.pressed(KeyCode::ArrowUp) {
	let mut new_buffer = CircularQueue::with_capacity(path.ids.capacity() + 100);
	for item in path.ids.iter() {
	    match new_buffer.push(*item) {
		Some(id) => commands.entity(id).despawn(),
		None => ()
	    }
	}
	path.ids = new_buffer.clone();
    }
    if keyboard_input.pressed(KeyCode::ArrowDown) {
	let mut new_buffer = CircularQueue::with_capacity(path.ids.capacity() - 100);
	for item in path.ids.iter() {
	    match new_buffer.push(*item) {
		Some(id) => commands.entity(id).despawn(),
		None => ()
	    }
	}
	path.ids = new_buffer.clone();
    }
}
    
	

fn rotate_point(point: Vec3, origin: Vec3, angle: f32) -> Vec3 {
    let Vec3 { x, y, z } = point;
    let Vec3 { x: ox, y: oy, z: _ } = origin;

    Vec3::new(angle.cos() * (x - ox) - angle.sin() * (y - oy) + ox, angle.sin() * (x - ox) + angle.cos() * (y - oy) + oy, z)
}

fn setup(
    mut commands: Commands,
    window: Single<&mut Window>,
) {

    let x_scale: f32 = window.width() * SCALE_FACTOR;
    let y_scale: f32 = window.height() * SCALE_FACTOR;
    let paths: Vec<PathBuf> = fs::read_dir("./data").unwrap().into_iter()
	.map(|path| {
	    path.unwrap().path()
	})
	.collect();
    println!("Got File Names");
    let mut points: Vec<Vec3> = Vec::new();
    let mut gpx_data: Vec<_> = paths.into_iter() //.take(3)
	.map(|x| File::open(x).unwrap())
	.map(|file| BufReader::new(file))
	.map(|reader| read(reader).unwrap()).collect();

    println!("Loaded File Data");

    gpx_data.sort_by_key(|gpx| gpx.metadata.clone().unwrap().time.unwrap());

    println!("Sorted Files by Timestamp");

    for gpx in gpx_data {
	for track in gpx.tracks {
	    for segment in track.segments {
		for point in segment.points {
		    points.push(Vec3::new(point.point().x() as f32,
				 point.point().y() as f32,
				 point.elevation.unwrap() as f32));
		}
	    }
	}
    }

    println!("Size: {}", points.len());

    let Vec3{ x: start_x, y: start_y, z: _} = points[0];
    let mut normalized: Vec<Vec3> = Vec::new();
    for point in points.into_iter() {
	let new_point = Vec3::new((point.x - start_x) * x_scale, (point.y - start_y) * y_scale, point.z);
	normalized.push(new_point);
    }
    let transform = Transform::from_xyz(0.0, 0.0, CAM_HEIGHT);
    let state = State::Run;
    commands.spawn((
	Camera2d,
	Camera {
	    clear_color: ClearColorConfig::Custom(Color::BLACK),
	    ..default()
	},
	transform
    ));

    commands.insert_resource(Path {
	original_path: normalized.clone(),
	points: normalized,
	ids: CircularQueue::with_capacity(QUEUE_LENGTH),
	curr: 0,
	last_update: 0.0,
	max_x: 0.0, max_y: 0.0,
	min_x: 0.0, min_y: 0.0,
	next_angle: spiral});
    commands.insert_resource(state);
}

fn move_system(time: Res<Time>,
	       mut commands: Commands,
	       camera: Single<(&mut Transform, &mut Projection), With<Camera>>,
	       mut path: ResMut<Path>,
	       mut state: ResMut<State>,
	       mut materials: ResMut<Assets<ColorMaterial>>,
	       mut meshes: ResMut<Assets<Mesh>>,
	       window: Single<&mut Window>,
	       step_query: Query<(Entity, &Step)>) {
    let (mut transform, mut projection) = camera.into_inner();
    match *state {
	State::Run => {
	    path.last_update += time.delta_secs();
	    let Vec3{ x: x_1, y: y_1, z: _ } = path.points[path.curr];
	    let Vec3{ x: x_2, y: y_2, z: _ } = path.points[path.curr + CHUNK_SIZE];
	    if x_2 > path.max_x { path.max_x = x_2; }
	    if x_2 < path.min_x { path.min_x = x_2; }
	    if y_2 > path.max_y { path.max_y = y_2; }
	    if y_2 < path.min_y { path.min_y = y_2; }
	    let dist = ((x_2 - x_1).powf(2.0) + (y_2 - y_1).powf(2.0)).sqrt();
	    if path.curr % 10000 == 0 && path.curr != 0 {
		println!("Running Time: {}", time.elapsed_secs());
		println!("{}", path.curr);
	    }
	    transform.translation.smooth_nudge(&Vec3::new(x_1, y_1, CAM_HEIGHT), 100.0, time.delta_secs());
	    if path.last_update * SPEED > dist {
		path.last_update = 0.0;
		for i in 0..CHUNK_SIZE {
		    let Vec3{ x: sx, y: sy, z: _ } = path.points[path.curr + i];
		    let Vec3{ x: ex, y: ey, z: _ } = path.points[path.curr + i + 1];
		    let id = commands.spawn((
			Mesh2d(meshes.add(Segment3d::new(
			    Vec3::new(sx, sy, 0.0),
			    Vec3::new(ex, ey, 0.0),
			))),
			MeshMaterial2d(materials.add(Color::WHITE)),
			Step));
		    match path.ids.push(id.id()) {
			Some(id) => commands.entity(id).despawn(),
			None => ()
		    }
		}
		
		if path.curr + (2 * CHUNK_SIZE) + 1 >= path.points.len() {
		    println!("Total Running Time: {}", time.elapsed_secs());
		    let mid_x = (path.max_x + path.min_x) / 2.0;
		    let mid_y = (path.max_y + path.min_y) / 2.0;
		    let diff_x = path.max_x - path.min_y;
		    let diff_y = path.max_y - path.min_y;
		    let mut z_diff = diff_y / window.height();
		    if diff_x > diff_y {
			z_diff = diff_x / window.width();
		    }
		    *state = State::Zoom(mid_x, mid_y, z_diff);
		    for (entity, _) in step_query {
			commands.entity(entity).despawn()
		    }

		    for i in 0..(path.points.len() / FINAL_CHUNK - 1) {
			let Vec3{ x: sx, y: sy, z: _ } = path.points[i * FINAL_CHUNK];
			let Vec3{ x: ex, y: ey, z: _ } = path.points[i * FINAL_CHUNK + FINAL_CHUNK];
			commands.spawn((
			    Mesh2d(meshes.add(Segment3d::new(
				Vec3::new(sx, sy, 0.0),
				Vec3::new(ex, ey, 0.0),
			    ))),
			    MeshMaterial2d(materials.add(Color::WHITE)),
			    Step));
		    }

		} else {
		    path.curr += CHUNK_SIZE;
		}
	    }
	}
	State::Zoom(x_diff, y_diff, z_diff) => {
	    transform.translation.smooth_nudge(&Vec3::new(x_diff, y_diff, 0.0),
					       //f32::ln(1.0 / z_diff),
					       0.1 * PAN_SPEED,
					       time.delta_secs());

	    // Camera zoom controls
	    if let Projection::Orthographic(projection2d) = &mut *projection {
		projection2d.scale += (z_diff / projection2d.scale) * time.delta_secs() * PAN_SPEED;
		
		if projection2d.scale > z_diff + 10.0 {
		    println!("Finished Zoom");
		    *state = State::Image;
		    path.last_update = 0.0;
		}
	   } 
	}
	State::Image => {
	    path.last_update += time.elapsed_secs();
	    if path.last_update > IMAGE_TIME {
		path.curr = 0;
		let mut prev: Vec3 = Vec3::new(0.0, 0.0, 0.0);
		let mut translation: Vec3 = Vec3::new(0.0, 0.0, 0.0);
		let mut normalized: Vec<Vec3> = Vec::new();
		let mut prev_angle: f32 = 0.0;
		for point in path.original_path.clone().into_iter() {
		    let mut new_point = point;
		    new_point += translation;
		    prev_angle = (path.next_angle)(prev_angle);
		    let rotated = rotate_point(new_point, prev, prev_angle);
		    translation += rotated - new_point;
		    prev = rotated;
		    normalized.push(rotated);
		}
		path.max_x = 0.0;
		path.max_y = 0.0;
		path.min_x = 0.0;
		path.min_y = 0.0;
		path.points = normalized;
		path.ids = CircularQueue::with_capacity(QUEUE_LENGTH);
		for (entity, _) in step_query {
		    commands.entity(entity).despawn()
		}
		transform.translation = Vec3::new(0.0, 0.0, CAM_HEIGHT);
		*state = State::Run;
		path.last_update = 0.0;
		if let Projection::Orthographic(projection2d) = &mut *projection {
		    projection2d.scale = 1.0;
		}

	    }
	}
    }
}

