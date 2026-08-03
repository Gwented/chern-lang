pub mod containers;
//TEST: IGNORE THIS
/// Provides methods for a u32 that is split into u16 | u16
#[derive(Default, Clone, Copy)]
pub struct SharedU32 {
    pub shared_inner: u32,
}

const LEFT_MASK: u32 = 0xFFFF_0000;
const RIGHT_MASK: u32 = 0x0000_FFFF;

impl SharedU32 {
    pub const fn new(left: u16, right: u16) -> SharedU32 {
        let shared_inner = ((left as u32) << 16) | (right as u32);
        SharedU32 { shared_inner }
    }

    pub const fn from_u32(shared_inner: u32) -> SharedU32 {
        SharedU32 { shared_inner }
    }

    pub const fn shared_inner(&self) -> u32 {
        self.shared_inner
    }

    pub const fn set_shared_inner(&mut self, inner: u32) {
        self.shared_inner = inner;
    }

    pub const fn left(&self) -> u16 {
        (self.shared_inner >> 16) as u16
    }

    pub const fn set_left(&mut self, val: u16) {
        self.shared_inner = (self.shared_inner & RIGHT_MASK) | ((val as u32) << 16);
    }

    pub const fn add_left(&mut self, val: u16) {
        let new_left = self.left().wrapping_add(val);
        self.set_left(new_left);
    }

    pub const fn sub_left(&mut self, val: u16) {
        let new_left = self.left().wrapping_sub(val);
        self.set_left(new_left);
    }

    pub const fn right(&self) -> u16 {
        (self.shared_inner & RIGHT_MASK) as u16
    }

    pub const fn set_right(&mut self, val: u16) {
        self.shared_inner = (self.shared_inner & LEFT_MASK) | (val as u32);
    }

    pub const fn add_right(&mut self, val: u16) {
        let new_right = self.right().wrapping_add(val);
        self.set_right(new_right);
    }

    pub const fn sub_right(&mut self, val: u16) {
        let new_right = self.right().wrapping_sub(val);
        self.set_right(new_right);
    }

    pub const fn add(mut self, other: SharedU32) -> SharedU32 {
        self.add_left(other.left());
        self.add_right(other.right());
        self
    }

    pub const fn sub(mut self, other: SharedU32) -> SharedU32 {
        self.sub_left(other.left());
        self.sub_right(other.right());
        self
    }

    pub const fn add_assign(&mut self, other: SharedU32) {
        self.add_left(other.left());
        self.add_right(other.right());
    }

    pub const fn sub_assign(&mut self, other: SharedU32) {
        self.sub_left(other.left());
        self.sub_right(other.right());
    }
}

impl std::ops::Add for SharedU32 {
    type Output = SharedU32;
    fn add(self, other: SharedU32) -> SharedU32 {
        self.add(other)
    }
}

impl std::ops::Sub for SharedU32 {
    type Output = SharedU32;
    fn sub(self, other: SharedU32) -> SharedU32 {
        self.sub(other)
    }
}

impl std::ops::AddAssign for SharedU32 {
    fn add_assign(&mut self, other: SharedU32) {
        self.add_assign(other);
    }
}

impl std::ops::SubAssign for SharedU32 {
    fn sub_assign(&mut self, other: SharedU32) {
        self.sub_assign(other);
    }
}

impl std::fmt::Debug for SharedU32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedU32")
            .field("left", &self.left())
            .field("right", &self.right())
            .field(
                "inner",
                &format!("{} | {:#010x}", self.shared_inner, self.shared_inner),
            )
            .finish()
    }
}

/// Tracker that stores a "freeze" flag which takes the last bit in it's 32 bits, which allows it to stay 4
/// bytes instead of memory padding from a `bool`.
#[derive(Debug, Default)]
pub struct FreezeTrackerU32 {
    inner: u32,
}

// Not necessary. But it would be 8 bytes which would cause the entire program to otherwise combust.
impl FreezeTrackerU32 {
    const SIGNAL_FLAG: u32 = 0x8000_0000;
    const VAL_MASK: u32 = 0x7FFF_FFFF;

    pub fn new(inner: u32) -> FreezeTrackerU32 {
        FreezeTrackerU32 { inner }
    }

    pub const fn val(&self) -> u32 {
        self.inner & Self::VAL_MASK
    }

    pub const fn increment(&mut self) {
        // If inner takes over val radius we instantly combust.
        if !self.is_frozen() {
            self.inner += 1;
        }
    }

    pub const fn increment_many(&mut self, amt: u32) {
        if !self.is_frozen() {
            self.inner += amt;
        }
    }

    pub const fn reset_soft(&mut self) {
        if !self.is_frozen() {
            self.inner = 1;
        }
    }

    pub const fn freeze(&mut self) {
        self.inner |= Self::SIGNAL_FLAG;
    }

    pub const fn is_frozen(&self) -> bool {
        (self.inner & Self::SIGNAL_FLAG) != 0
    }
}

// Ignore me
// Ok but wouldn't making this a trait generate behavior instead of requiring a match each time?
// Mid train thought cut-off
pub enum SignalTrackerOptions {
    NoEffect,
    Freeze,
}
