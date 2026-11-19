mod cake;
mod fruit;

pub use cake::Cake;
pub use fruit::Fruit;

/// Use the same model registration in every function and the migration CLI.
pub fn all() -> toasty::ModelSet {
    toasty::models!(Cake, Fruit)
}
