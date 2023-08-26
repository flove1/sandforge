#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
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
        self.x >= min && self.x <= max && self.y >= min && self.y <= max
    }

    pub fn add(&self, x: i64, y: i64) -> Self {
        Self { 
            x: self.x + x,
            y: self.y + y,
        }
    }

    pub fn inc(&mut self, x: i64, y: i64) {
        self.x += x;
        self.y += y;
    }

    /// Wraps vector's fields around boundaries and return modified vector and vector, that shows in which way they were wrapped
    pub fn wrap(mut self, min: i64, max: i64) -> (Self, Self) {
        let mut offset = vec2!(0, 0);
        offset.x = (((max - self.x - 1) >> 63) & 1) - (((self.x - min) >> 63) & 1);
        offset.y = (((max - self.y - 1) >> 63) & 1) - (((self.y - min) >> 63) & 1);

        let range = max - min;
        self.x = ((self.x - min) % range + range) % range + min;
        self.y = ((self.y - min) % range + range) % range + min;

        (self, offset)
    }

    pub fn clamp(mut self, min: i64, max: i64) -> Self {
        self.x = self.x.clamp(min, max);
        self.y = self.y.clamp(min, max);
        self
    }

    pub fn to_index(&self, size: i64) -> usize {
        (self.y * size + self.x) as usize
    }

    pub fn is_zero(&self) -> bool {
        self.x == 0 && self.y == 0
    }
}

impl std::ops::Add<Vector2> for Vector2 {
    type Output = Vector2;

    fn add(self, other: Vector2) -> Vector2 {
        Vector2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl<'a> std::ops::Add<Vector2> for &'a Vector2 {
    type Output = Vector2;

    fn add(self, other: Vector2) -> Vector2 {
        Vector2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl<'b> std::ops::Add<&'b Vector2> for Vector2 {
    type Output = Vector2;

    fn add(self, other: &'b Vector2) -> Vector2 {
        Vector2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl<'a, 'b> std::ops::Add<&'b Vector2> for &'a Vector2 {
    type Output = Vector2;

    fn add(self, other: &'b Vector2) -> Vector2 {
        Vector2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}