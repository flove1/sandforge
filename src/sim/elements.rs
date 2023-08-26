use super::cell::Cell;
use super::chunk::ChunkApi;

#[derive(Default, Clone, Copy, PartialEq)]
pub enum Element {
    #[default]
    Empty,
    Stone,
    Water,
    Sand,
    GlowingSand,
}


impl ToString for Element {
    fn to_string(&self) -> String {
        match self {
            Element::Empty => String::from("Eraser"),
            Element::Stone => String::from("Stone"),
            Element::Water => String::from("Water"),
            Element::Sand => String::from("Sand"),
            Element::GlowingSand => String::from("Glowing Sand"),
        }
    }
}

pub fn update_sand<'a>(_cell: &Cell, mut api: ChunkApi<'a>, _dt: f32) -> ChunkApi<'a> {
    let dx = api.rand_dir();

    if api.match_element(0, 1, Element::Empty) {
        api.swap(0, 1);
    } 
    else if api.match_element(dx, 1, Element::Empty) {
        api.swap(dx, 1);
    } 
    else if api.match_element(0, 1, Element::Water) {
        api.swap(0, 1);
    }

    api
}

pub fn update_liquid<'a>(cell: &mut Cell, mut api: ChunkApi<'a>, _dt: f32) -> ChunkApi<'a> {
    let mut dx = api.rand_dir();

    if api.match_element(0, 1, Element::Empty) {
        api.swap(0, 1);
        if api.once_in(20) {
            //randomize direction when falling sometimes
            cell.ra = 100 + api.rand_int(50) as u8;
        }

        return api;
    } 
    else if api.match_element(dx, 0, Element::Empty) && api.match_element(dx, 1, Element::Empty){
        api.swap(dx, 1);
        return api;
    }
    else if api.match_element(-dx, 0, Element::Empty) && api.match_element(-dx, 1, Element::Empty) {
        api.swap(-dx, 1);
        return api;
    }

    dx = if cell.ra % 2 == 0 { 1 } else { -1 };    

    let dx0 = api.get(dx, 0);
    let dxd = api.get(dx * 2, 0);

    if dx0.element == Element::Empty && dxd.element == Element::Empty {
        // scoot double
        cell.rb = 6;
        api.swap(dx * 2, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if nbr.element == Element::Water {
            if nbr.ra % 2 != cell.ra % 2 {
                api.set(dx, dy,
                    Cell {
                        ra: cell.ra,
                        ..*cell
                    },
                )
            }
        }
    } else if dx0.element == Element::Empty {
        cell.rb = 3;
        api.swap(dx, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if nbr.element == Element::Water {
            if nbr.ra % 2 != cell.ra % 2 {
                api.set(
                    dx,
                    dy,
                    Cell {
                        ra: cell.ra,
                        ..*cell
                    },
                )
            }
        }
    } else if cell.rb == 0 {
        if api.match_element(-dx, 0, Element::Empty) {
            // bump
            cell.ra = ((cell.ra as i64) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        cell.rb -= 1;
    }

    api
}