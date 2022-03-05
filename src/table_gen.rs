use crate::table::*;
use rug::Integer;

pub const POLYNOMIAL: u64 = 0xbfe6b8a5bf378d83;

fn reduce(value: Integer, modulo: &Integer) -> u64 {
    let modulo_bits = modulo.significant_bits();
    let value_bits = value.significant_bits();
    if value_bits < modulo_bits {
        return value.to_u64_wrapping();
    }

    let delta = value_bits - modulo_bits;

    let mut result = value;
    for i in (0..=delta).rev() {
        if result.get_bit(modulo_bits + i - 1) {
            result ^= modulo.clone() << i;
        }
    }

    result.to_u64_wrapping()
}

impl Table {
    pub fn new(window_size: usize) -> Self {
        let mut res = Self {
            shift: [0; 256],
            drop: [0; 256],
        };

        let modulo = Integer::from(POLYNOMIAL);
        let degree = modulo.significant_bits();

        for i in 0..256 {
            res.shift[i] =
                reduce(Integer::from(i) << (degree - 1), &modulo) ^ (i << (degree - 1)) as u64;
            res.drop[i] = reduce(Integer::from(i) << (window_size * 8), &modulo);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_computes_correct_table() {
        let table = Table::new(64);

        assert_eq!(table.shift[0], 0);
        assert_eq!(table.drop[0], 0);

        assert_eq!(table.shift[8], 4548086706303466141);
        assert_eq!(table.drop[8], 4180238687019168624);

        assert_eq!(table.shift[75], 14406569746831191669);
        assert_eq!(table.drop[75], 2590085172916931783);

        assert_eq!(table.shift[182], 8333038893256783518);
        assert_eq!(table.drop[182], 1155685809517156813);

        assert_eq!(table.shift[255], 14665969062442009581);
        assert_eq!(table.drop[255], 4429513017793006038);
    }
}
