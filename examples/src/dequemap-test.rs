use rust_box::dequemap::DequeMap;

fn main() {
    std::env::set_var("RUST_LOG", "event_notify=info");
    env_logger::init();

    test_dequemap();
}

fn test_dequemap() {
    let mut map = DequeMap::new();
    map.push_back(2, 20);
    map.push_back(1, 10);
    map.push_back(9, 90);
    map.push_back(3, 30);
    map.push_back(5, 50);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), Some(&20));
    assert_eq!(map.get(&3), Some(&30));
    assert_eq!(map.get(&5), Some(&50));
    assert_eq!(map.get(&9), Some(&90));
    assert_eq!(map.len(), 5);
    assert_eq!(map.pop_front(), Some((2, 20)));
    assert_eq!(map.len(), 4);
    assert_eq!(map.pop_back(), Some((5, 50)));
    assert_eq!(map.len(), 3);
    println!("--test_dequemap({})--------", map.len());
    for (k, v) in map.iter() {
        println!("{} = {}", k, v);
    }

    let mut map1: DequeMap<i32, i32> = DequeMap::new();
    map1.push_back(7, 70);
    map1.push_back(9, 900);
    map.extend(map1);

    println!("--test_dequemap({})--------", map.len());
    for (i, (k, v)) in map.iter().enumerate() {
        println!("{} {} = {}", i, k, v);
    }

    assert_eq!(map.front(), Some((&1, &10)));
    assert_eq!(map.back(), Some((&7, &70)));

    let items: Vec<(i32, i32)> = map.iter().map(|t| (*t.0, *t.1)).collect();
    println!("items: {:?}", items);
    assert_eq!(items, [(1, 10), (9, 900), (3, 30), (7, 70)]);

    map.remove(&7);
    let items: Vec<(i32, i32)> = map.iter().map(|t| (*t.0, *t.1)).collect();
    println!("items: {:?}", items);
    assert_eq!(items, [(1, 10), (9, 900), (3, 30)]);
}
