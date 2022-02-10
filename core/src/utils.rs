use rust_sc2::prelude::Units;

pub trait Supply {
    fn supply(&self) -> u32;
}

impl Supply for Units {
    fn supply(&self) -> u32 {
        self.sum(|f| f.supply_cost()) as u32
    }
}
