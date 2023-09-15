#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
pub struct Pos2 {
    pub x: i32,
    pub y: i32,
}

#[macro_export]
macro_rules! pos2 {
    ($x:expr, $y:expr) => {
        Pos2{
            x: $x, 
            y: $y
        }
    };
}

impl Pos2 {
    pub fn is_between(&mut self, min: i32, max: i32) -> bool {
        self.x >= min && self.x <= max && self.y >= min && self.y <= max
    }

    pub fn add(&self, x: i32, y: i32) -> Self {
        Self { 
            x: self.x + x,
            y: self.y + y,
        }
    }

    pub fn change(&mut self, x: i32, y: i32) {
        self.x += x;
        self.y += y;
    }

    /// Wraps vector's fields around boundaries and return modified vector and vector, that shows in which way they were wrapped
    pub fn wrap(mut self, min: i32, max: i32) -> (Self, Self) {
        let mut offset = pos2!(0, 0);
        offset.x = (((max - self.x - 1) >> 31) & 1) - (((self.x - min) >> 31) & 1);
        offset.y = (((max - self.y - 1) >> 31) & 1) - (((self.y - min) >> 31) & 1);

        let range = max - min;
        self.x = ((self.x - min) % range + range) % range + min;
        self.y = ((self.y - min) % range + range) % range + min;

        (self, offset)
    }

    pub fn clamp(mut self, min: i32, max: i32) -> Self {
        self.x = self.x.clamp(min, max);
        self.y = self.y.clamp(min, max);
        
        self
    }

    pub fn distance_to(&self, other: &Self) -> f32 {
        (((other.x - self.x).pow(2) + (other.y - self.y).pow(2)) as f32).sqrt()
    }

    pub fn to_index(&self, size: i32) -> usize {
        (self.y * size + self.x) as usize
    }

    pub fn is_zero(&self) -> bool {
        self.x == 0 && self.y == 0
    }
}

impl std::ops::Add<Pos2> for Pos2 {
    type Output = Pos2;

    fn add(self, other: Pos2) -> Pos2 {
        Pos2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl<'a> std::ops::Add<Pos2> for &'a Pos2 {
    type Output = Pos2;

    fn add(self, other: Pos2) -> Pos2 {
        Pos2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl<'b> std::ops::Add<&'b Pos2> for Pos2 {
    type Output = Pos2;

    fn add(self, other: &'b Pos2) -> Pos2 {
        Pos2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl<'a, 'b> std::ops::Add<&'b Pos2> for &'a Pos2 {
    type Output = Pos2;

    fn add(self, other: &'b Pos2) -> Pos2 {
        Pos2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}


#[derive(Default, PartialEq, PartialOrd, Clone, Copy, Debug)]
pub struct Pos2F {
    pub x: f32,
    pub y: f32,
}

#[macro_export]
macro_rules! pos2f {
    ($x:expr, $y:expr) => {
        Pos2F{
            x: $x, 
            y: $y
        }
    };
}

impl Pos2F {
    pub fn is_between(&mut self, min: f32, max: f32) -> bool {
        self.x >= min && self.x <= max && self.y >= min && self.y <= max
    }

    pub fn add(&self, x: f32, y: f32) -> Self {
        Self { 
            x: self.x + x,
            y: self.y + y,
        }
    }

    pub fn inc(&mut self, x: f32, y: f32) {
        self.x += x;
        self.y += y;
    }

    pub fn clamp(mut self, min: f32, max: f32) -> Self {
        self.x = self.x.clamp(min, max);
        self.y = self.y.clamp(min, max);
        self
    }

    pub fn distance_to_point(&self, other: &Self) -> f32 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2)).sqrt()
    }

    pub fn distance_to_line(&self, line_start: &Self, line_end: &Self) -> f32 {
        let line_length = line_start.distance_to_point(line_end);
        let numerator = ((line_end.y - line_start.y) * self.x - (line_end.x - line_start.x) * self.y + line_end.x * line_start.y - line_end.y * line_start.x).abs();
        numerator / line_length
    }
    
    pub fn is_zero(&self) -> bool {
        self.x.abs() < 0.001 && self.y.abs() < 0.001
    }
    
    pub fn round(&self) -> Pos2 {
        Pos2 {
            x: self.x.round() as i32,
            y: self.y.round() as i32,
        }
    }
}