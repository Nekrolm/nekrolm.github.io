// 
//  This file was 75% Vibe-coded!
//  That's why it's incredibly bad in terms of 
//  performance and implementation.
//  But it works.
//

use std::collections::HashMap;

use bevy::{prelude::*, window::PrimaryWindow};

use rand::seq::SliceRandom;

// Grid configuration
const GRID_SIZE: usize = 10;
const CELL_SIZE: f32 = 40.0;
const NUM_MINES: usize = 15;

fn main() {
    // For non-wasm targets, you can run the app normally:
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, handle_mouse_click)
        .add_systems(PostUpdate, (update_mines_left_system, win_condition, loose_condition))
        .run();
}

#[derive(Resource)]
struct MinesGenerated(bool);

#[derive(Component)]
struct Cell {
    x: usize,
    y: usize,
    mine: bool,
    revealed: bool,
    flagged: bool,
    adjacent: u8,
}

#[derive(Component)]
struct FlagMarker;

#[derive(Component)]
struct MinesLeftText;

/// A mapping from grid coordinates (x, y) to the corresponding cell entity.
#[derive(Resource, Default)]
struct GridMap(HashMap<(usize, usize), Entity>);

fn setup(mut commands: Commands) {
    commands.insert_resource(MinesGenerated(false));
    // Spawn an orthographic camera.
    commands.spawn(Camera2d::default());

    // Create grid of cells.

    let grid_map = (0..GRID_SIZE)
        .flat_map(|x| (0..GRID_SIZE).map(move |y| (x, y)))
        .map(|p @ (x, y)| {
            let cell = Cell {
                x,
                y,
                mine: false, // You can randomly set mines later
                revealed: false,
                adjacent: 0,
                flagged: false,
            };

            // Spawn a sprite for each cell.
            let entity = commands
                .spawn((
                    Sprite {
                        color: Color::srgb(0.7, 0.7, 0.7),
                        ..Default::default()
                    },
                    Transform {
                        translation: Vec3::new(
                            x as f32 * CELL_SIZE - (GRID_SIZE as f32 * CELL_SIZE / 2.0)
                                + CELL_SIZE / 2.0,
                            y as f32 * CELL_SIZE - (GRID_SIZE as f32 * CELL_SIZE / 2.0)
                                + CELL_SIZE / 2.0,
                            0.0,
                        ),
                        scale: Vec3::splat(CELL_SIZE - 2.0),
                        ..Default::default()
                    },
                ))
                .insert(cell)
                .id();

            (p, entity)
        })
        .collect();
    commands.insert_resource(GridMap(grid_map));

    // Spawn a UI text element at the top-left to show the number of mines left.
    commands
        .spawn(Text(format!("Mines left: {}", NUM_MINES)))
        .insert(MinesLeftText);
}

fn handle_mouse_click(
    mut commands: Commands,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut mines_generated: ResMut<MinesGenerated>,
    mut query: Query<(Entity, &mut Cell, &mut Sprite, &Transform)>,
    grid_map: Res<GridMap>,
    mut flag_marker_query: Query<(Entity, &Parent), With<FlagMarker>>,
) {
    let window = primary_window.get_single().unwrap();
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Convert the cursor position from screen to world coordinates.
    let window_size = Vec2::new(window.width(), window.height());
    let mut world_pos = cursor_pos - window_size / 2.0;
    world_pos.y *= -1.0;
    // Compute the grid's bottom-left origin.
    let grid_origin = Vec2::new(
        -(GRID_SIZE as f32 * CELL_SIZE) / 2.0,
        -(GRID_SIZE as f32 * CELL_SIZE) / 2.0,
    );
    let relative_pos = world_pos - grid_origin;
    let grid_x = (relative_pos.x / CELL_SIZE).floor() as usize;
    let grid_y = (relative_pos.y / CELL_SIZE).floor() as usize;

    if grid_x >= GRID_SIZE || grid_y > GRID_SIZE {
        return;
    }

    if mouse_button_input.just_pressed(MouseButton::Left) {
        // On the very first click, generate mines (avoiding the clicked cell)
        if !mines_generated.0 {
            generate_mines(&mut query, (grid_x, grid_y));
            compute_adjacent_counts(&mut query);
            mines_generated.0 = true;
        }

        // Reveal the clicked cell.
        for (_, mut cell, mut sprite, transform) in query.iter_mut() {
            if cell.x == grid_x && cell.y == grid_y && !cell.revealed {
                // If the clicked cell is empty (no adjacent mines), flood fill reveal.
                if !cell.mine && cell.adjacent == 0 {
                    flood_fill_reveal((grid_x, grid_y), &grid_map, &mut query, &mut commands);
                } else {
                    if !cell.flagged {
                        reveal_one(&mut commands, &mut *cell, &mut *sprite, transform);
                    }
                }
                break;
            }
        }
    } else if mouse_button_input.just_pressed(MouseButton::Right) {
        for (entity, mut cell, _sprite, transform) in query.iter_mut() {
            if cell.x == grid_x && cell.y == grid_y && !cell.revealed {
                if cell.flagged {
                    // Remove flag: update cell state and despawn flag marker.
                    cell.flagged = false;
                    for (flag_entity, parent) in flag_marker_query.iter_mut() {
                        if parent.get() == entity {
                            commands.entity(flag_entity).despawn_recursive();
                        }
                    }
                } else {
                    // Add flag: update cell state and spawn flag marker as child.
                    cell.flagged = true;
                    commands.entity(entity).with_children(|parent| {
                        parent
                            .spawn((
                                Text2d::new("*"),
                                TextColor(Color::srgb(0.3, 0.9, 0.9)),
                                Transform {
                                    translation: Vec3::new(0.0, 0.0, 1.0),
                                    scale: 1.0 / transform.scale,
                                    ..Default::default()
                                },
                            ))
                            .insert(FlagMarker);
                    });
                }
            }
        }
    }
}

fn reveal_one(
    commands: &mut Commands,
    cell: &mut Cell,
    sprite: &mut Sprite,
    transform: &Transform,
) {
    cell.revealed = true;
    // Change its color to indicate it's been revealed.

    sprite.color = if cell.mine {
        Color::srgb(1.0, 0.0, 0.0)
    } else {
        Color::srgb(0.9, 0.9, 0.9)
    };

    // If not a mine and there is at least one adjacent mine,
    // spawn a text entity displaying the count.
    if !cell.mine && cell.adjacent > 0 {
        commands.spawn((
            Text2d::new(cell.adjacent.to_string()),
            TextColor(Color::srgb(0.3, 0.9, 0.1)),
            Transform {
                translation: transform.translation + Vec3::new(0.0, 0.0, 1.0),
                ..Default::default()
            },
        ));
    }
}

/// Recursively reveal all connected cells that are not adjacent to any mines.
fn flood_fill_reveal(
    start: (usize, usize),
    grid_map: &GridMap,
    query: &mut Query<(Entity, &mut Cell, &mut Sprite, &Transform)>,
    commands: &mut Commands,
) {
    let mut to_visit = vec![start];
    while let Some((x, y)) = to_visit.pop() {
        if let Some(&entity) = grid_map.0.get(&(x, y)) {
            if let Ok((_, mut cell, mut sprite, transform)) = query.get_mut(entity) {
                if !cell.revealed && !cell.flagged {
                    reveal_one(commands, &mut *cell, &mut *sprite, transform);

                    // If this cell is empty, add all its neighbors.
                    if cell.adjacent == 0 && !cell.mine {
                        for dx in -1i32..=1 {
                            for dy in -1i32..=1 {
                                if dx == 0 && dy == 0 {
                                    continue;
                                }
                                let nx = x as i32 + dx;
                                let ny = y as i32 + dy;
                                if nx >= 0
                                    && ny >= 0
                                    && (nx as usize) < GRID_SIZE
                                    && (ny as usize) < GRID_SIZE
                                {
                                    to_visit.push((nx as usize, ny as usize));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn generate_mines(
    query: &mut Query<(Entity, &mut Cell, &mut Sprite, &Transform)>,
    first_click: (usize, usize),
) {
    // Collect all cell positions except the first clicked cell.
    let mut available_positions = Vec::new();
    for (_, cell, _, _) in query.iter() {
        if cell.x == first_click.0 && cell.y == first_click.1 {
            continue;
        }
        available_positions.push((cell.x, cell.y));
    }
    // Shuffle and select positions for mines.
    let mut rng = rand::rng();
    available_positions.shuffle(&mut rng);
    let mines_positions = available_positions
        .into_iter()
        .take(NUM_MINES)
        .collect::<Vec<_>>();

    // Mark cells as mines based on the selected positions.
    for (_, mut cell, _, _) in query.iter_mut() {
        if mines_positions.contains(&(cell.x, cell.y)) {
            cell.mine = true;
        }
    }
}

fn compute_adjacent_counts(query: &mut Query<(Entity, &mut Cell, &mut Sprite, &Transform)>) {
    // Gather data on all cells.
    let cells_data: Vec<(usize, usize, bool)> = query
        .iter()
        .map(|(_, cell, _, _)| (cell.x, cell.y, cell.mine))
        .collect();

    // For each cell, count the number of adjacent mines.
    for (_, mut cell, _, _) in query.iter_mut() {
        if cell.mine {
            continue;
        }
        let mut count = 0;
        for (nx, ny, is_mine) in &cells_data {
            let dx = *nx as isize - cell.x as isize;
            let dy = *ny as isize - cell.y as isize;
            if dx.abs() <= 1 && dy.abs() <= 1 && *is_mine {
                count += 1;
            }
        }
        cell.adjacent = count;
    }
}

/// System to update the UI text that shows the number of mines left.
fn update_mines_left_system(
    query_cells: Query<&Cell>,
    mut query_ui: Query<&mut Text, With<MinesLeftText>>,
) {
    let flagged_count = query_cells.iter().filter(|cell| cell.flagged).count();
    let mines_left = NUM_MINES as isize - flagged_count as isize;
    for mut text in query_ui.iter_mut() {
        text.0 = format!("Mines left: {}", mines_left);
    }
}

fn win_condition(
    mut commands: Commands,
    mines_generated: Res<MinesGenerated>,
    query_cells: Query<&Cell>,
) {
    if !mines_generated.0 {
        return;
    }
    let solved = query_cells
        .iter()
        .filter(|cell| !cell.mine)
        .all(|cell| cell.revealed);

    if solved {
        commands.spawn((
            Text2d::new("CONGRATULATION!"),
            TextColor(Color::srgb(0.1, 0.9, 0.8)),
            Transform {
                // translation: pos,
                scale: 2.0 * Vec3 { x: 1., y: 1., z: 1. },
                ..Default::default()
            },
        ));
    }
}

fn loose_condition(
    mut commands: Commands,
    query_cells: Query<&Cell>,
) {
    
    let dead = query_cells
        .iter()
        .filter(|cell| cell.mine)
        .any(|cell| cell.revealed);

    if dead {
        commands.spawn((
            Text2d::new("WASTED!"),
            TextColor(Color::srgb(1., 0.1, 0.1)),
            Transform {
                // translation: pos,
                scale: 3.0 * Vec3 { x: 1., y: 1., z: 1. },
                ..Default::default()
            },
        ));
    }
}
