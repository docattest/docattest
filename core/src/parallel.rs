//! Mapping a slice across a few threads while keeping its order.

/// Below this many items, starting threads costs more than it saves.
const SERIAL_BELOW: usize = 16;
/// The most threads one map starts, so a large machine does not start dozens for one command.
const MAX_THREADS: usize = 8;

/// `items` mapped by `map`, in their order. A panic in `map` is raised again here, so no
/// result is ever lost and the output always has one value for each item.
pub(crate) fn map_in_parallel<T: Sync, R: Send>(items: &[T], map: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(MAX_THREADS);
    if items.len() < SERIAL_BELOW || threads < 2 {
        return items.iter().map(&map).collect();
    }
    let chunk = items.len().div_ceil(threads);
    let map = &map;
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(map).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_order_of_the_items() {
        let items: Vec<usize> = (0..100).collect();
        assert_eq!(map_in_parallel(&items, |n| n * 2), items.iter().map(|n| n * 2).collect::<Vec<_>>());
    }

    #[test]
    #[should_panic(expected = "item 70")]
    fn raises_a_panic_instead_of_dropping_results() {
        let items: Vec<usize> = (0..100).collect();
        map_in_parallel(&items, |&n| if n == 70 { panic!("item 70") } else { n });
    }
}
