#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        if items.is_empty() { None } else { Some(items[self.below(items.len())]) }
    }
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn hash01(x: i64, y: i64, salt: u64) -> f32 {
    let h = mix((x as u64).wrapping_mul(0x9E37_79B9) ^ (y as u64).wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35));
    (h >> 40) as f32 / (1u64 << 24) as f32
}

pub fn noise(x: f32, y: f32, salt: u64) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (x0 as i64, y0 as i64);
    let a = hash01(xi, yi, salt);
    let b = hash01(xi + 1, yi, salt);
    let c = hash01(xi, yi + 1, salt);
    let d = hash01(xi + 1, yi + 1, salt);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    top + (bottom - top) * sy
}

pub fn seed_from_clock() -> u64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42);
    mix(nanos) % 1_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn shuffle_keeps_every_item() {
        let mut items: Vec<u32> = (0..50).collect();
        Rng::new(3).shuffle(&mut items);
        let mut sorted = items.clone();
        sorted.sort();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
        assert_ne!(items, sorted);
    }

    #[test]
    fn noise_stays_in_unit_range() {
        for i in 0..500 {
            let v = noise(i as f32 * 0.37, i as f32 * 0.11, 9);
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
