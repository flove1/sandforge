use crate::cell::CellAction;

use super::cell::Cell;
use super::chunk::PixelToChunkApi;

#[derive(Default, Clone, Copy)]
pub enum Element {
    #[default]
    Empty,
    Stone,
    Water,
    Sand,
    GlowingSand,
}

pub fn update_sand(mut cell: Cell, mut api: PixelToChunkApi, _dt: f32) -> Vec<CellAction> {
    let mut actions: Vec<CellAction> = vec![];
    let dx = api.rand_dir();

    if matches!(api.get(0, 1).element, Element::Empty ) {
        actions.push(CellAction::Swap(0, 1));
    } else if matches!(api.get(dx, 0).element, Element::Empty ) {
        actions.push(CellAction::Swap(dx, 0));
    } else if matches!(api.get(0, 1).element, Element::Water) {
        actions.push(CellAction::Swap(0, 1));
    }
    // else {
    //     actions.push(CellAction::Sleep());
    //     return  actions;
    // }
    
    actions.push(CellAction::Update(cell));
    actions
}

pub fn update_liquid(mut cell: Cell, mut api: PixelToChunkApi, _dt: f32) -> Vec<CellAction> {
    let mut actions: Vec<CellAction> = vec![];
    let mut dx = api.rand_dir();

    if matches!(api.get(0, 1).element, Element::Empty) {
        actions.push(CellAction::Swap(0, 1));
        if api.once_in(20) {
            //randomize direction when falling sometimes
            cell.ra = 100 + api.rand_int(50) as u8;
        }
        actions.push(CellAction::Update(cell));
        return actions;
    } else if matches!(api.get(dx, 1).element, Element::Empty) {
        //fall diagonally
        actions.push(CellAction::Swap(dx, 1));
        actions.push(CellAction::Update(cell));
        return actions;
    } else if matches!(api.get(-dx, 1).element, Element::Empty) {
        actions.push(CellAction::Swap(-dx, 1));
        actions.push(CellAction::Update(cell));
        return actions;
    }

    dx = if cell.ra % 2 == 0 { 1 } else { -1 };
    let dx0 = *api.get(dx, 0);

    if matches!(api.get(dx, 0).element, Element::Empty) && matches!(api.get(dx * 2, 0).element, Element::Empty) {
        // scoot double
        cell.rb = 6;

        actions.push(CellAction::Swap(2 * dx, 0));
        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if matches!(nbr.element, Element::Water) {
            if nbr.ra % 2 != cell.ra % 2 {
                actions.push(CellAction::Set(dx, dy, Cell {
                    ra: cell.ra,
                    ..cell
                }));
            }
        }
    } else if matches!(dx0.element, Element::Empty) {
        actions.push(CellAction::Set(dx, 0, Cell { rb: 3, ..cell }));
        actions.push(CellAction::Update(dx0));
        
        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if matches!(nbr.element, Element::Water) {
            if nbr.ra % 2 != cell.ra % 2 {
                actions.push(CellAction::Set(dx, dy, Cell {
                    ra: cell.ra,
                    ..cell
                }));
            }
        }
    } else if cell.rb == 0 {
        if matches!(api.get(-dx, 0).element, Element::Empty) {
            // bump
            cell.ra = ((cell.ra as i64) + dx) as u8;
            actions.push(CellAction::Update(cell));
        }
    } else {
        // become less certain (more bumpable)
        cell.rb = cell.rb - 1;
        actions.push(CellAction::Update(cell));
    }

    actions    
}