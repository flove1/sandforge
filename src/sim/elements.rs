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

pub fn update_sand(mut cell: Cell, mut api: ChunkApi, _dt: f32) {
    cell.iter_bit = !cell.iter_bit;
    let dx = api.rand_dir();

    if api.match_element(0, 1, Element::Empty) {
        api.swap(0, 1);
    } else if api.match_element(dx, 0, Element::Empty) && api.match_element(dx, 1, Element::Empty) {
        api.swap(dx, 0);
    } else if api.match_element(0, 1, Element::Water) {
        api.swap(0, 1);
    }
    
    api.update(cell);
}

pub fn update_liquid(mut cell: Cell, mut api: ChunkApi, _dt: f32) {
    cell.iter_bit = !cell.iter_bit;
    let mut dx = api.rand_dir();

    if matches!(api.get(0, 1).element, Element::Empty) {
        api.swap(0, 1);
        if api.once_in(20) {
            //randomize direction when falling sometimes
            cell.ra = 100 + api.rand_int(50) as u8;
        }
        api.update(cell);
        return;
    } else if matches!(api.get(dx, 1).element, Element::Empty) {
        //fall diagonally
        api.swap(dx, 1);
        api.update(cell);
        return;
    } else if matches!(api.get(-dx, 1).element, Element::Empty) {
        api.swap(-dx, 1);
        api.update(cell);
        return;
    }

    dx = if cell.ra % 2 == 0 { 1 } else { -1 };
    let dx0 = api.get(dx, 0);

    if matches!(api.get(dx, 0).element, Element::Empty) && matches!(api.get(dx * 2, 0).element, Element::Empty) {
        // scoot double
        cell.rb = 6;
        api.swap(2 * dx, 0);
        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if matches!(nbr.element, Element::Water) {
            if nbr.ra % 2 != cell.ra % 2 {
                api.set(dx, dy, Cell {
                    ra: cell.ra,
                    ..cell
                });
            }
        }
    } else if matches!(dx0.element, Element::Empty) {
        api.set(dx, 0, Cell { rb: 3, ..cell });
        api.update(dx0);
        
        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if matches!(nbr.element, Element::Water) {
            if nbr.ra % 2 != cell.ra % 2 {
                api.set(dx, dy, Cell {
                    ra: cell.ra,
                    ..cell
                });
            }
        }
    } else if cell.rb == 0 {
        if matches!(api.get(-dx, 0).element, Element::Empty) {
            // bump
            cell.ra = ((cell.ra as i64) + dx) as u8;
            api.update(cell);
        }
    } else {
        // become less certain (more bumpable)
        cell.rb = cell.rb - 1;
        api.update(cell);
    }
}