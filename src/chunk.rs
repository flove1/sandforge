
use async_channel::Sender;
use bevy::{asset::Handle, ecs::entity::Entity, render::{render_asset::RenderAssetUsages, render_resource::{Extent3d, TextureDimension, TextureFormat}, texture::Image}};
use bevy_math::{ivec2, IVec2, URect};

use crate::{constants::{CHUNK_CELLS, CHUNK_SIZE}, dirty_rect::{RenderMessage, UpdateMessage}, materials::{Material, PhysicsType}, pixel::{Pixel, SimulationType, WALL}};

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
        ChunkData { cells, texture: Handle::default(), entity }
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
            RenderAssetUsages::all()
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

        self.cells.iter().enumerate()
            .for_each(|(index, pixel)| {
                image.data[index * 4 .. (index + 1) * 4].copy_from_slice(&pixel.material.color);
                if pixel.on_fire {
                    image.data[index * 4 .. (index + 1) * 4].copy_from_slice(&fire_colors[fastrand::i32(0..fire_colors.len() as i32) as usize]);
                }
                else {
                    let mut color = pixel.get_color();

                    if let PhysicsType::Liquid(parameters) = pixel.material.matter_type {
                        color[3] = (f32::clamp(parameters.volume * 5.0, 0.1, 0.7) * 255.0) as u8;
                    }

                    if let SimulationType::Displaced(dx, dy) = pixel.simulation {
                        color[0] = f32::sqrt(dx.powi(2) + dy.powi(2)) as u8 * 16;
                        color[1] = 0;
                        color[2] = 0;
                    }

                    image.data[index * 4 .. (index + 1) * 4].copy_from_slice(&color);
                }
            });
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
                    image.data[index * 4 .. (index + 1) * 4].copy_from_slice(&fire_colors[fastrand::i32(0..fire_colors.len() as i32) as usize]);
                }
                else {
                    let mut color = pixel.get_color();

                    if let PhysicsType::Liquid(parameters) = pixel.material.matter_type {
                        color[3] = (f32::clamp(parameters.volume * 5.0, 0.1, 0.7) * 255.0) as u8;
                    }

                    if let SimulationType::Displaced(dx, dy) = pixel.simulation {
                        color[0] = f32::sqrt(dx.powi(2) + dy.powi(2)) as u8 * 16;
                        color[1] = 0;
                        color[2] = 0;
                    }

                    image.data[index * 4 .. (index + 1) * 4].copy_from_slice(&color);
                }
            }
        }
    }
}


        
// 6 7 8
// 3 4 5
// 0 1 2
pub type ChunkCenter = Option<*mut Pixel>;
pub type ChunkCorners = [Option<*mut Pixel>; 4];
pub type ChunkSides = [Option<*mut Pixel>; 4];

pub struct ChunkGroup {
    pub center: ChunkCenter,
    pub corners: ChunkCorners,
    pub sides: ChunkSides,
}

impl ChunkGroup {    
    fn chunk_offset_to_id(&self, chunk_offset: IVec2) -> i32 {
        (chunk_offset.y + 1) * 3 + chunk_offset.x + 1
    }

    fn get(&self, cell_position: IVec2) -> Option<&Pixel> {
        let chunk_offset = cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }

        let id = self.chunk_offset_to_id(chunk_offset);

        let cell_position = cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE);
        let cell_index = (cell_position.y * CHUNK_SIZE + cell_position.x) as usize;

        match id {
            4 => Some(unsafe { self.center.as_ref().unwrap().add(cell_index).as_ref().unwrap() }),
            0 | 2 | 6 | 8 => {
                let mut corner_index = 0;

                if chunk_offset.x > 0 {
                    corner_index += 1;
                } 

                if chunk_offset.y > 0 {
                    corner_index += 2;
                }

                self.corners[corner_index].as_ref().map(|chunk| unsafe { chunk.add(cell_index).as_ref().unwrap() })
            }
            1 | 3 | 5 | 7 => {
                let mut side_index = 0;

                if chunk_offset.y > 0 {
                    side_index += 3;
                }

                if chunk_offset.x != 0 {
                    side_index = ((chunk_offset.x + 1).signum() + 1) as usize;
                }

                self.sides[side_index].as_ref().map(|chunk| unsafe { chunk.add(cell_index).as_ref().unwrap() })
            }
            _ => unreachable!()
        }
    }

    fn get_mut(&mut self, cell_position: IVec2) -> Option<&mut Pixel> {
        let chunk_offset = cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }
        
        let id = self.chunk_offset_to_id(chunk_offset);

        let cell_position = cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE);
        let cell_index = (cell_position.y * CHUNK_SIZE + cell_position.x) as usize;

        match id {
            4 => Some(unsafe { self.center.as_mut().unwrap().add(cell_index).as_mut().unwrap() }),
            0 | 2 | 6 | 8 => {
                let mut corner_index = 0;

                if chunk_offset.x > 0 {
                    corner_index += 1;
                } 

                if chunk_offset.y > 0 {
                    corner_index += 2;
                }

                if let Some(chunk) = &mut self.corners[corner_index] {
                    Some(unsafe { chunk.add(cell_index).as_mut().unwrap() })
                }
                else {
                    None
                }
            }
            1 | 3 | 5 | 7 => {
                let mut side_index = 0;

                if chunk_offset.y > 0 {
                    side_index += 3;
                }

                if chunk_offset.x != 0 {
                    side_index = ((chunk_offset.x + 1).signum() + 1) as usize;
                }

                if let Some(chunk) = &mut self.sides[side_index] {
                    Some(unsafe { chunk.add(cell_index).as_mut().unwrap() })
                }
                else {
                    None
                }
            }
            _ => unreachable!()
        }
    }
}

impl std::ops::Index<IVec2> for ChunkGroup {
    type Output = Pixel;
    #[track_caller]
    fn index(&self, idx: IVec2) -> &Self::Output {
        self.get(idx).expect("Invalid index position.")
    }
}
impl std::ops::IndexMut<IVec2> for ChunkGroup {
    #[track_caller]
    fn index_mut(&mut self, idx: IVec2) -> &mut Self::Output {
        self.get_mut(idx).expect("Invalid index position.")
    }
}


pub struct ChunkApi<'a> {
    pub(super) chunk_position: IVec2,
    pub(super) cell_position: IVec2,
    pub(super) chunk_group: &'a mut ChunkGroup,
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

    pub fn get_matter(&self, dx: i32, dy: i32) -> PhysicsType {
        let cell_position = self.cell_position + ivec2(dx, dy);

        match self.chunk_group.get(cell_position) {
            Some(pixel) => pixel.material.matter_type,
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

    pub fn swap(&mut self, dx:i32, dy: i32) {
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
                chunk_position: self.chunk_position + cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                awake_surrouding: true
            })
            .unwrap();

        self.render_send
            .try_send(RenderMessage {
                cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                chunk_position: self.chunk_position + cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
            })
            .unwrap();
    }

    pub fn update(&mut self, pixel: Pixel) {
        self.chunk_group[self.cell_position] = pixel;

        self.render_send
            .try_send(RenderMessage {
                cell_position: self.cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                chunk_position: self.chunk_position + self.cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
            })
            .unwrap();
    }

    pub fn mark_updated(&mut self) {
        self.chunk_group[self.cell_position].updated_at = self.clock;

        self.render_send
            .try_send(RenderMessage {
                cell_position: self.cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                chunk_position: self.chunk_position + self.cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
            })
            .unwrap();
    }
    
    pub fn rand_int(&mut self, n: i32) -> i32 {
        fastrand::i32(0..n)
    }
 
    pub fn rand_dir(&mut self) -> i32 {
        let i = self.rand_int(1000);
        if i%2 == 0 {
            -1
        }
        else {
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