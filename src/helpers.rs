//Bresenham’s line-drawing algorithm
pub fn line_from_pixels<F: FnMut(i32, i32)>(
    x1: i32, 
    y1: i32, 
    x2: i32, 
    y2: i32, 
    function: &mut F
) {
    let dx:i32 = i32::abs(x2 - x1);
    let dy:i32 = i32::abs(y2 - y1);
    let sx:i32 = { if x1 < x2 { 1 } else { -1 } };
    let sy:i32 = { if y1 < y2 { 1 } else { -1 } };

    let mut error:i32 = (if dx > dy  { dx } else { -dy }) / 2 ;
    let mut current_x:i32 = x1;
    let mut current_y:i32 = y1;

    loop {
        function(current_x, current_y);

        if current_x == x2 && current_y == y2 { break; }
        let error2:i32 = error;

        if error2 > -dx {
            error -= dy;
            current_x += sx;
        }
        if error2 < dy {
            error += dx;
            current_y += sy;
        }
    }   
}