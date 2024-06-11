use bevy::prelude::*;

use async_channel::Sender;
use bevy::{
    asset::{ Assets, Handle },
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{ Extent3d, TextureDimension, TextureFormat },
        texture::{ BevyDefault, Image },
    },
};
use bevy_math::{ ivec2, IVec2, URect, UVec2, Vec2 };

use bevy_rapier2d::prelude::*;
use crate::constants::{ CHUNK_CELLS, CHUNK_SIZE };

use super::{
    chunk_groups::ChunkGroup,
    dirty_rect::{ RenderMessage, UpdateMessage },
    materials::PhysicsType,
    colliders::douglas_peucker,
    pixel::{ Pixel, WALL },
};

impl std::ops::Index<IVec2> for ChunkData {
    type Output = Pixel;
    #[track_caller]
    fn index(&self, position: IVec2) -> &Self::Output {
        &self.pixels[(position.y * CHUNK_SIZE + position.x) as usize]
    }
}

impl std::ops::IndexMut<IVec2> for ChunkData {
    #[track_caller]
    fn index_mut(&mut self, position: IVec2) -> &mut Self::Output {
        &mut self.pixels[(position.y * CHUNK_SIZE + position.x) as usize]
    }
}

#[derive(Component)]
pub struct Chunk;

#[derive(Component)]
pub struct ChunkCollider;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChunkState {
    Initialized,
    Generating,
    Populating,
    Active,
    Sleeping,
}

#[derive(Clone)]
pub struct ChunkData {
    pub pixels: Vec<Pixel>,
    pub texture: Handle<Image>,
    pub background: Handle<Image>,
    pub lighting: Handle<Image>,
    pub state: ChunkState,
}

impl Default for ChunkData {
    fn default() -> Self {
        Self {
            pixels: vec![Pixel::default(); CHUNK_CELLS as usize],
            texture: Handle::default(),
            background: Handle::default(),
            lighting: Handle::default(),
            state: ChunkState::Initialized,
        }
    }
}

impl ChunkData {
    pub fn new_image() -> Image {
        Image::new(
            Extent3d {
                height: CHUNK_SIZE as u32,
                width: CHUNK_SIZE as u32,
                ..Default::default()
            },
            TextureDimension::D2,
            vec![0; (CHUNK_CELLS * 4) as usize],
            TextureFormat::bevy_default(),
            RenderAssetUsages::all()
        )
    }

    pub fn update_textures(&self, images: &mut Assets<Image>, lighting: [f32; 3]) {
        self.update_textures_part(
            images,
            lighting,
            URect::from_corners(UVec2::ZERO, UVec2::splat(CHUNK_SIZE as u32))
        );
    }

    pub fn build_colliders(&self) -> Result<Vec<Collider>, String> {
        let values = self.pixels
            .iter()
            .map(|pixel| {
                if pixel.physics_type == PhysicsType::Static { 1.0 } else { 0.0 }
            })
            .collect::<Vec<f64>>();

        let contour_generator = contour::ContourBuilder::new(
            CHUNK_SIZE as u32,
            CHUNK_SIZE as u32,
            false
        );

        contour_generator
            .contours(&values, &[1.0])
            .map(|contours| {
                contours[0]
                    .geometry()
                    .0.iter()
                    .map(|polygon| {
                        let points = polygon
                            .interiors()
                            .iter()
                            .chain(std::iter::once(polygon.exterior()))
                            .map(|line| {
                                line.0
                                    .iter()
                                    .map(
                                        |point|
                                            Vec2::new(
                                                (point.x as f32) + 0.5,
                                                (point.y as f32) + 0.5
                                            ) / (CHUNK_SIZE as f32)
                                    )
                                    .collect::<Vec<Vec2>>()
                            })
                            .map(|line| {
                                douglas_peucker(&line, 0.25 / (CHUNK_SIZE.pow(2) as f32))
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

    pub fn update_textures_part(&self, images: &mut Assets<Image>, lighting_color: [f32; 3], rect: URect) {
        let Some(mut terrain) = images.remove(self.texture.clone()) else {
            panic!();
        };
        let Some(mut lighting) = images.remove(self.lighting.clone()) else {
            panic!();
        };
        let Some(background) = images.remove(self.background.clone()) else {
            panic!();
        };

        let fire_colors: [[u8; 4]; 5] = [
            [0xa9, 0x43, 0x1e, 0xff],
            [0xd7, 0x88, 0x25, 0xff],
            [0xea, 0xaa, 0x00, 0xff],
            [0xe1, 0xcd, 0x00, 0xff],
            [0xee, 0xdc, 0x00, 0xff],
        ];

        for x in rect.min.x..rect.max.x {
            for y in rect.min.y..rect.max.y {
                let index = (y * (CHUNK_SIZE as u32) + x) as usize;
                let pixel = &self.pixels[index];

                let texture_range = index * 4..(index + 1) * 4;

                if pixel.on_fire {
                    terrain.data[texture_range.clone()].copy_from_slice(
                        &fire_colors[fastrand::i32(0..fire_colors.len() as i32) as usize]
                    );
                } else {
                    let color = pixel.get_color();
                    terrain.data[texture_range.clone()].copy_from_slice(&color);
                }

                let terrain_opactiy = terrain.data[index * 4 + 3];
                let background_opactiy = background.data[index * 4 + 3];

                let lighting_value = if terrain_opactiy == 255 {
                    0.0
                } else {
                    (1.0 - (terrain_opactiy as f32) / 255.0) *
                        (1.0 - (background_opactiy as f32) / 255.0)
                };

                if let Some(color) = pixel.material.lighting {
                    lighting.data[texture_range.clone()].copy_from_slice(
                        &[
                            u8::max(color[0], (lighting_color[0] * 255.0 * lighting_value) as u8),
                            u8::max(color[1], (lighting_color[1] * 255.0 * lighting_value) as u8),
                            u8::max(color[2], (lighting_color[2] * 255.0 * lighting_value) as u8),
                            127,
                        ]
                    );
                } else if pixel.on_fire {
                    let color = fire_colors[fastrand::i32(0..fire_colors.len() as i32) as usize];
                    lighting.data[texture_range.clone()].copy_from_slice(
                        &[
                            u8::max(color[0], (lighting_color[0] * 255.0 * lighting_value) as u8),
                            u8::max(color[1], (lighting_color[1] * 255.0 * lighting_value) as u8),
                            u8::max(color[2], (lighting_color[2] * 255.0 * lighting_value) as u8),
                            127,
                        ]
                    );
                } else {
                    lighting.data[texture_range.clone()].copy_from_slice(
                        &[
                            (lighting_color[0] * 255.0 * lighting_value) as u8,
                            (lighting_color[1] * 255.0 * lighting_value) as u8,
                            (lighting_color[2] * 255.0 * lighting_value) as u8,
                            127,
                        ]
                    );
                }
            }
        }

        images.insert(self.texture.clone(), terrain);
        images.insert(self.lighting.clone(), lighting);
        images.insert(self.background.clone(), background);
    }
}

pub struct ChunkApi<'a> {
    pub chunk_position: IVec2,
    pub cell_position: IVec2,
    pub chunk_group: &'a mut ChunkGroup<Pixel>,
    pub update_send: &'a Sender<UpdateMessage>,
    pub render_send: &'a Sender<RenderMessage>,
    pub collider_send: &'a Sender<IVec2>,
    pub clock: u8,
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
            Some(pixel) => pixel.physics_type.clone(),
            None => PhysicsType::Static,
        }
    }

    pub fn set(&mut self, dx: i32, dy: i32, pixel: Pixel) {
        let cell_position = self.cell_position + ivec2(dx, dy);
        self.chunk_group[cell_position] = pixel;
        self.keep_alive(dx, dy);
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
                chunk_position: self.chunk_position +
                cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                awake_surrouding: true,
            })
            .unwrap();

        self.render_send
            .try_send(RenderMessage {
                cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                chunk_position: self.chunk_position +
                cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
            })
            .unwrap();
    }

    pub fn update(&mut self, pixel: Pixel) {
        self.chunk_group[self.cell_position] = pixel;

        self.render_send
            .try_send(RenderMessage {
                cell_position: self.cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                chunk_position: self.chunk_position +
                self.cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
            })
            .unwrap();
    }

    pub fn collider_changed(&mut self, dx: i32, dy: i32) {
        let cell_position = self.cell_position + IVec2::new(dx, dy);
        let chunk_offset = cell_position.div_euclid(IVec2::splat(CHUNK_SIZE));

        self.collider_send
            .try_send(self.chunk_position + chunk_offset)
            .ok();
    }

    pub fn mark_updated(&mut self) {
        self.chunk_group[self.cell_position].updated_at = self.clock;

        self.render_send
            .try_send(RenderMessage {
                cell_position: self.cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                chunk_position: self.chunk_position +
                self.cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
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

    pub fn once_in(&mut self, n: i32) -> bool {
        self.rand_int(n) == 0
    }

    pub fn switch_position(&mut self, cell_position: IVec2) {
        self.cell_position = cell_position;
    }
}
