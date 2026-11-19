#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    pub personal: u64,
    pub junk: u64,
    pub unused: u64,
    pub other: u64,
    pub free: u64,
}

impl Usage {
    pub fn total(self) -> u64 {
        self.personal
            .saturating_add(self.junk)
            .saturating_add(self.unused)
            .saturating_add(self.other)
            .saturating_add(self.free)
    }

    pub fn used(self) -> u64 {
        self.total().saturating_sub(self.free)
    }
}

pub(super) fn segment_widths(amounts: &[u64; 5], width: usize) -> [usize; 5] {
    let total: u128 = amounts.iter().map(|&amount| u128::from(amount)).sum();
    if total == 0 {
        return [0; 5];
    }
    let mut out = [0; 5];
    let mut remainders = [(0, 0u128); 5];
    let mut allocated = 0;
    for (i, amount) in amounts.iter().enumerate() {
        let numerator = width as u128 * u128::from(*amount);
        out[i] = (numerator / total) as usize;
        allocated += out[i];
        remainders[i] = (i, numerator % total);
    }
    remainders
        .sort_by_key(|&(index, remainder)| (amounts[index] == 0, std::cmp::Reverse(remainder)));
    for &(index, _) in remainders.iter().take(width.saturating_sub(allocated)) {
        out[index] += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::segment_widths;

    #[test]
    fn meter_segments_fill_the_width_without_overflow() {
        assert_eq!(
            segment_widths(&[1, 1, 1, 1, 1], 17).iter().sum::<usize>(),
            17
        );
        assert_eq!(
            segment_widths(&[u64::MAX, u64::MAX, 0, 0, 1], 100)
                .iter()
                .sum::<usize>(),
            100
        );
        assert_eq!(segment_widths(&[u64::MAX, u64::MAX, 0, 0, 0], 99)[4], 0);
        assert_eq!(segment_widths(&[0; 5], 20), [0; 5]);
    }
}
