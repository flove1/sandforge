use std::slice::Iter;

use super::cell::Cell;
use super::chunk::ChunkApi;

#[derive(Default, Clone, Copy, PartialEq)]
pub enum Element {
    #[default]
    Empty,
    Stone,
    Water,
    Sand,
    Wood,
    Dirt,
    Gas,
    Coal
}

// enum ElementProperties {
//     Burning(u8),
//     Melting(u8),
// }

impl Element {
    pub fn color(&self) -> [u8; 4] {
        match self {
            Element::Empty => [0x00, 0x00, 0x00, 0xff],
            Element::Stone => [0x77, 0x77, 0x77, 0xff],
            Element::Sand => [0xf2, 0xf1, 0xa3, 0xff],
            Element::Water => [0x47, 0x7C, 0xB8, 0xff],
            // Element::Fire => [0xe8, 0x6a, 0x17, 0xff],
            Element::Wood => [0x6a, 0x4b, 0x35, 0xff],
            Element::Dirt => [0x6d, 0x5f, 0x3d, 0xff],
            Element::Gas => [0x55, 0x74, 0x56, 0x99],
            Element::Coal => [0x09, 0x09, 0x09, 0xff],
        }
    }

    pub fn iterator() -> Iter<'static, Element> {
        static ELEMENTS: [Element; 8] = [
            Element::Empty,
            Element::Stone,
            Element::Sand,
            Element::Water,
            // Element::Fire,
            Element::Wood,
            Element::Dirt,
            Element::Gas,
            Element::Coal
        ];
        ELEMENTS.iter()
    }
}

impl ToString for Element {
    fn to_string(&self) -> String {
        match self {
            Element::Empty => String::from("Eraser"),
            Element::Stone => String::from("Stone"),
            Element::Water => String::from("Water"),
            Element::Sand => String::from("Sand"),
            Element::Coal => String::from("Coal"),
            // Element::Fire => String::from("Fire"),
            Element::Wood => String::from("Wood"),
            Element::Dirt => String::from("Dirt"),
            Element::Gas => String::from("Gas")
        }
    }
}

pub fn update_sand<'a, 'b>(cell: &Cell, mut api: ChunkApi<'a, 'b>, _dt: f32) -> ChunkApi<'a, 'b> {
    let dx = api.rand_dir();
    
    if api.match_element(0, 1, Element::Empty) {
        if api.once_in(5) && api.match_element(dx, 1, Element::Empty) {
            api.swap(dx, 1);
        }
        else {
            api.swap(0, 1);
        }
    } 
    else if api.match_element(dx, 1, Element::Empty) {
        api.swap(dx, 1);
    } 
    else if api.match_element(-dx, 1, Element::Empty) {
        api.swap(-dx, 1);
    } 
    else if api.match_element(0, 1, Element::Water) {
        api.swap(0, 1);
    }

    api.update(*cell);

    api
}

// pub fn update_fire<'a, 'b>(cell: &mut Cell, mut api: ChunkApi<'a, 'b>, _dt: f32) -> ChunkApi<'a, 'b> {
//     let directions = [(0, 1), (1, 0), (0, -1), (-1, 0), (1, 1), (-1, -1), (1, -1), (-1, 1)];

//     for (dx, dy) in directions {
//         let cell = api.get(dx, dy);

//         let modifier = if dy == -1 { 0.5 } else { 1.0 };

//         match cell.element {
//             Element::Wood => {
//                 if api.once_in((100.0 * modifier) as i32) {
//                     api.set(dx, dy, Cell::new_with_rb(Element::Fire, cell.clock, 80))
//                 }
//             }

//             Element::Coal => {
//                 if api.once_in((500.0 * modifier) as i32) {
//                     api.set(dx, dy, Cell::new_with_rb(Element::Fire, cell.clock, 100))
//                 }
//             }

//             Element::Gas => {
//                 if api.once_in(10) {
//                     api.set(dx, dy, Cell::new_with_rb(Element::Fire, cell.clock, 10))
//                 }
//             }   
            
//             _ => {}
//         }
//     }



//     if api.once_in(2) {
//         cell.rb -= 1;
//     }

//     if cell.rb == 0 {
//         cell.element = Element::Empty;
//     }

//     api.set(0, 0, *cell);
//     api
// }

pub fn update_liquid<'a, 'b>(cell: &mut Cell, mut api: ChunkApi<'a, 'b>, _dt: f32) -> ChunkApi<'a, 'b> {
    let mut dx = api.rand_dir();

    if api.match_element(0, 1, Element::Empty) {
        if api.once_in(5) {
            //randomize direction when falling sometimes
            cell.ra = api.rand_int(20) as u8;
        }

        if api.once_in(5) && api.match_element(((cell.ra % 2) * 2) as i32 - 1, 1, Element::Empty) {
            api.swap(((cell.ra % 2) * 2) as i32 - 1, 1);
        }
        else {
            api.swap(0, 1);
        }

        api.update(*cell);
        return api;
    }
    else if api.match_element(dx, 0, Element::Empty) && api.match_element(dx, 1, Element::Empty){
        api.swap(dx, 0);

        api.update(*cell);
        return api;
    }
    else if api.match_element(-dx, 0, Element::Empty) && api.match_element(-dx, 1, Element::Empty){
        api.swap(-dx, 0);

        api.update(*cell);
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
            cell.ra = ((cell.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        cell.rb -= 1;
    }

    api.update(*cell);
    api
}

pub fn update_gas<'a, 'b>(cell: &mut Cell, mut api: ChunkApi<'a, 'b>, _dt: f32) -> ChunkApi<'a, 'b> {
    let mut dx = api.rand_dir();

    if api.match_element(dx, 0, Element::Empty) && api.match_element(dx, -1, Element::Empty){
        api.swap(dx, 0);
    }
    else if api.match_element(-dx, 0, Element::Empty) && api.match_element(-dx, -1, Element::Empty){
        api.swap(-dx, 0);
    }
    
    if api.match_element(0, -1, Element::Empty) {
        api.swap(0, -1);
        if api.once_in(20) {
            //randomize direction when falling sometimes
            cell.ra = api.rand_int(20) as u8;
        }

        api.update(*cell);
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
            cell.ra = ((cell.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        cell.rb -= 1;
    }

    api.update(*cell);

    api
}