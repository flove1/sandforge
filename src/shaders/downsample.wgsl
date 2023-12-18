// // Compute shader to downsample a texture by 4 times
// [[group(0), binding(0)]] var src_texture: texture_2d<f32>;
// [[group(0), binding(1)]] var dst_texture: texture_2d<f32>;

// // Compute shader function to perform downsampling
// [[stage(compute), workgroup_size(1, 1, 1)]]
// fn main(@builtin(global_invocation_id) id: vec3<u32>) {
//     // Get the dimensions of the source texture
//     let src_width = textureDimensions(src_texture).x;
//     let src_height = textureDimensions(src_texture).y;

//     // Calculate dimensions for the downsampled texture
//     let dst_width = src_width / 4u32;
//     let dst_height = src_height / 4u32;

//     // Calculate the coordinates in the downsampled texture
//     let dst_x = id.x * 4u32;
//     let dst_y = id.y * 4u32;

//     // Ensure the thread is within bounds of the downsampled texture
//     if dst_x < dst_width && dst_y < dst_height {
//         // Perform downsampling by averaging 4x4 blocks of the source texture
//         var sum: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 0.0);
//         for (var i = 0u; i < 4u; i = i + 1u) {
//             for (var j = 0u; j < 4u; j = j + 1u) {
//                 let src_x = dst_x + i;
//                 let src_y = dst_y + j;
//                 let texel = textureLoad(src_texture, vec2<i32>(i32(src_x), i32(src_y)), 2);
//                 sum = sum + texel;
//             }
//         }

//         // Average the sum to get the downsampled value
//         let average = sum / 16.0;

//         // Write the downsampled value to the destination texture
//         textureStore(dst_texture, vec2<i32>(dst_x / 4, dst_y / 4), average);
//     }
// }
