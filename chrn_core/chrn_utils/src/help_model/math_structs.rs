use std::{fmt::Debug, ops::Range};

pub struct Tensor2<T: Debug + PartialEq> {
    pub inner: Vec<T>,
    pub rows: usize,
    pub cols: usize,
}

impl Tensor2<f32> {
    pub fn with_zeros(rows: usize, cols: usize) -> Tensor2<f32> {
        let size = rows * cols;
        let inner = vec![0f32; size];

        Tensor2 { inner, rows, cols }
    }

    pub fn with_random(rows: usize, cols: usize, range: Range<f32>) -> Tensor2<f32> {
        let size = rows * cols;
        let mut inner = vec![0f32; size];

        for i in 0..inner.len() {
            inner[i] = rand::random_range(range.clone());
        }

        Tensor2 { inner, rows, cols }
    }

    pub fn dot(&self, other: &Tensor2<f32>) -> Tensor2<f32> {
        if self.rows * self.cols != other.rows * other.cols {
            panic!(
                "Attempt to dot product `self` {}x{} with `other` {}x{}",
                self.rows, self.cols, other.rows, other.cols
            );
        }

        let inner_size = self.rows * other.cols;
        let mut inner_res = vec![0f32; inner_size];

        for row in 0..self.rows {
            for col in 0..other.cols {
                let mut sum = 0f32;
                for k in 0..self.cols {
                    sum += self.extract(row, k) * other.extract(k, col);
                }
                inner_res[row * other.cols + col] = sum;
            }
        }

        Tensor2 {
            inner: inner_res,
            rows: self.rows,
            cols: other.cols,
        }
    }
}

impl<T: Debug + PartialEq> Tensor2<T> {
    pub fn extract(&self, row: usize, col: usize) -> &T {
        if row > self.rows - 1 || col > self.cols - 1 {
            panic!(
                "Given index of row {} and col {} is invalid for `self` of row {} and col {}",
                row, col, self.rows, self.cols
            );
        }

        // Suspicious
        let idx = row * self.cols + col;
        &self.inner[idx]
    }

    pub fn extract_mut(&mut self, row: usize, col: usize) -> &mut T {
        if row > self.rows - 1 || col > self.cols - 1 {
            panic!(
                "Given index of row {} and col {} is invalid for `self` of row {} and col {}",
                row, col, self.rows, self.cols
            );
        }

        let idx = row * self.cols + col;
        &mut self.inner[idx]
    }

    pub fn extract_row(&self, row: usize) -> &[T] {
        let row_idx = row * self.cols;
        &self.inner[row_idx..row_idx + self.cols]
    }

    pub fn extract_row_mut(&mut self, row: usize) -> &mut [T] {
        let row_idx = row * self.cols;
        &mut self.inner[row_idx..row_idx + self.cols]
    }

    pub fn get(&self, row: usize, col: usize) -> Option<&T> {
        if row > self.rows - 1 || col > self.cols - 1 {
            panic!(
                "Given index of row {} and col {} is invalid for `self` of row {} and col {}",
                row, col, self.rows, self.cols
            );
        }

        // Suspicious
        let idx = row * self.cols + col;
        Some(&self.inner[idx])
    }

    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut T> {
        if row > self.rows - 1 || col > self.cols - 1 {
            panic!(
                "Given index of row {} and col {} is invalid for `self` of row {} and col {}",
                row, col, self.rows, self.cols
            );
        }

        let idx = row * self.cols + col;
        Some(&mut self.inner[idx])
    }

    pub fn get_row(&self, row: usize) -> Option<&[T]> {
        let row_idx = row * self.cols;
        Some(&self.inner[row_idx..row_idx + self.cols])
    }

    pub fn get_row_mut(&mut self, row: usize) -> Option<&mut [T]> {
        let row_idx = row * self.cols;
        Some(&mut self.inner[row_idx..row_idx + self.cols])
    }
}

impl<T: Debug + PartialEq> Debug for Tensor2<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut info = String::from("[\n  ");

        for row in 0..self.rows {
            info.push('[');
            for col in 0..self.cols {
                let element = self.extract(row, col);
                info.push_str(&format!("{element:?}"));

                if col + 1 < self.cols {
                    info.push_str(", ");
                }
            }

            info.push(']');
            if row + 1 < self.rows {
                info.push_str(", ");
            }
        }

        info.push_str("\n]");
        info.push_str(&format!("\nrows = {}\ncolumns = {}", self.rows, self.cols));

        write!(f, "{info}")
    }
}

impl From<&Vec<Vec<f32>>> for Tensor2<f32> {
    fn from(other: &Vec<Vec<f32>>) -> Self {
        let rows = other.len();
        let cols = other[0].len();

        let mut tensor2: Tensor2<f32> = Tensor2::with_zeros(rows, cols);

        for row in 0..rows {
            // Incomprehensive
            if other[row].len() > cols {
                panic!(
                    "Found other col {} after smaller col {}",
                    other[row].len(),
                    cols
                );
            }

            for col in 0..cols {
                *tensor2.extract_mut(row, col) = other[row][col];
            }
        }

        tensor2
    }
}

// #[macro_export]
// macro_rules! tensor2f32 {
//     (($rows:expr; $cols:expr); $($matrix:expr),*) => {
//         let mut _tensor2: $crate::help_model::math_structs::Tensor2<f32> = Tensor2::with_zeros($rows, $cols);
//
//         $(
//         for row in 0..$rows {
//             for col in 0..$cols {
//
//             }
//         }
//         )*
//
//         _tensor2
//     };
// }
