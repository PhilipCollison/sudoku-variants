//! This module contains logic for generating random Sudoku.
//!
//! Generation of Sudoku puzzles is done by first generating a full grid with a
//! [Generator](struct.Generator.html) and then removing some clues using a
//! [Solver](struct.Solver.html).

use crate::Sudoku;
use crate::constraint::Constraint;
use crate::error::{SudokuError, SudokuResult};
use crate::solver::{BacktrackingSolver, Solution, Solver};

use rand::Rng;
use rand::rngs::ThreadRng;

/// A generator randomly generates a full [Sudoku](../struct.Sudoku.html), that
/// is, a Sudoku with no missing digits. It uses a random number generator to
/// decide the content. For most cases, sensible defaults are provided by
/// [Generator::new_default](struct.Generator.html#method.new_default).
pub struct Generator<R: Rng> {
    rng: R
}

impl Generator<ThreadRng> {

    /// Creates a new generator that uses a
    /// [ThreadRng](https://rust-random.github.io/rand/rand/rngs/struct.ThreadRng.html)
    /// to generate the random digits.
    pub fn new_default() -> Generator<ThreadRng> {
        Generator::new(rand::thread_rng())
    }
}

fn shuffle<T>(rng: &mut impl Rng, nums: impl Iterator<Item = T>) -> Vec<T> {
    let mut vec: Vec<T> = nums.collect();
    let len = vec.len();

    for i in 1..(len - 1) {
        let j = rng.gen_range(0, i + 1);
        vec.swap(i, j);
    }

    vec
}

impl<R: Rng> Generator<R> {

    /// Creates a new generator that uses the given random number generator to
    /// generate random digits.
    pub fn new(rng: R) -> Generator<R> {
        Generator {
            rng
        }
    }

    fn generate_rec<C: Constraint + Clone>(&mut self, sudoku: &mut Sudoku<C>,
            column: usize, row: usize) -> bool {
        let size = sudoku.grid().size();
        
        if row == size {
            return true;
        }

        let next_column = (column + 1) % size;
        let next_row =
            if next_column == 0 { row + 1 } else { row };
        
        for number in shuffle(&mut self.rng, 1..=size) {
            if sudoku.is_valid_number(column, row, number).unwrap() {
                sudoku.grid_mut().set_cell(column, row, number).unwrap();

                if self.generate_rec(sudoku, next_column, next_row) {
                    return true;
                }

                sudoku.grid_mut().clear_cell(column, row).unwrap();
            }
        }

        false
    }

    /// Generates a new random [Sudoku](../struct.Sudoku.html) with all digits
    /// that matches the given parameters. If it is not possible, an error will
    /// be returned.
    ///
    /// It is guaranteed that
    /// [Sudoku.is_valid](../struct.Sudoku.html#method.is_valid) on the result
    /// returns `true`.
    ///
    /// # Arguments
    ///
    /// * `block_width`: The horizontal dimension of one sub-block of the grid.
    /// To ensure a square grid, this is also the number of blocks that compose
    /// the grid vertically. For an ordinary Sudoku grid, this is 3. Must be
    /// greater than 0.
    /// * `block_height`: The vertical dimension of one sub-block of the grid.
    /// To ensure a square grid, this is also the number of blocks that compose
    /// the grid horizontally. For an ordinary Sudoku grid, this is 3. Must be
    /// greater than 0.
    /// * `constraint`: The constraint which will be matched by the generated
    /// Sudoku, which will also be contained and checked by the output Sudoku.
    ///
    /// # Errors
    ///
    /// * `SudokuError::InvalidDimensions` If `block_width` or `block_height`
    /// is invalid (zero).
    /// * `SudokuError::UnsatisfiableConstraint` If there are no grids with the
    /// given dimensions that match the provided `constraint`.
    pub fn generate<C: Constraint + Clone>(&mut self, block_width: usize,
            block_height: usize, constraint: C) -> SudokuResult<Sudoku<C>> {
        let mut sudoku =
            Sudoku::new_empty(block_width, block_height, constraint)?;

        if self.generate_rec(&mut sudoku, 0, 0) {
            Ok(sudoku)
        }
        else {
            Err(SudokuError::UnsatisfiableConstraint)
        }
    }
}

/// A reducer can be applied to the output of a
/// [Generator](struct.Generator.html) to remove numbers from the grid as long
/// as it is still uniquely solveable using the provided
/// [Solver](../solver/trait.Solver.html). This may be intentionally
/// suboptimal to control the difficulty. A random number generator decides
/// which digits are removed.
///
/// [Reducer::new_default](#new_default) will yield a reducer with the highest
/// difficulty (a perfect backtracking solver) and a
/// [ThreadRng](https://rust-random.github.io/rand/rand/rngs/struct.ThreadRng.html).
pub struct Reducer<S: Solver, R: Rng> {
    solver: S,
    rng: R
}

impl Reducer<BacktrackingSolver, ThreadRng> {

    /// Generates a new reducer with a
    /// [BacktrackingSolver](../solver/BacktrackingSolver.html) to check unique
    /// solveability and a
    /// [ThreadRng](https://rust-random.github.io/rand/rand/rngs/struct.ThreadRng.html)
    /// to decide which digits are removed.
    pub fn new_default() -> Reducer<BacktrackingSolver, ThreadRng> {
        Reducer::new(BacktrackingSolver, rand::thread_rng())
    }
}

impl<S: Solver, R: Rng> Reducer<S, R> {

    /// Creates a new reducer with the given solver and random number gnerator.
    ///
    /// # Arguments
    ///
    /// * `solver`: A [Solver](../solver/trait.Solver.html) to be used to check
    /// whether a reduced Sudoku is still uniquely solveable. This controls the
    /// difficulty by specifying a strategy that must be able to solve the
    /// Sudoku.
    /// * `rng`: A random number generator that decides which digits are
    /// removed.
    pub fn new(solver: S, rng: R) -> Reducer<S, R> {
        Reducer {
            solver,
            rng
        }
    }

    /// Reduces the given Sudoku as much as possible. That is, removes random
    /// digits until all remaining ones are necessary for the solver used by
    /// this reducer to still be able to solve the Sudoku. All changes are done
    /// to the given mutable Sudoku.
    pub fn reduce<C: Constraint + Clone>(&mut self, sudoku: &mut Sudoku<C>) {
        let size = sudoku.grid().size();
        let coords= (0..size)
            .flat_map(|column| (0..size).map(move |row| (column, row)));

        for (column, row) in shuffle(&mut self.rng, coords) {
            if let Some(number) =
                    sudoku.grid().get_cell(column, row).unwrap() {
                sudoku.grid_mut().clear_cell(column, row).unwrap();

                if let Solution::Unique(_) = self.solver.solve(sudoku) { }
                else {
                    sudoku.grid_mut().set_cell(column, row, number).unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::constraint::DefaultConstraint;

    const DEFAULT_BLOCK_WIDTH: usize = 3;
    const DEFAULT_BLOCK_HEIGHT: usize = 3;

    fn generate_default() -> Sudoku<DefaultConstraint> {
        let mut generator = Generator::new_default();
        generator.generate(DEFAULT_BLOCK_WIDTH, DEFAULT_BLOCK_HEIGHT,
            DefaultConstraint).unwrap()
    }

    fn reduce_default() -> Sudoku<DefaultConstraint> {
        let mut sudoku = generate_default();
        let mut reducer = Reducer::new_default();
        reducer.reduce(&mut sudoku);
        sudoku
    }

    #[test]
    fn generated_sudoku_valid() {
        let sudoku = generate_default();
        assert!(sudoku.is_valid(), "Generated Sudoku not valid.");
    }

    #[test]
    fn generated_sudoku_full() {
        let sudoku = generate_default();
        let size = DEFAULT_BLOCK_WIDTH * DEFAULT_BLOCK_HEIGHT;
        assert_eq!(size * size, sudoku.grid().count_clues(),
            "Generated Sudoku is not full.");
    }

    #[test]
    fn reduced_sudoku_valid_and_not_full() {
        let sudoku = reduce_default();
        let size = DEFAULT_BLOCK_WIDTH * DEFAULT_BLOCK_HEIGHT;
        assert!(sudoku.is_valid(), "Reduced Sudoku not valid.");
        assert!(sudoku.grid().count_clues() <= size * (size - 1),
            "Reduced Sudoku has too many clues.");
    }

    #[test]
    fn reduced_sudoku_uniquely_solveable() {
        let sudoku = reduce_default();
        let solver = BacktrackingSolver;
        let solution = solver.solve(&sudoku);

        if let Solution::Unique(_) = solution { }
        else {
            panic!("Reduced Sudoku not uniquely solveable.")
        }
    }

    /// This is a deliberately bad solver which only checks differet options
    /// for the top-left cell of each Sudoku. If any other cells are missing,
    /// or there are multiple options for the top-left cell, the solver returns
    /// `Solution::Ambiguous`.
    struct TopLeftSolver;

    impl Solver for TopLeftSolver {
        fn solve(&self, sudoku: &Sudoku<impl Constraint + Clone>) -> Solution {
            let size = sudoku.grid().size();
            let cells = size * size;
            let clues = sudoku.grid().count_clues();

            if clues == cells {
                // Sudoku is full anyway
                return Solution::Unique(sudoku.grid().clone());
            }
            else if clues < cells - 1 {
                // Sudoku missing other digit anyway
                return Solution::Ambiguous;
            }

            if let Some(_) = sudoku.grid().get_cell(0, 0).unwrap() {
                // Somewhere else a cell must be missing
                Solution::Ambiguous
            }
            else {
                let mut number = None;

                for i in 1..=size {
                    if sudoku.is_valid_number(0, 0, i).unwrap() {
                        if number == None {
                            number = Some(i);
                        }
                        else {
                            // Multiple options for top-left cell
                            return Solution::Ambiguous;
                        }
                    }
                }

                if let Some(number) = number {
                    let mut result_grid = sudoku.grid().clone();
                    result_grid.set_cell(0, 0, number).unwrap();
                    Solution::Unique(result_grid)
                }
                else {
                    Solution::Impossible
                }
            }
        }
    }

    #[test]
    fn reduced_sudoku_solveable_by_solver() {
        let mut sudoku = generate_default();
        let mut reducer = Reducer::new(TopLeftSolver, rand::thread_rng());
        reducer.reduce(&mut sudoku);

        let size = DEFAULT_BLOCK_WIDTH * DEFAULT_BLOCK_HEIGHT;
        assert_eq!(size * size - 1, sudoku.grid().count_clues(),
            "Reduced Sudoku missing too many clues or not reduced at all.");
        assert_eq!(None, sudoku.grid().get_cell(0, 0).unwrap(),
            "Reduced Sudoku missing wrong clue.");
    }
}
