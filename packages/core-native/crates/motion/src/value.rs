#[derive(Debug, Clone, PartialEq)]
pub enum MotionValue<T> {
    Fixed(T),
    Animated { current: T, target: T },
}

impl<T> MotionValue<T> {
    pub fn current(&self) -> &T {
        match self {
            Self::Fixed(value) => value,
            Self::Animated { current, .. } => current,
        }
    }

    pub fn target(&self) -> &T {
        match self {
            Self::Fixed(value) => value,
            Self::Animated { target, .. } => target,
        }
    }
}

impl<T: Clone> MotionValue<T> {
    pub fn fixed(value: T) -> Self {
        Self::Fixed(value)
    }

    pub fn animated(current: T, target: T) -> Self {
        Self::Animated { current, target }
    }
}
