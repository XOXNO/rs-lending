use multiversx_sc::imports::*;

use crate::errors::*;

/// Returns the sorted middle, or the average of the two middle indexed items if the
/// vector has an even number of elements.
pub fn calculate<M: ManagedTypeApi>(
    list: &mut [BigUint<M>],
) -> Result<Option<BigUint<M>>, StaticSCError> {
    if list.is_empty() {
        return Result::Ok(None);
    }
    list.sort_unstable();
    let len = list.len();
    let middle_index = len / 2;
    if len % 2 == 0 {
        // [2.1,2.2,2.0,2.3]
        let median1 = list.get(middle_index - 1).ok_or(MEDIAN_1_INVALID_INDEX)?; // 2.2
        let median2 = list.get(middle_index).ok_or(MEDIAN_2_INVALID_INDEX)?; // 2.0
        Result::Ok(Some((median1.clone() + median2.clone()) / 2u64)) // 4.2 / 2 = 2.1
    } else {
        let median = list.get(middle_index).ok_or(MEDIAN_INVALID_INDEX)?;
        Result::Ok(Some(median.clone()))
    }
}
