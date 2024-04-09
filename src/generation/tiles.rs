use bevy::{
    asset::Assets,
    ecs::system::{Res, ResMut, Resource},
    gizmos::gizmos::Gizmos,
    reflect::Reflect,
    render::{color::Color, texture::Image},
    utils::HashMap,
};
use bevy_math::{ivec2, vec2, IVec2, Vec2};
use itertools::Itertools;

use crate::{
    assets::TileAssets, constants::CHUNK_SIZE, generation::biome::Biome,
    registries::Registries,
};

#[derive(Reflect, PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum ConstraintType {
    Edge(EdgePosition),
    Corner(CornerPosition),
}

impl ConstraintType {
    pub fn get_opposite(&self, offset: IVec2) -> Self {
        match self {
            ConstraintType::Edge(edge) => ConstraintType::Edge(match edge {
                EdgePosition::Top => EdgePosition::Bottom,
                EdgePosition::Right => EdgePosition::Left,
                EdgePosition::Bottom => EdgePosition::Top,
                EdgePosition::Left => EdgePosition::Right,
            }),
            ConstraintType::Corner(corner) => ConstraintType::Corner(match corner {
                CornerPosition::TopLeft => match offset {
                    IVec2 { x: 1, y: -1 } => CornerPosition::BottomRight,
                    IVec2 { x: 0, y: -1 } => CornerPosition::BottomLeft,
                    IVec2 { x: 1, y: 0 } => CornerPosition::TopRight,
                    _ => panic!(),
                },
                CornerPosition::TopRight => match offset {
                    IVec2 { x: -1, y: -1 } => CornerPosition::BottomLeft,
                    IVec2 { x: 0, y: -1 } => CornerPosition::BottomRight,
                    IVec2 { x: -1, y: 0 } => CornerPosition::TopLeft,
                    _ => panic!(),
                },
                CornerPosition::BottomLeft => match offset {
                    IVec2 { x: 1, y: 1 } => CornerPosition::TopRight,
                    IVec2 { x: 0, y: 1 } => CornerPosition::TopLeft,
                    IVec2 { x: 1, y: 0 } => CornerPosition::BottomRight,
                    _ => panic!(),
                },
                CornerPosition::BottomRight => match offset {
                    IVec2 { x: -1, y: 1 } => CornerPosition::TopLeft,
                    IVec2 { x: 0, y: 1 } => CornerPosition::TopRight,
                    IVec2 { x: -1, y: 0 } => CornerPosition::BottomLeft,
                    _ => panic!(),
                },
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileOrientation {
    Vertical,
    Horizontal,
}

#[derive(Reflect, PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum EdgePosition {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Reflect, PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum CornerPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub orientation: TileOrientation,
    pub parts: HashMap<IVec2, TilePart>,
}

#[derive(Reflect, Default, Debug, Clone)]
pub struct TilePart {
    pub constraints: HashMap<ConstraintType, [u8; 4]>,
    pub data: Vec<[u8; 4]>,
}

pub fn get_pixel_index(width: i32, x: i32, y: i32) -> i32 {
    y * width + x
}

pub fn parse_tiles(
    mut registries: ResMut<Registries>,
    tiles_texture_assets: Res<TileAssets>,
    assets: Res<Assets<Image>>,
) {
    let border = 2;
    let tiles_texture = assets.get(tiles_texture_assets.caves.clone()).unwrap();
    let image_width = tiles_texture.width() as i32;
    let image_height = tiles_texture.height() as i32;

    // to do not include borders
    let mut tile_size = -2;
    let mut count_h_x = 0;
    let mut count_h_y = 0;
    let mut count_v_x = 0;
    let mut count_v_y = 0;

    let mut current_pixel = border;

    while current_pixel < image_height {
        let index = get_pixel_index(image_width, border, current_pixel) as usize * 4;
        let color = &tiles_texture.data[index..index + 4];

        if color.iter().all(|channel| *channel == 255) {
            break;
        }

        tile_size += 1;
        current_pixel += 1;
    }

    current_pixel = border;
    let mut prev_color = vec![];

    // Walk the pixels downards, count the white gaps between tiles and when the gap is bigger we have reached the end and start of vertical tiles
    while current_pixel < image_height {
        let index = get_pixel_index(image_width, border, current_pixel) as usize * 4;
        let color = &tiles_texture.data[index..index + 4];

        if color.iter().all(|channel| *channel == 255) {
            if prev_color.iter().all(|channel| *channel == 255) {
                // We have reached the end
                current_pixel += border;
                break;
            }
            count_h_y += 1;
        }

        prev_color = color.to_vec();
        current_pixel += 1;
    }

    // We continue going downwards but now count_v_y
    while current_pixel < image_height {
        let index = get_pixel_index(image_width, border, current_pixel) as usize * 4;
        let color = &tiles_texture.data[index..index + 4];
        if color.iter().all(|channel| *channel == 255) {
            if prev_color.iter().all(|channel| *channel == 255) {
                break;
            }
            count_v_y += 1;
        }
        prev_color = color.to_vec();
        current_pixel += 1;
    }

    // Now count from left to right count_h_x
    current_pixel = border;
    prev_color.clear();
    while current_pixel < image_width {
        let index = get_pixel_index(image_width, current_pixel, border) as usize * 4;
        let color = &tiles_texture.data[index..index + 4];

        if color.iter().all(|channel| *channel == 255) {
            if prev_color.iter().all(|channel| *channel == 255) {
                break;
            }
            count_h_x += 1;
        }

        prev_color = color.to_vec();
        current_pixel += 1;
    }

    // Lastly count_v_x
    current_pixel = border;
    prev_color.clear();
    while current_pixel < image_width {
        let index = get_pixel_index(image_width, border, current_pixel) as usize * 4;
        let color = &tiles_texture.data[index..index + 4];

        if color.iter().all(|channel| *channel == 255) {
            if prev_color.iter().all(|channel| *channel == 255) {
                break;
            }
            count_v_x += 1;
        }
        prev_color = color.to_vec();
        current_pixel += 1;
    }

    let mut tile_geometry_horizontal = HashMap::new();
    let mut tile_geometry_vertical = HashMap::new();

    let half_tile_size = f32::floor(tile_size as f32 * 0.5) as i32;
    let three_halfs_tile_size = f32::floor(tile_size as f32 * 1.5) as i32;

    tile_geometry_horizontal.insert(
        IVec2::ZERO,
        HashMap::from([
            (
                ConstraintType::Edge(EdgePosition::Top),
                IVec2::new(half_tile_size, 0),
            ),
            (
                ConstraintType::Edge(EdgePosition::Left),
                IVec2::new(0, half_tile_size),
            ),
            (
                ConstraintType::Edge(EdgePosition::Bottom),
                IVec2::new(half_tile_size, tile_size + 1),
            ),
            (
                ConstraintType::Corner(CornerPosition::TopLeft),
                IVec2::new(0, 0),
            ),
            (
                ConstraintType::Corner(CornerPosition::TopRight),
                IVec2::new(tile_size, 0),
            ),
            (
                ConstraintType::Corner(CornerPosition::BottomLeft),
                IVec2::new(0, tile_size + 1),
            ),
            (
                ConstraintType::Corner(CornerPosition::BottomRight),
                IVec2::new(tile_size, tile_size + 1),
            ),
        ]),
    );

    tile_geometry_horizontal.insert(
        IVec2::new(1, 0),
        HashMap::from([
            (
                ConstraintType::Edge(EdgePosition::Top),
                IVec2::new(three_halfs_tile_size, 0),
            ),
            (
                ConstraintType::Edge(EdgePosition::Right),
                IVec2::new(tile_size * 2 + 1, half_tile_size),
            ),
            (
                ConstraintType::Edge(EdgePosition::Bottom),
                IVec2::new(three_halfs_tile_size, tile_size + 1),
            ),
            (
                ConstraintType::Corner(CornerPosition::TopLeft),
                IVec2::new(tile_size, 0),
            ),
            (
                ConstraintType::Corner(CornerPosition::TopRight),
                IVec2::new(tile_size * 2 + 1, 0),
            ),
            (
                ConstraintType::Corner(CornerPosition::BottomLeft),
                IVec2::new(tile_size, tile_size + 1),
            ),
            (
                ConstraintType::Corner(CornerPosition::BottomRight),
                IVec2::new(tile_size * 2 + 1, tile_size + 1),
            ),
        ]),
    );

    tile_geometry_vertical.insert(
        IVec2::ZERO,
        HashMap::from([
            (
                ConstraintType::Edge(EdgePosition::Left),
                IVec2::new(0, three_halfs_tile_size),
            ),
            (
                ConstraintType::Edge(EdgePosition::Right),
                IVec2::new(tile_size + 1, three_halfs_tile_size),
            ),
            (
                ConstraintType::Edge(EdgePosition::Bottom),
                IVec2::new(half_tile_size, tile_size * 2 + 1),
            ),
            (
                ConstraintType::Corner(CornerPosition::TopLeft),
                IVec2::new(0, tile_size),
            ),
            (
                ConstraintType::Corner(CornerPosition::TopRight),
                IVec2::new(tile_size + 1, tile_size),
            ),
            (
                ConstraintType::Corner(CornerPosition::BottomLeft),
                IVec2::new(0, tile_size * 2 + 1),
            ),
            (
                ConstraintType::Corner(CornerPosition::BottomRight),
                IVec2::new(tile_size + 1, tile_size * 2 + 1),
            ),
        ]),
    );

    tile_geometry_vertical.insert(
        IVec2::new(0, 1),
        HashMap::from([
            (
                ConstraintType::Edge(EdgePosition::Top),
                IVec2::new(half_tile_size, 0),
            ),
            (
                ConstraintType::Edge(EdgePosition::Left),
                IVec2::new(0, half_tile_size),
            ),
            (
                ConstraintType::Edge(EdgePosition::Right),
                IVec2::new(tile_size + 1, half_tile_size),
            ),
            (
                ConstraintType::Corner(CornerPosition::TopLeft),
                IVec2::new(0, 0),
            ),
            (
                ConstraintType::Corner(CornerPosition::TopRight),
                IVec2::new(tile_size + 1, 0),
            ),
            (
                ConstraintType::Corner(CornerPosition::BottomLeft),
                IVec2::new(0, tile_size),
            ),
            (
                ConstraintType::Corner(CornerPosition::BottomRight),
                IVec2::new(tile_size + 1, tile_size),
            ),
        ]),
    );

    let mut tiles = vec![];

    for y in 0..count_h_y {
        for x in 0..count_h_x {
            let corner = IVec2::new(
                x * tile_size * 2 + 3 * x + border,
                y * tile_size + 3 * y + border,
            );

            let mut tile = Tile {
                orientation: TileOrientation::Horizontal,
                parts: HashMap::new(),
            };

            tile.parts.extend(tile_geometry_horizontal.iter().map(
                |(part_position, constraint_positions)| {
                    let mut part = TilePart {
                        constraints: HashMap::new(),
                        data: vec![[0; 4]; (tile_size * tile_size) as usize],
                    };

                    part.constraints.extend(constraint_positions.iter().map(
                        |(constraint, position)| {
                            let index = get_pixel_index(
                                image_width,
                                corner.x + position.x,
                                corner.y + position.y,
                            ) as usize
                                * 4;

                            let color = &tiles_texture.data[index..index + 4];

                            (
                                *constraint,
                                color.try_into().expect("error converting color"),
                            )
                        },
                    ));

                    for (x, y) in (0..tile_size).cartesian_product(0..tile_size) {
                        let index = get_pixel_index(
                            image_width,
                            corner.x + x + part_position.x * tile_size + 1,
                            corner.y + y + 1,
                        ) as usize
                            * 4;

                        let color = &tiles_texture.data[index..index + 4];
                        part.data[((tile_size - 1 - y) * tile_size + x) as usize] =
                            color.try_into().expect("wrong format");
                    }

                    (*part_position, part)
                },
            ));

            tiles.push(tile);
        }
    }

    for y in 0..count_v_y {
        for x in 0..count_v_x {
            let corner = IVec2::new(
                x * tile_size + 3 * x + border,
                y * tile_size * 2 + 3 * y + border * 2 + count_h_y * (tile_size + 3),
            );

            let mut tile = Tile {
                orientation: TileOrientation::Vertical,
                parts: HashMap::new(),
            };

            tile.parts.extend(tile_geometry_vertical.iter().map(
                |(part_position, constraint_positions)| {
                    let mut part = TilePart {
                        constraints: HashMap::new(),
                        data: vec![[0; 4]; (tile_size * tile_size) as usize],
                    };

                    part.constraints.extend(constraint_positions.iter().map(
                        |(constraint, position)| {
                            let index = get_pixel_index(
                                image_width,
                                corner.x + position.x,
                                corner.y + position.y,
                            ) as usize
                                * 4;

                            let color = &tiles_texture.data[index..index + 4];

                            (
                                *constraint,
                                color.try_into().expect("error converting color"),
                            )
                        },
                    ));

                    for (x, y) in (0..tile_size).cartesian_product(0..tile_size) {
                        let index = get_pixel_index(
                            image_width,
                            corner.x + x + part_position.x * tile_size + 1,
                            corner.y + y + (1 - part_position.y) * tile_size + 1,
                        ) as usize
                            * 4;

                        let color = &tiles_texture.data[index..index + 4];
                        part.data[((tile_size - 1 - y) * tile_size + x) as usize] =
                            color.try_into().expect("wrong format");
                    }

                    (*part_position, part)
                },
            ));

            tiles.push(tile);
        }
    }

    registries.biomes.register(
        "caves",
        Biome {
            color: [0xff, 0xff, 0x00, 0xff],
            tiles,
            tile_size: tile_size as u8,
        },
    );
}

#[derive(Reflect, Default, Resource)]
pub struct TileGenerator {
    pub tiles: HashMap<IVec2, TilePart>,
    pub scale: i32,
}

impl TileGenerator {
    pub fn get_tile(&mut self, chunk_position: IVec2, biome: &Biome) -> Option<TilePart> {
        let part_position = chunk_position.div_euclid(IVec2::ONE * self.scale);

        if self.tiles.contains_key(&part_position) {
            return Some(self.tiles.get(&part_position).cloned().unwrap());
        }

        let orientation = match (-part_position.x - part_position.y) % 4 {
            0 => TileOrientation::Horizontal,
            -2 | 2 => TileOrientation::Vertical,
            _ => return None,
        };

        let valid_tiles = biome
            .tiles
            .iter()
            .filter(|tile| tile.orientation == orientation)
            .filter(|tile| {
                tile.parts
                    .keys()
                    .all(|offset| !self.tiles.contains_key(&(part_position + *offset)))
            })
            .filter(|tile| {
                tile.parts.iter().all(|(offset, part)| {
                    let position = part_position + *offset;

                    [
                        (
                            ivec2(-1, -1),
                            vec![ConstraintType::Corner(CornerPosition::TopRight)],
                        ),
                        (
                            ivec2(0, -1),
                            vec![
                                ConstraintType::Corner(CornerPosition::TopLeft),
                                ConstraintType::Edge(EdgePosition::Top),
                                ConstraintType::Corner(CornerPosition::TopRight),
                            ],
                        ),
                        (
                            ivec2(1, -1),
                            vec![ConstraintType::Corner(CornerPosition::TopLeft)],
                        ),
                        (
                            ivec2(-1, 0),
                            vec![
                                ConstraintType::Corner(CornerPosition::TopRight),
                                ConstraintType::Edge(EdgePosition::Right),
                                ConstraintType::Corner(CornerPosition::BottomRight),
                            ],
                        ),
                        (
                            ivec2(1, 0),
                            vec![
                                ConstraintType::Corner(CornerPosition::TopLeft),
                                ConstraintType::Edge(EdgePosition::Left),
                                ConstraintType::Corner(CornerPosition::BottomLeft),
                            ],
                        ),
                        (
                            ivec2(-1, 1),
                            vec![ConstraintType::Corner(CornerPosition::BottomRight)],
                        ),
                        (
                            ivec2(0, 1),
                            vec![
                                ConstraintType::Corner(CornerPosition::BottomLeft),
                                ConstraintType::Edge(EdgePosition::Bottom),
                                ConstraintType::Corner(CornerPosition::BottomRight),
                            ],
                        ),
                        (
                            ivec2(1, 1),
                            vec![ConstraintType::Corner(CornerPosition::BottomLeft)],
                        ),
                    ]
                    .iter()
                    .all(|(offset_to_check, constraints_to_check)| {
                        let Some(part_to_check) = self.tiles.get(&(position + *offset_to_check))
                        else {
                            return true;
                        };

                        constraints_to_check.iter().all(|constraint| {
                            let Some(color_1) = part_to_check.constraints.get(constraint) else {
                                return true;
                            };

                            part.constraints
                                .get(&(constraint.get_opposite(*offset_to_check)))
                                .map(|color_2| color_1 == color_2)
                                .unwrap_or(false)
                        })
                    })
                })
            })
            .collect_vec();

        if valid_tiles.is_empty() {
            return None;
        }

        let tile = valid_tiles[fastrand::usize(0..valid_tiles.len())];

        for (offset, part) in tile.parts.iter() {
            self.tiles.insert(part_position + *offset, part.clone());
        }

        self.tiles.get(&part_position).cloned()
    }
}

pub fn debug_tiles(tile_generator: ResMut<TileGenerator>, mut gizmos: Gizmos) {
    for (position, tile_part) in tile_generator.tiles.iter() {
        for (constraint, color) in tile_part.constraints.iter() {
            let pixel_size = 1.0 / CHUNK_SIZE as f32;
            let constraint_position = match constraint {
                ConstraintType::Edge(edge) => match edge {
                    EdgePosition::Top => vec2(0.5 + pixel_size / 2.0, 1.0),
                    EdgePosition::Right => vec2(1.0, 0.5 + pixel_size / 2.0),
                    EdgePosition::Bottom => vec2(0.5 + pixel_size / 2.0, pixel_size),
                    EdgePosition::Left => vec2(pixel_size, 0.5 + pixel_size / 2.0),
                },
                ConstraintType::Corner(corner) => match corner {
                    CornerPosition::TopLeft => vec2(pixel_size, 1.0),
                    CornerPosition::TopRight => vec2(1.0, 1.0),
                    CornerPosition::BottomLeft => vec2(pixel_size, pixel_size),
                    CornerPosition::BottomRight => vec2(1.0, pixel_size),
                },
            };

            gizmos.rect_2d(
                (position.as_vec2() + Vec2::ONE / CHUNK_SIZE as f32 * 0.5 + constraint_position
                    - pixel_size)
                    * tile_generator.scale as f32,
                0.0,
                Vec2::ONE / CHUNK_SIZE as f32 * tile_generator.scale as f32,
                Color::rgba_u8(color[0], color[1], color[2], color[3]),
            );
        }
    }
}
