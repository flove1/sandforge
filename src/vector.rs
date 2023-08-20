#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct Vector2 {
    pub x: i64,
    pub y: i64,
}

#[macro_export]
macro_rules! vec2 {
    ($x:expr, $y:expr) => {
        Vector2{
            x: $x, 
            y: $y
        }
    };
}

impl Vector2 {
    pub fn is_between(&mut self, min: i64, max: i64) -> bool {
        self.x >= min && self.x <= max - 1 && self.y >= min && self.y <= max - 1
    }

    pub fn wrap(&mut self, min: i64, max: i64) {
        //idk if branchless variant is better, but i like it
        let range = max - min;
        self.x = ((self.x - min) % range + range) % range + min;
        self.y = ((self.y - min) % range + range) % range + min;

        // if self.x < min {
        //     self.x = max - self.x;
        // }
        // else if self.x > max {
        //     self.x = max - self.x + min;
        // }

        // if self.y < min {
        //     self.y = max - self.y;
        // }
        // else if self.x > max {
        //     self.y = max - self.y + min;
        // }
    }

    pub fn wrap_and_return_offset(&mut self, min: i64, max: i64) -> Vector2 {
        //same reason 
        let mut offset = vec2!(0, 0);
        offset.x = (((max - self.x - 1) >> 63) & 1) - (((self.x - min) >> 63) & 1);
        offset.y = (((max - self.y - 1) >> 63) & 1) - (((self.y - min) >> 63) & 1);

        let range = max - min;
        self.x = ((self.x - min) % range + range) % range + min;
        self.y = ((self.y - min) % range + range) % range + min;

        offset
    }
}


