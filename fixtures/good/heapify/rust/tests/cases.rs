use harness::heapify;

#[test]
fn small_heaps() {
    let mut a: [u32; 0] = [];
    heapify(&mut a);
    assert_eq!(a, []);
    let mut b = [42u32];
    heapify(&mut b);
    assert_eq!(b, [42]);
    let mut c = [3u32, 2, 1];
    heapify(&mut c);
    assert_eq!(c, [1, 2, 3]);
}

#[test]
fn larger_heaps() {
    let mut a = [3u32, 1, 3, 1];
    heapify(&mut a);
    assert_eq!(a, [1, 1, 3, 3]);
    let mut b = [9u32, 3, 7, 1, 8, 2, 6, 4];
    heapify(&mut b);
    assert_eq!(b, [1, 3, 2, 4, 8, 7, 6, 9]);
}
