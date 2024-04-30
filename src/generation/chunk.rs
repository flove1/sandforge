// use bevy::{prelude::*, sprite::Anchor, utils::HashMap};
// use bevy_math::{ivec2, vec2};
// use bevy_rapier2d::dynamics::RigidBody;
// use itertools::Itertools;

// use crate::{
//     constants::CHUNK_SIZE,
//     helpers::to_index,
//     registries::Registries,
//     simulation::{
//         chunk::ChunkData,
//         chunk_groups::ChunkGroupCustom,
//         chunk_manager::{ChunkManager, Chunks},
//         pixel::Pixel,
//     },
// };

// use super::tiles::TileGenerator;

// #[derive(Event)]
// pub struct ChunkGenerationEvent(pub IVec2);

// pub fn generate_chunk(
//     mut ev_chunkgen: EventReader<ChunkGenerationEvent>,
//     mut chunk_manager: ResMut<ChunkManager>,
//     mut commands: Commands,
//     mut images: ResMut<Assets<Image>>,
//     mut tile_generator: ResMut<TileGenerator>,
//     registries: Res<Registries>,
//     chunks_query: Query<Entity, With<Chunks>>,
// ) {
//     let biomes = registries.biomes.lock().unwrap();
//     let biome = biomes.get("caves").unwrap();
//     let chunks_entity = chunks_query.single();

//     let materials = registries.materials.lock().unwrap();
//     let element = materials.get("stone").unwrap();
//     let mut chunks_to_add = HashMap::new();

//     for ev in ev_chunkgen.read() {
//         let chunk_position = ev.0;

//         if chunk_manager.chunks.contains_key(&chunk_position)
//             || chunks_to_add.contains_key(&chunk_position)
//         {
//             continue;
//         }

//         let Some(part) = tile_generator.get_tile(chunk_position, biome) else {
//             continue;
//         };

//         let mut noise_data =
//             vec![vec![0.0; (CHUNK_SIZE.pow(2)) as usize]; tile_generator.scale.pow(2) as usize];

//         let mut noise_group = ChunkGroupCustom {
//             chunks: HashMap::new(),
//             size: CHUNK_SIZE,
//         };

//         for (x, y) in (-1..tile_generator.scale + 1).cartesian_product(-1..tile_generator.scale + 1)
//         {
//             let position = ivec2(x, y);

//             if position.cmpeq(-IVec2::ONE).any()
//                 || position.cmpeq(IVec2::ONE * tile_generator.scale).any()
//             {
//                 if let Some(chunk) = chunk_manager
//                     .get_chunk_mut(&(chunk_position + position))
//                     .or(chunks_to_add.get_mut(&(chunk_position + position)))
//                 {
//                     let ptr = chunk.layout.as_mut_ptr();
//                     noise_group.chunks.insert(position, ptr);
//                 }
//             } else {
//                 let ptr = noise_data[to_index!(position, tile_generator.scale)].as_mut_ptr();
//                 noise_group.chunks.insert(position, ptr);
//             }
//         }

//         for (chunk_x, chunk_y) in
//             (0..tile_generator.scale).cartesian_product(0..tile_generator.scale)
//         {
//             let chunk_position = ivec2(chunk_x, chunk_y);
//             for (x, y) in (0..CHUNK_SIZE).cartesian_product(0..CHUNK_SIZE) {
//                 let position = ivec2(x, y);
//                 let converted = position.as_vec2() / CHUNK_SIZE as f32 * biome.tile_size as f32 / tile_generator.scale as f32;
//                 let in_part_position = ((chunk_position + chunk_position).rem_euclid(IVec2::ONE * tile_generator.scale)).as_vec2()
//                     * biome.tile_size as f32
//                     / tile_generator.scale as f32;

//                 let index = to_index!(
//                     (converted + in_part_position)
//                         .floor()
//                         .as_ivec2()
//                         .min(IVec2::ONE * (biome.tile_size as i32 - 1)),
//                         biome.tile_size as i32
//                 );
        
//                 let color = part.data[index];
//                 let color_no_alpha = &color[0..3];

//                 if let Ok(value) = color_no_alpha.iter().all_equal_value() {
//                     noise_group[position + chunk_position * CHUNK_SIZE] = *value as f32 / 255.0;
//                 }
//             }
//         }

//         let smoothing_iterations = 2;

//         let ca_iterations = 2;
//         let threshold = 0.2;

//         let mut steps = (0..CHUNK_SIZE)
//             .cartesian_product(0..CHUNK_SIZE)
//             .collect_vec();
//         fastrand::shuffle(&mut steps);

//         for _ in 0..smoothing_iterations {
//             for (chunk_x, chunk_y) in
//                 (0..tile_generator.scale).cartesian_product(0..tile_generator.scale)
//             {
//                 let chunk_position = ivec2(chunk_x, chunk_y);
//                 fastrand::shuffle(&mut steps);

//                 for (x, y) in steps.iter() {
//                     let position = ivec2(*x, *y) + chunk_position * CHUNK_SIZE;
//                     if noise_group.get(position).is_none() {
//                         continue;
//                     };

//                     let mut sum = 0.0;
//                     let mut count = 0;

//                     let offsets = (-1..=1)
//                         .cartesian_product(-1..=1)
//                         .map(|(x, y)| ivec2(x, y))
//                         .filter(|offset| noise_group.get(position + *offset).is_some())
//                         .collect_vec();

//                     for offset in offsets.iter() {
//                         sum += noise_group[position + *offset];
//                         count += 1;
//                     }

//                     for offset in offsets.iter() {
//                         let offseted_position = position + *offset;
//                         if offseted_position.cmpge(IVec2::ZERO).all()
//                             && offseted_position
//                                 .cmplt(IVec2::ONE * CHUNK_SIZE * tile_generator.scale)
//                                 .all()
//                         {
//                             noise_group[position + *offset] = sum / count as f32;
//                         }
//                     }
//                 }
//             }
//         }

//         for _ in 0..ca_iterations {
//             for (chunk_x, chunk_y) in
//                 (0..tile_generator.scale).cartesian_product(0..tile_generator.scale)
//             {
//                 let chunk_position = ivec2(chunk_x, chunk_y);
//                 fastrand::shuffle(&mut steps);

//                 for (x, y) in steps.iter() {
//                     let position = ivec2(*x, *y) + chunk_position * CHUNK_SIZE;

//                     if noise_group.get(position).is_none() {
//                         continue;
//                     };

//                     let mut sum = 0.0;
//                     let mut count = 0;

//                     let border_flag = position.cmpeq(IVec2::ZERO).any()
//                         || position
//                             .cmpeq(IVec2::ONE * (CHUNK_SIZE * tile_generator.scale - 1))
//                             .any();

//                     if border_flag {
//                         count += 1;
//                     }

//                     for (dx, dy) in (-1..=1).cartesian_product(-1..=1) {
//                         if let Some(value) = noise_group.get(position + ivec2(dx, dy)) {
//                             if *value > threshold {
//                                 count += 1;
//                                 sum += value;
//                             }
//                         }
//                     }

//                     if count <= 4 {
//                         noise_group[position] = 0.0;
//                     } else {
//                         if !border_flag {
//                             count += 1;
//                         }

//                         noise_group[position] = (sum + noise_group[position]) / (count) as f32;
//                     }
//                 }
//             }
//         }

//         for (index, layout) in noise_data.drain(..).enumerate() {
//             let chunk_position = ivec2(
//                 (index as i32).rem_euclid(tile_generator.scale),
//                 (index as i32).div_euclid(tile_generator.scale),
//             ) + chunk_position;

//             let mut chunk = ChunkData::default();
//             for (x, y) in (0..CHUNK_SIZE).cartesian_product(0..CHUNK_SIZE) {
//                 let position = ivec2(x, y);
//                 let alpha_value = (layout[to_index!(position, CHUNK_SIZE)] * 255.0_f32)
//                     .floor()
//                     .clamp(0.0, 255.0) as u8;

//                 if alpha_value > 10 {
//                     chunk[position] = Pixel::new(
//                         element.clone().into(),
//                     ).with_clock(chunk_manager.clock());
//                 }
//             }

//             chunk.layout = layout;
//             chunks_to_add.insert(chunk_position, chunk);
//         }
//     }

//     for (position, mut chunk) in chunks_to_add.into_iter() {
//         let image_handle = images.add(ChunkData::new_image());
//         let mut entity_command = commands.spawn((
//             RigidBody::Fixed,
//             SpriteBundle {
//                 texture: image_handle.clone(),
//                 sprite: Sprite {
//                     custom_size: Some(vec2(1.0, 1.0)),
//                     anchor: Anchor::BottomLeft,
//                     flip_y: true,
//                     ..Default::default()
//                 },
//                 transform: Transform::from_translation(Vec3::new(
//                     position.x as f32,
//                     position.y as f32,
//                     0.,
//                 )),
//                 ..Default::default()
//             },
//         ));

//         if let Ok(colliders) = chunk.build_colliders() {
//             entity_command.with_children(|children| {
//                 for collider in colliders {
//                     children.spawn((
//                         collider,
//                         TransformBundle {
//                             local: Transform::IDENTITY,
//                             ..Default::default()
//                         },
//                     ));
//                 }
//             });
//         }

//         let id = entity_command.id();

//         commands.entity(chunks_entity).push_children(&[id]);

//         chunk.texture = image_handle.clone();
//         chunk.entity = Some(id);

//         chunk.update_all(images.get_mut(&image_handle.clone()).unwrap());

//         chunk_manager.chunks.insert(position, chunk);
//     }
// }
