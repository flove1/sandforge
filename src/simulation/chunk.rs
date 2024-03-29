use async_channel::Sender;
use bevy::{
    asset::Handle,
    ecs::entity::Entity,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        texture::Image,
    },
};
use bevy_math::{ivec2, IVec2, URect, Vec2};

use bevy_rapier2d::prelude::*;
use crate::constants::{CHUNK_CELLS, CHUNK_SIZE, COLLIDER_PRECISION};

use super::{
    chunk_groups::ChunkGroup3x3,
    dirty_rect::{RenderMessage, UpdateMessage},
    materials::{Material, PhysicsType},
    mesh::douglas_peucker,
    pixel::{Pixel, WALL},
};

impl std::ops::Index<IVec2> for ChunkData {
    type Output = Pixel;
    #[track_caller]
    fn index(&self, position: IVec2) -> &Self::Output {
        &self.cells[(position.y * CHUNK_SIZE + position.x) as usize]
    }
}

impl std::ops::IndexMut<IVec2> for ChunkData {
    #[track_caller]
    fn index_mut(&mut self, position: IVec2) -> &mut Self::Output {
        &mut self.cells[(position.y * CHUNK_SIZE + position.x) as usize]
    }
}

#[derive(Clone)]
pub struct ChunkData {
    pub entity: Option<Entity>,
    pub cells: Vec<Pixel>,
    pub texture: Handle<Image>,
}

impl Default for ChunkData {
    fn default() -> Self {
        Self {
            entity: None,
            cells: vec![Pixel::default(); CHUNK_CELLS as usize],
            texture: Handle::default(),
        }
    }
}

impl ChunkData {
    pub fn new(entity: Option<Entity>) -> ChunkData {
        let cells = vec![Pixel::default(); CHUNK_CELLS as usize];
        ChunkData {
            cells,
            texture: Handle::default(),
            entity,
        }
    }

    pub fn new_image() -> Image {
        Image::new(
            Extent3d {
                height: CHUNK_SIZE as u32,
                width: CHUNK_SIZE as u32,
                ..Default::default()
            },
            TextureDimension::D2,
            vec![0; (CHUNK_CELLS * 4) as usize],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        )
    }

    pub fn update_all(&self, image: &mut Image) {
        let fire_colors: [[u8; 4]; 5] = [
            [0xA9, 0x43, 0x1E, 0xFF],
            [0xD7, 0x88, 0x25, 0xFF],
            [0xEA, 0xAA, 0x00, 0xFF],
            [0xE1, 0xCD, 0x00, 0xFF],
            [0xEE, 0xDC, 0x00, 0xFF],
        ];

        self.cells.iter().enumerate().for_each(|(index, pixel)| {
            image.data[index * 4..(index + 1) * 4].copy_from_slice(&pixel.material.color);
            if pixel.on_fire {
                image.data[index * 4..(index + 1) * 4].copy_from_slice(
                    &fire_colors[fastrand::i32(0..fire_colors.len() as i32) as usize],
                );
            } else {
                let mut color = pixel.get_color();

                if let PhysicsType::Liquid(parameters) = pixel.material.physics_type {
                    color[3] = (f32::clamp(parameters.volume * 5.0, 0.1, 0.7) * 255.0) as u8;
                }

                // if let SimulationType::Displaced(dx, dy) = pixel.simulation {
                //     color[0] = f32::sqrt(dx.powi(2) + dy.powi(2)) as u8 * 16;
                //     color[1] = 0;
                //     color[2] = 0;
                // }

                image.data[index * 4..(index + 1) * 4].copy_from_slice(&color);
            }
        });
    }

    pub fn build_colliders(&self) -> Result<Vec<Collider>, String> {
        let values = self
            .cells
            .iter()
            .map(|pixel| {
                if pixel.material.physics_type == PhysicsType::Static {
                    1.0
                } else {
                    0.0
                }
            })
            .collect::<Vec<f64>>();

        let contour_generator =
            contour::ContourBuilder::new(CHUNK_SIZE as u32, CHUNK_SIZE as u32, false);
        contour_generator
            .contours(&values, &[1.0])
            .map(|contours| {
                contours[0]
                    .geometry()
                    .0
                    .iter()
                    .map(|polygon| {
                        let points = polygon
                            .interiors()
                            .iter()
                            .chain(std::iter::once(polygon.exterior()))
                            .map(|line| {
                                line.0
                                    .iter()
                                    .map(|point| {
                                        Vec2::new(
                                            point.x as f32 / CHUNK_SIZE as f32,
                                            point.y as f32 / CHUNK_SIZE as f32,
                                        )
                                    })
                                    .collect::<Vec<Vec2>>()
                            })
                            .map(|line| {
                                douglas_peucker(
                                    &line,
                                    COLLIDER_PRECISION / CHUNK_SIZE.pow(2) as f32,
                                )
                            })
                            .filter(|points| points.len() > 2)
                            .collect::<Vec<Vec<Vec2>>>();

                        points
                    })
                    .filter(|polygon| !polygon.is_empty())
                    .flat_map(|boundaries| {
                        boundaries
                            .iter()
                            .map(|boundary| {
                                let vertices = boundary
                                    .iter()
                                    .map(|point| Vec2::new(point[0], point[1]))
                                    .collect();

                                Collider::polyline(vertices, None)
                            })
                            .collect::<Vec<Collider>>()
                    })
                    .collect::<Vec<Collider>>()
            })
            .map_err(|_| "no contours were found".to_string())
    }

    pub fn update_rect(&self, image: &mut Image, rect: URect) {
        let fire_colors: [[u8; 4]; 5] = [
            [0xA9, 0x43, 0x1E, 0xFF],
            [0xD7, 0x88, 0x25, 0xFF],
            [0xEA, 0xAA, 0x00, 0xFF],
            [0xE1, 0xCD, 0x00, 0xFF],
            [0xEE, 0xDC, 0x00, 0xFF],
        ];

        for x in rect.min.x..rect.max.x {
            for y in rect.min.y..rect.max.y {
                let index = (y * CHUNK_SIZE as u32 + x) as usize;
                let pixel = &self.cells[index];
                if pixel.on_fire {
                    image.data[index * 4..(index + 1) * 4].copy_from_slice(
                        &fire_colors[fastrand::i32(0..fire_colors.len() as i32) as usize],
                    );
                } else {
                    let mut color = pixel.get_color();

                    if let PhysicsType::Liquid(parameters) = pixel.material.physics_type {
                        color[3] = (f32::clamp(parameters.volume * 5.0, 0.1, 0.7) * 255.0) as u8;
                    }

                    // if let SimulationType::Displaced(dx, dy) = pixel.simulation {
                    //     color[0] = f32::sqrt(dx.powi(2) + dy.powi(2)) as u8 * 16;
                    //     color[1] = 0;
                    //     color[2] = 0;
                    // }

                    image.data[index * 4..(index + 1) * 4].copy_from_slice(&color);
                }
            }
        }
    }
}

pub struct ChunkApi<'a> {
    pub(super) chunk_position: IVec2,
    pub(super) cell_position: IVec2,
    pub(super) chunk_group: &'a mut ChunkGroup3x3,
    pub(super) update_send: &'a Sender<UpdateMessage>,
    pub(super) render_send: &'a Sender<RenderMessage>,
    pub(super) clock: u8,
}

impl<'a> ChunkApi<'a> {
    pub fn get(&self, dx: i32, dy: i32) -> Pixel {
        let cell_position = self.cell_position + ivec2(dx, dy);

        match self.chunk_group.get(cell_position) {
            Some(pixel) => pixel.clone(),
            None => WALL.clone(),
        }
    }

    pub fn get_counter(&self, dx: i32, dy: i32) -> u8 {
        let cell_position = self.cell_position + ivec2(dx, dy);

        match self.chunk_group.get(cell_position) {
            Some(pixel) => pixel.updated_at,
            None => 0,
        }
    }

    pub fn get_physics_type(&self, dx: i32, dy: i32) -> PhysicsType {
        let cell_position = self.cell_position + ivec2(dx, dy);

        match self.chunk_group.get(cell_position) {
            Some(pixel) => pixel.material.physics_type,
            None => PhysicsType::Static,
        }
    }

    pub fn is_empty(&self, dx: i32, dy: i32) -> bool {
        let cell_position = self.cell_position + ivec2(dx, dy);

        match self.chunk_group.get(cell_position) {
            Some(pixel) => pixel.is_empty(),
            None => false,
        }
    }

    pub fn match_element(&self, dx: i32, dy: i32, material: &Material) -> bool {
        let cell_position = self.cell_position + ivec2(dx, dy);

        match self.chunk_group.get(cell_position) {
            Some(pixel) => pixel.material.id == material.id,
            None => false,
        }
    }

    pub fn set(&mut self, dx: i32, dy: i32, pixel: Pixel) {
        let cell_position = self.cell_position + ivec2(dx, dy);
        self.chunk_group[cell_position] = pixel;
    }

    pub fn swap(&mut self, dx: i32, dy: i32) {
        let cell_position_1 = self.cell_position;
        let cell_position_2 = self.cell_position + ivec2(dx, dy);

        let temp = self.chunk_group[cell_position_1].clone();
        self.chunk_group[cell_position_1] = self.chunk_group[cell_position_2].clone();
        self.chunk_group[cell_position_2] = temp;

        self.keep_alive(0, 0);
        self.keep_alive(dx, dy);

        self.cell_position += ivec2(dx, dy);
    }

    pub fn keep_alive(&mut self, dx: i32, dy: i32) {
        let cell_position = self.cell_position + ivec2(dx, dy);

        self.update_send
            .try_send(UpdateMessage {
                cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                chunk_position: self.chunk_position
                    + cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                awake_surrouding: true,
            })
            .unwrap();

        self.render_send
            .try_send(RenderMessage {
                cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                chunk_position: self.chunk_position
                    + cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
            })
            .unwrap();
    }

    pub fn update(&mut self, pixel: Pixel) {
        self.chunk_group[self.cell_position] = pixel;

        self.render_send
            .try_send(RenderMessage {
                cell_position: self
                    .cell_position
                    .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                    .as_uvec2(),
                chunk_position: self.chunk_position
                    + self.cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
            })
            .unwrap();
    }

    pub fn mark_updated(&mut self) {
        self.chunk_group[self.cell_position].updated_at = self.clock;

        self.render_send
            .try_send(RenderMessage {
                cell_position: self
                    .cell_position
                    .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                    .as_uvec2(),
                chunk_position: self.chunk_position
                    + self.cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
            })
            .unwrap();
    }

    pub fn rand_int(&mut self, n: i32) -> i32 {
        fastrand::i32(0..n)
    }

    pub fn rand_dir(&mut self) -> i32 {
        let i = self.rand_int(1000);
        if i % 2 == 0 {
            -1
        } else {
            1
        }
    }

    pub fn rand_vec(&mut self) -> (i32, i32) {
        let i = self.rand_int(2000);
        match i % 9 {
            0 => (1, 1),
            1 => (1, 0),
            2 => (1, -1),
            3 => (0, -1),
            4 => (-1, -1),
            5 => (-1, 0),
            6 => (-1, 1),
            7 => (0, 1),
            _ => (0, 0),
        }
    }

    pub fn rand_vec_8(&mut self) -> (i32, i32) {
        let i = self.rand_int(8);
        match i {
            0 => (1, 1),
            1 => (1, 0),
            2 => (1, -1),
            3 => (0, -1),
            4 => (-1, -1),
            5 => (-1, 0),
            6 => (-1, 1),
            _ => (0, 1),
        }
    }

    pub fn once_in(&mut self, n: i32) -> bool {
        self.rand_int(n) == 0
    }

    pub fn switch_position(&mut self, cell_position: IVec2) {
        self.cell_position = cell_position;
    }
}
